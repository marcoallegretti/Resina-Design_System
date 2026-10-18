use cxx_qt_lib::QString;
use resina_environment::{EnvironmentSnapshot, SafeArea};
use resina_model::{FontFamilyRole, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, CompiledTheme, compile_theme_source_with_sources,
    resolve_command_label,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_MEASUREMENT_BYTES: usize = 64 * 1024;

type ThemeEntry = (String, BTreeMap<String, String>, Rc<CompiledTheme>);

thread_local! {
    // Theme compilation is pure; consecutive frames repeat the same source bytes.
    static THEME: RefCell<Option<ThemeEntry>> = const { RefCell::new(None) };
}

#[cfg(test)]
thread_local! {
    pub(crate) static THEME_COMPILES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cxx::bridge(namespace = "resina::qt")]
pub(crate) mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
        include!("label.h");
        type NativeLabelMeasure;
        fn measure(self: &NativeLabelMeasure, request: &QString) -> Result<QString>;
    }
    extern "Rust" {
        fn resolve_label(request: &QString, producer: &NativeLabelMeasure) -> QString;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LabelRequest {
    theme_source: String,
    external_sources: BTreeMap<String, String>,
    environment: EnvironmentSnapshot,
    text: String,
    font_family: FontMapping,
    minimum_size: SurfaceSize,
    maximum_size: SurfaceSize,
    padding: SafeArea,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontMapping {
    role: FontFamilyRole,
    name: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Measurement {
    Extent { width: f64, height: f64 },
    Error { message: String },
}

pub(crate) struct PreparedContent {
    pub label: CommandLabelIr,
    pub theme: Rc<CompiledTheme>,
    pub environment: EnvironmentSnapshot,
}

pub(crate) fn parse_label_request(document: serde_json::Value) -> Result<LabelRequest, String> {
    if !document.is_object() {
        return Err("Qt label request must be an object".to_owned());
    }
    for field in ["fontFamily", "padding"] {
        if !document
            .get(field)
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(format!("Qt label {field} must be an object"));
        }
    }
    serde_json::from_value(document).map_err(|error| error.to_string())
}

pub(crate) fn prepare_content(
    request: LabelRequest,
    producer: &ffi::NativeLabelMeasure,
) -> Result<PreparedContent, String> {
    let theme = compiled_theme(&request.theme_source, &request.external_sources)?;
    let resolved = theme.resolve(&request.environment).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let typography = &resolved.typography()[&TypographyRole::Label];
    if request.font_family.role != typography.family_role()
        || request.font_family.name.trim().is_empty()
    {
        return Err(
            "Qt label requires an explicit mapping for its resolved font-family role".to_owned(),
        );
    }
    let label = resolve_command_label(
        CommandLabelInput {
            text: &request.text,
            typography,
            minimum_size: request.minimum_size,
            maximum_size: request.maximum_size,
            padding: request.padding,
            direction: request.environment.layout_direction(),
        },
        |input| -> Result<SurfaceSize, String> {
            let native_request = json!({
                "text": input.text, "family": request.font_family.name,
                "pixelSize": input.typography.font_size(), "weight": input.typography.font_weight(),
                "spacing": input.typography.letter_spacing(), "lineHeight": input.typography.line_height(),
                "maximumWidth": input.maximum_width
            });
            let response = producer.measure(&QString::from(native_request.to_string()))
                .map_err(|error| error.to_string())?.to_string();
            if response.len() > MAX_MEASUREMENT_BYTES {
                return Err("Qt label measurement exceeds 64 KiB".to_owned());
            }
            let response: Measurement = serde_json::from_value(
                parse_token_document(&response).map_err(|error| error.to_string())?,
            ).map_err(|error| error.to_string())?;
            match response {
                Measurement::Extent { width, height } => Ok(SurfaceSize { width, height }),
                Measurement::Error { message } => Err(message),
            }
        },
    ).map_err(|error| error.to_string())?;
    Ok(PreparedContent {
        label,
        theme,
        environment: request.environment,
    })
}

pub(crate) fn compiled_theme(
    source: &str,
    external_sources: &BTreeMap<String, String>,
) -> Result<Rc<CompiledTheme>, String> {
    let cached = THEME
        .try_with(|entry| {
            entry
                .borrow()
                .as_ref()
                .filter(|(cached, sources, _)| cached == source && sources == external_sources)
                .map(|(_, _, theme)| Rc::clone(theme))
        })
        .ok()
        .flatten();
    if let Some(theme) = cached {
        return Ok(theme);
    }
    #[cfg(test)]
    THEME_COMPILES.set(THEME_COMPILES.get() + 1);
    let theme = Rc::new(
        compile_theme_source_with_sources(source, external_sources)
            .map_err(|error| error.to_string())?,
    );
    let _ = THEME.try_with(|entry| {
        *entry.borrow_mut() = Some((
            source.to_owned(),
            external_sources.clone(),
            Rc::clone(&theme),
        ))
    });
    Ok(theme)
}

fn prepare(request: &str, producer: &ffi::NativeLabelMeasure) -> Result<serde_json::Value, String> {
    if request.len() > MAX_REQUEST_BYTES {
        return Err("Qt label request exceeds 8 MiB".to_owned());
    }
    let document = parse_token_document(request).map_err(|error| error.to_string())?;
    let content = prepare_content(parse_label_request(document)?, producer)?;
    Ok(json!({"label": content.label}))
}

fn resolve_label(request: &QString, producer: &ffi::NativeLabelMeasure) -> QString {
    let result =
        prepare(&request.to_string(), producer).unwrap_or_else(|error| json!({"error": error}));
    QString::from(result.to_string())
}

#[cfg(test)]
mod theme_cache_tests {
    use super::*;

    fn sources() -> (String, BTreeMap<String, String>) {
        (
            include_str!("../../../../tokens/themes/light.json").to_owned(),
            BTreeMap::from([(
                "foundation.json".to_owned(),
                include_str!("../../../../tokens/foundation.json").to_owned(),
            )]),
        )
    }

    #[test]
    fn identical_sources_reuse_the_compiled_theme() {
        let (theme, external) = sources();
        let first = compiled_theme(&theme, &external).unwrap();
        assert!(Rc::ptr_eq(
            &first,
            &compiled_theme(&theme, &external).unwrap()
        ));
        let dark = include_str!("../../../../tokens/themes/dark.json");
        let other = compiled_theme(dark, &external).unwrap();
        assert!(!Rc::ptr_eq(&first, &other));
        assert!(!Rc::ptr_eq(
            &first,
            &compiled_theme(&theme, &external).unwrap()
        ));
    }

    #[test]
    fn a_failing_source_is_diagnosed_again_and_not_kept() {
        let (theme, external) = sources();
        let kept = compiled_theme(&theme, &external).unwrap();
        let broken = theme.replacen('{', "{\"unknownMember\": 1, ", 1);
        let compiles = THEME_COMPILES.get();
        let first = compiled_theme(&broken, &external).err().unwrap();
        assert_eq!(compiled_theme(&broken, &external).err().unwrap(), first);
        assert_eq!(THEME_COMPILES.get(), compiles + 2);
        assert!(Rc::ptr_eq(
            &kept,
            &compiled_theme(&theme, &external).unwrap()
        ));
    }
}
