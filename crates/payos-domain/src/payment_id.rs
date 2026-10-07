use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use crate::error::IdError;
use crate::prefixed_id;

const PREFIX: &str = "pay_";

/// Ödeme kimliği. UUIDv7 olmak zorundadır: partition aralığı içindeki zaman
/// damgasından türetilir. UUID üretimi saat okuduğu için uygulama katmanında
/// yapılır; domain yalnızca hazır UUID'yi doğrular.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaymentId(Uuid);

impl PaymentId {
    pub fn from_uuid(uuid: Uuid) -> Result<Self, IdError> {
        if uuid.get_version_num() != 7 {
            return Err(IdError::NotV7);
        }
        Ok(Self(uuid))
    }

    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl FromStr for PaymentId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_uuid(prefixed_id::parse(s, PREFIX)?)
    }
}

impl fmt::Display for PaymentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        prefixed_id::fmt(f, PREFIX, &self.0)
    }
}
