#ifndef RESINA_QT_RUNTIME_POINTER_H
#define RESINA_QT_RUNTIME_POINTER_H

#include <QPointer>
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>
#include <memory>

namespace resina::qt {
class CommandPointerItem : public QQuickItem {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QObject* controller READ controller WRITE setController NOTIFY controllerChanged)
    Q_PROPERTY(QQuickItem* coordinateFrame READ coordinateFrame WRITE setCoordinateFrame NOTIFY coordinateFrameChanged)
    Q_PROPERTY(QString targetPublication READ targetPublication NOTIFY targetPublicationChanged)
    Q_PROPERTY(QString lastError READ lastError NOTIFY lastErrorChanged)
public:
    explicit CommandPointerItem(QQuickItem* parent = nullptr);
    ~CommandPointerItem() override;
    QObject* controller() const { return controller_; }
    void setController(QObject* controller);
    QQuickItem* coordinateFrame() const { return frame_; }
    void setCoordinateFrame(QQuickItem* frame);
    QString targetPublication() const;
    QString lastError() const { return error_; }
    Q_INVOKABLE bool prepareTarget(const QString& request);
    Q_INVOKABLE void invalidate();
    bool contains(const QPointF& point) const override;
signals:
    void controllerChanged();
    void coordinateFrameChanged();
    void targetPublicationChanged();
    void lastErrorChanged();
protected:
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void mouseUngrabEvent() override;
    void itemChange(ItemChange change, const ItemChangeData& data) override;
    void geometryChange(const QRectF& current, const QRectF& previous) override;
private slots:
    void captureRequested(QString id, bool acquire);
private:
    struct Target;
    std::unique_ptr<Target> target_;
    QPointer<QObject> controller_;
    QPointer<QQuickItem> frame_;
    QMetaObject::Connection captureConnection_;
    QMetaObject::Connection controllerConnection_;
    QMetaObject::Connection frameConnection_;
    QMetaObject::Connection deviceConnection_;
    QList<QMetaObject::Connection> windowConnections_;
    QMouseEvent* delivery_ = nullptr;
    bool releasing_ = false;
    bool cancelPending_ = false;
    bool preparing_ = false;
    QString error_;
    QString pointerId_;
    bool contextValid() const;
    bool dispatch(const QString& event);
    void cancel();
    void setError(const QString& error);
    void observeWindow();
};
}
#endif
