use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

pub fn parse_token_document(source: &str) -> Result<Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let document = UniqueValue { path: "#".into() }.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(document)
}

struct UniqueValue {
    path: String,
}

impl<'de> DeserializeSeed<'de> for UniqueValue {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor { path: self.path })
    }
}

struct UniqueValueVisitor {
    path: String,
}

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(UniqueValue {
            path: format!("{}/{}", self.path, values.len()),
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(name) = object.next_key::<String>()? {
            let path = format!("{}/{}", self.path, crate::escape_pointer_segment(&name));
            if values.contains_key(&name) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON member at {path}"
                )));
            }
            let value = object.next_value_seed(UniqueValue { path })?;
            values.insert(name, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/source-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = parse_token_document(vector["source"].as_str().unwrap());
            if let Some(expected) = vector.get("expected") {
                assert_eq!(result.unwrap(), *expected, "{}", vector["name"]);
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}",
                    vector["name"]
                );
            }
        }
    }
}
