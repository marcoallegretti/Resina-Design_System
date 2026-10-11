#ifndef RESINA_QT_RUNTIME_CONTEXT_H
#define RESINA_QT_RUNTIME_CONTEXT_H

#include <QQuickItem>
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
}
#endif
