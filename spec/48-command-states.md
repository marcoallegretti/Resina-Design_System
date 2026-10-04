# Command interaction projection (candidate, schema 0.1.0)

Blueprint §§59–67 require composed interaction feedback. This operation projects a current validated [activation state](42-command-activation.md) and explicit hover observation into the semantic state set used by [command paint](43-command-paint.md) and [sampled paint](45-command-motion.md). It prevents their producers from independently recreating availability, actual focus and held-gesture feedback. It defines ordinary command signals, not a complete Button or native event adapter.

## Inputs and ownership

The [request](../schemas/command-states-request.schema.json) requires version 0.1.0, `activation` and Boolean `hovered`. Every activation field remains required and validated, including explicit null hold. Unknown/duplicate members, unsupported versions, disabled holds, unfocused keyboard holds and empty pointer IDs fail diagnostically. No missing input acquires a default.

The owner supplies a coherent snapshot for one command after committing the latest activation transition. Actual focus comes from native observation; a queued focus request is insufficient. Hover is an explicit current observation owned by the input producer. It MUST NOT be inferred from focus, a held key, availability, pointer capture or the held pointer's `inside` field. Those observations can differ: an outside held pointer may coexist with hover from another pointer. A producer without hover supplies false explicitly, without removing keyboard or touch feedback.

## Projection

1. Include `disabled` exactly when activation is unavailable.
2. Include `hover` exactly when `hovered` is true, including when disabled.
3. Include `pressed` exactly when the validated activation state's momentary pressed signal is true: an inside held pointer or a held Space/Enter key. An outside pointer hold retains its lifecycle but does not produce pressed feedback.
4. If none of disabled, hover or pressed applies, include `rest` as this command's body base signal.
5. Include `focused` independently exactly when activation reports actual focus, including when disabled.

Return a nonempty [state set](14-interaction.md) at version 0.1.0, serialized in its canonical order. The rest rule belongs to this command operation; generic state-set validation and composition do not infer rest. Focused rest therefore contains both `rest` and `focused`. Disabled plus hover remains representable; the existing command body phase rule suppresses hover's body response while retaining its signal and the independent focus cue. No selected, checked, active, busy, validation or dragging state is invented. Commands needing those signals require an extended component contract rather than discarding them through this profile.

Projection performs no activation, pointer capture, focus transfer, native access or paint resolution. It does not turn availability into focusability. In particular, a disabled discoverable command retains actual focused feedback. Use the same committed activation snapshot for this operation and [accessible semantics](47-command-accessibility.md). Feed the resulting state set into the command's surface intent before resolving paint or motion. A failed paint guard remains diagnostic; projection does not weaken contrast or capability requirements. This output is interaction policy, not renderer-specific IR.

## Conformance

The [public cases](../conformance/interaction/command-states-cases.json) independently specify rest, hover, actual focus, disabled discovery, pointer membership and both command keys, plus invalid inputs. Rust integration tests route actual lifecycle transitions through projection and guarded paint: leaving/re-entering a captured pointer, semantic invocation superseding a held gesture, orphan release, availability changes and successful focus loss. They check body phase, exact retained states and independent focus paint. These do not establish native capture, screen-reader delivery, complete component conformance or material calibration.

Rust provides `resolve_command_states` for validated typed input and `resolve_command_states_source` for strict source input. `SurfaceIntent::with_states` consumes an existing validated intent and replaces only its state set, preserving form, treatments and semantic roles; producers need not manipulate serialized fields. `resina-command-states <path|->` follows the existing UTF-8 1 MiB command boundary: success 0 with one complete state-set JSON and empty stderr, diagnostic failure 1 without output, usage error 2. The [external checker](../tools/check_command_states_backend.py) runs valid cases twice, checks exact signals and exercises duplicate members. An independent backend can use the public law and cases without importing Rust or a toolkit.
