use crate::{
    LegibilityGuardError, LegibilityGuardPair, ThemeCompilationError,
    compile_theme_source_with_sources, resolve_legibility_guard, theme_legibility::role_name,
};
use resina_color::{ColorConversionError, gamut_map_oklch_to_srgb, normalize_hue, srgb_to_oklch};
use resina_model::ColorRole;
use resina_tokens::parse_token_document;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, fmt};

pub const ACHROMATIC_SEED_CHROMA: f64 = 0.02;
pub const DERIVED_COLOR_GROUP: &str = "derivedColor";
pub const DERIVED_COLOR_ROLES: [ColorRole; 5] = [
    ColorRole::AccentPrimary,
    ColorRole::AccentSecondary,
    ColorRole::AccentTertiary,
    ColorRole::Selection,
    ColorRole::Focus,
];
const THEMES: [&str; 2] = ["light", "dark"];

#[derive(Debug)]
pub enum ThemeDerivationError {
    Parse(serde_json::Error),
    InvalidRequest(&'static str),
    UnsupportedVersion,
    InvalidSeed(&'static str),
    Themes(Vec<(&'static str, DerivedThemeError)>),
}

#[derive(Debug)]
pub enum DerivedThemeError {
    Template(ThemeCompilationError),
    TranslucentTemplateRole(ColorRole),
    OpaqueOverride(ColorRole),
    ReservedGroup,
    Conversion(ColorConversionError),
    Derived(ThemeCompilationError),
    Guard(LegibilityGuardError),
}

impl fmt::Display for DerivedThemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Template(error) => write!(formatter, "template compilation failed: {error}"),
            Self::TranslucentTemplateRole(role) => {
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
            Self::ReservedGroup => write!(
                formatter,
                "template already defines tokens in the {DERIVED_COLOR_GROUP} group"
            ),
            Self::Conversion(error) => write!(formatter, "color derivation failed: {error}"),
            Self::Derived(error) => write!(formatter, "derived theme compilation failed: {error}"),
            Self::Guard(error) => error.fmt(formatter),
        }
    }
}

impl fmt::Display for ThemeDerivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => {
                write!(formatter, "theme derivation request parse failed: {error}")
            }
            Self::InvalidRequest(reason) => {
                write!(formatter, "invalid theme derivation request: {reason}")
            }
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidSeed(reason) => write!(formatter, "invalid seed: {reason}"),
            Self::Themes(errors) => {
                formatter.write_str("theme derivation failed")?;
                for (theme, error) in errors {
                    write!(formatter, "\n{theme} theme: {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ThemeDerivationError {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDerivation {
    schema_version: &'static str,
    seed: DerivedSeed,
    themes: BTreeMap<&'static str, DerivedTheme>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedSeed {
    oklch: [f64; 3],
    achromatic: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedTheme {
    source: Value,
    roles: BTreeMap<ColorRole, DerivedRole>,
    legibility_guard: Vec<LegibilityGuardPair>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedRole {
    target_oklch: [f64; 3],
    color: Value,
}

impl ThemeDerivation {
    pub fn seed(&self) -> DerivedSeed {
        self.seed
    }

    pub fn theme(&self, name: &str) -> Option<&DerivedTheme> {
        self.themes.get(name)
    }
}

impl DerivedSeed {
    pub fn oklch(&self) -> [f64; 3] {
        self.oklch
    }

    pub fn achromatic(&self) -> bool {
        self.achromatic
    }
}

impl DerivedTheme {
    pub fn source(&self) -> &Value {
        &self.source
    }

    pub fn legibility_guard(&self) -> &[LegibilityGuardPair] {
        &self.legibility_guard
    }

    pub fn role(&self, role: ColorRole) -> Option<([f64; 3], [f64; 3])> {
        self.roles.get(&role).map(|derived| {
            let components = &derived.color["components"];
            (
                derived.target_oklch,
                [0, 1, 2].map(|index| components[index].as_f64().unwrap_or(f64::NAN)),
            )
        })
    }
}

pub fn derive_theme_request_source(source: &str) -> Result<ThemeDerivation, ThemeDerivationError> {
    let document = parse_token_document(source).map_err(ThemeDerivationError::Parse)?;
    let request = document
        .as_object()
        .ok_or(ThemeDerivationError::InvalidRequest(
            "request must be a JSON object",
        ))?;
    match request.get("schemaVersion") {
        Some(Value::String(version)) if version == "0.1.0" => {}
        Some(Value::String(_)) => return Err(ThemeDerivationError::UnsupportedVersion),
        _ => {
            return Err(ThemeDerivationError::InvalidRequest(
                "schemaVersion must be a string",
            ));
        }
    }
    if request.len() != 4 {
        return Err(ThemeDerivationError::InvalidRequest(
            "request needs exactly schemaVersion, seed, templates and externalSources",
        ));
    }
    let seed = seed_components(request.get("seed"))?;
    let templates = request
        .get("templates")
        .and_then(Value::as_object)
        .filter(|templates| templates.len() == 2)
        .ok_or(ThemeDerivationError::InvalidRequest(
            "templates must be an object with light and dark source text",
        ))?;
    let mut template_sources = Vec::new();
    for name in THEMES {
        let text = templates.get(name).and_then(Value::as_str).ok_or(
            ThemeDerivationError::InvalidRequest(
                "templates must be an object with light and dark source text",
            ),
        )?;
        template_sources.push((name, text));
    }
    let external_sources = request
        .get("externalSources")
        .and_then(Value::as_object)
        .ok_or(ThemeDerivationError::InvalidRequest(
            "externalSources must be an object of source text",
        ))?
        .iter()
        .map(|(uri, text)| {
            text.as_str()
                .map(|text| (uri.clone(), text.to_owned()))
                .ok_or(ThemeDerivationError::InvalidRequest(
                    "externalSources must be an object of source text",
                ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    derive_themes(seed, &template_sources, &external_sources)
}

fn seed_components(seed: Option<&Value>) -> Result<[f64; 3], ThemeDerivationError> {
    let seed = seed
        .and_then(Value::as_object)
        .ok_or(ThemeDerivationError::InvalidSeed(
            "seed must be an sRGB color object",
        ))?;
    if seed.len() != 2 || seed.get("colorSpace").and_then(Value::as_str) != Some("srgb") {
        return Err(ThemeDerivationError::InvalidSeed(
            "seed needs exactly colorSpace srgb and components",
        ));
    }
    let components = seed
        .get("components")
        .and_then(Value::as_array)
        .filter(|components| components.len() == 3)
        .ok_or(ThemeDerivationError::InvalidSeed(
            "seed components must be three numbers",
        ))?;
    let mut values = [0.0; 3];
    for (value, component) in values.iter_mut().zip(components) {
        *value = component.as_f64().ok_or(ThemeDerivationError::InvalidSeed(
            "seed components must be three numbers",
        ))?;
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            return Err(ThemeDerivationError::InvalidSeed(
                "seed components must be finite and within [0, 1]",
            ));
        }
    }
    Ok(values)
}

pub fn derive_seed(seed: [f64; 3]) -> Result<DerivedSeed, ColorConversionError> {
    let [lightness, chroma, hue] = srgb_to_oklch(seed)?;
    let achromatic = chroma < ACHROMATIC_SEED_CHROMA;
    Ok(DerivedSeed {
        oklch: [lightness, chroma, if achromatic { 0.0 } else { hue }],
        achromatic,
    })
}

fn derive_themes(
    seed: [f64; 3],
    templates: &[(&'static str, &str)],
    external_sources: &BTreeMap<String, String>,
) -> Result<ThemeDerivation, ThemeDerivationError> {
    let derived_seed = derive_seed(seed).map_err(|error| {
        ThemeDerivationError::Themes(
            THEMES
                .iter()
                .map(|theme| (*theme, DerivedThemeError::Conversion(error)))
                .collect(),
        )
    })?;
    let mut themes = BTreeMap::new();
    let mut errors = Vec::new();
    for (name, template) in templates {
        match derive_theme(derived_seed, template, external_sources) {
            Ok(theme) => {
                themes.insert(*name, theme);
            }
            Err(error) => errors.push((*name, error)),
        }
    }
    if !errors.is_empty() {
        return Err(ThemeDerivationError::Themes(errors));
    }
    Ok(ThemeDerivation {
        schema_version: "0.1.0",
        seed: derived_seed,
        themes,
    })
}

fn derive_theme(
    seed: DerivedSeed,
    template: &str,
    external_sources: &BTreeMap<String, String>,
) -> Result<DerivedTheme, DerivedThemeError> {
    let compiled = compile_theme_source_with_sources(template, external_sources)
        .map_err(DerivedThemeError::Template)?;
    let prefix = format!("{DERIVED_COLOR_GROUP}.");
    if compiled
        .tokens()
        .keys()
        .any(|path| path == DERIVED_COLOR_GROUP || path.starts_with(&prefix))
    {
        return Err(DerivedThemeError::ReservedGroup);
    }
    let mut source = parse_token_document(template)
        .map_err(|error| DerivedThemeError::Template(ThemeCompilationError::Parse(error)))?;
    let overrides = &source["opaqueColorAssignments"]["roles"];
    let mut template_oklch = BTreeMap::new();
    for role in DERIVED_COLOR_ROLES {
        let color = &compiled.color_fallbacks()[&role];
        if color.alpha() != 1.0 {
            return Err(DerivedThemeError::TranslucentTemplateRole(role));
        }
        if overrides.get(role_name(role)).is_some() {
            return Err(DerivedThemeError::OpaqueOverride(role));
        }
        template_oklch.insert(
            role,
            srgb_to_oklch(color.components()).map_err(DerivedThemeError::Conversion)?,
        );
    }
    let primary_hue = template_oklch[&ColorRole::AccentPrimary][2];
    let mut roles = BTreeMap::new();
    let mut group = Map::new();
    for role in DERIVED_COLOR_ROLES {
        let [lightness, chroma, hue] = template_oklch[&role];
        let target = if seed.achromatic {
            [lightness, 0.0, 0.0]
        } else {
            let offset = match role {
                ColorRole::AccentSecondary | ColorRole::AccentTertiary => hue - primary_hue,
                _ => 0.0,
            };
            [lightness, chroma, normalize_hue(seed.oklch[2] + offset)]
        };
        let components = gamut_map_oklch_to_srgb(target).map_err(DerivedThemeError::Conversion)?;
        let color = json!({"colorSpace": "srgb", "components": components});
        insert_token(&mut group, &role_name(role), color.clone());
        roles.insert(
            role,
            DerivedRole {
                target_oklch: target,
                color,
            },
        );
    }
    let group = Value::Object(group);
    if let Some(tokens) = source.get_mut("tokens").and_then(Value::as_object_mut) {
        tokens.insert(DERIVED_COLOR_GROUP.into(), group);
    } else if let Some(order) = source
        .pointer_mut("/tokenResolver/resolutionOrder")
        .and_then(Value::as_array_mut)
    {
        order.push(json!({
            "type": "set",
            "name": DERIVED_COLOR_GROUP,
            "sources": [{ DERIVED_COLOR_GROUP: group }]
        }));
    }
    for role in DERIVED_COLOR_ROLES {
        let name = role_name(role);
        source["colorAssignments"]["roles"][&name] = json!(format!("{DERIVED_COLOR_GROUP}.{name}"));
    }
    let derived = compile_theme_source_with_sources(&source.to_string(), external_sources)
        .map_err(DerivedThemeError::Derived)?;
    let legibility_guard = resolve_legibility_guard(derived.opaque_color_fallbacks())
        .map_err(DerivedThemeError::Guard)?;
    Ok(DerivedTheme {
        source,
        roles,
        legibility_guard,
    })
}

pub(crate) fn insert_token(group: &mut Map<String, Value>, path: &str, value: Value) {
    let mut node = group;
    let mut segments = path.split('.').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            node.insert(segment.into(), json!({"$type": "color", "$value": value}));
            return;
        }
        node = node
            .entry(segment)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("derived token groups are objects");
    }
}
