#ifndef RESINA_QT_RUNTIME_CONTEXT_H
#define RESINA_QT_RUNTIME_CONTEXT_H

#include <QCoreApplication>
#include <QQuickItem>
#include <QQuickWindow>
#include <QThread>
#include <QTransform>

namespace resina::qt {
inline bool translationOnly(const QQuickItem* item, QTransform* sceneTransform = nullptr) {
    if (!item) return false;
    bool valid = false;
    const auto transform = item->itemTransform(nullptr, &valid);
    if (!valid || !transform.isAffine() || transform.m11() != 1 || transform.m22() != 1
        || transform.m12() != 0 || transform.m21() != 0
        || !qIsFinite(transform.dx()) || !qIsFinite(transform.dy())) return false;
    if (sceneTransform) *sceneTransform = transform;
    return true;
}

inline bool nativeContextMatches(const QObject* owner, QObject* object, double x, double y, double deviceScale) {
    const auto* application = QCoreApplication::instance();
    // Reject before inspecting item state unless the call is on the owning UI thread.
    if (!application || QThread::currentThread() != application->thread()
        || owner->thread() != application->thread() || !object || object->thread() != application->thread()
        || !qIsFinite(x) || !qIsFinite(y) || !qIsFinite(deviceScale) || deviceScale <= 0) return false;
    const auto* item = qobject_cast<QQuickItem*>(object);
    if (!item || !item->window() || item->window()->thread() != application->thread()) return false;
    QTransform transform;
    return translationOnly(item, &transform)
        && qAbs(transform.dx() - x) <= 1.0 / 1024
        && qAbs(transform.dy() - y) <= 1.0 / 1024
        && item->window()->devicePixelRatio() == deviceScale;
}
}
#endif
