use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use unicode_xid::UnicodeXID;

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
/// Shares logic with cargo for validity of crate names
/// 
/// 1. Can't be empty
/// 2. Can't start with digit
/// 3. First letter must be Unicode XID or _
/// 4. Must continue with Unicode XID Continue or - (includes _)
pub struct CrateName(String);
impl AsRef<str> for CrateName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl Display for CrateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}
impl FromStr for CrateName {
    type Err = InvalidCrateName;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        match chars.next() {
            Some(letter) if letter.is_ascii_digit() => return Err(InvalidCrateName::StartsWithDigit),
            None => return Err(InvalidCrateName::Empty),
            Some('_') => {}
            Some(letter) if !letter.is_xid_start() => return Err(InvalidCrateName::FirstLetterNotUXID),
            _ => {}
        }
        for ch in chars {
            match ch {
                '-' => {},
                ch if !ch.is_xid_continue() => return  Err(InvalidCrateName::LetterNotUXID),
                _ => {},
            }
        }
        Ok(CrateName(s.to_string()))
    }
}
impl<'de> Deserialize<'de> for CrateName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|e: InvalidCrateName| serde::de::Error::custom(e.to_string()))
    }
}
#[derive(Debug)]
pub enum InvalidCrateName {
    Empty,
    StartsWithDigit,
    FirstLetterNotUXID,
    LetterNotUXID,
}
impl std::error::Error for InvalidCrateName {}
impl std::fmt::Display for InvalidCrateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("crate name is empty"),
            Self::StartsWithDigit => f.write_str("crate name starts with a digit"),
            Self::FirstLetterNotUXID => f.write_str("first letter is not unicode XID start or '_'"),
            Self::LetterNotUXID => f.write_str("characters after first must be unicode XID"),
        }
    }
}
