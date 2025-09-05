use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
pub enum InfoHashError {
    #[error("Invalid hex encoding: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    #[error("Invalid infohash length: {0}")]
    InvalidLength(usize),
}

#[derive(Debug)]
pub struct InfoHash(Vec<u8>);

impl InfoHash {
    pub fn from_str(s: &str) -> Result<Self, InfoHashError> {
        let decoded = hex::decode(s)?;
        match decoded.len() {
            20 | 32 => Ok(Self(decoded)),
            _ => Err(InfoHashError::InvalidLength(decoded.len())),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InfoHashError> {
        if bytes.len() != 20 && bytes.len() != 32 {
            return Err(InfoHashError::InvalidLength(bytes.len()));
        }

        Ok(Self(bytes.to_vec()))
    }

    pub fn to_string(&self) -> String {
        hex::encode(&self.0)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Display for InfoHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Serialize for InfoHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for InfoHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct InfoHashVisitor;

        impl<'de> serde::de::Visitor<'de> for InfoHashVisitor {
            type Value = InfoHash;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a hex string or byte array representing an infohash")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                InfoHash::from_str(value).map_err(serde::de::Error::custom)
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                InfoHash::from_bytes(value).map_err(serde::de::Error::custom)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    bytes.push(byte);
                }
                InfoHash::from_bytes(&bytes).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_any(InfoHashVisitor)
    }
}
