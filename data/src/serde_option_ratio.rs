use crate::Ratio;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

type Passthrough = Option<String>;

pub fn serialize<S>(ratio: &Option<Ratio>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = match ratio {
        Some(ratio) => Some(ratio.to_string()),
        None => None,
    };
    Passthrough::serialize(&s, serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Ratio>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Passthrough::deserialize(deserializer)?;
    Ok(match s {
        Some(s) => Some(Ratio::from_str(s.as_ref()).map_err(de::Error::custom)?),
        None => None,
    })
}
