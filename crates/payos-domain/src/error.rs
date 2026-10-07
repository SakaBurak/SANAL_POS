use payos_core::MoneyError;
use thiserror::Error;
use time::OffsetDateTime;

use crate::status::PaymentStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IdError {
    #[error("id has an invalid prefix")]
    InvalidPrefix,
    #[error("id has an invalid format")]
    InvalidFormat,
    #[error("payment id must be a UUIDv7")]
    NotV7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unknown payment status")]
pub struct ParseStatusError;

/// Kalıcı kayıttan yüklenen verinin domain kurallarına uymadığını belirtir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RestoreError {
    #[error("refunded currency differs from payment currency")]
    CurrencyMismatch,
    #[error("refunded amount exceeds payment amount")]
    RefundExceedsAmount,
    #[error("refunded amount is inconsistent with status {status}")]
    RefundInconsistentWithStatus { status: PaymentStatus },
    #[error("unknown context does not match payment status")]
    UnknownContextMismatch,
    #[error("version must be at least 1")]
    InvalidVersion,
    #[error("updated_at is earlier than created_at")]
    UpdatedBeforeCreated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PaymentError {
    #[error("invalid payment transition from {from} to {to}")]
    InvalidTransition {
        from: PaymentStatus,
        to: PaymentStatus,
    },
    #[error("payment is not in unknown state (current: {status})")]
    NotInUnknownState { status: PaymentStatus },
    #[error("refund amount must be greater than zero")]
    ZeroRefund,
    #[error("total refund exceeds captured amount")]
    RefundExceedsCaptured,
    #[error("payment version overflow")]
    VersionOverflow,
    #[error("timestamp {now} is earlier than last update {updated_at}")]
    TimestampBeforeLastUpdate {
        updated_at: OffsetDateTime,
        now: OffsetDateTime,
    },
    #[error(transparent)]
    Money(#[from] MoneyError),
}
