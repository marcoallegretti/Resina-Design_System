---
name: resina-backends
description: Use for CPU, GUIdo, Slint, Quickshell and Web realization or native integration boundaries.
---

# Backend realization

Read [AGENTS.md](../../../AGENTS.md) and the owning backend's current README:

| Realization | Documentation |
| --- | --- |
| CPU paint/raster | [resina-raster](../../../backends/cpu/resina-raster/README.md) |
| GUIdo | [resina-guido](../../../backends/guido/resina-guido/README.md) |
| Slint generated source | [resina-slint](../../../backends/slint/resina-slint/README.md) |
| Slint runtime | [resina-slint-runtime](../../../backends/slint/resina-slint-runtime/README.md) |
| Quickshell/QML | [resina-qml](../../../backends/quickshell/resina-qml/README.md) |
| Web/SVG | [resina-svg](../../../backends/web/resina-svg/README.md) |
| Native Lab consumer | [Material Board](../../../lab/guido/README.md) |

Trace validated portable input through preparation and native publication. Keep
Resina semantics and fallback decisions in their established shared owner.
Translate toolkit capabilities and native state at the integration boundary;
do not let backend identity become a normative input.

Inspect placement, device-grid sampling, alpha/color handling, clipping, text
measurement and complete snapshot publication where affected. Reject preparation
failures before publishing partial/stale channels. Verify lifetime and thread
constraints from the pinned dependency and current adapter code.

Distinguish paint preparation from native event routing, clock scheduling,
focus/action delivery and accessibility-tree integration. Verify each claimed
capability at its actual boundary. Protocol output or a screenshot cannot alone
prove native interactive behavior.

The GUIdo adapter, Slint runtime and GUIdo Lab are separate workspaces. Run their
explicit manifest checks and required native tests from their READMEs and
[CI](../../../.github/workflows/reference.yml); root workspace success omits them.
GUIdo requires actual GPU evidence, prepared scenes and an explicit font. A missing
adapter or runtime is a limitation, not a passing rendering check.

Load [visual verification](../resina-visual-verification/SKILL.md) for appearance,
typography, motion or raster changes. Use shared normative scenarios before
backend-specific probes and preserve a valid lower-capability realization.
