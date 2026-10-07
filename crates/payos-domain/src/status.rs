use std::fmt;
use std::str::FromStr;

use crate::error::ParseStatusError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaymentStatus {
    Created,
    RequiresAction,
    Authorized,
    Captured,
    Failed,
    Voided,
    PartiallyRefunded,
    Refunded,
    Unknown,
    Disputed,
}

impl PaymentStatus {
    pub const ALL: [Self; 10] = [
        Self::Created,
        Self::RequiresAction,
        Self::Authorized,
        Self::Captured,
        Self::Failed,
        Self::Voided,
        Self::PartiallyRefunded,
        Self::Refunded,
        Self::Unknown,
        Self::Disputed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::RequiresAction => "requires_action",
            Self::Authorized => "authorized",
            Self::Captured => "captured",
            Self::Failed => "failed",
            Self::Voided => "voided",
            Self::PartiallyRefunded => "partially_refunded",
            Self::Refunded => "refunded",
            Self::Unknown => "unknown",
            Self::Disputed => "disputed",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Failed | Self::Voided | Self::Refunded | Self::Disputed
        )
    }

    /// Durum düzeyindeki geçiş tablosu. Gerekli koşuldur ama yeterli değildir:
    /// `Unknown`'dan çıkışın kesin hedefini bekleyen işlem belirler.
    pub const fn can_transition_to(self, next: Self) -> bool {
        use PaymentStatus::*;
        matches!(
            (self, next),
            (Created, RequiresAction | Authorized | Failed | Unknown)
                | (RequiresAction, Authorized | Failed | Unknown)
                | (Authorized, Captured | Voided | Unknown)
                | (Captured, PartiallyRefunded | Refunded | Disputed | Unknown)
                | (
                    PartiallyRefunded,
                    PartiallyRefunded | Refunded | Disputed | Unknown
                )
                | (
                    Unknown,
                    Authorized | Failed | Captured | Voided | PartiallyRefunded | Refunded
                )
        )
    }
}

impl FromStr for PaymentStatus {
    type Err = ParseStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == s)
            .ok_or(ParseStatusError)
    }
}

impl fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
