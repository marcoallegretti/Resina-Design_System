#include "command.h"
#include "resina-qt-runtime/src/lib.cxx.h"
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QThread>
#include <stdexcept>

struct CommandController::Owner { rust::Box<resina::qt::CommandOwner> state = resina::qt::new_command_owner(); };
CommandController::CommandController(QObject* parent) : QObject(parent), owner_(std::make_unique<Owner>()),
    publication_(resina::qt::command_initial_publication(*owner_->state)) {}
CommandController::~CommandController() = default;
bool CommandController::dispatch(const QString& source) {
    auto* application = QCoreApplication::instance();
    if (!application || thread() != application->thread() || QThread::currentThread() != thread()) {
        return false;
    }
    if (publishing_) { error_ = "command publication is in progress"; return false; }
    QString publication;
    try {
        // The Rust borrow ends before any native observer can destroy this QObject.
        publication = resina::qt::dispatch_command(*owner_->state, source);
    } catch (const std::exception& error) {
        error_ = QString::fromUtf8(error.what());
        return false;
    }
    const auto result = QJsonDocument::fromJson(publication.toUtf8()).object();
    const auto activation = result;
    const auto capture = activation.value("capture").toObject();
    const QPointer<CommandController> self(this);
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
    if (activation.value("activate").toBool()) emit activated();
    return true;
}
