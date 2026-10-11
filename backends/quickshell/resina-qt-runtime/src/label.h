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
    Q_PROPERTY(QString publication READ publication NOTIFY publicationChanged)
    QML_ELEMENT

public:
    using QObject::QObject;
    QString publication() const { return publication_; }
    // The composing owner commits only an already validated native view snapshot.
    Q_INVOKABLE void publishSnapshot(const QString& publication);
    Q_INVOKABLE QString resolveSnapshot(const QString& request, QObject* producer) const;
    Q_INVOKABLE QString resolvePaintedSnapshot(const QString& request, QObject* producer, const QString& options) const;
    Q_INVOKABLE QString qualifySnapshot(const QString& request, QObject* producer, const QString& options) const;
    Q_INVOKABLE QString composeRequest(const QString& request, const QString& activation, bool hovered, bool focusable) const;
    Q_INVOKABLE bool matchesNativeContext(QObject* item, double x, double y, double deviceScale) const;
signals:
    void publicationChanged();
private:
    QString publication_;
};
}
#endif
