#include "platform.h"
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QMap>
#include <QVariantMap>

using AppearanceSettings = QMap<QString, QVariantMap>;
Q_DECLARE_METATYPE(AppearanceSettings)

namespace resina::qt {
namespace {
const QString service = QStringLiteral("org.freedesktop.portal.Desktop");
const QString path = QStringLiteral("/org/freedesktop/portal/desktop");
const QString interface = QStringLiteral("org.freedesktop.portal.Settings");
const QString appearance = QStringLiteral("org.freedesktop.appearance");

// The portal specifies that unknown values mean "no preference".
QVariant preference(const QVariant& value) {
    bool valid = false;
    const uint level = value.toUInt(&valid);
    return QVariant(valid && level == 1);
}

QVariant scheme(const QVariant& value) {
    bool valid = false;
    const uint code = value.toUInt(&valid);
    if (valid && code == 1) return QStringLiteral("dark");
    if (valid && code == 2) return QStringLiteral("light");
    return QStringLiteral("none");
}
}

PlatformAppearance::PlatformAppearance(QObject* parent) : QObject(parent) {
    qDBusRegisterMetaType<AppearanceSettings>();
    auto bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        ready_ = true;
        error_ = QStringLiteral("platform appearance requires a D-Bus session bus");
        return;
    }
    bus.connect(service, path, interface, QStringLiteral("SettingChanged"), this,
                SLOT(settingChanged(QString,QString,QDBusVariant)));
    auto call = QDBusMessage::createMethodCall(service, path, interface, QStringLiteral("ReadAll"));
    call << QStringList{appearance};
    auto* watcher = new QDBusPendingCallWatcher(bus.asyncCall(call), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this](QDBusPendingCallWatcher* finished) {
        finished->deleteLater();
        const QDBusPendingReply<AppearanceSettings> reply = *finished;
        ready_ = true;
        if (reply.isError()) {
            error_ = QStringLiteral("platform appearance portal is unavailable: ") + reply.error().message();
        } else {
            const auto settings = reply.value().value(appearance);
            for (auto it = settings.cbegin(); it != settings.cend(); ++it) apply(it.key(), it.value());
        }
        emit changed();
    });
}

void PlatformAppearance::apply(const QString& key, const QVariant& value) {
    if (key == QStringLiteral("reduced-motion")) reducedMotion_ = preference(value);
    else if (key == QStringLiteral("contrast")) highContrast_ = preference(value);
    else if (key == QStringLiteral("color-scheme")) colorScheme_ = scheme(value);
}

void PlatformAppearance::settingChanged(const QString& ns, const QString& key, const QDBusVariant& value) {
    if (ns != appearance) return;
    apply(key, value.variant());
    emit changed();
}
}
