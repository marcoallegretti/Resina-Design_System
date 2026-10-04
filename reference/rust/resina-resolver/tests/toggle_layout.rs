use resina_resolver::resolve_toggle_layout_source;
use serde_json::Value;

#[test]
fn public_layouts_match_authored_endpoints_or_fail_diagnostically() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/toggle-layout-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_toggle_layout_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

#[test]
fn mirrored_endpoints_and_reserved_target_preserve_track_ownership() {
    use resina_environment::{EnvironmentSnapshot, LayoutDirection, SafeArea};
    use resina_model::{PhysicalBounds, SurfaceSize};
    use resina_resolver::{
        HitRegionInput, ToggleLayoutInput, resolve_hit_region, resolve_toggle_layout,
    };
    let request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/hit-region-request.json"
    ))
    .unwrap();
    let environment: EnvironmentSnapshot =
        serde_json::from_value(request["environment"].clone()).unwrap();
    let insets = SafeArea {
        start: 4.0,
        end: 6.0,
        top: 3.0,
        bottom: 5.0,
    };
    let track = SurfaceSize {
        width: 52.0,
        height: 28.0,
    };
    let thumb = SurfaceSize {
        width: 20.0,
        height: 20.0,
    };
    let layout = |direction, checked| {
        resolve_toggle_layout(ToggleLayoutInput {
            track_size: track,
            thumb_size: thumb,
            insets: &insets,
            layout_direction: direction,
            checked,
        })
        .unwrap()
    };
    let off = layout(LayoutDirection::Ltr, false);
    let on = layout(LayoutDirection::Ltr, true);
    assert!(!off.checked());
    assert!(on.checked());
    assert_eq!(off.layout_direction(), LayoutDirection::Ltr);
    assert_eq!(off.track_bounds(), on.track_bounds());
    assert_eq!(off.thumb_bounds(), off.off_thumb_bounds());
    assert_eq!(on.thumb_bounds(), on.on_thumb_bounds());
    let rtl = layout(LayoutDirection::Rtl, false);
    assert_eq!(
        rtl.off_thumb_bounds().x + off.off_thumb_bounds().x + thumb.width,
        track.width
    );
    assert_eq!(
        rtl.on_thumb_bounds().x + off.on_thumb_bounds().x + thumb.width,
        track.width
    );
    assert_eq!(rtl.thumb_bounds().y, off.thumb_bounds().y);
    let target = resolve_hit_region(HitRegionInput {
        environment: &environment,
        visual_bounds: off.track_bounds(),
        available_bounds: PhysicalBounds {
            x: -100.0,
            y: -100.0,
            width: 300.0,
            height: 300.0,
        },
        component_minimum: track,
        occupied_regions: &[],
    })
    .unwrap();
    for part in [
        off.track_bounds(),
        off.off_thumb_bounds(),
        off.on_thumb_bounds(),
        rtl.off_thumb_bounds(),
        rtl.on_thumb_bounds(),
    ] {
        assert!(target.contains_bounds(part).unwrap());
    }
}

#[test]
fn typed_layout_rejects_nonfinite_geometry_and_unrepresentable_thumb() {
    use resina_environment::{LayoutDirection, SafeArea};
    use resina_model::SurfaceSize;
    use resina_resolver::{ToggleLayoutInput, resolve_toggle_layout};
    let mut insets = SafeArea {
        start: 4.0,
        end: 6.0,
        top: 3.0,
        bottom: 5.0,
    };
    for value in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        assert!(
            resolve_toggle_layout(ToggleLayoutInput {
                track_size: SurfaceSize {
                    width: value,
                    height: 28.0
                },
                thumb_size: SurfaceSize {
                    width: 20.0,
                    height: 20.0
                },
                insets: &insets,
                layout_direction: LayoutDirection::Ltr,
                checked: false
            })
            .is_err()
        );
    }
    insets.end = f64::NAN;
    assert!(
        resolve_toggle_layout(ToggleLayoutInput {
            track_size: SurfaceSize {
                width: 52.0,
                height: 28.0
            },
            thumb_size: SurfaceSize {
                width: 20.0,
                height: 20.0
            },
            insets: &insets,
            layout_direction: LayoutDirection::Ltr,
            checked: true
        })
        .is_err()
    );
    insets = SafeArea {
        start: 0.0,
        end: 0.0,
        top: 0.0,
        bottom: 0.0,
    };
    let result = resolve_toggle_layout(ToggleLayoutInput {
        track_size: SurfaceSize {
            width: f64::MAX,
            height: 20.0,
        },
        thumb_size: SurfaceSize {
            width: f64::MIN_POSITIVE,
            height: 20.0,
        },
        insets: &insets,
        layout_direction: LayoutDirection::Ltr,
        checked: false,
    });
    assert!(result.unwrap_err().to_string().contains("representable"));
}
