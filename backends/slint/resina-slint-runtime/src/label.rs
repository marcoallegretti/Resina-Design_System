use crate::MAX_COORDINATE_ERROR;
use resina_model::SurfaceSize;
use resina_resolver::LabelMeasureInput;
use skrifa::{FontRef, MetadataProvider, instance::Size};
use slint::{SharedString, Window, fontique_011::fontique};
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum LabelMeasureError {
    BlankText,
    UnknownFamily(String),
    FontUnavailable(String),
    FontMetrics,
    FontWeight,
    Extent,
    Wrapping { offered: f32, minimum: f32 },
    Precision(&'static str),
}

impl fmt::Display for LabelMeasureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankText => f.write_str("command label must not be blank"),
            Self::UnknownFamily(name) => write!(f, "Slint font family {name:?} is not registered"),
            Self::FontUnavailable(name) => write!(f, "Slint font family {name:?} cannot be loaded"),
            Self::FontMetrics => {
                f.write_str("selected Slint font has invalid natural line metrics")
            }
            Self::FontWeight => f.write_str("Slint font weight must be an integer in 1..=1000"),
            Self::Extent => f.write_str("Slint complete label extent is empty, nonfinite or exceeds the wrapping constraint"),
            Self::Wrapping { offered, minimum } => write!(f, "Slint label wrap width {offered} is smaller than the measured word minimum {minimum}"),
            Self::Precision(field) => write!(f, "Slint label {field} exceeds coordinate precision"),
        }
    }
}

impl std::error::Error for LabelMeasureError {}

/// Input for the bundled resina-label-measure.slint component on the target UI thread.
/// Font size, letter spacing and wrapping width are logical pixels, already text-scaled.
/// Weight and line-height factor are unitless; device scale is the actual window scale.
#[derive(Debug, Clone)]
pub struct NativeLabelMeasure {
    pub text: SharedString,
    pub font_family: SharedString,
    pub font_size: f32,
    pub font_weight: i32,
    pub letter_spacing: f32,
    pub line_height_factor: f32,
    /// Logical wrapping constraint, rounded inward. None requests complete natural measurement.
    pub maximum_width: Option<f32>,
    pub device_scale: f32,
}

impl NativeLabelMeasure {
    /// Validate the exact published request's three logical-pixel readouts.
    pub fn complete(
        &self,
        width: f32,
        height: f32,
        minimum_required_width: f32,
    ) -> Result<SurfaceSize, LabelMeasureError> {
        if [width, height, minimum_required_width]
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(LabelMeasureError::Extent);
        }
        if let Some(offered) = self.maximum_width {
            if !offered.is_finite() || offered <= 0.0 {
                return Err(LabelMeasureError::Extent);
            }
            if minimum_required_width > offered {
                return Err(LabelMeasureError::Wrapping {
                    offered,
                    minimum: minimum_required_width,
                });
            }
            if width > offered {
                return Err(LabelMeasureError::Extent);
            }
        }
        if minimum_required_width > width {
            return Err(LabelMeasureError::Extent);
        }
        Ok(SurfaceSize {
            width: f64::from(width),
            height: f64::from(height),
        })
    }
}

fn scalar(value: f64, field: &'static str) -> Result<f32, LabelMeasureError> {
    let native = value as f32;
    if !native.is_finite() || (value - f64::from(native)).abs() > MAX_COORDINATE_ERROR {
        Err(LabelMeasureError::Precision(field))
    } else {
        Ok(native)
    }
}

/// UI-scoped font query cache for the bundled native measurement component.
pub struct LabelMeasurer<'window> {
    window: &'window Window,
    collection: fontique::Collection,
    source_cache: fontique::SourceCache,
}

impl<'window> LabelMeasurer<'window> {
    /// Register fonts before constructing the measurer.
    pub fn new(window: &'window Window) -> Self {
        Self {
            window,
            collection: slint::fontique_011::shared_collection(),
            source_cache: fontique::SourceCache::default(),
        }
    }

    /// Translate typography using the selected native face and actual window scale.
    pub fn prepare(
        &mut self,
        font_family: &str,
        input: LabelMeasureInput<'_>,
    ) -> Result<NativeLabelMeasure, LabelMeasureError> {
        if input.text.trim().is_empty() {
            return Err(LabelMeasureError::BlankText);
        }
        let typography = input.typography;
        let font_size = scalar(typography.font_size(), "font size")?;
        if font_size < 0.01 {
            return Err(LabelMeasureError::Precision("font size"));
        }
        let weight = typography.font_weight();
        if weight.fract() != 0.0 || !(1.0..=1000.0).contains(&weight) {
            return Err(LabelMeasureError::FontWeight);
        }
        let letter_spacing = scalar(typography.letter_spacing(), "letter spacing")?;
        let maximum_width = input
            .maximum_width
            .map(|width| {
                let mut native = scalar(width, "wrap width")?;
                if f64::from(native) > width {
                    native = native.next_down();
                }
                if native <= 0.0 || (width - f64::from(native)).abs() > MAX_COORDINATE_ERROR {
                    return Err(LabelMeasureError::Precision("wrap width"));
                }
                Ok(native)
            })
            .transpose()?;
        let device_scale = self.window.scale_factor();
        if !device_scale.is_finite() || device_scale <= 0.0 {
            return Err(LabelMeasureError::Precision("device scale"));
        }
        let family = self
            .collection
            .family_id(font_family)
            .ok_or_else(|| LabelMeasureError::UnknownFamily(font_family.to_owned()))?;
        let mut query = self.collection.query(&mut self.source_cache);
        query.set_families([fontique::QueryFamily::Id(family)]);
        query.set_attributes(fontique::Attributes {
            weight: fontique::FontWeight::new(weight as f32),
            style: fontique::FontStyle::Normal,
            ..Default::default()
        });
        let mut matched = None;
        query.matches_with(|font| {
            matched = Some(font.clone());
            fontique::QueryStatus::Stop
        });
        let font =
            matched.ok_or_else(|| LabelMeasureError::FontUnavailable(font_family.to_owned()))?;
        let face = FontRef::from_index(font.blob.data(), font.index)
            .map_err(|_| LabelMeasureError::FontMetrics)?;
        let location = face.axes().location(font.synthesis.variation_settings());
        let metrics = face.metrics(Size::unscaled(), &location);
        let natural_ratio =
            (metrics.ascent - metrics.descent + metrics.leading) / f32::from(metrics.units_per_em);
        if !natural_ratio.is_finite() || natural_ratio <= 0.0 {
            return Err(LabelMeasureError::FontMetrics);
        }
        let line_height_factor = scalar(
            typography.line_height() / f64::from(natural_ratio),
            "line height factor",
        )?;
        let desired_height = typography.font_size() * typography.line_height();
        let native_height = font_size * (natural_ratio * line_height_factor);
        if line_height_factor <= 0.0
            || native_height <= 0.0
            || !desired_height.is_finite()
            || !native_height.is_finite()
            || (desired_height - f64::from(native_height)).abs() > MAX_COORDINATE_ERROR
        {
            return Err(LabelMeasureError::Precision("resolved line height"));
        }
        Ok(NativeLabelMeasure {
            text: input.text.into(),
            font_family: font_family.into(),
            font_size,
            font_weight: weight as i32,
            letter_spacing,
            line_height_factor,
            maximum_width,
            device_scale,
        })
    }
}
