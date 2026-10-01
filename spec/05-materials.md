# Material families and role assignments (candidate, schema 0.1.0)

Resina defines four material families: `cast`, `frost`, `elastomer`, and `gel`. These are design intent, not renderer primitives. Cast is structurally stable, Frost is pigmented and diffusive, Elastomer is tactile and compressible, and Gel is viscous and transient. A backend may realize a family with different techniques according to its capabilities, but it MUST preserve the family's semantic role. Treatment, elevation, pigment, and interaction state are separate concerns.

Components request material roles. A selected theme supplies a [material assignment](../schemas/material-assignments.schema.json) for one resolution context. This document specifies the role mapping contract; it does not define theme packaging or require one universal mapping. The assignment has `schemaVersion` `0.1.0` and MUST name every role:

| Role group | Roles |
| --- | --- |
| `surface` | `base`, `content`, `chrome`, `raised`, `transient` |
| `control` | `passive`, `interactive`, `primary` |
| `feedback` | `focus`, `selection`, `drag` |

Each role maps to exactly one material family. Consumers MUST reject missing roles, unknown roles, unknown families, and unsupported schema versions. They MUST NOT fill omissions with implicit assignments or infer a family from a component name, backend, or device category. `surface.base`, `surface.content`, `surface.chrome`, and `surface.raised` are structural roles and MUST NOT map to Gel; the blueprint reserves Gel for transient and feedback use. Other family choices are theme decisions and are not fixed by this contract.

Role resolution is a lookup. Capability and accessibility fallback happens after the family is chosen and MUST NOT silently change the role assignment. The [conformance vectors](../conformance/materials/role-assignment-vectors.json) include one illustrative assignment and invalid cases; the illustrative assignment is not a required theme palette or product default. The Rust reference model implements this contract without a rendering dependency.

Optical treatments are a separate axis. Their names and the initial nesting rule are defined in [Optical treatments and nesting](06-optics.md).
