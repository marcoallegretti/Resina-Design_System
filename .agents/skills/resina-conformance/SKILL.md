---
name: resina-conformance
description: Use for independent vectors, schema cases, strict public protocol checkers and verification gaps.
---

# Conformance evidence

Read [AGENTS.md](../../../AGENTS.md), the affected specification,
[conformance](../../../conformance/), [tools](../../../tools/) and the relevant
checker call in [CI](../../../.github/workflows/reference.yml).

Name what the evidence proves and its boundary. Semantic resolution, portable
paint, raster transport, native input and assistive technology delivery are
distinct claims. A fixture exercising one slice does not certify a complete
component, material calibration or a whole backend.

Prefer independently evaluable expectations: authored vectors, rational/reference
arithmetic where applicable, explicit invariants and acceptance/rejection cases.
Do not derive expected output exclusively by calling the implementation under
test. Preserve strict diagnostics, transport behavior and bounded resource cases.

Public checker invocation follows its actual CLI, for example:

```sh
cargo build -p resina-resolver --bin resina-slider-value --locked
python tools/check_slider_value_backend.py -- target/debug/resina-slider-value -
```

Use `.exe` on Windows. Consult the selected checker's README/CI pairing instead
of inferring every executable name from the checker filename.

For shared rendering scenarios, read [scene preparation](../../../conformance/scenes/README.md)
and [raster evidence](../../../conformance/raster/README.md). Preserve authored
sources and actual capability/preference settings. Explain any changed expected
result against the normative contract before accepting it.

Run the relevant checker tests as well as the checker against the real public
command. Use [harness](../../commands/harness.md) to identify missing proof and
report unavailable native evidence honestly.
