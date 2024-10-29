use serde::de::Unexpected;
use std::fmt::Display;

macro_rules! non_empty_string {
    ($type:ident) => {
        #[derive(Clone, Debug, serde::Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $type(String);
        impl AsRef<str> for $type {
            fn as_ref(&self) -> &str {
                self.0.as_ref()
            }
        }
        impl std::ops::Deref for $type {
            type Target = str;
            fn deref(&self) -> &Self::Target {
                self.as_ref()
            }
        }
        impl $type {
            pub fn new(i: impl Into<String>) -> Option<Self> {
                let s: String = i.into();
                (!s.is_empty()).then_some(Self(s))
            }
        }
        impl<'de> serde::Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Self::new(String::deserialize(deserializer)?).ok_or_else(|| {
                    serde::de::Error::invalid_value(Unexpected::Str(""), &"non-empty string")
                })
            }
        }
        impl Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
        impl std::str::FromStr for $type {
            type Err = IsEmpty;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s).ok_or(IsEmpty)
            }
        }
    };
}

#[derive(Clone, Copy, Debug)]
pub struct IsEmpty;
impl std::error::Error for IsEmpty {}
impl Display for IsEmpty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("is empty")
    }
}
non_empty_string!(Description);
non_empty_string!(Keyword);

#[cfg(test)]
mod tests {
    use crate::non_empty_strings::Description;

    #[test]
    fn empty_errors() {
        let test = "";
        assert!(test.parse::<Description>().is_err())
    }
    #[test]
    fn non_empty_is_fine() {
        let test = "test";
        assert_eq!(test.parse::<Description>().unwrap().as_ref(), "test");
    }
}
