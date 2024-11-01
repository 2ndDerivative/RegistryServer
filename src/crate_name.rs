use std::{fmt::Display, hash::Hash, str::FromStr};

use serde::{Deserialize, Serialize};
use unicode_xid::UnicodeXID;

#[derive(Clone, Debug, Serialize)]
/// Shares logic with cargo for validity of crate names
///
/// 1. Can't be empty
/// 2. Can't start with digit
/// 3. First letter must be Unicode XID or _
/// 4. Must continue with Unicode XID Continue or - (includes _)
pub struct CrateName(String);
impl CrateName {
    pub fn original_str(&self) -> &str {
        &self.0
    }
    pub fn normalized(&self) -> String {
        self.0.replace('-', "_").to_lowercase()
    }
}
impl PartialEq for CrateName {
    fn eq(&self, other: &Self) -> bool {
        self.normalized() == other.normalized()
    }
}
impl Eq for CrateName {}
impl PartialOrd for CrateName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CrateName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.normalized().cmp(&other.normalized())
    }
}
impl Hash for CrateName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.normalized().hash(state);
    }
}
impl Display for CrateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.original_str())
    }
}
impl FromStr for CrateName {
    type Err = InvalidCrateName;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if is_reserved_file_name(&s.to_ascii_uppercase()) {
            return Err(InvalidCrateName::IsReservedFileName);
        }
        let mut chars = s.chars();
        match chars.next() {
            Some(letter) if letter.is_ascii_digit() => {
                return Err(InvalidCrateName::StartsWithDigit)
            }
            None => return Err(InvalidCrateName::Empty),
            Some('_') => {}
            Some(letter) if !letter.is_xid_start() => {
                return Err(InvalidCrateName::FirstLetterNotUXID)
            }
            _ => {}
        }
        for ch in chars {
            match ch {
                '-' => {}
                ch if !ch.is_xid_continue() => return Err(InvalidCrateName::LetterNotUXID),
                _ => {}
            }
        }
        Ok(CrateName(s.to_string()))
    }
}
impl<'de> Deserialize<'de> for CrateName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|e: InvalidCrateName| serde::de::Error::custom(e.to_string()))
    }
}
#[derive(Debug, PartialEq)]
pub enum InvalidCrateName {
    IsReservedFileName,
    Empty,
    StartsWithDigit,
    FirstLetterNotUXID,
    LetterNotUXID,
}
impl std::error::Error for InvalidCrateName {}
impl std::fmt::Display for InvalidCrateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IsReservedFileName => f.write_str("invalid windows filesystem names not allowed"),
            Self::Empty => f.write_str("crate name is empty"),
            Self::StartsWithDigit => f.write_str("crate name starts with a digit"),
            Self::FirstLetterNotUXID => f.write_str("first letter is not unicode XID start or '_'"),
            Self::LetterNotUXID => f.write_str("characters after first must be unicode XID"),
        }
    }
}

fn is_reserved_file_name(s: &str) -> bool {
    matches!(
        s,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM0"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "COM¹"
            | "COM²"
            | "COM³"
            | "LPT0"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
            | "LPT¹"
            | "LPT²"
            | "LPT³"
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::crate_name::{CrateName, InvalidCrateName};

    #[test]
    fn disallow_lowercase_aux() {
        assert_eq!(
            CrateName::from_str("nul"),
            Err(InvalidCrateName::IsReservedFileName)
        );
    }
    #[test]
    fn disallow_empty() {
        assert_eq!(CrateName::from_str(""), Err(InvalidCrateName::Empty));
    }
    #[test]
    fn disallow_emoji() {
        assert_eq!(
            CrateName::from_str("❤️"),
            Err(InvalidCrateName::FirstLetterNotUXID)
        );
    }
}
