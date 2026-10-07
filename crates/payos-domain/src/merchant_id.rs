use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use crate::error::IdError;
use crate::prefixed_id;

const PREFIX: &str = "mer_";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MerchantId(Uuid);

impl MerchantId {
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl FromStr for MerchantId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        prefixed_id::parse(s, PREFIX).map(Self)
    }
}

impl fmt::Display for MerchantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        prefixed_id::fmt(f, PREFIX, &self.0)
    }
}
