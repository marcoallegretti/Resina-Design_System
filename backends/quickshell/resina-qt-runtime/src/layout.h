#ifndef RESINA_QT_RUNTIME_LAYOUT_H
#define RESINA_QT_RUNTIME_LAYOUT_H

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

namespace resina::qt {
class CommandLayoutResolver : public QObject {
    Q_OBJECT
    QML_ELEMENT

public:
    using QObject::QObject;
    Q_INVOKABLE QString resolveCommandLayout(const QString& request) const;
};
}
#endif
