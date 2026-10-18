#include "surface.h"
#include "context.h"
#include "resina-qt-runtime/src/surface.cxx.h"

namespace resina::qt {
QString SurfacePaintResolver::resolveSurfaceView(const QString& request, const QString& options) const {
    return resolve_surface_view(request, options);
}

QString SurfacePaintResolver::resolveOpaqueColor(const QString& themeRequest, const QString& role) const {
    return resolve_opaque_color(themeRequest, role);
}

bool SurfacePaintResolver::matchesNativeContext(QObject* item, double x, double y, double deviceScale) const {
    return nativeContextMatches(this, item, x, y, deviceScale);
}

bool SurfacePaintResolver::presentsNativeWindow(QObject* item) const {
    return exposedNativeWindow(this, item) != nullptr;
}
}
