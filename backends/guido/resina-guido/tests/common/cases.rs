use resina_environment::LayoutDirection;
use serde_json::Value;

pub struct LabelCase {
    pub name: String,
    pub text: String,
    pub direction: LayoutDirection,
}

pub fn cases() -> Vec<LabelCase> {
    let mut cases: Vec<_> = [
        ("save", "Save", LayoutDirection::Ltr),
        (
            "expanded",
            "Verbindung erneut herstellen",
            LayoutDirection::Ltr,
        ),
        ("arabic", "إعادة الاتصال بالشبكة", LayoutDirection::Rtl),
    ]
    .into_iter()
    .map(|(name, text, direction)| LabelCase {
        name: name.to_owned(),
        text: text.to_owned(),
        direction,
    })
    .collect();
    let expansion: Value = serde_json::from_str(include_str!(
        "../../../../../conformance/content/command-label-expansion.json"
    ))
    .unwrap();
    let baseline = expansion["baselineText"].as_str().unwrap();
    assert!(baseline.is_ascii() && !baseline.trim().is_empty());
    let baseline_length = baseline.chars().count() as f64;
    let mut targets = Vec::new();
    for case in expansion["cases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        assert!(text.is_ascii() && !text.trim().is_empty());
        assert_eq!(
            text.chars().count() as f64,
            case["scalarLength"].as_f64().unwrap()
        );
        let target = case["targetExpansion"].as_f64().unwrap();
        assert!(!targets.contains(&target));
        assert!((text.chars().count() as f64 / baseline_length / target - 1.0).abs() <= 0.05);
        targets.push(target);
        cases.push(LabelCase {
            name: case["name"].as_str().unwrap().to_owned(),
            text: text.to_owned(),
            direction: LayoutDirection::Ltr,
        });
    }
    targets.sort_by(f64::total_cmp);
    assert_eq!(targets, [1.0, 1.5, 2.0]);
    let mut names = std::collections::BTreeSet::new();
    for case in &cases {
        assert!(names.insert(&case.name));
    }
    cases
}
