use resina_environment::DensityPreference;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DensityModes<T> {
    compact: T,
    standard: T,
    comfortable: T,
    immersive: T,
}

pub const DENSITY_MODES: [DensityPreference; 4] = [
    DensityPreference::Compact,
    DensityPreference::Standard,
    DensityPreference::Comfortable,
    DensityPreference::Immersive,
];

impl<T> DensityModes<T> {
    pub fn from_fn(mut value: impl FnMut(DensityPreference) -> T) -> Self {
        Self {
            compact: value(DensityPreference::Compact),
            standard: value(DensityPreference::Standard),
            comfortable: value(DensityPreference::Comfortable),
            immersive: value(DensityPreference::Immersive),
        }
    }

    pub fn get(&self, mode: DensityPreference) -> &T {
        match mode {
            DensityPreference::Compact => &self.compact,
            DensityPreference::Standard => &self.standard,
            DensityPreference::Comfortable => &self.comfortable,
            DensityPreference::Immersive => &self.immersive,
        }
    }

    pub fn try_map<U, E>(
        &self,
        mut convert: impl FnMut(DensityPreference, &T) -> Result<U, E>,
    ) -> Result<DensityModes<U>, E> {
        Ok(DensityModes {
            compact: convert(DensityPreference::Compact, &self.compact)?,
            standard: convert(DensityPreference::Standard, &self.standard)?,
            comfortable: convert(DensityPreference::Comfortable, &self.comfortable)?,
            immersive: convert(DensityPreference::Immersive, &self.immersive)?,
        })
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DensityModes<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, DensityModesInput<T>>(deserializer)?;
        Ok(Self {
            compact: input.compact,
            standard: input.standard,
            comfortable: input.comfortable,
            immersive: input.immersive,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DensityModesInput<T> {
    compact: T,
    standard: T,
    comfortable: T,
    immersive: T,
}
