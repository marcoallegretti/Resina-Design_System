use crate::{SliderStops, SliderStopsError, SliderTieBreak, SliderValueIr};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SliderValuePolicy {
    Continuous,
    Stops {
        stops: SliderStops,
        #[serde(rename = "tieBreak")]
        tie_break: SliderTieBreak,
    },
}
impl SliderValuePolicy {
    pub fn validate_current(&self, current: &SliderValueIr) -> Result<(), SliderStopsError> {
        match self {
            Self::Continuous => Ok(()),
            Self::Stops { stops, .. } => stops.current_index(current).map(|_| ()),
        }
    }
}
