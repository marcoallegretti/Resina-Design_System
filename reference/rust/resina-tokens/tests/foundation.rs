use resina_tokens::resolve_token_source;
use serde_json::Value;

#[test]
fn authored_spatial_scale_resolves_to_conformance_values() {
    let tokens = resolve_token_source(include_str!("../../../../tokens/foundation.json")).unwrap();
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/spatial/foundation-vectors.json"
    ))
    .unwrap();
    assert_eq!(tokens.len(), vectors.len());
    let mut previous = None;
    for vector in vectors {
        let path = vector["path"].as_str().unwrap();
        let token = &tokens[path];
        assert_eq!(token.token_type, "dimension", "{path}");
        assert_eq!(token.value, vector["value"], "{path}");
        let distance = token.value["value"].as_f64().unwrap();
        if let Some(previous) = previous {
            assert!(distance > previous, "{path} must increase");
        }
        previous = Some(distance);
    }
}
