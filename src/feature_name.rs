use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use unicode_xid::UnicodeXID;

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureName(String);
impl AsRef<str> for FeatureName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for FeatureName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|e: InvalidFeatureName| serde::de::Error::custom(e.to_string()))
    }
}
impl Display for FeatureName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl FromStr for FeatureName {
    type Err = InvalidFeatureName;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        match chars.next() {
            None => return Err(InvalidFeatureName::Empty),
            Some(ch) if !(ch.is_xid_start() || ch == '_' || ch.is_ascii_digit()) => {
                return Err(InvalidFeatureName::InvalidStart)
            }
            Some(_) => {}
        }
        for ch in chars {
            match ch {
                '-' | '+' | '.' => {}
                ch if !ch.is_xid_continue() => return Err(InvalidFeatureName::InvalidCharacter),
                _ => {}
            }
        }
        Ok(Self(s.to_string()))
    }
}
#[derive(Debug)]
pub enum InvalidFeatureName {
    Empty,
    InvalidStart,
    InvalidCharacter,
}
impl std::error::Error for InvalidFeatureName {}
impl Display for InvalidFeatureName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("feature name is empty"),
            Self::InvalidStart => f.write_str("invalid first character. Must be Unicode XID start, digit, or an underscore"),
            Self::InvalidCharacter => f.write_str("invalid non-start character. Must be Unicode XID continue, digit or '+', '-', ':' or '.'"),
        }
    }
}
