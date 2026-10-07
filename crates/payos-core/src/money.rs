use std::fmt;
use std::str::FromStr;

use crate::error::MoneyError;

/// Para biriminin en küçük birimi cinsinden negatif olmayan tutar (ör. TRY için kuruş).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MinorAmount(i64);

impl MinorAmount {
    pub const ZERO: Self = Self(0);

    pub fn new(value: i64) -> Result<Self, MoneyError> {
        if value < 0 {
            return Err(MoneyError::NegativeAmount);
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> i64 {
        self.0
    }
}

impl fmt::Display for MinorAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// ISO 4217 para birimi.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Currency {
    TRY,
    USD,
    EUR,
}

impl Currency {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TRY => "TRY",
            Self::USD => "USD",
            Self::EUR => "EUR",
        }
    }

    /// Ana birimdeki ondalık hane sayısı (ISO 4217 "minor unit").
    pub const fn minor_unit_digits(self) -> u8 {
        match self {
            Self::TRY | Self::USD | Self::EUR => 2,
        }
    }
}

impl FromStr for Currency {
    type Err = MoneyError;

    /// Yalnızca büyük harfli ISO kodunu tam eşleşmeyle kabul eder.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TRY" => Ok(Self::TRY),
            "USD" => Ok(Self::USD),
            "EUR" => Ok(Self::EUR),
            _ => Err(MoneyError::InvalidCurrency),
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Para birimiyle birlikte taşınan, negatif olmayan tutar.
///
/// `Add`, `Sub`, `Mul` ve `PartialOrd` bilerek uygulanmaz: aritmetik yalnızca
/// para birimi ve taşma kontrolü yapan `checked_*` metotlarıyla yapılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money {
    amount: MinorAmount,
    currency: Currency,
}

impl Money {
    pub fn new(amount: i64, currency: Currency) -> Result<Self, MoneyError> {
        Ok(Self {
            amount: MinorAmount::new(amount)?,
            currency,
        })
    }

    pub const fn zero(currency: Currency) -> Self {
        Self {
            amount: MinorAmount::ZERO,
            currency,
        }
    }

    pub const fn amount(self) -> MinorAmount {
        self.amount
    }

    pub const fn currency(self) -> Currency {
        self.currency
    }

    pub const fn is_zero(self) -> bool {
        self.amount.0 == 0
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, MoneyError> {
        self.ensure_same_currency(rhs)?;
        let sum = self
            .amount
            .0
            .checked_add(rhs.amount.0)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self {
            amount: MinorAmount(sum),
            currency: self.currency,
        })
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, MoneyError> {
        self.ensure_same_currency(rhs)?;
        if rhs.amount > self.amount {
            return Err(MoneyError::InsufficientAmount);
        }
        let diff = self
            .amount
            .0
            .checked_sub(rhs.amount.0)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self {
            amount: MinorAmount(diff),
            currency: self.currency,
        })
    }

    fn ensure_same_currency(self, other: Self) -> Result<(), MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        Ok(())
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}
