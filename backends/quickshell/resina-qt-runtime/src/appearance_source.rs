use resina_model::OpaqueSurfaceAppearance;
use serde::{Deserialize, Deserializer, de::Error};
use serde_json::value::RawValue;

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<OpaqueSurfaceAppearance, D::Error> {
    let source = Box::<RawValue>::deserialize(deserializer)?;
    resina_resolver::parse_opaque_surface_appearance_source(source.get()).map_err(D::Error::custom)
}
