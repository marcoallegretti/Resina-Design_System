#include "pointer.h"
#include "resina-qt-runtime/src/pointer.cxx.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QMouseEvent>
#include <QQuickWindow>
#include <QScopedValueRollback>
#include <QThread>
#include <QTransform>
#include <stdexcept>

namespace resina::qt {
namespace {
QString pointerId(const QMouseEvent* event) {
    return QStringLiteral("qt-mouse-%1-%2")
        .arg(reinterpret_cast<quintptr>(event->pointingDevice()), 0, 16).arg(event->points().first().id());
}
bool identity(const QQuickItem* item) {
    bool valid = false;
    const auto transform = item->itemTransform(nullptr, &valid);
    return valid && transform.isAffine() && transform.m11() == 1 && transform.m22() == 1
        && transform.m12() == 0 && transform.m21() == 0
        && qIsFinite(transform.dx()) && qIsFinite(transform.dy());
}
QString pointerEvent(const char* kind, const QString& id, bool inside) {
    return QString::fromUtf8(QJsonDocument(QJsonObject{{"kind", kind},
        {"id", id}, {"inside", inside}}).toJson(QJsonDocument::Compact));
}
}

struct CommandPointerItem::Target {
    struct Ancestor {
        QPointer<QQuickItem> item;
        bool clipped;
        QPointF origin;
        QSizeF size;
    };
    rust::Box<PointerTarget> region;
    QPointF ownerOrigin;
    QPointF frameOrigin;
    QSizeF size;
    QPointer<QQuickWindow> window;
    QSize windowSize;
    QList<Ancestor> ancestors;
};

CommandPointerItem::CommandPointerItem(QQuickItem* parent) : QQuickItem(parent) {
    setAcceptedMouseButtons(Qt::LeftButton);
    setKeepMouseGrab(true);
    connect(this, &QQuickItem::windowChanged, this, [this] { invalidate(); observeWindow(); });
    connect(this, &QQuickItem::parentChanged, this, [this] { invalidate(); });
}

CommandPointerItem::~CommandPointerItem() {
    cancel();
    // QQuickItem teardown emits signals after derived members are destroyed.
    disconnect(this, nullptr, this, nullptr);
    disconnect(captureConnection_);
    disconnect(controllerConnection_);
    disconnect(frameConnection_);
    disconnect(deviceConnection_);
    for (const auto& connection : windowConnections_) disconnect(connection);
}

void CommandPointerItem::setError(const QString& error) {
    if (error_ != error) { error_ = error; emit lastErrorChanged(); }
}

void CommandPointerItem::setController(QObject* controller) {
    if (controller_ == controller) return;
    invalidate();
    if (cancelPending_) {
        const QPointer<QObject> replacement(controller);
        QMetaObject::invokeMethod(this, [this, replacement] { setController(replacement); }, Qt::QueuedConnection);
        return;
    }
    disconnect(captureConnection_);
    disconnect(controllerConnection_);
    controller_ = controller;
    if (controller_) {
        captureConnection_ = connect(controller_, SIGNAL(captureRequested(QString,bool)),
            this, SLOT(captureRequested(QString,bool)));
        if (!captureConnection_ || !controller_->inherits("CommandController")) {
            disconnect(captureConnection_);
            controller_ = nullptr;
            setError("pointer owner requires a CommandController");
        } else {
            controllerConnection_ = connect(controller_, &QObject::destroyed, this, [this] {
                invalidate();
                emit controllerChanged();
            });
        }
    }
    emit controllerChanged();
}

void CommandPointerItem::setCoordinateFrame(QQuickItem* frame) {
    if (frame_ == frame) return;
    invalidate();
    disconnect(frameConnection_);
    frame_ = frame;
    if (frame_) frameConnection_ = connect(frame_, &QObject::destroyed, this, [this] {
        invalidate();
        emit coordinateFrameChanged();
    });
    emit coordinateFrameChanged();
}

void CommandPointerItem::observeWindow() {
    for (const auto& connection : windowConnections_) disconnect(connection);
    windowConnections_.clear();
    if (!window()) return;
    windowConnections_.append(connect(window(), &QWindow::activeChanged, this, [this] {
        if (!window()->isActive()) cancel();
    }));
    windowConnections_.append(connect(window(), &QWindow::visibleChanged, this, [this] {
        if (!window()->isVisible()) cancel();
    }));
    windowConnections_.append(connect(window(), &QQuickWindow::afterAnimating, this, [this] {
        if (target_ && !contextValid()) invalidate();
    }));
}

bool CommandPointerItem::contextValid() const {
    if (!(target_ && frame_ && controller_ && window() && frame_->window() == window()
        && target_->window == window() && identity(this) && identity(frame_)
        && target_->ownerOrigin == mapToScene(QPointF(0, 0))
        && target_->frameOrigin == frame_->mapToScene(QPointF(0, 0))
        && target_->size == QSizeF(width(), height()) && target_->windowSize == window()->size())) return false;
    auto ancestor = parentItem();
    for (const auto& recorded : target_->ancestors) {
        if (recorded.item != ancestor || !ancestor || recorded.clipped != ancestor->clip()) return false;
        if (recorded.clipped && (!identity(ancestor)
            || recorded.origin != ancestor->mapToScene(QPointF(0, 0))
            || recorded.size != QSizeF(ancestor->width(), ancestor->height()))) return false;
        ancestor = ancestor->parentItem();
    }
    return !ancestor;
}

QString CommandPointerItem::targetPublication() const {
    return contextValid() ? target_publication(*target_->region) : QString();
}

bool CommandPointerItem::prepareTarget(const QString& request) {
    if (preparing_) { setError("pointer target publication is in progress"); return false; }
    const QScopedValueRollback<bool> publicationGuard(preparing_, true);
    invalidate();
    try {
        if (QThread::currentThread() != thread() || !controller_ || !frame_ || !window()
            || frame_->window() != window() || !identity(this) || !identity(frame_))
            throw std::runtime_error("target requires a live UI-thread controller and identity-mapped native frame");
        const auto origin = frame_->mapFromItem(this, QPointF(0, 0));
        auto region = prepare_target(request, origin.x(), origin.y(), width(), height());
        const auto windowOrigin = frame_->mapFromScene(QPointF(0, 0));
        prepare_target(request, windowOrigin.x(), windowOrigin.y(), window()->width(), window()->height());
        QList<Target::Ancestor> ancestors;
        for (auto ancestor = parentItem(); ancestor; ancestor = ancestor->parentItem()) {
            const auto ancestorOrigin = ancestor->mapToScene(QPointF(0, 0));
            ancestors.append({ancestor, ancestor->clip(), ancestorOrigin,
                QSizeF(ancestor->width(), ancestor->height())});
            if (ancestor->clip()) {
                if (!identity(ancestor)) throw std::runtime_error("native target clipping must be axis-aligned without scale");
                const auto local = frame_->mapFromItem(ancestor, QPointF(0, 0));
                prepare_target(request, local.x(), local.y(), ancestor->width(), ancestor->height());
            }
        }
        target_ = std::make_unique<Target>(Target{std::move(region), mapToScene(QPointF(0, 0)),
            frame_->mapToScene(QPointF(0, 0)), QSizeF(width(), height()), window(), window()->size(), ancestors});
        setError(QString());
        emit targetPublicationChanged();
        if (!contextValid()) {
            invalidate();
            setError("pointer target context changed during publication");
            return false;
        }
        return true;
    } catch (const std::exception& error) {
        setError(QString::fromUtf8(error.what()));
        return false;
    }
}

bool CommandPointerItem::contains(const QPointF& point) const {
    if (!contextValid() || cancelPending_) return false;
    const auto local = frame_->mapFromItem(this, point);
    try { return target_contains(*target_->region, local.x(), local.y()); }
    catch (const std::exception&) { return false; }
}

bool CommandPointerItem::dispatch(const QString& event) {
    bool accepted = false;
    if (!controller_ || QThread::currentThread() != thread() || controller_->thread() != thread()
        || !QMetaObject::invokeMethod(controller_, "dispatch", Qt::DirectConnection,
        Q_RETURN_ARG(bool, accepted), Q_ARG(QString, event))) {
        setError("pointer owner lost its UI-thread command controller");
        return false;
    }
    setError(accepted ? QString() : controller_->property("lastError").toString());
    return accepted;
}

void CommandPointerItem::cancel() {
    if (!controller_) return;
    const auto hold = QJsonDocument::fromJson(controller_->property("publication").toString().toUtf8())
        .object().value("state").toObject().value("hold").toObject();
    if (pointerId_.isEmpty() || hold.value("kind").toString() != "pointer" || hold.value("id").toString() != pointerId_) {
        cancelPending_ = false;
        return;
    }
    const auto event = QString::fromUtf8(QJsonDocument(QJsonObject{{"kind", "pointerCancel"},
        {"id", pointerId_}}).toJson(QJsonDocument::Compact));
    if (dispatch(event)) {
        cancelPending_ = false;
        return;
    }
    if (cancelPending_) return;
    cancelPending_ = true;
    QMetaObject::invokeMethod(this, [this] { cancelPending_ = false; cancel(); }, Qt::QueuedConnection);
}

void CommandPointerItem::invalidate() {
    const bool hadTarget = bool(target_);
    target_.reset();
    cancel();
    if (hadTarget) emit targetPublicationChanged();
}

void CommandPointerItem::captureRequested(QString id, bool acquire) {
    if (id != pointerId_) return;
    if (acquire) {
        if (!delivery_ || !contextValid() || !isEnabled() || !isVisible()) { cancel(); return; }
        delivery_->setExclusiveGrabber(delivery_->point(0), this);
        if (delivery_->exclusiveGrabber(delivery_->point(0)) != this) { cancel(); return; }
        disconnect(deviceConnection_);
        deviceConnection_ = connect(delivery_->pointingDevice(), &QObject::destroyed, this, [this] { cancel(); });
    } else {
        releasing_ = true;
        ungrabMouse();
        releasing_ = false;
        disconnect(deviceConnection_);
        pointerId_.clear();
    }
}

void CommandPointerItem::mousePressEvent(QMouseEvent* event) {
    event->ignore();
    if (event->button() != Qt::LeftButton || event->modifiers() != Qt::NoModifier
        || !window() || !window()->isActive() || !contains(event->position())) return;
    const auto previous = QJsonDocument::fromJson(controller_->property("publication").toString().toUtf8())
        .object().value("state").toObject().value("hold");
    if (!previous.isNull()) return;
    pointerId_ = pointerId(event);
    delivery_ = event;
    const bool accepted = dispatch(pointerEvent("pointerDown", pointerId_, true));
    delivery_ = nullptr;
    const auto state = controller_ ? QJsonDocument::fromJson(controller_->property("publication").toString().toUtf8())
        .object().value("state").toObject().value("hold").toObject() : QJsonObject();
    event->setAccepted(accepted && !pointerId_.isEmpty() && state.value("id").toString() == pointerId_ && contextValid());
    if (!event->isAccepted()) cancel();
}

void CommandPointerItem::mouseMoveEvent(QMouseEvent* event) {
    if (pointerId_.isEmpty() || pointerId(event) != pointerId_) { event->ignore(); return; }
    if (!contextValid()) { invalidate(); event->ignore(); return; }
    if (!dispatch(pointerEvent("pointerMove", pointerId_, contains(event->position())))) cancel();
    event->accept();
}

void CommandPointerItem::mouseReleaseEvent(QMouseEvent* event) {
    if (event->button() != Qt::LeftButton || pointerId_.isEmpty() || pointerId(event) != pointerId_) { event->ignore(); return; }
    if (!contextValid()) { invalidate(); event->ignore(); return; }
    if (!dispatch(pointerEvent("pointerUp", pointerId_, contains(event->position())))) cancel();
    event->accept();
}

void CommandPointerItem::mouseUngrabEvent() { if (!releasing_) cancel(); }

void CommandPointerItem::itemChange(ItemChange change, const ItemChangeData& data) {
    QQuickItem::itemChange(change, data);
    if ((change == ItemEnabledHasChanged && !isEnabled())
        || (change == ItemVisibleHasChanged && !isVisible())) cancel();
}

void CommandPointerItem::geometryChange(const QRectF& current, const QRectF& previous) {
    QQuickItem::geometryChange(current, previous);
    if (current != previous) invalidate();
}
}
