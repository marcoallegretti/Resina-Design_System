use resina_resolver::{SliderStops, SliderTieBreak, SliderValuePolicy};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Policy {
    Continuous,
    Stops {
        minimum: f64,
        maximum: f64,
        values: Vec<f64>,
        #[serde(rename = "tieBreak")]
        tie_break: SliderTieBreak,
    },
}
impl Policy {
    pub fn resolve(&self) -> SliderValuePolicy {
        match self {
            Self::Continuous => SliderValuePolicy::Continuous,
            Self::Stops {
                minimum,
                maximum,
                values,
                tie_break,
            } => SliderValuePolicy::Stops {
                stops: SliderStops::try_new(*minimum, *maximum, values).unwrap(),
                tie_break: *tie_break,
            },
        }
    }
}
