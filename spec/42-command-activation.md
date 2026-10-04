# Command activation lifecycle (candidate, 0.1.0)

Blueprint §§59–67 and §98 require portable interaction and composed states.
This contract supplies the activation lifecycle for an ordinary command control.
It emits an activation intent and the momentary pressed signal; it does not
execute a product action or define a complete Button component, toggle selection,
hover, focus navigation, appearance, motion or accessibility tree.

The [request](../schemas/activation-request.schema.json) requires version `0.1.0`,
a [state](../schemas/activation-state.schema.json) and one event. The state requires
its own version, explicit `enabled`, actual native `focused`, and `hold`: null,
a pointer hold (`kind: pointer`, nonempty opaque `id`, `inside`), or a keyboard
hold (`kind: key`, `key: space` or `enter`). Disabled states MUST have null holds.
Keyboard holds MUST have actual focus. Pointer holds do not require focus:
pointer activation must remain usable on platforms that do not focus on press.
Pointer identities MUST be compared exactly, without normalization or coercion.
Every field is required; unknown or duplicate members and unsupported values fail.

The producer maps only primary command gestures to pointer events, and only
unmodified command Space/Enter gestures to key events. Other keys, chords and
secondary-button gestures belong to their appropriate owners. `inside` is the
current hit-region membership, including on release, not a cached hover flag.
Focused means successful native focus, never a queued request. Repeat metadata
MUST come from actual native key delivery.
Once a gesture is held, the producer MUST continue routing its termination even
if modifiers change. If delivery is interrupted, cancel explicitly; do not leave
a keyboard hold waiting for a release filtered as a new modified gesture.

## Transitions

| Event | Rule |
| --- | --- |
| `pointerDown(id, inside)` | When enabled, inside and with no hold, hold that pointer and request capture. Do not activate. |
| `pointerMove(id, inside)` | For the held pointer only, update inside. Retain the hold outside so re-entry can restore pressed feedback. |
| `pointerUp(id, inside)` | For the held pointer only, clear the hold and release capture. Activate exactly when release is inside. |
| `pointerCancel(id)` | For the held pointer only, clear the hold and release capture without activation. |
| `keyDown(key, repeat)` | When enabled, focused, not repeated and with no hold, hold that key. Enter activates immediately; Space does not. |
| `keyUp(key)` | For the held key only, clear the hold. Space activates; Enter does not activate again. |
| `focus(focused)` | Record actual focus. Losing focus cancels keyboard holds without activation; pointer holds remain independent. |
| `availability(enabled)` | Record availability. Disabling cancels every hold and releases pointer capture without activation. Enabling does not restore a cancelled hold. |
| `invoke` | When enabled, emit one activation and cancel any existing hold, releasing capture. This handles semantic activation, such as assistive technology actions. It does not require or change focus. |
| `cancel` | Cancel any hold and release pointer capture without activation. |

Well-formed events that do not satisfy their rule leave state unchanged and do
not activate. This includes orphan releases, other pointer/key identities,
repeats, competing presses and unavailable activation. Every event is validated
before these guards, so an empty pointer ID is invalid even when disabled.
The first held gesture owns the lifecycle. Repeated Enter does not activate again;
one press produces at most one activation. A semantic invoke supersedes an armed
gesture; its later release cannot produce a second activation.

The [result](../schemas/activation-result.schema.json) supplies version, complete
next `state`, `activate`, derived `pressed` and explicit `capture`. `pressed` is
true for a keyboard hold or an inside pointer hold. Outside pointer holds retain
capture but produce false pressed feedback. Capture is null, or `acquire`/`release`
with the exact pointer ID. These are interaction effects, separate from paint IR.
An activating result MUST remain enabled. Acquisition MUST accompany an inside
pointer hold without activation; release MUST leave no hold. The result schema
checks these local invariants as well as pressed feedback. It cannot compare
capture IDs to state IDs or establish the preceding event history; conformance
still checks the complete transition against its input.

Commit next state before delivering an activation to product code, which may
disable or remove the control. Execute each activation intent once; replaying a
request for conformance is not permission to replay its side effect. The backend
MUST ensure continued delivery for the held pointer outside the control. Native
capture, or equivalent owner-level routing, can provide this. If neither is
available, cancel explicitly instead of starting an unreliable gesture. Lost
capture, window deactivation, removal and interruption require `cancel` (or the
matching pointer cancellation); clearing state privately can lose release effects.
Do not feed both raw gesture events and a synthesized click for the same gesture.
Semantic invoke is a separate producer path and must not duplicate raw delivery.

Preserve focused/disabled and other semantic layers independently when composing
visual state. This output does not invent selected, checked or busy. A component
decides whether busy makes activation unavailable and supplies that decision via
enabled. Accessibility names, labels, focus recovery and action semantics remain
component/product responsibilities.

The pointer policy follows [W3C pointer cancellation guidance](https://www.w3.org/WAI/WCAG22/Understanding/pointer-cancellation.html).
Space-on-release and Enter-on-press are supported by the
[W3C button example source](https://github.com/w3c/aria-practices/blob/main/content/patterns/button/examples/js/button.js).
Repeat suppression, gesture ownership and invoke supersession are explicit Resina
decisions. These sources do not establish full component conformance.

`resina-activation <path|->` reads strict UTF-8 JSON up to 1 MiB. Success exits 0
with complete JSON; invalid input exits 1 with a diagnostic and no result; usage
errors exit 2. Public cases define both successful transitions and failures.
