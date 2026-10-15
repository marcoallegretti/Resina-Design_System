#ifndef RESINA_QT_RUNTIME_SURFACE_H
#define RESINA_QT_RUNTIME_SURFACE_H

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

namespace resina::qt {
class SurfacePaintResolver : public QObject {
    Q_OBJECT
    QML_ELEMENT
public:
    using QObject::QObject;
    Q_INVOKABLE QString resolveSurfaceView(const QString& request, const QString& options) const;
    Q_INVOKABLE bool matchesNativeContext(QObject* item, double x, double y, double deviceScale) const;
};
}
#endif
