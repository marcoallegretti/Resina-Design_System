#ifndef RESINA_QT_RUNTIME_PLATFORM_H
#define RESINA_QT_RUNTIME_PLATFORM_H

#include <QDBusVariant>
#include <QObject>
#include <QString>
#include <QVariant>
#include <QtQml/qqmlregistration.h>

namespace resina::qt {
class PlatformAppearance : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool ready READ ready NOTIFY changed)
    Q_PROPERTY(QString error READ error NOTIFY changed)
    Q_PROPERTY(QVariant reducedMotion READ reducedMotion NOTIFY changed)
    Q_PROPERTY(QVariant highContrast READ highContrast NOTIFY changed)
    Q_PROPERTY(QVariant colorScheme READ colorScheme NOTIFY changed)
public:
    explicit PlatformAppearance(QObject* parent = nullptr);
    bool ready() const { return ready_; }
    QString error() const { return error_; }
    QVariant reducedMotion() const { return reducedMotion_; }
    QVariant highContrast() const { return highContrast_; }
    QVariant colorScheme() const { return colorScheme_; }
signals:
    void changed();
private slots:
    void settingChanged(const QString& ns, const QString& key, const QDBusVariant& value);
private:
    void read();
    void apply(const QString& key, const QVariant& value);
    quint64 reads_ = 0;
    bool ready_ = false;
    QString error_;
    QVariant reducedMotion_;
    QVariant highContrast_;
    QVariant colorScheme_;
};
}
#endif
