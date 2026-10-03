use crate::{
    CompiledTheme, ElevationDepthResolutionError, ExtrudedContourError, ExtrudedContourResult,
    FocusIndicatorError, FocusIndicatorResult, KeyLightError, PlacedContour, ShapeFallbackError,
    SrgbFallback, ThemeResolutionError, resolve_elevation_depth, resolve_extruded_contour,
    resolve_focus_indicator, resolve_key_light, resolve_shape_fallback,
    srgb_input::{SrgbInput, SrgbInputError},
    theme_request::compile_theme_request_document,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    CornerRadius, ElevationDepthAssignments, KeyLight, LogicalCornerRadii, OpticalTreatment,
    PhysicalVector, ShapeFallbackAssignments, SurfaceIntent, SurfaceSize,
};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

pub struct FocusIrInput<'a> {
    pub surface: &'a SurfaceIntent,
    pub size: SurfaceSize,
    pub shape_assignments: &'a ShapeFallbackAssignments,
    pub depth_assignments: &'a ElevationDepthAssignments,
    pub key_light: &'a KeyLight,
    pub surrounding_color: &'a SrgbFallback,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FocusIrRequest {
    schema_version: String,
    theme: Value,
    surface: SurfaceIntent,
    size: SurfaceSize,
    shape_assignments: ShapeFallbackAssignments,
    depth_assignments: ElevationDepthAssignments,
    key_light: KeyLight,
    surrounding_color: SrgbInput,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusRingGeometry {
    silhouette: ExtrudedContourResult,
    inner: PlacedContour,
    outer: PlacedContour,
}
impl FocusRingGeometry {
    pub fn silhouette(&self) -> &ExtrudedContourResult {
        &self.silhouette
    }
    pub fn inner(&self) -> &PlacedContour {
        &self.inner
    }
    pub fn outer(&self) -> &PlacedContour {
        &self.outer
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusIndicatorIr {
    schema_version: &'static str,
    indicator: FocusIndicatorResult,
    geometry: FocusRingGeometry,
}
impl FocusIndicatorIr {
    pub fn indicator(&self) -> &FocusIndicatorResult {
        &self.indicator
    }
    pub fn geometry(&self) -> &FocusRingGeometry {
        &self.geometry
    }
}

#[derive(Debug)]
pub enum FocusIrError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Theme(ThemeResolutionError),
    Indicator(FocusIndicatorError),
    Shape(ShapeFallbackError),
    Depth(Vec<ElevationDepthResolutionError>),
    Light(KeyLightError),
    Contour(ExtrudedContourError),
    EmptySurface,
    ActiveTreatment,
    OutsetOverflow,
    UnrepresentableRing,
}
impl fmt::Display for FocusIrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "focus IR parse failed: {e}"),
            Self::Request(e) => write!(f, "invalid focus IR request: {e}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Theme(e) => write!(f, "focus IR theme: {e}"),
            Self::Indicator(e) => write!(f, "focus IR indicator: {e}"),
            Self::Shape(e) => write!(f, "focus IR shape: {e}"),
            Self::Depth(errors) => {
                f.write_str("focus IR depth binding failed")?;
                for e in errors {
                    write!(f, "\n  {e}")?;
                }
                Ok(())
            }
            Self::Light(e) => write!(f, "focus IR light: {e}"),
            Self::Contour(e) => write!(f, "focus IR contour: {e}"),
            Self::EmptySurface => {
                f.write_str("focus IR requires positive surface width and height")
            }
            Self::OutsetOverflow => f.write_str("focus ring outset coordinates must be finite"),
            Self::ActiveTreatment => {
                f.write_str("focus IR requires effective current treatment none")
            }
            Self::UnrepresentableRing => {
                f.write_str("focus ring cannot be represented at these numeric bounds")
            }
        }
    }
}
impl std::error::Error for FocusIrError {}

pub fn resolve_focus_ir_source(source: &str) -> Result<FocusIndicatorIr, FocusIrError> {
    let document = parse_token_document(source).map_err(FocusIrError::Parse)?;
    let request: FocusIrRequest =
        serde_json::from_value(document).map_err(FocusIrError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(FocusIrError::UnsupportedVersion);
    }
    let (theme, environment) =
        compile_theme_request_document(request.theme).map_err(FocusIrError::Theme)?;
    let surrounding = request.surrounding_color.into_fallback().map_err(|e| {
        FocusIrError::Indicator(match e {
            SrgbInputError::InvalidColorSpace => FocusIndicatorError::InvalidColorSpace,
            SrgbInputError::Color(e) => FocusIndicatorError::Color(e),
        })
    })?;
    resolve_focus_ir(
        &theme,
        &environment,
        FocusIrInput {
            surface: &request.surface,
            size: request.size,
            shape_assignments: &request.shape_assignments,
            depth_assignments: &request.depth_assignments,
            key_light: &request.key_light,
            surrounding_color: &surrounding,
        },
    )
}

pub fn resolve_focus_ir(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    input: FocusIrInput<'_>,
) -> Result<FocusIndicatorIr, FocusIrError> {
    let resolution = theme
        .resolve(environment)
        .map_err(|e| FocusIrError::Theme(ThemeResolutionError::Resolve(e)))?;
    resolve_focus_ir_with_snapshot(theme, environment, &resolution, input)
}

pub(crate) fn resolve_focus_ir_with_snapshot(
    theme: &CompiledTheme,
    environment: &EnvironmentSnapshot,
    resolution: &crate::HeadlessResolution,
    input: FocusIrInput<'_>,
) -> Result<FocusIndicatorIr, FocusIrError> {
    let indicator = resolve_focus_indicator(input.surface, resolution, input.surrounding_color)
        .map_err(FocusIrError::Indicator)?;
    if indicator.binding().treatment_stack().treatments().last() != Some(&OpticalTreatment::None) {
        return Err(FocusIrError::ActiveTreatment);
    }
    let radii = resolve_shape_fallback(
        indicator.binding().form().shape(),
        input.size,
        input.shape_assignments,
        theme.tokens(),
    )
    .map_err(FocusIrError::Shape)?;
    if input.size.width == 0.0 || input.size.height == 0.0 {
        return Err(FocusIrError::EmptySurface);
    }
    let depths = resolve_elevation_depth(input.depth_assignments, theme.tokens())
        .map_err(FocusIrError::Depth)?;
    let depth = depths[&indicator.binding().form().elevation()]["value"]
        .as_f64()
        .expect("validated depth is numeric");
    let lighting = resolve_key_light(input.key_light, depth, &[]).map_err(FocusIrError::Light)?;
    let contour = |size, radii| {
        resolve_extruded_contour(
            size,
            radii,
            environment.layout_direction(),
            lighting.side_offset(),
        )
        .map_err(FocusIrError::Contour)
    };
    let silhouette = contour(input.size, radii)?;
    let outset = |extent: f64| -> Result<PlacedContour, FocusIrError> {
        let size = SurfaceSize {
            width: input.size.width + 2.0 * extent,
            height: input.size.height + 2.0 * extent,
        };
        let grow = |corner: CornerRadius| CornerRadius {
            x: corner.x + extent,
            y: corner.y + extent,
        };
        let grown = LogicalCornerRadii {
            top_start: grow(radii.top_start),
            top_end: grow(radii.top_end),
            bottom_end: grow(radii.bottom_end),
            bottom_start: grow(radii.bottom_start),
        };
        if !size.width.is_finite() || !size.height.is_finite() {
            return Err(FocusIrError::OutsetOverflow);
        }
        if size.width <= input.size.width || size.height <= input.size.height {
            return Err(FocusIrError::UnrepresentableRing);
        }
        Ok(PlacedContour::new(
            PhysicalVector {
                x: -extent,
                y: -extent,
            },
            contour(size, grown)?,
        ))
    };
    let inner = outset(indicator.gap())?;
    let outer = outset(indicator.gap() + indicator.stroke_width())?;
    let inner_bounds = inner
        .contour()
        .bounds()
        .expect("positive surface has bounds");
    let outer_bounds = outer
        .contour()
        .bounds()
        .expect("positive surface has bounds");
    if outer_bounds.width <= inner_bounds.width || outer_bounds.height <= inner_bounds.height {
        return Err(FocusIrError::UnrepresentableRing);
    }
    Ok(FocusIndicatorIr {
        schema_version: "0.1.0",
        indicator,
        geometry: FocusRingGeometry {
            silhouette,
            inner,
            outer,
        },
    })
}
