#include "toggle.h"
#include "resina-qt-runtime/src/toggle.cxx.h"
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QThread>
#include <stdexcept>

struct ToggleController::Owner { rust::Box<resina::qt::ToggleOwner> state = resina::qt::new_toggle_owner(); };
ToggleController::ToggleController(QObject* parent) : QObject(parent), owner_(std::make_unique<Owner>()),
    publication_(resina::qt::toggle_initial_publication(*owner_->state)) {}
ToggleController::~ToggleController() = default;
bool ToggleController::dispatch(const QString& source) { return deliver(source, false); }
bool ToggleController::setChecked(const QString& source) { return deliver(source, true); }
bool ToggleController::deliver(const QString& source, bool replaceChecked) {
    auto* application = QCoreApplication::instance();
    if (!application || thread() != application->thread() || QThread::currentThread() != thread()) {
        return false;
    }
    if (publishing_) { error_ = "toggle publication is in progress"; return false; }
    QString publication;
    try {
        // The Rust borrow ends before any native observer can destroy this QObject.
        publication = resina::qt::dispatch_toggle(*owner_->state, source , replaceChecked);
    } catch (const std::exception& error) {
        error_ = QString::fromUtf8(error.what());
        return false;
    }
    const auto result = QJsonDocument::fromJson(publication.toUtf8()).object();
    const auto activation = result.value("activation").toObject();
    const auto capture = activation.value("capture").toObject();
    const QPointer<ToggleController> self(this);
    publishing_ = true;
    publication_ = publication;
    error_.clear();
    emit publicationChanged();
    if (!self) return true;
    publishing_ = false;
    if (!capture.isEmpty()) {
        emit captureRequested(capture.value("id").toString(), capture.value("kind").toString() == "acquire");
        if (!self) return true;
    }
    if (activation.value("activate").toBool()) emit activated(result.value("checked").toBool());
    return true;
}
