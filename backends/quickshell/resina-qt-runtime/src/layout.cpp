#include "layout.h"
#include "resina-qt-runtime/src/layout.cxx.h"

namespace resina::qt {
QString CommandLayoutResolver::resolveCommandLayout(const QString& request) const {
    return resolve_command_layout_profile(request);
}
}
