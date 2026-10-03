use resina_model::{ContourSegment, PhysicalVector};

pub fn contour_support(contour: &resina_resolver::ExtrudedContourResult, n: PhysicalVector) -> f64 {
    let dot = |p: PhysicalVector| p.x * n.x + p.y * n.y;
    contour
        .segments()
        .iter()
        .map(|segment| match segment {
            ContourSegment::Line { from, to } => dot(*from).max(dot(*to)),
            ContourSegment::Arc {
                center,
                radii,
                start,
                end,
            } => {
                let radial = PhysicalVector {
                    x: radii.x * n.x,
                    y: radii.y * n.y,
                };
                let length = radial.x.hypot(radial.y);
                let radial = PhysicalVector {
                    x: radial.x / length,
                    y: radial.y / length,
                };
                let evaluate = |u: PhysicalVector| {
                    dot(PhysicalVector {
                        x: center.x + radii.x * u.x,
                        y: center.y + radii.y * u.y,
                    })
                };
                let endpoints = evaluate(*start).max(evaluate(*end));
                if start.x * radial.y - start.y * radial.x >= 0.0
                    && radial.x * end.y - radial.y * end.x >= 0.0
                {
                    endpoints.max(evaluate(radial))
                } else {
                    endpoints
                }
            }
        })
        .fold(f64::NEG_INFINITY, f64::max)
}
