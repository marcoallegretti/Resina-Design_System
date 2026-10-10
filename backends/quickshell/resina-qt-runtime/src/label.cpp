#include "label.h"
#include "context.h"
#include "resina-qt-runtime/src/label.cxx.h"
#include "resina-qt-runtime/src/snapshot.cxx.h"
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMetaObject>
#include <QThread>
#include <QQuickWindow>
#include <QVariant>
#include <stdexcept>

namespace resina::qt {
QString NativeLabelMeasure::measure(const QString& request) const {
    const auto* application = QCoreApplication::instance();
    if (!application || QThread::currentThread() != application->thread()
        || !producer_ || producer_->thread() != application->thread())
        throw std::runtime_error("native label measurement requires a live UI-thread producer");
    QVariant result;
    const bool invoked = QMetaObject::invokeMethod(producer_.data(), "measureJson",
        Qt::DirectConnection, Q_RETURN_ARG(QVariant, result), Q_ARG(QVariant, QVariant(request)));
    if (!invoked || !producer_ || result.metaType() != QMetaType::fromType<QString>())
        throw std::runtime_error("native label producer did not return a JSON string");
    return result.toString();
}

QString CommandLabelResolver::resolveLabel(const QString& request, QObject* producer) const {
    // The QML-owned producer is observed only during this synchronous call;
    // no event loop, queued invocation or raw pointer survives in Rust.
    const NativeLabelMeasure measurement(producer);
    return resolve_label(request, measurement);
}

void CommandSnapshotResolver::publishSnapshot(const QString& publication) {
    publishFrame(publication, QString(), nullptr);
}

void CommandSnapshotResolver::publishFrame(const QString& publication, const QString& motion, QObject* frame) {
    const auto* application = QCoreApplication::instance();
    if (!application || QThread::currentThread() != application->thread()
        || thread() != application->thread()) return;
    const bool changed = publication_ != publication;
    const bool motionChangedValue = motion_ != motion;
    if (frame && frame->thread() != application->thread()) return;
    const bool frameChangedValue = frame_ != frame;
    const QPointer<QObject> requestedFrame(frame);
    const QString requestedMotion = motion;
    const QPointer<CommandSnapshotResolver> owner(this);
    disconnect(frameDestroyed_);
    publication_ = publication;
    motion_ = motion;
    frame_ = frame;
    if (frame) frameDestroyed_ = connect(frame, &QObject::destroyed, this, [this] { publishSnapshot(QString()); });
    if (frameChangedValue) emit frameChanged();
    if (owner && changed && frame_ == requestedFrame && publication_ == publication) emit publicationChanged();
    // An observer may retire/reconfigure or destroy this native owner.
    if (owner && motionChangedValue && frame_ == requestedFrame && motion_ == requestedMotion) emit motionChanged();
}

QString CommandSnapshotResolver::resolveMotionSnapshot(const QString& request, QObject* producer, const QString& options, const QString& motion, bool render) const {
    const NativeLabelMeasure measurement(producer);
    return resolve_motion_snapshot(request, measurement, options, motion, render);
}

QString CommandSnapshotResolver::retargetMotion(const QString& source) const { return retarget_motion(source); }
QString CommandSnapshotResolver::validateMotionChannels(const QString& source) const { return validate_motion_channels(source); }

QString CommandSnapshotResolver::resolveSnapshot(const QString& request, QObject* producer) const {
    const NativeLabelMeasure measurement(producer);
    return resolve_snapshot(request, measurement);
}

QString CommandSnapshotResolver::resolvePaintedSnapshot(const QString& request, QObject* producer, const QString& options) const {
    const NativeLabelMeasure measurement(producer);
    return resolve_painted_snapshot(request, measurement, options);
}

QString CommandSnapshotResolver::qualifySnapshot(const QString& request, QObject* producer, const QString& options) const {
    const NativeLabelMeasure measurement(producer);
    return qualify_snapshot(request, measurement, options);
}

QString CommandSnapshotResolver::composeRequest(const QString& request, const QString& activation, bool hovered, bool focusable) const {
    return compose_request(request, activation, hovered, focusable);
}

bool CommandSnapshotResolver::matchesNativeContext(QObject* object, double x, double y, double deviceScale) const {
    const auto* application = QCoreApplication::instance();
    // Reject before inspecting item state unless the call is on the owning UI thread.
    if (!application || QThread::currentThread() != application->thread()
        || thread() != application->thread() || !object || object->thread() != application->thread()
        || !qIsFinite(x) || !qIsFinite(y) || !qIsFinite(deviceScale) || deviceScale <= 0) return false;
    const auto* item = qobject_cast<QQuickItem*>(object);
    if (!item || !item->window() || item->window()->thread() != application->thread()) return false;
    QTransform transform;
    return translationOnly(item, &transform)
        && qAbs(transform.dx() - x) <= 1.0 / 1024
        && qAbs(transform.dy() - y) <= 1.0 / 1024
        && item->window()->devicePixelRatio() == deviceScale;
}

bool CommandClock::onUiThread() const {
    const auto* application = QCoreApplication::instance();
    return application && QThread::currentThread() == application->thread() && thread() == application->thread();
}

QString CommandClock::sample() const {
    if (!onUiThread() || !clock_.isValid() || !QElapsedTimer::isMonotonic())
        return QStringLiteral("{\"error\":\"Qt command clock requires a started monotonic UI-thread clock\"}");
    const double time = double(clock_.nsecsElapsed()) / 1e9;
    return QString::fromUtf8(QJsonDocument(QJsonObject{{"time",time}}).toJson(QJsonDocument::Compact));
}

QString CommandClock::restart() {
    if (!onUiThread() || !QElapsedTimer::isMonotonic())
        return QStringLiteral("{\"error\":\"Qt command clock requires the monotonic UI thread\"}");
    QElapsedTimer next;
    next.start();
    const double previous = clock_.isValid() ? double(clock_.durationTo(next).count()) / 1e9 : 0.0;
    clock_ = next;
    return QString::fromUtf8(QJsonDocument(QJsonObject{{"previousTime",previous}}).toJson(QJsonDocument::Compact));
}

void CommandClock::invalidate() { if (onUiThread()) clock_.invalidate(); }

}
