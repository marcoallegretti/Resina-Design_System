use resina_color::{ColorConversionError, linear_srgb_to_srgb, srgb_to_linear_srgb};
use serde_json::Value;

fn components(value: &Value) -> [f64; 3] {
    assert_eq!(value.as_array().unwrap().len(), 3);
    std::array::from_fn(|index| value[index].as_f64().unwrap())
}

#[test]
fn public_decoding_vectors() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/color/srgb-decoding-vectors.json"
    ))
    .unwrap();
    for vector in vectors {
        assert_ne!(
            vector.get("error").is_some(),
            vector.get("linearSrgb").is_some()
        );
        let result = srgb_to_linear_srgb(components(&vector["srgb"]));
        if let Some(error) = vector["error"].as_str() {
            assert_eq!(error, "OutOfRangeColorComponent");
            let index = vector["index"].as_u64().unwrap();
            assert_eq!(
                result,
                Err(ColorConversionError::OutOfRangeColorComponent {
                    color_space: "srgb",
                    index: usize::try_from(index).unwrap()
                })
            );
        } else {
            for (actual, expected) in result
                .unwrap()
                .into_iter()
                .zip(components(&vector["linearSrgb"]))
            {
                assert!((actual - expected).abs() <= 1e-12, "{}", vector["name"]);
            }
        }
    }
}

#[test]
fn every_channel_rejects_nonfinite_and_out_of_range_values() {
    for index in 0..3 {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
            let mut input = [0.5; 3];
            input[index] = value;
            let expected = if value.is_finite() {
                ColorConversionError::OutOfRangeColorComponent {
                    color_space: "srgb",
                    index,
                }
            } else {
                ColorConversionError::NonFiniteColorComponent {
                    color_space: "srgb",
                    index,
                }
            };
            assert_eq!(srgb_to_linear_srgb(input), Err(expected));
        }
    }
}

#[test]
fn eight_bit_values_round_trip_without_channel_coupling() {
    for value in 0..=255 {
        let input = [f64::from(value) / 255.0, 0.01, 0.9];
        let result = linear_srgb_to_srgb(srgb_to_linear_srgb(input).unwrap()).unwrap();
        for (actual, expected) in result.into_iter().zip(input) {
            assert!((actual - expected).abs() <= 1e-12);
        }
    }
    let boundary = linear_srgb_to_srgb(srgb_to_linear_srgb([0.04045; 3]).unwrap()).unwrap();
    assert!((boundary[0] - 0.04045).abs() <= 3e-8);
}
