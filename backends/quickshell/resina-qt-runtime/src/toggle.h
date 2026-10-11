#ifndef RESINA_QT_RUNTIME_TOGGLE_H
#define RESINA_QT_RUNTIME_TOGGLE_H
#include <QObject>
#include <QtQml/qqmlregistration.h>
#include <memory>

class ToggleController : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString publication READ publication NOTIFY publicationChanged)
    Q_PROPERTY(QString lastError READ lastError)
public:
    explicit ToggleController(QObject* parent = nullptr);
    ~ToggleController() override;
    QString publication() const { return publication_; }
    QString lastError() const { return error_; }
    Q_INVOKABLE bool dispatch(const QString& source);
    Q_INVOKABLE bool setChecked(const QString& source);
signals:
    void publicationChanged();
    void captureRequested(QString id, bool acquire);
    void activated(bool checked);
private:
    struct Owner;
    std::unique_ptr<Owner> owner_;
    QString publication_;
    QString error_;
    bool publishing_ = false;
    bool deliver(const QString& source, bool replaceChecked);
};
#endif
