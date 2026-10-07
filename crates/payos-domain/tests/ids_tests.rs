use payos_domain::{IdError, MerchantId, PaymentId};
use uuid::{Builder, Uuid};

fn v7_uuid() -> Uuid {
    Builder::from_unix_timestamp_millis(1_700_000_000_000, &[7; 10]).into_uuid()
}

#[allow(clippy::unwrap_used)]
fn v4_uuid() -> Uuid {
    Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap()
}

#[test]
fn payment_id_round_trips_through_canonical_string() {
    let id = PaymentId::from_uuid(v7_uuid()).unwrap();
    let text = id.to_string();

    assert!(text.starts_with("pay_"));
    assert_eq!(text.len(), 36);
    assert_eq!(text.parse::<PaymentId>(), Ok(id));
}

#[test]
fn payment_id_accepts_v7_uuid() {
    let uuid = v7_uuid();
    let id = PaymentId::from_uuid(uuid).unwrap();

    assert_eq!(id.as_uuid(), &uuid);
    assert_eq!(id.as_uuid().get_version_num(), 7);
}

#[test]
fn payment_id_rejects_non_v7_uuid() {
    assert_eq!(PaymentId::from_uuid(v4_uuid()), Err(IdError::NotV7));

    let text = format!("pay_{}", v4_uuid().simple());
    assert_eq!(text.parse::<PaymentId>(), Err(IdError::NotV7));
}

#[test]
fn payment_id_rejects_wrong_prefix() {
    let hex = v7_uuid().simple().to_string();

    for input in [format!("mer_{hex}"), format!("PAY_{hex}"), hex] {
        assert_eq!(
            input.parse::<PaymentId>(),
            Err(IdError::InvalidPrefix),
            "input: {input}"
        );
    }
}

#[test]
fn payment_id_rejects_non_canonical_formats() {
    let uuid = v7_uuid();
    let upper = format!("pay_{}", uuid.simple().to_string().to_uppercase());
    let hyphenated = format!("pay_{}", uuid.hyphenated());
    let short = format!("pay_{}", &uuid.simple().to_string()[..31]);
    let non_hex = format!("pay_{}", "g".repeat(32));

    for input in [upper, hyphenated, short, non_hex, "pay_".to_string()] {
        assert_eq!(
            input.parse::<PaymentId>(),
            Err(IdError::InvalidFormat),
            "input: {input}"
        );
    }
}

#[test]
fn merchant_id_round_trips_and_accepts_any_uuid_version() {
    for uuid in [v7_uuid(), v4_uuid()] {
        let id = MerchantId::from_uuid(uuid);
        let text = id.to_string();

        assert!(text.starts_with("mer_"));
        assert_eq!(text.parse::<MerchantId>(), Ok(id));
        assert_eq!(id.as_uuid(), &uuid);
    }
}

#[test]
fn merchant_id_rejects_payment_prefix() {
    let text = format!("pay_{}", v7_uuid().simple());

    assert_eq!(text.parse::<MerchantId>(), Err(IdError::InvalidPrefix));
}
