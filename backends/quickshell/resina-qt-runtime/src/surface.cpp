#include "surface.h"
#include "context.h"
#include "resina-qt-runtime/src/surface.cxx.h"

namespace resina::qt {
QString SurfacePaintResolver::resolveSurfaceView(const QString& request, const QString& options) const {
    return resolve_surface_view(request, options);
}

bool SurfacePaintResolver::matchesNativeContext(QObject* item, double x, double y, double deviceScale) const {
    return nativeContextMatches(this, item, x, y, deviceScale);
}
}
