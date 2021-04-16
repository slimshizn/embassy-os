use std::borrow::Cow;
use std::fmt::Debug;

use serde::{Deserialize, Deserializer, Serialize};

use crate::Error;

#[derive(Debug, thiserror::Error)]
#[error("Invalid ID")]
pub struct InvalidId;
impl From<InvalidId> for Error {
    fn from(err: InvalidId) -> Self {
        Error::new(err, crate::error::ErrorKind::InvalidPackageId)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Id<S: AsRef<str> = String>(S);
impl<S: AsRef<str>> Id<S> {
    pub fn try_from(value: S) -> Result<Self, InvalidId> {
        if value
            .as_ref()
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '-')
        {
            Ok(Id(value))
        } else {
            Err(InvalidId)
        }
    }
}
impl<S: AsRef<str>> std::fmt::Display for Id<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_ref())
    }
}
impl<S: AsRef<str>> AsRef<str> for Id<S> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl<'de> Deserialize<'de> for Id<Cow<'de, str>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Id<Cow<'de, str>>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a string with only lowercase letters and hyphens"
                )
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Id::try_from(Cow::Owned(v.to_owned())).map_err(serde::de::Error::custom)
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Id::try_from(Cow::Owned(v)).map_err(serde::de::Error::custom)
            }
            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Id::try_from(Cow::Borrowed(v)).map_err(serde::de::Error::custom)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Id::try_from(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
impl<'de> Deserialize<'de> for Id<&'de str> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Id::try_from(<&'de str>::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ImageId<S: AsRef<str> = String>(Id<S>);
impl<S: AsRef<str>> std::fmt::Display for ImageId<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}
impl<S: AsRef<str>> ImageId<S> {
    pub fn for_package(
        &self,
        pkg_id: &crate::s9pk::manifest::PackageId,
        pkg_version: &emver::Version,
    ) -> String {
        format!("start9/{}/{}:{}", pkg_id, self.0, pkg_version)
    }
}
impl<'de, S> Deserialize<'de> for ImageId<S>
where
    S: AsRef<str>,
    Id<S>: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(ImageId(Deserialize::deserialize(deserializer)?))
    }
}
