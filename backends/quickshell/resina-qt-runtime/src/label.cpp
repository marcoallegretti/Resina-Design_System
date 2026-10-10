#include "label.h"
#include "resina-qt-runtime/src/label.cxx.h"
#include "resina-qt-runtime/src/snapshot.cxx.h"
#include <QCoreApplication>
#include <QMetaObject>
#include <QThread>
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

QString CommandSnapshotResolver::resolveSnapshot(const QString& request, QObject* producer) const {
    const NativeLabelMeasure measurement(producer);
    return resolve_snapshot(request, measurement);
}
}
