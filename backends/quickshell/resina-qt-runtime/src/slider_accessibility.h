#ifndef RESINA_QT_RUNTIME_SLIDER_ACCESSIBILITY_H
#define RESINA_QT_RUNTIME_SLIDER_ACCESSIBILITY_H

#include <QJsonObject>
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>

namespace resina::qt {
// Native semantic leaf. Its composing owner supplies qualified presentation
// semantics and adopts every candidate before publishing a new complete frame.
class SliderAccessibilityItem : public QQuickItem {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString publication READ publication NOTIFY publicationChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)
    Q_PROPERTY(bool sourceAvailable READ sourceAvailable WRITE setSourceAvailable NOTIFY sourceAvailableChanged)
public:
    explicit SliderAccessibilityItem(QQuickItem* parent = nullptr);
    QString publication() const { return publication_; }
    QString lastError() const { return error_; }
    bool sourceAvailable() const { return sourceAvailable_; }
    void setSourceAvailable(bool available);
    Q_INVOKABLE bool publish(const QString& publication);
    Q_INVOKABLE void invalidate();
    QString nativeValueText() const;
    Q_INVOKABLE QString accessibleValueText() const;
    QJsonObject semantics() const { return semantics_; }
    double stepSize() const { return stepSize_; }
    bool available() const;
    void adjust(const QJsonObject& adjustment);
signals:
    void publicationChanged();
    void lastErrorChanged();
    void sourceAvailableChanged();
    // Carries reference candidate, candidateBase and presentation revision.
    // No Qt property write or product effect is performed by this bridge.
    void adjustmentRequested(QString candidate);
private:
    QString publication_;
    QString error_;
    QJsonObject semantics_;
    double stepSize_ = 0;
    bool sourceAvailable_ = false;
    bool delivering_ = false;
    void setError(const QString& error);
    void notifyState();
};
}
#endif
