use cxx_qt_lib::QString;
use resina_environment::{EnvironmentSnapshot, SafeArea};
use resina_model::{FontFamilyRole, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelInput, compile_theme_source_with_sources, resolve_command_label,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_MEASUREMENT_BYTES: usize = 64 * 1024;

#[cxx::bridge(namespace = "resina::qt")]
mod ffi {
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
struct LabelRequest {
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

fn prepare(request: &str, producer: &ffi::NativeLabelMeasure) -> Result<serde_json::Value, String> {
    if request.len() > MAX_REQUEST_BYTES {
        return Err("Qt label request exceeds 8 MiB".to_owned());
    }
    let document = parse_token_document(request).map_err(|error| error.to_string())?;
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
    let request: LabelRequest =
        serde_json::from_value(document).map_err(|error| error.to_string())?;
    let theme = compile_theme_source_with_sources(&request.theme_source, &request.external_sources)
        .map_err(|error| error.to_string())?;
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
    Ok(json!({"label": label}))
}

fn resolve_label(request: &QString, producer: &ffi::NativeLabelMeasure) -> QString {
    let result =
        prepare(&request.to_string(), producer).unwrap_or_else(|error| json!({"error": error}));
    QString::from(result.to_string())
}
