use crate::Ratio;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

type Passthrough = String;

pub fn serialize<S>(ratio: &Ratio, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = ratio.to_string();
    Passthrough::serialize(&s, serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Ratio, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Passthrough::deserialize(deserializer)?;
    Ratio::from_str(s.as_ref()).map_err(de::Error::custom)
}
