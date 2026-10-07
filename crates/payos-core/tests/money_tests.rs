use payos_core::{Currency, MinorAmount, Money, MoneyError};

#[allow(clippy::unwrap_used)]
fn try_money(amount: i64) -> Money {
    Money::new(amount, Currency::TRY).unwrap()
}

#[test]
fn creates_money_with_amount_and_currency() {
    let money = Money::new(1000, Currency::TRY).unwrap();

    assert_eq!(money.amount().value(), 1000);
    assert_eq!(money.currency(), Currency::TRY);
    assert!(!money.is_zero());
}

#[test]
fn zero_has_zero_amount_and_given_currency() {
    let zero = Money::zero(Currency::USD);

    assert_eq!(zero.amount(), MinorAmount::ZERO);
    assert_eq!(zero.currency(), Currency::USD);
    assert!(zero.is_zero());
    assert_eq!(zero, Money::new(0, Currency::USD).unwrap());
}

#[test]
fn rejects_negative_money() {
    assert_eq!(
        Money::new(-1, Currency::TRY),
        Err(MoneyError::NegativeAmount)
    );
    assert_eq!(
        Money::new(i64::MIN, Currency::EUR),
        Err(MoneyError::NegativeAmount)
    );
}

#[test]
fn rejects_negative_minor_amount() {
    assert_eq!(MinorAmount::new(-100), Err(MoneyError::NegativeAmount));
    assert_eq!(MinorAmount::new(0), Ok(MinorAmount::ZERO));
}

#[test]
fn adds_same_currency() {
    let sum = try_money(1000).checked_add(try_money(500)).unwrap();

    assert_eq!(sum, try_money(1500));
}

#[test]
fn adding_zero_is_identity() {
    let money = try_money(1250);

    assert_eq!(
        money.checked_add(Money::zero(Currency::TRY)).unwrap(),
        money
    );
}

#[test]
fn cannot_add_different_currencies() {
    let try_amount = Money::new(100, Currency::TRY).unwrap();
    let usd_amount = Money::new(100, Currency::USD).unwrap();

    assert_eq!(
        try_amount.checked_add(usd_amount),
        Err(MoneyError::CurrencyMismatch)
    );
}

#[test]
fn detects_addition_overflow() {
    let max = try_money(i64::MAX);

    assert_eq!(max.checked_add(try_money(1)), Err(MoneyError::Overflow));
    assert_eq!(max.checked_add(Money::zero(Currency::TRY)), Ok(max));
}

#[test]
fn subtracts_same_currency() {
    let diff = try_money(1000).checked_sub(try_money(250)).unwrap();

    assert_eq!(diff, try_money(750));
}

#[test]
fn subtracting_equal_amount_gives_zero() {
    let diff = try_money(1000).checked_sub(try_money(1000)).unwrap();

    assert!(diff.is_zero());
    assert_eq!(diff.currency(), Currency::TRY);
}

#[test]
fn rejects_insufficient_amount_on_subtraction() {
    assert_eq!(
        try_money(100).checked_sub(try_money(101)),
        Err(MoneyError::InsufficientAmount)
    );
}

#[test]
fn cannot_subtract_different_currencies() {
    let eur = Money::new(100, Currency::EUR).unwrap();

    assert_eq!(
        try_money(500).checked_sub(eur),
        Err(MoneyError::CurrencyMismatch)
    );
}

#[test]
fn currency_mismatch_is_reported_before_insufficient_amount() {
    let usd = Money::new(1000, Currency::USD).unwrap();

    assert_eq!(
        try_money(1).checked_sub(usd),
        Err(MoneyError::CurrencyMismatch)
    );
}

#[test]
fn parses_supported_currencies() {
    assert_eq!("TRY".parse::<Currency>(), Ok(Currency::TRY));
    assert_eq!("USD".parse::<Currency>(), Ok(Currency::USD));
    assert_eq!("EUR".parse::<Currency>(), Ok(Currency::EUR));
}

#[test]
fn rejects_unknown_currencies() {
    for input in ["BTC", "ABC", "TL", "try", " TRY", "TRY ", "", "TRYY"] {
        assert_eq!(
            input.parse::<Currency>(),
            Err(MoneyError::InvalidCurrency),
            "input: {input:?}"
        );
    }
}

#[test]
fn currency_round_trips_through_string() {
    for currency in [Currency::TRY, Currency::USD, Currency::EUR] {
        assert_eq!(currency.to_string().parse::<Currency>(), Ok(currency));
        assert_eq!(currency.code(), currency.to_string());
        assert_eq!(currency.minor_unit_digits(), 2);
    }
}

#[test]
fn displays_money_as_minor_amount_and_code() {
    assert_eq!(try_money(1250).to_string(), "1250 TRY");
    assert_eq!(Money::zero(Currency::EUR).to_string(), "0 EUR");
}
