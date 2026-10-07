//! Ödeme domain modeli ve durum makinesi.
//! Saf Rust: veritabanı, ağ ve saat okuma yoktur; zaman dışarıdan parametre olarak gelir.

pub mod error;
pub mod merchant_id;
pub mod payment;
pub mod payment_id;
pub mod status;

mod prefixed_id;

pub use error::{IdError, ParseStatusError, PaymentError, RestoreError};
pub use merchant_id::MerchantId;
pub use payment::{BankOutcome, Payment, PaymentSnapshot, PendingOperation, UnknownSnapshot};
pub use payment_id::PaymentId;
pub use status::PaymentStatus;
