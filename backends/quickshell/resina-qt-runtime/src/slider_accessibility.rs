use cxx_qt_lib::QString;
use resina_model::{SliderOrientation, SliderValue};
use resina_resolver::{resolve_slider_adjustment_source, resolve_slider_value_source};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};

#[cxx::bridge(namespace = "resina::qt")]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
    }
    extern "Rust" {
        fn validate_slider_accessibility_publication(source: &QString) -> QString;
        fn prepare_slider_accessibility_adjustment(
            source: &QString,
            adjustment: &QString,
            source_available: bool,
        ) -> QString;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Publication {
    accessibility: Accessibility,
    revision: String,
    step_size: f64,
    value_policy: ContinuousPolicy,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum ContinuousPolicy {
    Continuous,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Accessibility {
    schema_version: String,
    role: String,
    name: String,
    description: Value,
    value: SliderValue,
    value_text: Value,
    state: State,
    orientation: SliderOrientation,
    actions: [Action; 3],
    focusable: bool,
    relationships: [Value; 0],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    enabled: bool,
    read_only: bool,
    focused: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    kind: String,
    available: bool,
}

fn optional_text(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn parse(source: &str) -> Result<Publication, String> {
    if source.len() > 64 * 1024 {
        return Err("Qt Slider accessibility publication exceeds 64 KiB".to_owned());
    }
    let document = parse_token_document(source).map_err(|e| e.to_string())?;
    for value in [
        &document,
        &document["accessibility"],
        &document["accessibility"]["value"],
        &document["accessibility"]["state"],
        &document["valuePolicy"],
    ] {
        if !value.is_object() {
            return Err("Qt Slider accessibility members must be objects".to_owned());
        }
    }
    let a = &document["accessibility"];
    if !a
        .as_object()
        .is_some_and(|a| a.contains_key("description") && a.contains_key("valueText"))
        || !a["actions"]
            .as_array()
            .is_some_and(|actions| actions.iter().all(Value::is_object))
    {
        return Err(
            "Qt Slider semantic nullable fields and action objects are required".to_owned(),
        );
    }
    let request: Publication = serde_json::from_str(source).map_err(|e| e.to_string())?;
    // Keep numeric literals intact for the existing portable precision boundary.
    let root: std::collections::BTreeMap<String, &RawValue> =
        serde_json::from_str(source).map_err(|e| e.to_string())?;
    let semantic: std::collections::BTreeMap<String, &RawValue> =
        serde_json::from_str(root["accessibility"].get()).map_err(|e| e.to_string())?;
    resolve_slider_value_source(semantic["value"].get()).map_err(|e| e.to_string())?;
    let step_validation = format!(
        r#"{{"schemaVersion":"0.1.0","current":{},"enabled":false,"readOnly":false,"adjustment":{{"kind":"increase","amount":{}}}}}"#,
        semantic["value"].get(),
        root["stepSize"].get()
    );
    resolve_slider_adjustment_source(&step_validation).map_err(|e| e.to_string())?;
    let a = &request.accessibility;
    let available = a.state.enabled && !a.state.read_only;
    if a.schema_version != "0.1.0"
        || a.role != "slider"
        || a.name.trim().is_empty()
        || !optional_text(&a.description)
        || !optional_text(&a.value_text)
        || (!a.focusable && (a.state.enabled || a.state.focused))
        || a.actions
            .iter()
            .zip(["setValue", "increase", "decrease"])
            .any(|(action, kind)| action.kind != kind || action.available != available)
        || request.revision.is_empty()
        || !request.step_size.is_finite()
        || request.step_size <= 0.0
    {
        return Err("incoherent Qt Slider accessibility publication".to_owned());
    }
    // These parsed fields are deliberately required at this mapping boundary.
    let _ = (a.orientation, &a.relationships, &request.value_policy);
    Ok(request)
}

fn validate_slider_accessibility_publication(source: &QString) -> QString {
    let result = parse(&source.to_string())
        .map(|_| json!({"valid": true}))
        .unwrap_or_else(|error| json!({"error": error}));
    QString::from(result.to_string())
}

fn prepare(source: &str, adjustment: &str, source_available: bool) -> Result<Value, String> {
    let request = parse(source)?;
    if adjustment.len() > 64 * 1024 {
        return Err("Qt Slider adjustment exceeds 64 KiB".to_owned());
    }
    let current = serde_json::to_string(&request.accessibility.value).map_err(|e| e.to_string())?;
    // The reference source parser validates domain, finite/exact numbers, action
    // shape and live permissions. Native AT delivery never writes a property.
    let input = format!(
        r#"{{"schemaVersion":"0.1.0","current":{current},"enabled":{},"readOnly":{},"adjustment":{adjustment}}}"#,
        request.accessibility.state.enabled && source_available,
        request.accessibility.state.read_only
    );
    let candidate = resolve_slider_adjustment_source(&input).map_err(|e| e.to_string())?;
    let base = resolve_slider_value_source(&current).map_err(|e| e.to_string())?;
    Ok(json!({"candidate": candidate, "candidateBase": base, "revision": request.revision}))
}

fn prepare_slider_accessibility_adjustment(
    source: &QString,
    adjustment: &QString,
    source_available: bool,
) -> QString {
    QString::from(
        prepare(
            &source.to_string(),
            &adjustment.to_string(),
            source_available,
        )
        .unwrap_or_else(|error| json!({"error": error}))
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn publication() -> Value {
        json!({"revision":"fixture-1", "stepSize":0.05,"valuePolicy":{"kind":"continuous"},
          "accessibility":{"schemaVersion":"0.1.0","role":"slider","name":"Volume",
          "description":null,"valueText":"40 percent","value":{"schemaVersion":"0.1.0","minimum":0,"maximum":1,"value":0.4},
          "state":{"enabled":true,"readOnly":false,"focused":false},"orientation":"horizontal",
          "actions":[{"kind":"setValue","available":true},{"kind":"increase","available":true},{"kind":"decrease","available":true}],
          "focusable":true,"relationships":[]}})
    }
    #[test]
    fn native_intent_uses_live_reference_guards_without_mutating_publication() {
        let source = publication().to_string();
        let result = prepare(&source, r#"{"kind":"setValue","value":0.65}"#, true).unwrap();
        assert_eq!(result["candidate"]["value"]["value"], 0.65);
        assert_eq!(result["candidateBase"]["value"], 0.4);
        assert!(result["candidate"]["changed"].as_bool().unwrap());
        assert!(!prepare(&source, r#"{"kind":"setValue","value":0.65}"#, false).unwrap()["candidate"]["accepted"].as_bool().unwrap());
        for field in ["enabled", "readOnly"] {
            let mut p = publication();
            p["accessibility"]["state"][field] = json!(field == "readOnly");
            for action in p["accessibility"]["actions"].as_array_mut().unwrap() {
                action["available"] = json!(false);
            }
            let result =
                prepare(&p.to_string(), r#"{"kind":"setValue","value":0.65}"#, true).unwrap();
            assert_eq!(result["candidate"]["accepted"], false);
            assert_eq!(result["candidate"]["value"]["value"], 0.4);
        }
        for adjustment in [
            r#"{"kind":"setValue","value":2}"#,
            r#"{"kind":"setValue","value":9007199254740993}"#,
            r#"{"kind":"increase","amount":0}"#,
            r#"{"kind":"increase","kind":"decrease","amount":0.1}"#,
        ] {
            assert!(prepare(&source, adjustment, true).is_err());
        }
    }
    #[test]
    fn mapping_accepts_shared_portable_accessibility_vectors() {
        let cases: Value = serde_json::from_str(include_str!(
            "../../../../conformance/accessibility/slider-cases.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            if let Some(expected) = case.get("expected") {
                let mut p = publication();
                p["accessibility"] = expected.clone();
                assert!(parse(&p.to_string()).is_ok(), "{}", case["name"]);
            }
        }
    }
    #[test]
    fn mapping_rejects_incoherence_and_unsupported_policy() {
        for (path, bad) in [
            (vec!["accessibility", "role"], json!("button")),
            (vec!["accessibility", "valueText"], json!(" ")),
            (vec!["accessibility", "focusable"], json!(false)),
            (vec!["revision"], json!("")),
            (vec!["stepSize"], json!(0)),
            (vec!["valuePolicy", "kind"], json!("stops")),
        ] {
            let mut p = publication();
            let mut field = &mut p;
            for key in path {
                field = &mut field[key];
            }
            *field = bad;
            assert!(parse(&p.to_string()).is_err());
        }
        for field in ["description", "valueText"] {
            let mut p = publication();
            p["accessibility"].as_object_mut().unwrap().remove(field);
            assert!(parse(&p.to_string()).is_err());
        }
        let mut p = publication();
        p["accessibility"]["actions"][0] = json!(["setValue", true]);
        assert!(parse(&p.to_string()).is_err());
        let source = publication().to_string();
        assert!(parse(&source.replace("0.05", "9007199254740993")).is_err());
        assert!(parse(&source.replace("0.4", "9007199254740993")).is_err());
        assert!(parse(&source.replace("\"stepSize\":", "\"unknown\":1,\"stepSize\":")).is_err());
        assert!(
            parse(&source.replace("\"revision\":", "\"revision\":\"duplicate\",\"revision\":"))
                .is_err()
        );
        assert!(parse(&" ".repeat(65537)).is_err());
    }
}
