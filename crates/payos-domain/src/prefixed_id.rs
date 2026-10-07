use std::fmt;

use uuid::Uuid;

use crate::error::IdError;

/// Kanonik biçim: `<prefix>` + 32 küçük harf hex (tiresiz UUID).
pub(crate) fn parse(input: &str, prefix: &str) -> Result<Uuid, IdError> {
    let hex = input.strip_prefix(prefix).ok_or(IdError::InvalidPrefix)?;
    let is_canonical =
        hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !is_canonical {
        return Err(IdError::InvalidFormat);
    }
    Uuid::try_parse(hex).map_err(|_| IdError::InvalidFormat)
}

pub(crate) fn fmt(f: &mut fmt::Formatter<'_>, prefix: &str, uuid: &Uuid) -> fmt::Result {
    write!(f, "{prefix}{}", uuid.simple())
}
