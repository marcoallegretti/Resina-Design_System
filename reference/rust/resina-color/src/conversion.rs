use std::fmt;

const SRGB_TO_XYZ: [[f64; 3]; 3] = [
    [506752.0 / 1228815.0, 87881.0 / 245763.0, 12673.0 / 70218.0],
    [87098.0 / 409605.0, 175762.0 / 245763.0, 12673.0 / 175545.0],
    [7918.0 / 409605.0, 87881.0 / 737289.0, 1001167.0 / 1053270.0],
];
const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0],
    [
        -851781.0 / 878810.0,
        1648619.0 / 878810.0,
        36519.0 / 878810.0,
    ],
    [705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0],
];
const D50_TO_D65: [[f64; 3]; 3] = [
    [0.955473421488075, -0.02309845494876471, 0.06325924320057072],
    [
        -0.0283697093338637,
        1.0099953980813041,
        0.021041441191917323,
    ],
    [
        0.012314014864481998,
        -0.020507649298898964,
        1.330365926242124,
    ],
];
const DISPLAY_P3_TO_XYZ: [[f64; 3]; 3] = [
    [
        608311.0 / 1250200.0,
        189793.0 / 714400.0,
        198249.0 / 1000160.0,
    ],
    [
        35783.0 / 156275.0,
        247089.0 / 357200.0,
        198249.0 / 2500400.0,
    ],
    [0.0, 32229.0 / 714400.0, 5220557.0 / 5000800.0],
];
const A98_RGB_TO_XYZ: [[f64; 3]; 3] = [
    [
        573536.0 / 994567.0,
        263643.0 / 1420810.0,
        187206.0 / 994567.0,
    ],
    [
        591459.0 / 1989134.0,
        6239551.0 / 9945670.0,
        374412.0 / 4972835.0,
    ],
    [
        53769.0 / 1989134.0,
        351524.0 / 4972835.0,
        4929758.0 / 4972835.0,
    ],
];
const PROPHOTO_RGB_TO_XYZ_D50: [[f64; 3]; 3] = [
    [0.7977666449006423, 0.13518129740053308, 0.0313477341283922],
    [0.2880748288194013, 0.711835234241873, 0.00008993693872564],
    [0.0, 0.0, 0.8251046025104602],
];
const REC2020_TO_XYZ: [[f64; 3]; 3] = [
    [
        63426534.0 / 99577255.0,
        20160776.0 / 139408157.0,
        47086771.0 / 278816314.0,
    ],
    [
        26158966.0 / 99577255.0,
        472592308.0 / 697040785.0,
        8267143.0 / 139408157.0,
    ],
    [0.0, 19567812.0 / 697040785.0, 295819943.0 / 278816314.0],
];
const XYZ_TO_LMS: [[f64; 3]; 3] = [
    [
        0.819_022_437_996_703,
        0.3619062600528904,
        -0.1288737815209879,
    ],
    [0.0329836539323885, 0.9292868615863434, 0.0361446663506424],
    [0.0481771893596242, 0.2642395317527308, 0.6335478284694309],
];
const LMS_TO_OKLAB: [[f64; 3]; 3] = [
    [
        0.210_454_268_309_314,
        0.7936177747023054,
        -0.0040720430116193,
    ],
    [
        1.9779985324311684,
        -2.428_592_242_048_58,
        0.450_593_709_617_411,
    ],
    [0.0259040424655478, 0.7827717124575296, -0.8086757549230774],
];
const OKLAB_TO_LMS: [[f64; 3]; 3] = [
    [1.0, 0.3963377773761749, 0.2158037573099136],
    [1.0, -0.1055613458156586, -0.0638541728258133],
    [1.0, -0.0894841775298119, -1.2914855480194092],
];
const LMS_TO_XYZ: [[f64; 3]; 3] = [
    [1.2268798758459243, -0.5578149944602171, 0.2813910456659647],
    [
        -0.0405757452148008,
        1.112_286_803_280_317,
        -0.0717110580655164,
    ],
    [-0.0763729366746601, -0.4214933324022432, 1.5869240198367816],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorConversionError {
    NonFiniteSrgbComponent {
        index: usize,
    },
    OutOfRangeSrgbComponent {
        index: usize,
    },
    NonFiniteOklabComponent {
        index: usize,
    },
    NonFiniteOklchComponent {
        index: usize,
    },
    OutOfRangeOklchComponent {
        index: usize,
    },
    NonFiniteColorComponent {
        color_space: &'static str,
        index: usize,
    },
    OutOfRangeColorComponent {
        color_space: &'static str,
        index: usize,
    },
    NonFiniteResult {
        index: usize,
    },
}

impl fmt::Display for ColorConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteSrgbComponent { index } => {
                write!(formatter, "nonfinite sRGB component at index {index}")
            }
            Self::OutOfRangeSrgbComponent { index } => {
                write!(formatter, "sRGB component outside [0, 1] at index {index}")
            }
            Self::NonFiniteOklabComponent { index } => {
                write!(formatter, "nonfinite Oklab component at index {index}")
            }
            Self::NonFiniteOklchComponent { index } => {
                write!(formatter, "nonfinite Oklch component at index {index}")
            }
            Self::OutOfRangeOklchComponent { index } => {
                write!(
                    formatter,
                    "Oklch component outside its range at index {index}"
                )
            }
            Self::NonFiniteColorComponent { color_space, index } => {
                write!(
                    formatter,
                    "nonfinite {color_space} component at index {index}"
                )
            }
            Self::OutOfRangeColorComponent { color_space, index } => {
                write!(
                    formatter,
                    "{color_space} component outside its range at index {index}"
                )
            }
            Self::NonFiniteResult { index } => {
                write!(formatter, "nonfinite conversion result at index {index}")
            }
        }
    }
}

impl std::error::Error for ColorConversionError {}

#[derive(Clone, Copy)]
struct ComponentRange {
    minimum: f64,
    maximum: f64,
    upper_exclusive: bool,
}

const UNIT_COMPONENT: ComponentRange = ComponentRange {
    minimum: 0.0,
    maximum: 1.0,
    upper_exclusive: false,
};
const HUE_COMPONENT: ComponentRange = ComponentRange {
    minimum: 0.0,
    maximum: 360.0,
    upper_exclusive: true,
};
const PERCENT_COMPONENT: ComponentRange = ComponentRange {
    minimum: 0.0,
    maximum: 100.0,
    upper_exclusive: false,
};
const UNBOUNDED_COMPONENT: ComponentRange = ComponentRange {
    minimum: f64::NEG_INFINITY,
    maximum: f64::INFINITY,
    upper_exclusive: false,
};
const NONNEGATIVE_COMPONENT: ComponentRange = ComponentRange {
    minimum: 0.0,
    maximum: f64::INFINITY,
    upper_exclusive: false,
};

enum WhitePoint {
    D50,
    D65,
}

pub fn srgb_to_oklab(srgb: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    for (index, component) in srgb.iter().enumerate() {
        if !component.is_finite() {
            return Err(ColorConversionError::NonFiniteSrgbComponent { index });
        }
        if !(0.0..=1.0).contains(component) {
            return Err(ColorConversionError::OutOfRangeSrgbComponent { index });
        }
    }
    let linear = srgb.map(linearize_srgb_component);
    let xyz = transform(&SRGB_TO_XYZ, linear);
    let lms = transform(&XYZ_TO_LMS, xyz).map(f64::cbrt);
    finite_result(transform(&LMS_TO_OKLAB, lms))
}

pub fn oklab_to_extended_srgb(oklab: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    for (index, component) in oklab.iter().enumerate() {
        if !component.is_finite() {
            return Err(ColorConversionError::NonFiniteOklabComponent { index });
        }
    }
    let lms = transform(&OKLAB_TO_LMS, oklab).map(|component| component.powi(3));
    let xyz = transform(&LMS_TO_XYZ, lms);
    let linear = transform(&XYZ_TO_SRGB, xyz);
    finite_result(linear.map(encode_extended_srgb_component))
}

pub fn oklch_to_oklab(oklch: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    for (index, component) in oklch.iter().enumerate() {
        if !component.is_finite() {
            return Err(ColorConversionError::NonFiniteOklchComponent { index });
        }
    }
    if !(0.0..=1.0).contains(&oklch[0]) {
        return Err(ColorConversionError::OutOfRangeOklchComponent { index: 0 });
    }
    if oklch[1] < 0.0 {
        return Err(ColorConversionError::OutOfRangeOklchComponent { index: 1 });
    }
    if !(0.0..360.0).contains(&oklch[2]) {
        return Err(ColorConversionError::OutOfRangeOklchComponent { index: 2 });
    }
    let angle = oklch[2].to_radians();
    finite_result([oklch[0], oklch[1] * angle.cos(), oklch[1] * angle.sin()])
}

pub fn linear_srgb_to_srgb(linear: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    validate_components(
        linear,
        "srgb-linear",
        [UNIT_COMPONENT, UNIT_COMPONENT, UNIT_COMPONENT],
    )?;
    finite_result(linear.map(encode_extended_srgb_component))
}

pub fn xyz_d65_to_extended_srgb(xyz: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    validate_components(xyz, "xyz-d65", [UNIT_COMPONENT; 3])?;
    finite_result(transform(&XYZ_TO_SRGB, xyz).map(encode_extended_srgb_component))
}

pub fn xyz_d50_to_extended_srgb(xyz: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    validate_components(xyz, "xyz-d50", [UNIT_COMPONENT; 3])?;
    xyz_d50_unchecked_to_extended_srgb(xyz)
}

fn xyz_d50_unchecked_to_extended_srgb(xyz: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    let adapted = transform(&D50_TO_D65, xyz);
    finite_result(transform(&XYZ_TO_SRGB, adapted).map(encode_extended_srgb_component))
}

pub fn lab_to_extended_srgb(lab: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    validate_components(
        lab,
        "lab",
        [PERCENT_COMPONENT, UNBOUNDED_COMPONENT, UNBOUNDED_COMPONENT],
    )?;
    let [lightness, a, b] = lab;
    let kappa = 24389.0 / 27.0;
    let epsilon = 216.0 / 24389.0;
    let fy = (lightness + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let xyz = [
        if fx.powi(3) > epsilon {
            fx.powi(3)
        } else {
            (116.0 * fx - 16.0) / kappa
        },
        if lightness > kappa * epsilon {
            fy.powi(3)
        } else {
            lightness / kappa
        },
        if fz.powi(3) > epsilon {
            fz.powi(3)
        } else {
            (116.0 * fz - 16.0) / kappa
        },
    ];
    let white = [0.3457 / 0.3585, 1.0, (1.0 - 0.3457 - 0.3585) / 0.3585];
    xyz_d50_unchecked_to_extended_srgb([xyz[0] * white[0], xyz[1] * white[1], xyz[2] * white[2]])
}

pub fn lch_to_lab(lch: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    validate_components(
        lch,
        "lch",
        [PERCENT_COMPONENT, NONNEGATIVE_COMPONENT, HUE_COMPONENT],
    )?;
    let angle = lch[2].to_radians();
    finite_result([lch[0], lch[1] * angle.cos(), lch[1] * angle.sin()])
}

pub fn display_p3_to_extended_srgb(rgb: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    predefined_rgb_to_extended_srgb(
        rgb,
        "display-p3",
        &DISPLAY_P3_TO_XYZ,
        linearize_srgb_component,
        WhitePoint::D65,
    )
}

pub fn a98_rgb_to_extended_srgb(rgb: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    predefined_rgb_to_extended_srgb(
        rgb,
        "a98-rgb",
        &A98_RGB_TO_XYZ,
        |c| c.powf(563.0 / 256.0),
        WhitePoint::D65,
    )
}

pub fn prophoto_rgb_to_extended_srgb(rgb: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    predefined_rgb_to_extended_srgb(
        rgb,
        "prophoto-rgb",
        &PROPHOTO_RGB_TO_XYZ_D50,
        |c| {
            if c <= 16.0 / 512.0 {
                c / 16.0
            } else {
                c.powf(1.8)
            }
        },
        WhitePoint::D50,
    )
}

pub fn rec2020_to_extended_srgb(rgb: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    predefined_rgb_to_extended_srgb(
        rgb,
        "rec2020",
        &REC2020_TO_XYZ,
        |c| c.powf(2.4),
        WhitePoint::D65,
    )
}

fn predefined_rgb_to_extended_srgb(
    rgb: [f64; 3],
    color_space: &'static str,
    to_xyz: &[[f64; 3]; 3],
    linearize: fn(f64) -> f64,
    white_point: WhitePoint,
) -> Result<[f64; 3], ColorConversionError> {
    validate_components(rgb, color_space, [UNIT_COMPONENT; 3])?;
    let xyz = transform(to_xyz, rgb.map(linearize));
    let xyz = match white_point {
        WhitePoint::D50 => transform(&D50_TO_D65, xyz),
        WhitePoint::D65 => xyz,
    };
    finite_result(transform(&XYZ_TO_SRGB, xyz).map(encode_extended_srgb_component))
}

pub fn hsl_to_srgb(hsl: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    validate_components(
        hsl,
        "hsl",
        [HUE_COMPONENT, PERCENT_COMPONENT, PERCENT_COMPONENT],
    )?;
    let hue = hsl[0];
    let saturation = hsl[1] / 100.0;
    let lightness = hsl[2] / 100.0;
    let channel = |offset: f64| {
        let k = (offset + hue / 30.0) % 12.0;
        let amplitude = saturation * lightness.min(1.0 - lightness);
        lightness - amplitude * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    finite_result([channel(0.0), channel(8.0), channel(4.0)])
}

pub fn hwb_to_srgb(hwb: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    validate_components(
        hwb,
        "hwb",
        [HUE_COMPONENT, PERCENT_COMPONENT, PERCENT_COMPONENT],
    )?;
    let white = hwb[1] / 100.0;
    let black = hwb[2] / 100.0;
    if white + black >= 1.0 {
        let gray = white / (white + black);
        return Ok([gray; 3]);
    }
    let pure = hsl_to_srgb([hwb[0], 100.0, 50.0])?;
    finite_result(pure.map(|channel| channel * (1.0 - white - black) + white))
}

fn validate_components(
    components: [f64; 3],
    color_space: &'static str,
    ranges: [ComponentRange; 3],
) -> Result<(), ColorConversionError> {
    for (index, component) in components.into_iter().enumerate() {
        if !component.is_finite() {
            return Err(ColorConversionError::NonFiniteColorComponent { color_space, index });
        }
        let range = &ranges[index];
        if component < range.minimum
            || if range.upper_exclusive {
                component >= range.maximum
            } else {
                component > range.maximum
            }
        {
            return Err(ColorConversionError::OutOfRangeColorComponent { color_space, index });
        }
    }
    Ok(())
}

pub(crate) fn linearize_srgb_component(component: f64) -> f64 {
    if component <= 0.04045 {
        component / 12.92
    } else {
        ((component + 0.055) / 1.055).powf(2.4)
    }
}

fn encode_extended_srgb_component(component: f64) -> f64 {
    let magnitude = component.abs();
    if magnitude > 0.0031308 {
        component.signum() * (1.055 * magnitude.powf(1.0 / 2.4) - 0.055)
    } else {
        12.92 * component
    }
}

fn transform(matrix: &[[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2])
}

fn finite_result(components: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    for (index, component) in components.iter().enumerate() {
        if !component.is_finite() {
            return Err(ColorConversionError::NonFiniteResult { index });
        }
    }
    Ok(components)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn components(value: &Value) -> [f64; 3] {
        let values = value.as_array().unwrap();
        [
            values[0].as_f64().unwrap(),
            values[1].as_f64().unwrap(),
            values[2].as_f64().unwrap(),
        ]
    }

    fn assert_close(actual: [f64; 3], expected: [f64; 3], name: &Value) {
        for index in 0..3 {
            assert!(
                (actual[index] - expected[index]).abs() < 1e-12,
                "{name}: channel {index}: {} != {}",
                actual[index],
                expected[index]
            );
        }
    }

    #[test]
    fn oklab_conversion_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/oklab-conversion-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            if let Some(srgb) = vector.get("srgb") {
                let srgb = components(srgb);
                if vector.get("error").is_some() {
                    assert_eq!(vector["error"], "OutOfRangeSrgbComponent");
                    assert_eq!(
                        srgb_to_oklab(srgb),
                        Err(ColorConversionError::OutOfRangeSrgbComponent {
                            index: vector["index"].as_u64().unwrap() as usize
                        }),
                        "{}",
                        vector["name"]
                    );
                } else {
                    let expected = components(&vector["oklab"]);
                    assert_close(srgb_to_oklab(srgb).unwrap(), expected, &vector["name"]);
                    assert_close(
                        oklab_to_extended_srgb(expected).unwrap(),
                        srgb,
                        &vector["name"],
                    );
                }
            }
            if let Some(expected) = vector.get("extendedSrgb") {
                let actual = oklab_to_extended_srgb(components(&vector["oklab"])).unwrap();
                assert_close(actual, components(expected), &vector["name"]);
            }
        }
    }

    #[test]
    fn oklch_conversion_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/oklch-conversion-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let actual = oklch_to_oklab(components(&vector["oklch"]));
            if vector.get("error").is_some() {
                assert_eq!(vector["error"], "OutOfRangeOklchComponent");
                assert_eq!(
                    actual,
                    Err(ColorConversionError::OutOfRangeOklchComponent {
                        index: vector["index"].as_u64().unwrap() as usize
                    }),
                    "{}",
                    vector["name"]
                );
            } else {
                assert_close(
                    actual.unwrap(),
                    components(&vector["oklab"]),
                    &vector["name"],
                );
            }
        }
    }

    #[test]
    fn xyz_conversion_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/xyz-conversion-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let xyz = components(&vector["xyz"]);
            let convert = match vector["space"].as_str().unwrap() {
                "xyz-d65" => xyz_d65_to_extended_srgb,
                "xyz-d50" => xyz_d50_to_extended_srgb,
                other => panic!("unexpected color space: {other}"),
            };
            let result = convert(xyz);
            if vector.get("error").is_some() {
                assert_eq!(vector["error"], "OutOfRangeColorComponent");
                assert_eq!(
                    result,
                    Err(ColorConversionError::OutOfRangeColorComponent {
                        color_space: if vector["space"] == "xyz-d65" {
                            "xyz-d65"
                        } else {
                            "xyz-d50"
                        },
                        index: vector["index"].as_u64().unwrap() as usize,
                    }),
                    "{}",
                    vector["name"]
                );
            } else {
                assert_close(
                    result.unwrap(),
                    components(&vector["srgb"]),
                    &vector["name"],
                );
            }
        }
    }

    #[test]
    fn rgb_conversion_conformance_vectors() {
        type Converter = fn([f64; 3]) -> Result<[f64; 3], ColorConversionError>;
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/rgb-conversion-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let rgb = components(&vector["rgb"]);
            let (convert, color_space): (Converter, &'static str) =
                match vector["space"].as_str().unwrap() {
                    "display-p3" => (display_p3_to_extended_srgb, "display-p3"),
                    "a98-rgb" => (a98_rgb_to_extended_srgb, "a98-rgb"),
                    "prophoto-rgb" => (prophoto_rgb_to_extended_srgb, "prophoto-rgb"),
                    "rec2020" => (rec2020_to_extended_srgb, "rec2020"),
                    other => panic!("unexpected color space: {other}"),
                };
            let result = convert(rgb);
            if vector.get("error").is_some() {
                assert_eq!(vector["error"], "OutOfRangeColorComponent");
                assert_eq!(
                    result,
                    Err(ColorConversionError::OutOfRangeColorComponent {
                        color_space,
                        index: vector["index"].as_u64().unwrap() as usize,
                    }),
                    "{}",
                    vector["name"]
                );
            } else {
                assert_close(
                    result.unwrap(),
                    components(&vector["extendedSrgb"]),
                    &vector["name"],
                );
            }
        }
    }

    #[test]
    fn lab_conversion_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/lab-conversion-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let space = vector["space"].as_str().unwrap();
            let source = components(&vector["components"]);
            let lab = if space == "lch" {
                let result = lch_to_lab(source);
                if vector.get("error").is_some() {
                    assert_eq!(
                        result,
                        Err(ColorConversionError::OutOfRangeColorComponent {
                            color_space: "lch",
                            index: vector["index"].as_u64().unwrap() as usize,
                        }),
                        "{}",
                        vector["name"]
                    );
                    continue;
                }
                let lab = result.unwrap();
                assert_close(lab, components(&vector["lab"]), &vector["name"]);
                lab
            } else {
                source
            };
            let result = lab_to_extended_srgb(lab);
            if vector.get("error").is_some() {
                assert_eq!(
                    result,
                    Err(ColorConversionError::OutOfRangeColorComponent {
                        color_space: "lab",
                        index: vector["index"].as_u64().unwrap() as usize,
                    }),
                    "{}",
                    vector["name"]
                );
            } else {
                assert_close(
                    result.unwrap(),
                    components(&vector["extendedSrgb"]),
                    &vector["name"],
                );
            }
        }
    }

    #[test]
    fn direct_srgb_conversion_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/srgb-direct-conversion-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let input = components(&vector["input"]);
            let space = vector["space"].as_str().unwrap();
            let actual = match space {
                "srgb-linear" => linear_srgb_to_srgb(input),
                "hsl" => hsl_to_srgb(input),
                "hwb" => hwb_to_srgb(input),
                _ => panic!("unknown vector space: {space}"),
            };
            if vector.get("error").is_some() {
                assert_eq!(vector["error"], "OutOfRangeColorComponent");
                let Err(ColorConversionError::OutOfRangeColorComponent { color_space, index }) =
                    actual
                else {
                    panic!("{}: {actual:?}", vector["name"]);
                };
                assert_eq!(color_space, space);
                assert_eq!(index, vector["index"].as_u64().unwrap() as usize);
            } else {
                assert_close(
                    actual.unwrap(),
                    components(&vector["expected"]),
                    &vector["name"],
                );
            }
        }
    }

    #[test]
    fn nonfinite_inputs_and_results_fail() {
        assert_eq!(
            srgb_to_oklab([f64::NAN, 0.0, 0.0]),
            Err(ColorConversionError::NonFiniteSrgbComponent { index: 0 })
        );
        assert_eq!(
            srgb_to_oklab([0.0, f64::INFINITY, 0.0]),
            Err(ColorConversionError::NonFiniteSrgbComponent { index: 1 })
        );
        assert_eq!(
            oklab_to_extended_srgb([0.0, 0.0, f64::NEG_INFINITY]),
            Err(ColorConversionError::NonFiniteOklabComponent { index: 2 })
        );
        assert!(matches!(
            oklab_to_extended_srgb([f64::MAX, 0.0, 0.0]),
            Err(ColorConversionError::NonFiniteResult { .. })
        ));
        assert_eq!(
            oklch_to_oklab([0.5, f64::NAN, 0.0]),
            Err(ColorConversionError::NonFiniteOklchComponent { index: 1 })
        );
        assert_eq!(
            hsl_to_srgb([0.0, f64::INFINITY, 50.0]),
            Err(ColorConversionError::NonFiniteColorComponent {
                color_space: "hsl",
                index: 1
            })
        );
        assert_eq!(
            xyz_d65_to_extended_srgb([0.0, f64::NAN, 0.0]),
            Err(ColorConversionError::NonFiniteColorComponent {
                color_space: "xyz-d65",
                index: 1
            })
        );
        assert_eq!(
            xyz_d50_to_extended_srgb([0.0, 0.0, f64::INFINITY]),
            Err(ColorConversionError::NonFiniteColorComponent {
                color_space: "xyz-d50",
                index: 2
            })
        );
        assert_eq!(
            prophoto_rgb_to_extended_srgb([f64::NAN, 0.0, 0.0]),
            Err(ColorConversionError::NonFiniteColorComponent {
                color_space: "prophoto-rgb",
                index: 0
            })
        );
        assert_eq!(
            lab_to_extended_srgb([50.0, f64::INFINITY, 0.0]),
            Err(ColorConversionError::NonFiniteColorComponent {
                color_space: "lab",
                index: 1,
            })
        );
        assert_eq!(
            lch_to_lab([50.0, 0.0, f64::NAN]),
            Err(ColorConversionError::NonFiniteColorComponent {
                color_space: "lch",
                index: 2,
            })
        );
        assert!(matches!(
            lab_to_extended_srgb([50.0, f64::MAX, 0.0]),
            Err(ColorConversionError::NonFiniteResult { .. })
        ));
    }
}
