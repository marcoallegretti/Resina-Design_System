#include "pointer.h"
#include "context.h"
#include "resina-qt-runtime/src/pointer.cxx.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QMouseEvent>
#include <QEnterEvent>
#include <QQuickWindow>
#include <QtCore/qscopeguard.h>
#include <QThread>
#include <QTimer>
#include <stdexcept>

namespace resina::qt {
namespace {
QJsonObject activationState(QObject* controller) {
    const auto publication = QJsonDocument::fromJson(controller->property("publication").toString().toUtf8()).object();
    const auto activation = controller->inherits("ToggleController")
        ? publication.value("activation").toObject() : publication;
    return activation.value("state").toObject();
}
QString pointerId(const QMouseEvent* event) {
    return QStringLiteral("qt-mouse-%1-%2")
        .arg(reinterpret_cast<quintptr>(event->pointingDevice()), 0, 16).arg(event->points().first().id());
}
QString pointerEvent(const char* kind, const QString& id, bool inside) {
    return QString::fromUtf8(QJsonDocument(QJsonObject{{"kind", kind},
        {"id", id}, {"inside", inside}}).toJson(QJsonDocument::Compact));
}
}

PendingPointerCancellation::PendingPointerCancellation(QObject* controller, QString id)
    : QObject(controller), controller_(controller), id_(std::move(id)) {
    connect(controller, SIGNAL(publicationChanged()), this, SLOT(observePublication()));
    QTimer::singleShot(0, this, [this] { deliver(); });
}
void PendingPointerCancellation::observePublication() {
    const auto result = QJsonDocument::fromJson(controller_->property("publication").toString().toUtf8()).object();
    const auto activation = controller_->inherits("ToggleController") ? result.value("activation").toObject() : result;
    const auto capture = activation.value("capture").toObject();
    // A released gesture cannot be resurrected by reusing its native pointer id.
    if (capture.value("kind").toString() == "release" && capture.value("id").toString() == id_)
        ended_ = true;
}
void PendingPointerCancellation::deliver() {
    const auto hold = controller_ ? activationState(controller_).value("hold").toObject() : QJsonObject();
    if (ended_ || hold.value("kind").toString() != "pointer" || hold.value("id").toString() != id_) {
        deleteLater();
        return;
    }
    const QString event = QString::fromUtf8(QJsonDocument(QJsonObject{{"kind", "pointerCancel"},
        {"id", id_}}).toJson(QJsonDocument::Compact));
    const QPointer<PendingPointerCancellation> self(this);
    bool accepted = false;
    const bool invoked = QMetaObject::invokeMethod(controller_, "dispatch", Qt::DirectConnection,
        Q_RETURN_ARG(bool, accepted), Q_ARG(QString, event));
    if (!self) return;
    if (invoked && !accepted) QTimer::singleShot(0, this, [this] { deliver(); });
    else deleteLater();
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
    connect(this, &QQuickItem::windowChanged, this, [this] {
        const QPointer<CommandPointerItem> self(this);
        invalidate();
        if (self) observeWindow();
    });
    connect(this, &QQuickItem::parentChanged, this, [this] { invalidate(); });
}

CommandPointerItem::~CommandPointerItem() {
    if (observedWindow_) observedWindow_->removeEventFilter(this);
    blockSignals(true);
    clearHover();
    if (controller_ && !pointerId_.isEmpty()) {
        const auto hold = activationState(controller_).value("hold").toObject();
        if (hold.value("kind").toString() == "pointer" && hold.value("id").toString() == pointerId_)
            new PendingPointerCancellation(controller_, pointerId_);
    }
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
    const QPointer<QObject> replacement(controller);
    const QPointer<CommandPointerItem> self(this);
    invalidate();
    if (!self) return;
    if (cancelPending_) {
        QMetaObject::invokeMethod(this, [this, replacement] { setController(replacement); }, Qt::QueuedConnection);
        return;
    }
    disconnect(captureConnection_);
    disconnect(controllerConnection_);
    controller_ = replacement;
    if (controller_) {
        captureConnection_ = connect(controller_, SIGNAL(captureRequested(QString,bool)),
            this, SLOT(captureRequested(QString,bool)));
        if (!captureConnection_ || (!controller_->inherits("CommandController") && !controller_->inherits("ToggleController"))) {
            disconnect(captureConnection_);
            controller_ = nullptr;
            setError("pointer owner requires a CommandController or ToggleController");
            if (!self) return;
        } else {
            controllerConnection_ = connect(controller_, &QObject::destroyed, this, [this] {
                const QPointer<CommandPointerItem> self(this);
                invalidate();
                if (self) emit controllerChanged();
            });
        }
    }
    emit controllerChanged();
}

void CommandPointerItem::setCoordinateFrame(QQuickItem* frame) {
    if (frame_ == frame) return;
    const QPointer<QQuickItem> replacement(frame);
    const QPointer<CommandPointerItem> self(this);
    invalidate();
    if (!self) return;
    disconnect(frameConnection_);
    frame_ = replacement;
    if (frame_) frameConnection_ = connect(frame_, &QObject::destroyed, this, [this] {
        const QPointer<CommandPointerItem> self(this);
        invalidate();
        if (self) emit coordinateFrameChanged();
    });
    emit coordinateFrameChanged();
}

bool CommandPointerItem::hovered() const {
    return hovered_ && contextValid() && isVisible() && window()->isVisible();
}

void CommandPointerItem::setAcceptsNewGestures(bool value) {
    if (acceptsNewGestures_ != value) { acceptsNewGestures_ = value; emit acceptsNewGesturesChanged(); }
}

void CommandPointerItem::clearHover() {
    for (const auto& observation : hoverObservations_) disconnect(observation.destroyed);
    hoverObservations_.clear();
    enteringPosition_.reset();
    refreshHover();
}

void CommandPointerItem::refreshHover() {
    bool current = false;
    if (contextValid() && isVisible() && window()->isVisible()) {
        if (enteringPosition_) {
            const auto local = frame_->mapFromScene(*enteringPosition_);
            try { current = target_contains(*target_->region, local.x(), local.y()); }
            catch (const std::exception&) { current = false; }
        }
        for (const auto& observation : hoverObservations_) {
            if (current) break;
            // A fresh native position is independent of activation/capture/availability.
            const auto local = frame_->mapFromScene(observation.scenePosition);
            try { current = target_contains(*target_->region, local.x(), local.y()); }
            catch (const std::exception&) { current = false; }
            if (current) break;
        }
    }
    if (current != hovered_) { hovered_ = current; emit hoveredChanged(); }
}

void CommandPointerItem::observeHover(const QPointingDevice* device, const QPointF& scenePosition) {
    if (!device || device->type() != QInputDevice::DeviceType::Mouse
        || !window() || !isVisible() || !window()->isVisible()) return;
    enteringPosition_.reset();
    if (!hoverObservations_.contains(device)) {
        const auto destroyed = connect(device, &QObject::destroyed, this, [this, device] {
            hoverObservations_.remove(device);
            refreshHover();
        });
        hoverObservations_.insert(device, {scenePosition, destroyed});
    } else hoverObservations_[device].scenePosition = scenePosition;
    refreshHover();
}

bool CommandPointerItem::eventFilter(QObject* watched, QEvent* event) {
    const QPointer<CommandPointerItem> self(this);
    const QPointer<QObject> receiver(watched);
    if (watched == observedWindow_) {
        switch (event->type()) {
        case QEvent::MouseMove:
        case QEvent::MouseButtonPress:
        case QEvent::MouseButtonRelease:
        case QEvent::MouseButtonDblClick: {
            const auto mouse = static_cast<QMouseEvent*>(event);
            if (mouse->source() == Qt::MouseEventNotSynthesized)
                observeHover(mouse->pointingDevice(), mouse->scenePosition());
            break;
        }
        case QEvent::Enter: {
            const auto enter = static_cast<QEnterEvent*>(event);
            // Qt's Wayland enter path supplies no device identity. Keep its position
            // provisional until a mouse event supplies the actual device; never invent
            // a second device from QEnterEvent's default primary-pointing-device value.
            enteringPosition_ = enter->scenePosition();
            refreshHover();
            break;
        }
        case QEvent::Leave: clearHover(); break;
        default: break;
        }
    }
    if (!receiver) return true;
    return self ? QQuickItem::eventFilter(watched, event) : false;
}

void CommandPointerItem::observeWindow() {
    const QPointer<CommandPointerItem> self(this);
    if (observedWindow_) observedWindow_->removeEventFilter(this);
    clearHover();
    if (!self) return;
    observedWindow_ = window();
    if (observedWindow_) observedWindow_->installEventFilter(this);
    for (const auto& connection : windowConnections_) disconnect(connection);
    windowConnections_.clear();
    if (!window()) return;
    windowConnections_.append(connect(window(), &QWindow::activeChanged, this, [this] {
        if (!window()->isActive()) {
            const QPointer<CommandPointerItem> self(this);
            clearHover();
            if (self) cancel();
        }
    }));
    windowConnections_.append(connect(window(), &QWindow::visibleChanged, this, [this] {
        if (!window()->isVisible()) {
            const QPointer<CommandPointerItem> self(this);
            clearHover();
            if (self) cancel();
        }
    }));
    windowConnections_.append(connect(window(), &QQuickWindow::afterAnimating, this, [this] {
        if (target_ && !contextValid()) invalidate();
    }));
}

bool CommandPointerItem::contextValid() const {
    if (!(target_ && frame_ && controller_ && window() && frame_->window() == window()
        && target_->window == window() && translationOnly(this) && translationOnly(frame_)
        && target_->ownerOrigin == mapToScene(QPointF(0, 0))
        && target_->frameOrigin == frame_->mapToScene(QPointF(0, 0))
        && target_->size == QSizeF(width(), height()) && target_->windowSize == window()->size())) return false;
    auto ancestor = parentItem();
    for (const auto& recorded : target_->ancestors) {
        if (recorded.item != ancestor || !ancestor || recorded.clipped != ancestor->clip()) return false;
        if (recorded.clipped && (!translationOnly(ancestor)
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
    const QPointer<CommandPointerItem> self(this);
    preparing_ = true;
    const auto publicationGuard = qScopeGuard([self] { if (self) self->preparing_ = false; });
    invalidate();
    if (!self) return false;
    try {
        if (QThread::currentThread() != thread() || !controller_ || !frame_ || !window()
            || frame_->window() != window() || !translationOnly(this) || !translationOnly(frame_))
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
                if (!translationOnly(ancestor)) throw std::runtime_error("native target clipping must be axis-aligned without scale");
                const auto local = frame_->mapFromItem(ancestor, QPointF(0, 0));
                prepare_target(request, local.x(), local.y(), ancestor->width(), ancestor->height());
            }
        }
        target_ = std::make_unique<Target>(Target{std::move(region), mapToScene(QPointF(0, 0)),
            frame_->mapToScene(QPointF(0, 0)), QSizeF(width(), height()), window(), window()->size(), ancestors});
        setError(QString());
        if (!self) return false;
        refreshHover();
        if (!self) return false;
        emit targetPublicationChanged();
        if (!self) return false;
        if (!contextValid()) {
            invalidate();
            if (self) setError("pointer target context changed during publication");
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
    const QPointer<CommandPointerItem> self(this);
    const QPointer<QObject> controller(controller_);
    bool accepted = false;
    if (!controller || QThread::currentThread() != thread() || controller->thread() != thread()) {
        setError("pointer owner lost its UI-thread command controller");
        return false;
    }
    const bool invoked = QMetaObject::invokeMethod(controller, "dispatch", Qt::DirectConnection,
        Q_RETURN_ARG(bool, accepted), Q_ARG(QString, event));
    if (!self) return false;
    if (!invoked || !controller || controller_ != controller) {
        setError("pointer owner lost its UI-thread command controller");
        return false;
    }
    setError(accepted ? QString() : controller->property("lastError").toString());
    return self && accepted;
}

void CommandPointerItem::cancel() {
    if (!controller_) return;
    const auto hold = activationState(controller_).value("hold").toObject();
    if (pointerId_.isEmpty() || hold.value("kind").toString() != "pointer" || hold.value("id").toString() != pointerId_) {
        cancelPending_ = false;
        return;
    }
    const auto event = QString::fromUtf8(QJsonDocument(QJsonObject{{"kind", "pointerCancel"},
        {"id", pointerId_}}).toJson(QJsonDocument::Compact));
    const QPointer<CommandPointerItem> self(this);
    const bool accepted = dispatch(event);
    if (!self) return;
    if (accepted) {
        cancelPending_ = false;
        return;
    }
    if (cancelPending_) return;
    cancelPending_ = true;
    QMetaObject::invokeMethod(this, [this] { cancelPending_ = false; cancel(); }, Qt::QueuedConnection);
}

void CommandPointerItem::invalidate() {
    const QPointer<CommandPointerItem> self(this);
    const bool hadTarget = bool(target_);
    target_.reset();
    refreshHover();
    if (!self) return;
    cancel();
    if (self && hadTarget) emit targetPublicationChanged();
}

void CommandPointerItem::captureRequested(QString id, bool acquire) {
    const QPointer<CommandPointerItem> self(this);
    if (id != pointerId_) return;
    if (acquire) {
        if (!delivery_ || !contextValid() || !isEnabled() || !isVisible()) { cancel(); return; }
        delivery_->setExclusiveGrabber(delivery_->point(0), this);
        if (!self) return;
        if (delivery_->exclusiveGrabber(delivery_->point(0)) != this) { cancel(); return; }
        disconnect(deviceConnection_);
        deviceConnection_ = connect(delivery_->pointingDevice(), &QObject::destroyed, this, [this] { cancel(); });
    } else {
        releasing_ = true;
        ungrabMouse();
        if (!self) return;
        releasing_ = false;
        disconnect(deviceConnection_);
        pointerId_.clear();
    }
}

void CommandPointerItem::mousePressEvent(QMouseEvent* event) {
    event->ignore();
    if (!acceptsNewGestures_ || event->button() != Qt::LeftButton || event->modifiers() != Qt::NoModifier
        || !window() || !window()->isActive() || !contains(event->position())) return;
    const auto previous = activationState(controller_).value("hold");
    if (!previous.isNull()) return;
    pointerId_ = pointerId(event);
    delivery_ = event;
    const QPointer<CommandPointerItem> self(this);
    const bool accepted = dispatch(pointerEvent("pointerDown", pointerId_, true));
    if (!self) { event->ignore(); return; }
    delivery_ = nullptr;
    const auto state = controller_ ? activationState(controller_).value("hold").toObject() : QJsonObject();
    event->setAccepted(accepted && !pointerId_.isEmpty() && state.value("id").toString() == pointerId_ && contextValid());
    if (!event->isAccepted()) cancel();
}

void CommandPointerItem::mouseMoveEvent(QMouseEvent* event) {
    if (pointerId_.isEmpty() || pointerId(event) != pointerId_) { event->ignore(); return; }
    if (!contextValid()) { invalidate(); event->ignore(); return; }
    const QPointer<CommandPointerItem> self(this);
    const bool accepted = dispatch(pointerEvent("pointerMove", pointerId_, contains(event->position())));
    if (self && !accepted) cancel();
    event->setAccepted(bool(self));
}

void CommandPointerItem::mouseReleaseEvent(QMouseEvent* event) {
    if (event->button() != Qt::LeftButton || pointerId_.isEmpty() || pointerId(event) != pointerId_) { event->ignore(); return; }
    if (!contextValid()) { invalidate(); event->ignore(); return; }
    const QPointer<CommandPointerItem> self(this);
    const bool accepted = dispatch(pointerEvent("pointerUp", pointerId_, contains(event->position())));
    if (self && !accepted) cancel();
    event->setAccepted(bool(self));
}

void CommandPointerItem::mouseUngrabEvent() { if (!releasing_) cancel(); }

void CommandPointerItem::itemChange(ItemChange change, const ItemChangeData& data) {
    const QPointer<CommandPointerItem> self(this);
    QQuickItem::itemChange(change, data);
    if (!self) return;
    if (change == ItemVisibleHasChanged && !isVisible()) clearHover();
    if (!self) return;
    if ((change == ItemEnabledHasChanged && !isEnabled())
        || (change == ItemVisibleHasChanged && !isVisible())) cancel();
}

void CommandPointerItem::geometryChange(const QRectF& current, const QRectF& previous) {
    const QPointer<CommandPointerItem> self(this);
    QQuickItem::geometryChange(current, previous);
    if (self && current != previous) invalidate();
}
}
