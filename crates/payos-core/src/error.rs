use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MoneyError {
    #[error("amount must not be negative")]
    NegativeAmount,
    #[error("currency mismatch")]
    CurrencyMismatch,
    #[error("arithmetic overflow")]
    Overflow,
    #[error("insufficient amount")]
    InsufficientAmount,
    #[error("invalid currency code")]
    InvalidCurrency,
}
