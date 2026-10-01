# Semantic elevation (candidate)

Resina elevation roles are `embedded`, `base`, `raised`, `floating`, `overlay`, and `modal`, ordered from lower to greater visual separation. Elevation expresses a surface's relationship to nearby content. It may influence edge, thickness, highlight, shadow, and background separation; it is not a direct shadow or stacking-index command. `modal` names the strongest separation in this scale and does not itself impose interaction modality.

Every surface form names its elevation explicitly through the [surface form contract](09-geometry.md). The role is renderer-independent and remains meaningful when advanced shadows or transparency are unavailable. Numerical realization and component-specific restrictions require later calibrated tokens and fallback rules.
