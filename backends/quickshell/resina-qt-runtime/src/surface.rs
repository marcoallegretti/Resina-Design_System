use crate::snapshot::RenderOptions;
use cxx_qt_lib::QString;
use resina_model::PhysicalVector;
use resina_resolver::resolve_surface_paint_source;
use resina_tokens::parse_token_document;
use serde_json::{Value, json};

#[cxx::bridge(namespace = "resina::qt")]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
    }
    extern "Rust" {
        fn resolve_surface_view(request: &QString, options: &QString) -> QString;
    }
}

fn prepare(request: &str, options: &str) -> Result<Value, String> {
    if request.len() > 1024 * 1024 {
        return Err("Qt surface paint request exceeds 1 MiB".to_owned());
    }
    if options.len() > 64 * 1024 {
        return Err("Qt surface rendering options exceed 64 KiB".to_owned());
    }
    let document = parse_token_document(options).map_err(|error| error.to_string())?;
    if !document.is_object() {
        return Err("Qt surface rendering options must be an object".to_owned());
    }
    let options: RenderOptions =
        serde_json::from_value(document).map_err(|error| error.to_string())?;
    let paint = resolve_surface_paint_source(request).map_err(|error| error.to_string())?;
    let source = resina_qml::render_surface_paint(
        &paint,
        PhysicalVector {
            x: options.x,
            y: options.y,
        },
        options.device_scale,
        options.samples_per_axis,
    )
    .map_err(|error| error.to_string())?;
    Ok(json!({"paint": paint, "paintSource": source}))
}

fn resolve_surface_view(request: &QString, options: &QString) -> QString {
    QString::from(
        prepare(&request.to_string(), &options.to_string())
            .unwrap_or_else(|error| json!({"error": error}))
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn request() -> Value {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let body: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("conformance/ir/opaque-surface-request.json"))
                .unwrap(),
        )
        .unwrap();
        json!({"schemaVersion": "0.1.0", "body": body,
            "surroundingColor": {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}})
    }

    const OPTIONS: &str = r#"{"x":44.3,"y":26.1,"deviceScale":1.25,"samplesPerAxis":4}"#;

    #[test]
    fn publishes_reference_paint_with_its_native_realization() {
        let request = request().to_string();
        let result = prepare(&request, OPTIONS).unwrap();
        let reference = resolve_surface_paint_source(&request).unwrap();
        assert_eq!(result["paint"], serde_json::to_value(&reference).unwrap());
        let expected = resina_qml::render_surface_paint(
            &reference,
            PhysicalVector { x: 44.3, y: 26.1 },
            1.25,
            4,
        )
        .unwrap();
        assert_eq!(result["paintSource"], Value::String(expected));
    }

    #[test]
    fn rejects_invalid_requests_and_options_without_paint() {
        let request = request();
        let mut focused = request.clone();
        focused["body"]["surface"]["states"]["states"] = json!(["rest", "focused"]);
        focused.as_object_mut().unwrap().remove("surroundingColor");
        for (source, options) in [
            ("[]".to_owned(), OPTIONS),
            ("{}".to_owned(), OPTIONS),
            (focused.to_string(), OPTIONS),
            (request.to_string(), "[]"),
            (
                request.to_string(),
                r#"{"x":0,"y":0,"deviceScale":0,"samplesPerAxis":4}"#,
            ),
            (
                request.to_string(),
                r#"{"x":0,"y":0,"deviceScale":1,"samplesPerAxis":4,"extra":1}"#,
            ),
        ] {
            let result: Value = serde_json::from_str(
                &resolve_surface_view(&QString::from(source.as_str()), &QString::from(options))
                    .to_string(),
            )
            .unwrap();
            assert!(result["error"].is_string(), "{source} {options}");
            assert!(result.get("paint").is_none() && result.get("paintSource").is_none());
        }
    }
}
