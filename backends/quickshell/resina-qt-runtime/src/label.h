#ifndef RESINA_QT_RUNTIME_LABEL_H
#define RESINA_QT_RUNTIME_LABEL_H

#include <QObject>
#include <QPointer>
#include <QString>
#include <QtQml/qqmlregistration.h>

namespace resina::qt {
class NativeLabelMeasure {
public:
    explicit NativeLabelMeasure(QObject* producer) : producer_(producer) {}
    QString measure(const QString& request) const;

private:
    QPointer<QObject> producer_;
};

class CommandLabelResolver : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    using QObject::QObject;
    Q_INVOKABLE QString resolveLabel(const QString& request, QObject* producer) const;
};

class CommandSnapshotResolver : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    using QObject::QObject;
    Q_INVOKABLE QString resolveSnapshot(const QString& request, QObject* producer) const;
    Q_INVOKABLE QString resolvePaintedSnapshot(const QString& request, QObject* producer, const QString& options) const;
};
}
#endif
