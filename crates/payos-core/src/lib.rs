//! Saf domain mantığı: para, doğrulama ve ortak iş kuralları.
//! Ağ, disk ve zaman bağımlılığı içermez.

pub mod error;
pub mod money;

pub use error::MoneyError;
pub use money::{Currency, MinorAmount, Money};
