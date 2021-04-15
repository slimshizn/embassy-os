use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, thiserror::Error)]
#[error("Invalid Package ID")]
pub struct InvalidPackageId;
impl From<InvalidPackageId> for Error {
    fn from(err: InvalidPackageId) -> Self {
        Error::new(err, crate::error::ErrorKind::InvalidPackageId)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageId<S: AsRef<str> = String>(S);
impl<S: AsRef<str>> PackageId<S> {
    pub fn try_from(value: S) -> Result<Self, InvalidPackageId> {
        if value
            .as_ref()
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '-')
        {
            Ok(PackageId(value))
        } else {
            Err(InvalidPackageId)
        }
    }
}
