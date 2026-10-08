pub fn expected(operation: &str, name: &serde_json::Value) -> String {
    let messages: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/diagnostics.json")).unwrap();
    messages[operation][name.as_str().unwrap()]
        .as_str()
        .expect("reference diagnostic expectation is missing")
        .to_owned()
}
