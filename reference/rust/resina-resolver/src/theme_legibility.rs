use resina_color::{SrgbFallback, opaque_contrast_ratio};
use resina_model::ColorRole;
use serde::Serialize;
use std::{collections::BTreeMap, fmt};

pub const LEGIBILITY_TEXT_CONTRAST: f64 = 4.5;
pub const LEGIBILITY_NON_TEXT_CONTRAST: f64 = 3.0;

const BACKGROUNDS: [ColorRole; 4] = [
    ColorRole::SurfaceBase,
    ColorRole::SurfaceLow,
    ColorRole::SurfaceHigh,
    ColorRole::SurfaceChrome,
];
const TEXT: [ColorRole; 7] = [
    ColorRole::ContentPrimary,
    ColorRole::ContentSecondary,
    ColorRole::ContentMuted,
    ColorRole::StatusError,
    ColorRole::StatusWarning,
    ColorRole::StatusSuccess,
    ColorRole::StatusInfo,
];
const ACCENTS: [ColorRole; 3] = [
    ColorRole::AccentPrimary,
    ColorRole::AccentSecondary,
    ColorRole::AccentTertiary,
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegibilityGuardPair {
    foreground: ColorRole,
    background: ColorRole,
    minimum_contrast: f64,
    contrast_ratio: f64,
}

impl LegibilityGuardPair {
    pub fn foreground(&self) -> ColorRole {
        self.foreground
    }

    pub fn background(&self) -> ColorRole {
        self.background
    }

    pub fn minimum_contrast(&self) -> f64 {
        self.minimum_contrast
    }

    pub fn contrast_ratio(&self) -> f64 {
        self.contrast_ratio
    }

    pub fn passed(&self) -> bool {
        self.contrast_ratio >= self.minimum_contrast
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LegibilityGuardError {
    MissingRole(ColorRole),
    TranslucentRole(ColorRole),
    BelowMinimum(Vec<LegibilityGuardPair>),
}

impl fmt::Display for LegibilityGuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRole(role) => {
                write!(
                    formatter,
                    "legibility guard needs an opaque {} color",
                    role_name(*role)
                )
            }
            Self::TranslucentRole(role) => {
                write!(
                    formatter,
                    "legibility guard color {} is translucent",
                    role_name(*role)
                )
            }
            Self::BelowMinimum(pairs) => {
                formatter.write_str("legibility guard failed")?;
                for pair in pairs {
                    write!(
                        formatter,
                        "\n  {} over {}: {} < {}",
                        role_name(pair.foreground),
                        role_name(pair.background),
                        pair.contrast_ratio,
                        pair.minimum_contrast
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for LegibilityGuardError {}

pub(crate) fn role_name(role: ColorRole) -> String {
    serde_json::to_value(role)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

pub fn legibility_guard_pairs() -> Vec<(ColorRole, ColorRole, f64)> {
    let mut pairs = Vec::with_capacity(40);
    for background in BACKGROUNDS {
        pairs.extend(
            TEXT.into_iter()
                .map(|foreground| (foreground, background, LEGIBILITY_TEXT_CONTRAST)),
        );
        pairs.push((ColorRole::Outline, background, LEGIBILITY_NON_TEXT_CONTRAST));
        pairs.push((ColorRole::Focus, background, LEGIBILITY_NON_TEXT_CONTRAST));
    }
    pairs.push((
        ColorRole::ContentPrimary,
        ColorRole::Selection,
        LEGIBILITY_TEXT_CONTRAST,
    ));
    pairs.extend(
        ACCENTS
            .into_iter()
            .map(|accent| (ColorRole::ContentInverse, accent, LEGIBILITY_TEXT_CONTRAST)),
    );
    pairs
}

pub fn resolve_legibility_guard(
    opaque_colors: &BTreeMap<ColorRole, SrgbFallback>,
) -> Result<Vec<LegibilityGuardPair>, LegibilityGuardError> {
    let color = |role| {
        opaque_colors
            .get(&role)
            .ok_or(LegibilityGuardError::MissingRole(role))
    };
    let mut pairs = Vec::new();
    for (foreground, background, minimum_contrast) in legibility_guard_pairs() {
        let contrast_ratio = opaque_contrast_ratio(color(foreground)?, color(background)?)
            .map_err(|error| {
                LegibilityGuardError::TranslucentRole(match error {
                    resina_color::ContrastError::TranslucentForeground => foreground,
                    resina_color::ContrastError::TranslucentBackground => background,
                })
            })?;
        pairs.push(LegibilityGuardPair {
            foreground,
            background,
            minimum_contrast,
            contrast_ratio,
        });
    }
    let failed: Vec<_> = pairs
        .iter()
        .filter(|pair| !pair.passed())
        .cloned()
        .collect();
    if failed.is_empty() {
        Ok(pairs)
    } else {
        Err(LegibilityGuardError::BelowMinimum(failed))
    }
}
