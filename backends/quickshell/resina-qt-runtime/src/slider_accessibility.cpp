#include "slider_accessibility.h"
#include "resina-qt-runtime/src/slider_accessibility.cxx.h"
#include <QAccessibleObject>
#include <QCoreApplication>
#include <QJsonDocument>
#include <QPointer>
#include <QQuickWindow>
#include <QThread>
#include <cmath>

namespace resina::qt {
namespace {
class AccessibleSlider final : public QAccessibleObject,
                               public QAccessibleValueInterface,
                               public QAccessibleActionInterface,
                               public QAccessibleAttributesInterface {
public:
    explicit AccessibleSlider(SliderAccessibilityItem* item) : QAccessibleObject(item) {}
    SliderAccessibilityItem* item() const { return qobject_cast<SliderAccessibilityItem*>(object()); }
    QWindow* window() const override { return item() ? item()->window() : nullptr; }
    QAccessibleInterface* parent() const override {
        if (!item() || !item()->window()) return nullptr;
        auto* root = item()->window()->contentItem();
        for (auto* p = item()->parentItem(); p && p != root; p = p->parentItem()) {
            auto* iface = QAccessible::queryAccessibleInterface(p);
            if (iface && iface->indexOfChild(this) >= 0) return iface;
        }
        if (auto* rootInterface = item()->window()->accessibleRoot()) return rootInterface;
        return QAccessible::queryAccessibleInterface(item()->window());
    }
    QAccessibleInterface* child(int) const override { return nullptr; }
    QAccessibleInterface* childAt(int, int) const override { return nullptr; }
    int childCount() const override { return 0; }
    int indexOfChild(const QAccessibleInterface*) const override { return -1; }
    QRect rect() const override {
        if (!item() || !item()->window() || !item()->isVisible()) return {};
        const QRectF scene = item()->mapRectToScene(item()->boundingRect());
        const QPoint origin = item()->window()->mapToGlobal(QPoint());
        return scene.translated(origin).toAlignedRect();
    }
    QAccessible::Role role() const override { return QAccessible::Slider; }
    QString text(QAccessible::Text type) const override {
        if (!item()) return {};
        const auto a = item()->semantics();
        if (type == QAccessible::Name) return a.value("name").toString();
        if (type == QAccessible::Description) return a.value("description").toString();
        if (type == QAccessible::Value) return item()->nativeValueText();
        return {};
    }
    void setText(QAccessible::Text, const QString&) override {}
    QAccessible::State state() const override {
        QAccessible::State state;
        if (!item()) { state.invalid = true; return state; }
        const auto a = item()->semantics();
        const auto s = a.value("state").toObject();
        state.disabled = !s.value("enabled").toBool() || !item()->available();
        state.readOnly = s.value("readOnly").toBool();
        state.focusable = a.value("focusable").toBool() && !a.isEmpty();
        state.focused = item()->hasActiveFocus();
        state.invisible = !item()->isVisible();
        state.offscreen = !item()->window() || !item()->window()->isVisible();
        return state;
    }
    void* interface_cast(QAccessible::InterfaceType type) override {
        switch (type) {
        case QAccessible::ValueInterface: return static_cast<QAccessibleValueInterface*>(this);
        case QAccessible::ActionInterface: return static_cast<QAccessibleActionInterface*>(this);
        case QAccessible::AttributesInterface: return static_cast<QAccessibleAttributesInterface*>(this);
        default: return nullptr;
        }
    }
    QVariant currentValue() const override { return number("value"); }
    QVariant minimumValue() const override { return number("minimum"); }
    QVariant maximumValue() const override { return number("maximum"); }
    QVariant minimumStepSize() const override { return item() ? QVariant(item()->stepSize()) : QVariant(); }
    void setCurrentValue(const QVariant& value) override {
        // AT-SPI supplies a double. Reject unrelated QVariant conversions; no
        // strings, booleans or implicit domain clamping enter reference logic.
        if (!item() || value.metaType() != QMetaType::fromType<double>()) return;
        const double target = value.toDouble();
        if (!std::isfinite(target)) return;
        item()->adjust(QJsonObject{{"kind", "setValue"}, {"value", target}});
    }
    QStringList actionNames() const override {
        return {increaseAction(), decreaseAction()};
    }
    void doAction(const QString& action) override {
        if (!item()) return;
        if (action == increaseAction()) item()->adjust(QJsonObject{{"kind", "increase"}, {"amount", item()->stepSize()}});
        else if (action == decreaseAction()) item()->adjust(QJsonObject{{"kind", "decrease"}, {"amount", item()->stepSize()}});
    }
    QStringList keyBindingsForAction(const QString&) const override { return {}; }
    QList<QAccessible::Attribute> attributeKeys() const override {
        return {QAccessible::Attribute::Orientation};
    }
    QVariant attributeValue(QAccessible::Attribute key) const override {
        if (!item() || item()->semantics().isEmpty() || key != QAccessible::Attribute::Orientation) return {};
        return QVariant::fromValue(item()->semantics().value("orientation").toString() == "horizontal" ? Qt::Horizontal : Qt::Vertical);
    }
private:
    QVariant number(const char* key) const {
        if (!item() || item()->semantics().isEmpty()) return {};
        return item()->semantics().value("value").toObject().value(key).toDouble();
    }
};
QAccessibleInterface* sliderFactory(const QString& className, QObject* object) {
    // QML dynamic metaobjects are skipped by Qt's factory lookup. Restrict both
    // class and actual QObject type; all unrelated Qt items keep Qt factories.
    if (className != QString::fromLatin1(SliderAccessibilityItem::staticMetaObject.className())) return nullptr;
    auto* item = qobject_cast<SliderAccessibilityItem*>(object);
    return item ? new AccessibleSlider(item) : nullptr;
}
class FactoryLifetime final : public QObject {
public:
    explicit FactoryLifetime(QObject* application) : QObject(application) { QAccessible::installFactory(sliderFactory); }
    ~FactoryLifetime() override { QAccessible::removeFactory(sliderFactory); }
};
void installFactory() {
    static QPointer<FactoryLifetime> lifetime;
    auto* application = QCoreApplication::instance();
    if (!lifetime && application && QThread::currentThread() == application->thread())
        lifetime = new FactoryLifetime(application);
}
}

SliderAccessibilityItem::SliderAccessibilityItem(QQuickItem* parent) : QQuickItem(parent) {
    installFactory();
    connect(this, &QQuickItem::activeFocusChanged, this, &SliderAccessibilityItem::invalidate);
    connect(this, &QQuickItem::visibleChanged, this, &SliderAccessibilityItem::notifyState);
    connect(this, &QQuickItem::enabledChanged, this, &SliderAccessibilityItem::notifyState);
    connect(this, &QQuickItem::windowChanged, this, &SliderAccessibilityItem::notifyState);
}
bool SliderAccessibilityItem::available() const {
    return sourceAvailable_ && !publication_.isEmpty() && window() && window()->isVisible()
        && isVisible() && isEnabled() && width() > 0 && height() > 0;
}
void SliderAccessibilityItem::notifyState() {
    QAccessible::State state;
    state.disabled = state.readOnly = state.focusable = state.focused = state.invisible = state.offscreen = true;
    QAccessibleStateChangeEvent event(this, state);
    QAccessible::updateAccessibility(&event);
}
void SliderAccessibilityItem::setError(const QString& error) {
    if (error_ == error) return;
    error_ = error;
    emit lastErrorChanged();
}
void SliderAccessibilityItem::setSourceAvailable(bool value) {
    if (sourceAvailable_ == value) return;
    sourceAvailable_ = value;
    const QPointer<SliderAccessibilityItem> owner(this);
    emit sourceAvailableChanged();
    if (owner) notifyState();
}
QString SliderAccessibilityItem::nativeValueText() const {
    return semantics_.value("valueText").toString();
}
QString SliderAccessibilityItem::accessibleValueText() const {
    auto* iface = QAccessible::queryAccessibleInterface(const_cast<SliderAccessibilityItem*>(this));
    return iface ? iface->text(QAccessible::Value) : QString();
}
void SliderAccessibilityItem::invalidate() {
    const bool changed = !publication_.isEmpty();
    publication_.clear();
    semantics_ = {};
    stepSize_ = 0;
    const QPointer<SliderAccessibilityItem> owner(this);
    if (changed) emit publicationChanged();
    if (owner) notifyState();
}
bool SliderAccessibilityItem::publish(const QString& publication) {
    auto* application = QCoreApplication::instance();
    if (!application || thread() != application->thread() || QThread::currentThread() != thread()) return false;
    const auto validation = QJsonDocument::fromJson(validate_slider_accessibility_publication(publication).toUtf8()).object();
    if (!validation.value("valid").toBool()) {
        const QPointer<SliderAccessibilityItem> owner(this);
        invalidate();
        if (owner) setError(validation.value("error").toString());
        return false;
    }
    const auto document = QJsonDocument::fromJson(publication.toUtf8()).object();
    const auto a = document.value("accessibility").toObject();
    if (a.value("state").toObject().value("focused").toBool() != hasActiveFocus()) {
        const QPointer<SliderAccessibilityItem> owner(this);
        invalidate();
        if (owner) setError("Slider publication must reflect actual native focus");
        return false;
    }
    const bool changed = publication_ != publication;
    publication_ = publication;
    semantics_ = a;
    stepSize_ = document.value("stepSize").toDouble();
    const QPointer<SliderAccessibilityItem> owner(this);
    if (changed) emit publicationChanged();
    if (!owner || publication_ != publication) return false;
    setError({});
    if (!owner || publication_ != publication) return false;
    notifyState();
    if (!owner || publication_ != publication) return false;
    QAccessibleValueChangeEvent event(this, semantics_.value("value").toObject().value("value").toDouble());
    QAccessible::updateAccessibility(&event);
    return owner && publication_ == publication;
}
void SliderAccessibilityItem::adjust(const QJsonObject& adjustment) {
    if (delivering_ || publication_.isEmpty()) return;
    auto* application = QCoreApplication::instance();
    if (!application || thread() != application->thread() || QThread::currentThread() != thread()) return;
    const QString result = prepare_slider_accessibility_adjustment(publication_,
        QString::fromUtf8(QJsonDocument(adjustment).toJson(QJsonDocument::Compact)), available());
    const auto document = QJsonDocument::fromJson(result.toUtf8()).object();
    if (document.contains("error")) { setError(document.value("error").toString()); return; }
    if (!document.value("candidate").toObject().value("accepted").toBool()) return;
    const QPointer<SliderAccessibilityItem> owner(this);
    delivering_ = true;
    emit adjustmentRequested(result);
    if (owner) delivering_ = false;
}
}
