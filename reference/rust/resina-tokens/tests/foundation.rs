use resina_tokens::resolve_token_source;
use serde_json::Value;

#[test]
fn authored_foundation_scales_resolve_to_conformance_values() {
    let tokens = resolve_token_source(include_str!("../../../../tokens/foundation.json")).unwrap();
    let spatial: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/spatial/foundation-vectors.json"
    ))
    .unwrap();
    let type_sizes: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/typography/foundation-vectors.json"
    ))
    .unwrap();
    let radii: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/foundation-radius-vectors.json"
    ))
    .unwrap();
    let depths: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/elevation/foundation-depth-vectors.json"
    ))
    .unwrap();
    assert_eq!(
        tokens.len(),
        spatial.len() + type_sizes.len() + radii.len() + depths.len()
    );
    for vectors in [spatial, type_sizes, radii, depths] {
        let mut previous = None;
        for vector in vectors {
            let path = vector["path"].as_str().unwrap();
            let token = &tokens[path];
            assert_eq!(token.token_type, "dimension", "{path}");
            assert_eq!(token.value, vector["value"], "{path}");
            let size = token.value["value"].as_f64().unwrap();
            if let Some(previous) = previous {
                assert!(size > previous, "{path} must increase");
            }
            previous = Some(size);
        }
    }
}
