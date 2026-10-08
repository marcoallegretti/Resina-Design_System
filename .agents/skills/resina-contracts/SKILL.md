---
name: resina-contracts
description: Use for Resina normative specifications, authored definitions, schemas, tokens and public IR contracts.
---

# Normative contracts

Read [AGENTS.md](../../../AGENTS.md), the relevant sections of the
[blueprint](../../../RESINA_DESIGN_SYSTEM_BLUEPRINT.md) and affected implemented
contracts in [spec](../../../spec/), [definitions](../../../definitions/),
[schemas](../../../schemas/), [tokens](../../../tokens/) and
[conformance](../../../conformance/).

The blueprint describes intent; normative artifacts define implemented behavior.
Rust, rendering output and backend convenience cannot silently define a new law.
Check existing callers and vectors before proposing a semantic change.

For each change, identify the authored input, validation, deterministic output,
diagnostics, capability/fallback cases and downstream consumers. New normative
concepts, public IR or ownership changes use the contract's architectural decision
boundary before implementation. An accepted feature request is not evidence that
an unspecified mechanism has already been approved.

Keep contracts independent of toolkit, renderer, window system and product.
Express actual capabilities rather than backend identity. Add positive and
negative schema/conformance cases, including unsupported or unrepresentable
inputs. Coordinate affected reference and backend behavior; generated assets
remain realization rather than normative source.

Verify with `python .agents/scripts/verify.py schemas`, relevant reference tests
and public protocol checkers selected through [harness](../../commands/harness.md).
Do not broaden a contract merely to match one implementation's convenience.
