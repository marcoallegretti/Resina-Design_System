#include "platform.h"
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusServiceWatcher>
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

// The portal types these keys as uint32 and specifies that any other value means "no preference".
QVariant preference(const QVariant& value) {
    return QVariant(value.metaType() == QMetaType::fromType<uint>() && value.toUInt() == 1);
}

QVariant scheme(const QVariant& value) {
    if (value.metaType() == QMetaType::fromType<uint>() && value.toUInt() == 1) return QStringLiteral("dark");
    if (value.metaType() == QMetaType::fromType<uint>() && value.toUInt() == 2) return QStringLiteral("light");
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
    auto* watcher = new QDBusServiceWatcher(service, bus, QDBusServiceWatcher::WatchForOwnerChange, this);
    connect(watcher, &QDBusServiceWatcher::serviceOwnerChanged, this,
            [this](const QString&, const QString&, const QString& owner) {
        if (!owner.isEmpty()) {
            read();
            return;
        }
        ++reads_;
        reducedMotion_ = highContrast_ = colorScheme_ = QVariant();
        error_ = QStringLiteral("platform appearance portal left the session bus");
        emit changed();
    });
    read();
}

void PlatformAppearance::read() {
    auto call = QDBusMessage::createMethodCall(service, path, interface, QStringLiteral("ReadAll"));
    call << QStringList{appearance};
    const quint64 current = ++reads_;
    auto* watcher = new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(call), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, current](QDBusPendingCallWatcher* finished) {
        finished->deleteLater();
        if (current != reads_) return;
        const QDBusPendingReply<AppearanceSettings> reply = *finished;
        ready_ = true;
        reducedMotion_ = highContrast_ = colorScheme_ = QVariant();
        if (reply.isError()) {
            error_ = QStringLiteral("platform appearance portal is unavailable: ") + reply.error().message();
        } else {
            error_.clear();
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
    if (ns != appearance || !ready_ || !error_.isEmpty()) return;
    apply(key, value.variant());
    emit changed();
}
}
