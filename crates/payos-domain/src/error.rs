use payos_core::MoneyError;
use thiserror::Error;

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
    #[error(transparent)]
    Money(#[from] MoneyError),
}
