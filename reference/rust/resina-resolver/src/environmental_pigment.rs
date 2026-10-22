use crate::{
    ACHROMATIC_SEED_CHROMA, CompiledTheme, LegibilityGuardError, LegibilityGuardPair,
    ThemeCompilationError, compile_theme_source_with_sources, legibility_guard_pairs,
    resolve_legibility_guard, theme_derivation::insert_token, theme_legibility::role_name,
};
use resina_color::{
    ColorConversionError, OpaqueSrgbRange, SrgbFallback, composite_srgb_over_opaque,
    gamut_map_oklch_to_srgb, normalize_hue, opaque_contrast_over_range, resolve_srgb_fallback,
    srgb_to_oklch,
};
use resina_model::ColorRole;
use resina_tokens::parse_token_document;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, fmt};

pub const ADAPTED_COLOR_GROUP: &str = "adaptedColor";
pub const ADAPTABLE_ROLES: [ColorRole; 4] = [
    ColorRole::SurfaceBase,
    ColorRole::SurfaceLow,
    ColorRole::SurfaceHigh,
    ColorRole::SurfaceChrome,
];
const THEMES: [&str; 2] = ["light", "dark"];
const MAX_CHROMA: f64 = 0.4;

#[derive(Debug)]
pub enum PigmentAdaptationError {
    Parse(serde_json::Error),
    InvalidRequest(&'static str),
    UnsupportedVersion,
    InvalidProfile(&'static str),
    InvalidObservation(&'static str),
    Themes(Vec<(&'static str, AdaptedThemeError)>),
}

#[derive(Debug)]
pub enum AdaptedThemeError {
    Template(ThemeCompilationError),
    TranslucentCastRole(ColorRole),
    OpaqueOverride(ColorRole),
    OpaqueFrostRole,
    MissingFrostFallback,
    ReservedGroup,
    ChromaOutsideRange(ColorRole),
    Conversion(ColorConversionError),
    Adapted(ThemeCompilationError),
    Certification(String),
    Guard(LegibilityGuardError),
}

impl fmt::Display for AdaptedThemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Template(error) => write!(formatter, "template compilation failed: {error}"),
            Self::TranslucentCastRole(role) => {
                write!(
                    formatter,
                    "template role {} must be opaque",
                    role_name(*role)
                )
            }
            Self::OpaqueOverride(role) => write!(
                formatter,
                "template role {} must not have an opaque color assignment",
                role_name(*role)
            ),
            Self::OpaqueFrostRole => {
                formatter.write_str("template role surface.chrome must be translucent")
            }
            Self::MissingFrostFallback => {
                formatter.write_str("template role surface.chrome needs an opaque color assignment")
            }
            Self::ReservedGroup => write!(
                formatter,
                "template already defines tokens in the {ADAPTED_COLOR_GROUP} group"
            ),
            Self::ChromaOutsideRange(role) => write!(
                formatter,
                "authored chroma of {} lies outside its profile chroma range",
                role_name(*role)
            ),
            Self::Conversion(error) => write!(formatter, "color adaptation failed: {error}"),
            Self::Adapted(error) => write!(formatter, "adapted theme compilation failed: {error}"),
            Self::Certification(error) => {
                write!(formatter, "Frost certification failed: {error}")
            }
            Self::Guard(error) => error.fmt(formatter),
        }
    }
}

impl fmt::Display for PigmentAdaptationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => {
                write!(
                    formatter,
                    "pigment adaptation request parse failed: {error}"
                )
            }
            Self::InvalidRequest(reason) => {
                write!(formatter, "invalid pigment adaptation request: {reason}")
            }
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidProfile(reason) => write!(formatter, "invalid profile: {reason}"),
            Self::InvalidObservation(reason) => write!(formatter, "invalid observation: {reason}"),
            Self::Themes(errors) => {
                formatter.write_str("pigment adaptation failed")?;
                for (theme, error) in errors {
                    write!(formatter, "\n{theme} theme: {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for PigmentAdaptationError {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PigmentAdaptation {
    schema_version: &'static str,
    adaptation: Adaptation,
    themes: BTreeMap<&'static str, AdaptedTheme>,
}

#[derive(Debug, Clone, Copy)]
pub enum Adaptation {
    Applied { observation: ObservedColor },
    Authored { reason: &'static str },
}

impl Serialize for Adaptation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("applied", &self.applied())?;
        match self {
            Self::Applied { observation } => map.serialize_entry("observation", observation)?,
            Self::Authored { reason } => map.serialize_entry("reason", reason)?,
        }
        map.end()
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedColor {
    oklch: [f64; 3],
    achromatic: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptedTheme {
    source: Value,
    roles: BTreeMap<ColorRole, AdaptedRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frost_certification: Option<Vec<FrostBound>>,
    legibility_guard: Vec<LegibilityGuardPair>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptedRole {
    target_oklch: [f64; 3],
    color: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrostBound {
    foreground: ColorRole,
    minimum_contrast: f64,
    contrast_bound: f64,
    passed: bool,
}

impl PigmentAdaptation {
    pub fn adaptation(&self) -> Adaptation {
        self.adaptation
    }

    pub fn theme(&self, name: &str) -> Option<&AdaptedTheme> {
        self.themes.get(name)
    }
}

impl Adaptation {
    pub fn applied(&self) -> bool {
        matches!(self, Self::Applied { .. })
    }
}

impl AdaptedTheme {
    pub fn source(&self) -> &Value {
        &self.source
    }

    pub fn role(&self, role: ColorRole) -> Option<([f64; 3], [f64; 3])> {
        self.roles.get(&role).map(|adapted| {
            let components = &adapted.color["components"];
            (
                adapted.target_oklch,
                [0, 1, 2].map(|index| components[index].as_f64().unwrap_or(f64::NAN)),
            )
        })
    }

    pub fn frost_certification(&self) -> Option<&[FrostBound]> {
        self.frost_certification.as_deref()
    }

    pub fn legibility_guard(&self) -> &[LegibilityGuardPair] {
        &self.legibility_guard
    }
}

impl FrostBound {
    pub fn foreground(&self) -> ColorRole {
        self.foreground
    }

    pub fn minimum_contrast(&self) -> f64 {
        self.minimum_contrast
    }

    pub fn contrast_bound(&self) -> f64 {
        self.contrast_bound
    }

    pub fn passed(&self) -> bool {
        self.passed
    }
}

#[derive(Clone, Copy)]
struct Interval {
    min: f64,
    max: f64,
}

impl Interval {
    fn clamp(self, value: f64) -> f64 {
        value.max(self.min).min(self.max)
    }

    fn contains(self, value: f64) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

#[derive(Clone, Copy)]
struct RoleRanges {
    hue: Interval,
    chroma: Interval,
}

type ThemeRanges = BTreeMap<ColorRole, RoleRanges>;

struct Observation {
    color: [f64; 3],
    range: Option<([f64; 3], [f64; 3])>,
}

pub fn adapt_pigment_request_source(
    source: &str,
) -> Result<PigmentAdaptation, PigmentAdaptationError> {
    const SHAPE: &str = "request needs exactly schemaVersion, profile, accessibilityPreferences, templates, externalSources and optionally observation";
    let document = parse_token_document(source).map_err(PigmentAdaptationError::Parse)?;
    let request = document
        .as_object()
        .ok_or(PigmentAdaptationError::InvalidRequest(
            "request must be a JSON object",
        ))?;
    match request.get("schemaVersion") {
        Some(Value::String(version)) if version == "0.1.0" => {}
        Some(Value::String(_)) => return Err(PigmentAdaptationError::UnsupportedVersion),
        _ => {
            return Err(PigmentAdaptationError::InvalidRequest(
                "schemaVersion must be a string",
            ));
        }
    }
    let required = [
        "profile",
        "accessibilityPreferences",
        "templates",
        "externalSources",
    ];
    if required.iter().any(|member| !request.contains_key(*member))
        || request.len() != 5 + usize::from(request.contains_key("observation"))
    {
        return Err(PigmentAdaptationError::InvalidRequest(SHAPE));
    }
    let profile = parse_profile(&request["profile"])?;
    let observation = request
        .get("observation")
        .map(parse_observation)
        .transpose()?;
    let (reduced_transparency, high_contrast) =
        parse_preferences(&request["accessibilityPreferences"])?;
    let templates = exact_object(&request["templates"], &THEMES).ok_or(
        PigmentAdaptationError::InvalidRequest(
            "templates must be an object with light and dark source text",
        ),
    )?;
    let mut template_sources = Vec::new();
    for name in THEMES {
        let text = templates[name]
            .as_str()
            .ok_or(PigmentAdaptationError::InvalidRequest(
                "templates must be an object with light and dark source text",
            ))?;
        template_sources.push((name, text));
    }
    let external_sources = request["externalSources"]
        .as_object()
        .ok_or(PigmentAdaptationError::InvalidRequest(
            "externalSources must be an object of source text",
        ))?
        .iter()
        .map(|(uri, text)| {
            text.as_str()
                .map(|text| (uri.clone(), text.to_owned()))
                .ok_or(PigmentAdaptationError::InvalidRequest(
                    "externalSources must be an object of source text",
                ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let skipped = if high_contrast {
        Some("highContrast")
    } else if reduced_transparency {
        Some("reducedTransparency")
    } else if observation.is_none() {
        Some("noObservation")
    } else {
        None
    };
    let applied = match (skipped, &observation) {
        (None, Some(observation)) => Some((observation, observed_color(observation)?)),
        _ => None,
    };

    let mut themes = BTreeMap::new();
    let mut errors = Vec::new();
    for (name, template) in template_sources {
        match adapt_theme(template, &external_sources, &profile[name], applied) {
            Ok(theme) => {
                themes.insert(name, theme);
            }
            Err(error) => errors.push((name, error)),
        }
    }
    if !errors.is_empty() {
        return Err(PigmentAdaptationError::Themes(errors));
    }
    Ok(PigmentAdaptation {
        schema_version: "0.1.0",
        adaptation: match (applied, skipped) {
            (Some((_, observation)), _) => Adaptation::Applied { observation },
            (None, reason) => Adaptation::Authored {
                reason: reason.unwrap_or("noObservation"),
            },
        },
        themes,
    })
}

fn observed_color(observation: &Observation) -> Result<ObservedColor, PigmentAdaptationError> {
    let [lightness, chroma, hue] = srgb_to_oklch(observation.color).map_err(|error| {
        PigmentAdaptationError::Themes(
            THEMES
                .iter()
                .map(|theme| (*theme, AdaptedThemeError::Conversion(error)))
                .collect(),
        )
    })?;
    let achromatic = chroma < ACHROMATIC_SEED_CHROMA;
    Ok(ObservedColor {
        oklch: [lightness, chroma, if achromatic { 0.0 } else { hue }],
        achromatic,
    })
}

fn exact_object<'a>(value: &'a Value, members: &[&str]) -> Option<&'a Map<String, Value>> {
    value.as_object().filter(|object| {
        object.len() == members.len() && members.iter().all(|member| object.contains_key(*member))
    })
}

fn number_in(value: &Value, lower: f64, upper: f64) -> Option<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite() && (lower..=upper).contains(number))
}

fn parse_preferences(value: &Value) -> Result<(bool, bool), PigmentAdaptationError> {
    const SHAPE: &str = "accessibilityPreferences needs exactly boolean reducedMotion, reducedTransparency and highContrast";
    let invalid = PigmentAdaptationError::InvalidRequest(SHAPE);
    let preferences = exact_object(
        value,
        &["reducedMotion", "reducedTransparency", "highContrast"],
    )
    .ok_or(invalid)?;
    let flag = |name: &str| {
        preferences[name]
            .as_bool()
            .ok_or(PigmentAdaptationError::InvalidRequest(SHAPE))
    };
    flag("reducedMotion")?;
    Ok((flag("reducedTransparency")?, flag("highContrast")?))
}

fn parse_interval(
    value: &Value,
    lower: (f64, f64),
    upper: (f64, f64),
    reason: &'static str,
) -> Result<Interval, PigmentAdaptationError> {
    let invalid = PigmentAdaptationError::InvalidProfile(reason);
    let bounds = exact_object(value, &["min", "max"]).ok_or(invalid)?;
    let min = number_in(&bounds["min"], lower.0, lower.1)
        .ok_or(PigmentAdaptationError::InvalidProfile(reason))?;
    let max = number_in(&bounds["max"], upper.0, upper.1)
        .ok_or(PigmentAdaptationError::InvalidProfile(reason))?;
    Ok(Interval { min, max })
}

fn parse_profile(
    value: &Value,
) -> Result<BTreeMap<&'static str, ThemeRanges>, PigmentAdaptationError> {
    let invalid = PigmentAdaptationError::InvalidProfile;
    let profile = exact_object(value, &["schemaVersion", "themes"])
        .ok_or(invalid("profile needs exactly schemaVersion and themes"))?;
    if profile["schemaVersion"] != "0.1.0" {
        return Err(invalid("schemaVersion must be 0.1.0"));
    }
    let themes = exact_object(&profile["themes"], &THEMES)
        .ok_or(invalid("themes needs exactly light and dark"))?;
    let role_names = ADAPTABLE_ROLES.map(role_name);
    let role_names: Vec<&str> = role_names.iter().map(String::as_str).collect();
    let mut parsed = BTreeMap::new();
    for name in THEMES {
        let roles = exact_object(&themes[name], &role_names).ok_or(invalid(
            "each theme needs exactly surface.base, surface.low, surface.high and surface.chrome",
        ))?;
        let mut ranges = BTreeMap::new();
        for role in ADAPTABLE_ROLES {
            let entry = exact_object(&roles[&role_name(role)], &["hue", "chroma"])
                .ok_or(invalid("each role needs exactly hue and chroma ranges"))?;
            let hue = parse_interval(
                &entry["hue"],
                (-180.0, 0.0),
                (0.0, 180.0),
                "hue min must be in [-180, 0] and max in [0, 180]",
            )?;
            let chroma = parse_interval(
                &entry["chroma"],
                (0.0, MAX_CHROMA),
                (0.0, MAX_CHROMA),
                "chroma bounds must be finite numbers in [0, 0.4]",
            )?;
            if chroma.min > chroma.max {
                return Err(invalid("chroma min must not exceed chroma max"));
            }
            ranges.insert(role, RoleRanges { hue, chroma });
        }
        parsed.insert(name, ranges);
    }
    Ok(parsed)
}

fn parse_color(value: &Value) -> Option<[f64; 3]> {
    let color = exact_object(value, &["colorSpace", "components"])?;
    if color["colorSpace"] != "srgb" {
        return None;
    }
    let components = color["components"]
        .as_array()
        .filter(|items| items.len() == 3)?;
    let mut parsed = [0.0; 3];
    for (slot, component) in parsed.iter_mut().zip(components) {
        *slot = number_in(component, 0.0, 1.0)?;
    }
    Some(parsed)
}

fn parse_observation(value: &Value) -> Result<Observation, PigmentAdaptationError> {
    let invalid = PigmentAdaptationError::InvalidObservation;
    let observation = value
        .as_object()
        .filter(|object| {
            object.contains_key("color")
                && object.len() == 1 + usize::from(object.contains_key("range"))
        })
        .ok_or(invalid("observation needs a color and optionally a range"))?;
    let color = parse_color(&observation["color"]).ok_or(invalid(
        "color must be an sRGB color with components in [0, 1]",
    ))?;
    let range = match observation.get("range") {
        None => None,
        Some(range) => {
            let range = exact_object(range, &["lower", "upper"])
                .ok_or(invalid("range needs exactly lower and upper"))?;
            let bounds = "range bounds must be sRGB colors with components in [0, 1]";
            let lower = parse_color(&range["lower"]).ok_or(invalid(bounds))?;
            let upper = parse_color(&range["upper"]).ok_or(invalid(bounds))?;
            if (0..3).any(|channel| lower[channel] > upper[channel]) {
                return Err(invalid("range lower must not exceed upper in any channel"));
            }
            if (0..3)
                .any(|channel| color[channel] < lower[channel] || color[channel] > upper[channel])
            {
                return Err(invalid("color must lie inside the range"));
            }
            Some((lower, upper))
        }
    };
    Ok(Observation { color, range })
}

const TEMPLATE_HUE_CHROMA_FLOOR: f64 = 1e-4;

fn wrap_half_turn(degrees: f64) -> f64 {
    let wrapped = degrees.rem_euclid(360.0);
    if wrapped > 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    }
}

fn select_target(
    template: [f64; 3],
    ranges: RoleRanges,
    observed: ObservedColor,
) -> Result<([f64; 3], [f64; 3]), AdaptedThemeError> {
    let [lightness, template_chroma, template_hue] = template;
    let hue = if template_chroma < TEMPLATE_HUE_CHROMA_FLOOR {
        0.0
    } else {
        template_hue
    };
    let offset = if observed.achromatic {
        0.0
    } else {
        ranges.hue.clamp(wrap_half_turn(observed.oklch[2] - hue))
    };
    let target = [
        lightness,
        ranges.chroma.clamp(observed.oklch[1]),
        normalize_hue(hue + offset),
    ];
    let components = gamut_map_oklch_to_srgb(target).map_err(AdaptedThemeError::Conversion)?;
    Ok((target, components))
}

fn adapt_theme(
    template: &str,
    external_sources: &BTreeMap<String, String>,
    ranges: &ThemeRanges,
    applied: Option<(&Observation, ObservedColor)>,
) -> Result<AdaptedTheme, AdaptedThemeError> {
    let compiled = compile_theme_source_with_sources(template, external_sources)
        .map_err(AdaptedThemeError::Template)?;
    let prefix = format!("{ADAPTED_COLOR_GROUP}.");
    if compiled
        .tokens()
        .keys()
        .any(|path| path == ADAPTED_COLOR_GROUP || path.starts_with(&prefix))
    {
        return Err(AdaptedThemeError::ReservedGroup);
    }
    let mut source = parse_token_document(template)
        .map_err(|error| AdaptedThemeError::Template(ThemeCompilationError::Parse(error)))?;
    let overrides = source["opaqueColorAssignments"]["roles"].clone();
    let mut template_oklch = BTreeMap::new();
    for role in ADAPTABLE_ROLES {
        let color = &compiled.color_fallbacks()[&role];
        let overridden = overrides.get(role_name(role)).is_some();
        if role == ColorRole::SurfaceChrome {
            if color.alpha() == 1.0 {
                return Err(AdaptedThemeError::OpaqueFrostRole);
            }
            if !overridden {
                return Err(AdaptedThemeError::MissingFrostFallback);
            }
        } else {
            if color.alpha() != 1.0 {
                return Err(AdaptedThemeError::TranslucentCastRole(role));
            }
            if overridden {
                return Err(AdaptedThemeError::OpaqueOverride(role));
            }
        }
        let oklch = srgb_to_oklch(color.components()).map_err(AdaptedThemeError::Conversion)?;
        if !ranges[&role].chroma.contains(oklch[1]) {
            return Err(AdaptedThemeError::ChromaOutsideRange(role));
        }
        template_oklch.insert(role, oklch);
    }

    let mut roles = BTreeMap::new();
    let mut frost_certification = None;
    let mut final_theme = compiled;
    if let Some((observation, observed)) = applied {
        for role in ADAPTABLE_ROLES {
            let (target_oklch, components) =
                select_target(template_oklch[&role], ranges[&role], observed)?;
            let mut color = json!({"colorSpace": "srgb", "components": components});
            if role == ColorRole::SurfaceChrome {
                let Some((lower, upper)) = observation.range else {
                    continue;
                };
                color["alpha"] = json!(final_theme.color_fallbacks()[&role].alpha());
                let certification = certify_frost(&color, lower, upper, &final_theme)
                    .map_err(AdaptedThemeError::Certification)?;
                let certified = certification.iter().all(FrostBound::passed);
                frost_certification = Some(certification);
                if !certified {
                    continue;
                }
            }
            roles.insert(
                role,
                AdaptedRole {
                    target_oklch,
                    color,
                },
            );
        }
        let mut group = Map::new();
        for (role, adapted) in &roles {
            insert_token(&mut group, &role_name(*role), adapted.color.clone());
        }
        let group = Value::Object(group);
        if let Some(tokens) = source.get_mut("tokens").and_then(Value::as_object_mut) {
            tokens.insert(ADAPTED_COLOR_GROUP.into(), group);
        } else if let Some(order) = source
            .pointer_mut("/tokenResolver/resolutionOrder")
            .and_then(Value::as_array_mut)
        {
            order.push(json!({
                "type": "set",
                "name": ADAPTED_COLOR_GROUP,
                "sources": [{ ADAPTED_COLOR_GROUP: group }]
            }));
        }
        for role in roles.keys() {
            let name = role_name(*role);
            source["colorAssignments"]["roles"][&name] =
                json!(format!("{ADAPTED_COLOR_GROUP}.{name}"));
        }
        final_theme = compile_theme_source_with_sources(&source.to_string(), external_sources)
            .map_err(AdaptedThemeError::Adapted)?;
    }
    let legibility_guard = resolve_legibility_guard(final_theme.opaque_color_fallbacks())
        .map_err(AdaptedThemeError::Guard)?;
    Ok(AdaptedTheme {
        source,
        roles,
        frost_certification,
        legibility_guard,
    })
}

fn certify_frost(
    color: &Value,
    lower: [f64; 3],
    upper: [f64; 3],
    theme: &CompiledTheme,
) -> Result<Vec<FrostBound>, String> {
    let frost = resolve_srgb_fallback(color).map_err(|error| error.to_string())?;
    let composite = |components: [f64; 3]| -> Result<SrgbFallback, String> {
        let backdrop =
            resolve_srgb_fallback(&json!({"colorSpace": "srgb", "components": components}))
                .map_err(|error| error.to_string())?;
        composite_srgb_over_opaque(&frost, &backdrop).map_err(|error| error.to_string())
    };
    let range = OpaqueSrgbRange::try_new(composite(lower)?, composite(upper)?)
        .map_err(|error| error.to_string())?;
    legibility_guard_pairs()
        .into_iter()
        .filter(|(_, background, _)| *background == ColorRole::SurfaceChrome)
        .map(|(foreground, _, minimum_contrast)| {
            let color = theme
                .opaque_color_fallbacks()
                .get(&foreground)
                .ok_or_else(|| format!("missing opaque {} color", role_name(foreground)))?;
            let contrast_bound =
                opaque_contrast_over_range(color, &range).map_err(|error| error.to_string())?;
            Ok(FrostBound {
                foreground,
                minimum_contrast,
                contrast_bound,
                passed: contrast_bound >= minimum_contrast,
            })
        })
        .collect()
}
