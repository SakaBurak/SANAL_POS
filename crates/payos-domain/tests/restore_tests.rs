use payos_core::{Currency, Money};
use payos_domain::{
    BankOutcome, MerchantId, Payment, PaymentId, PaymentSnapshot, PaymentStatus, PendingOperation,
    RestoreError, UnknownSnapshot,
};
use time::{Duration, OffsetDateTime, UtcOffset};
use uuid::{Builder, Uuid};

const TOTAL: i64 = 10_000;

#[allow(clippy::unwrap_used)]
fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_700_000_000_i64.checked_add(seconds).unwrap()).unwrap()
}

#[allow(clippy::unwrap_used)]
fn try_money(amount: i64) -> Money {
    Money::new(amount, Currency::TRY).unwrap()
}

#[allow(clippy::unwrap_used)]
fn payment_at(now: OffsetDateTime) -> Payment {
    let uuid = Builder::from_unix_timestamp_millis(1_700_000_000_000, &[1; 10]).into_uuid();
    let id = PaymentId::from_uuid(uuid).unwrap();
    let merchant_id = MerchantId::from_uuid(Uuid::from_u128(42));
    Payment::new(id, merchant_id, try_money(TOTAL), now)
}

/// Komutlarla ulaşılabilen her durum ve her `Unknown` işlem türü.
#[allow(clippy::unwrap_used)]
fn reachable_payments() -> Vec<Payment> {
    let created = payment_at(at(0));

    let mut requires_action = created.clone();
    requires_action.require_action(at(1)).unwrap();

    let mut authorized = created.clone();
    authorized.authorize(at(1)).unwrap();

    let mut failed = created.clone();
    failed.fail(at(1)).unwrap();

    let mut captured = authorized.clone();
    captured.capture(at(2)).unwrap();

    let mut voided = authorized.clone();
    voided.void(at(2)).unwrap();

    let mut partially_refunded = captured.clone();
    partially_refunded.refund(try_money(4_000), at(3)).unwrap();

    let mut refunded = captured.clone();
    refunded.refund(try_money(TOTAL), at(3)).unwrap();

    let mut disputed = partially_refunded.clone();
    disputed.dispute(at(4)).unwrap();

    let mut unknown_authorize = requires_action.clone();
    unknown_authorize
        .mark_unknown(PendingOperation::Authorize, at(2))
        .unwrap();

    let mut unknown_capture = authorized.clone();
    unknown_capture
        .mark_unknown(PendingOperation::Capture, at(2))
        .unwrap();

    let mut unknown_void = authorized.clone();
    unknown_void
        .mark_unknown(PendingOperation::Void, at(2))
        .unwrap();

    let mut unknown_refund = partially_refunded.clone();
    unknown_refund
        .mark_unknown(PendingOperation::Refund(try_money(6_000)), at(4))
        .unwrap();

    vec![
        created,
        requires_action,
        authorized,
        failed,
        captured,
        voided,
        partially_refunded,
        refunded,
        disputed,
        unknown_authorize,
        unknown_capture,
        unknown_void,
        unknown_refund,
    ]
}

fn snapshot_with(status: PaymentStatus, refunded: i64) -> PaymentSnapshot {
    let mut snapshot = payment_at(at(0)).snapshot();
    snapshot.status = status;
    snapshot.refunded = try_money(refunded);
    snapshot
}

fn unknown_snapshot(
    operation: PendingOperation,
    previous_status: PaymentStatus,
    refunded: i64,
) -> PaymentSnapshot {
    let mut snapshot = snapshot_with(PaymentStatus::Unknown, refunded);
    snapshot.unknown = Some(UnknownSnapshot {
        operation,
        previous_status,
    });
    snapshot
}

// --- Round-trip ---

#[test]
fn every_reachable_payment_round_trips_through_snapshot() {
    let payments = reachable_payments();
    let statuses: Vec<_> = payments.iter().map(Payment::status).collect();
    for status in PaymentStatus::ALL {
        assert!(statuses.contains(&status), "{status} is not covered");
    }

    for payment in payments {
        let restored = Payment::restore(payment.snapshot()).unwrap();
        assert_eq!(restored, payment);
        assert_eq!(restored.snapshot(), payment.snapshot());
    }
}

#[test]
fn restored_unknown_payment_can_be_resolved() {
    let mut original = payment_at(at(0));
    original.authorize(at(1)).unwrap();
    original.capture(at(2)).unwrap();
    original
        .mark_unknown(PendingOperation::Refund(try_money(TOTAL)), at(3))
        .unwrap();

    let mut restored = Payment::restore(original.snapshot()).unwrap();
    restored
        .resolve_unknown(BankOutcome::Approved, at(4))
        .unwrap();

    assert_eq!(restored.status(), PaymentStatus::Refunded);
    assert_eq!(restored.refunded(), try_money(TOTAL));
    assert_eq!(restored.version(), 5);
}

#[test]
fn status_parses_from_its_own_text() {
    for status in PaymentStatus::ALL {
        assert_eq!(status.as_str().parse::<PaymentStatus>(), Ok(status));
    }
    assert!("Captured".parse::<PaymentStatus>().is_err());
    assert!("".parse::<PaymentStatus>().is_err());
    assert!("settled".parse::<PaymentStatus>().is_err());
}

// --- Zaman normalizasyonu ---

#[test]
fn timestamps_are_truncated_to_microseconds_in_utc() {
    let offset = UtcOffset::from_hms(3, 0, 0).unwrap();
    let local = (at(0) + Duration::nanoseconds(123_456_789)).to_offset(offset);

    let p = payment_at(local);

    assert_eq!(p.created_at().offset(), UtcOffset::UTC);
    assert_eq!(p.created_at().nanosecond(), 123_456_000);
    assert_eq!(p.created_at(), at(0) + Duration::microseconds(123_456));
    assert_eq!(p.updated_at(), p.created_at());
}

#[test]
fn monotonic_check_uses_microsecond_precision() {
    let mut p = payment_at(at(0) + Duration::nanoseconds(1_500));

    p.authorize(at(0) + Duration::nanoseconds(1_100)).unwrap();

    assert_eq!(p.updated_at(), at(0) + Duration::microseconds(1));
}

#[test]
fn restore_normalizes_timestamps() {
    let mut snapshot = payment_at(at(0)).snapshot();
    snapshot.created_at = at(0) + Duration::nanoseconds(999);
    snapshot.updated_at = at(1) + Duration::nanoseconds(1_999);

    let p = Payment::restore(snapshot).unwrap();

    assert_eq!(p.created_at(), at(0));
    assert_eq!(p.updated_at(), at(1) + Duration::microseconds(1));
}

// --- Geçersiz snapshot reddi ---

#[test]
fn rejects_version_zero() {
    let mut snapshot = payment_at(at(0)).snapshot();
    snapshot.version = 0;

    assert_eq!(
        Payment::restore(snapshot),
        Err(RestoreError::InvalidVersion)
    );
}

#[test]
fn rejects_updated_before_created() {
    let mut snapshot = payment_at(at(1)).snapshot();
    snapshot.updated_at = at(0);

    assert_eq!(
        Payment::restore(snapshot),
        Err(RestoreError::UpdatedBeforeCreated)
    );
}

#[test]
fn rejects_refund_in_other_currency() {
    let mut snapshot = payment_at(at(0)).snapshot();
    snapshot.refunded = Money::zero(Currency::USD);

    assert_eq!(
        Payment::restore(snapshot),
        Err(RestoreError::CurrencyMismatch)
    );
}

#[test]
fn rejects_refund_exceeding_amount() {
    let snapshot = snapshot_with(PaymentStatus::Refunded, TOTAL + 1);

    assert_eq!(
        Payment::restore(snapshot),
        Err(RestoreError::RefundExceedsAmount)
    );
}

#[test]
fn rejects_refund_inconsistent_with_status() {
    let cases = [
        (PaymentStatus::Created, 1),
        (PaymentStatus::Authorized, 1),
        (PaymentStatus::Captured, 4_000),
        (PaymentStatus::Voided, 1),
        (PaymentStatus::PartiallyRefunded, 0),
        (PaymentStatus::PartiallyRefunded, TOTAL),
        (PaymentStatus::Refunded, 0),
        (PaymentStatus::Refunded, 4_000),
        (PaymentStatus::Disputed, TOTAL),
    ];
    for (status, refunded) in cases {
        assert_eq!(
            Payment::restore(snapshot_with(status, refunded)),
            Err(RestoreError::RefundInconsistentWithStatus { status }),
            "{status} with refunded {refunded}"
        );
    }
}

#[test]
fn rejects_unknown_status_without_context() {
    let snapshot = snapshot_with(PaymentStatus::Unknown, 0);

    assert_eq!(
        Payment::restore(snapshot),
        Err(RestoreError::UnknownContextMismatch)
    );
}

#[test]
fn rejects_context_on_non_unknown_status() {
    let mut snapshot = unknown_snapshot(PendingOperation::Capture, PaymentStatus::Authorized, 0);
    snapshot.status = PaymentStatus::Authorized;

    assert_eq!(
        Payment::restore(snapshot),
        Err(RestoreError::UnknownContextMismatch)
    );
}

#[test]
fn rejects_previous_status_invalid_for_operation() {
    let cases = [
        (PendingOperation::Authorize, PaymentStatus::Authorized, 0),
        (PendingOperation::Capture, PaymentStatus::Created, 0),
        (PendingOperation::Void, PaymentStatus::Captured, 0),
        (
            PendingOperation::Refund(try_money(1_000)),
            PaymentStatus::Authorized,
            0,
        ),
        (PendingOperation::Authorize, PaymentStatus::Unknown, 0),
    ];
    for (operation, previous, refunded) in cases {
        assert_eq!(
            Payment::restore(unknown_snapshot(operation, previous, refunded)),
            Err(RestoreError::UnknownContextMismatch),
            "{operation:?} after {previous}"
        );
    }
}

#[test]
fn rejects_pending_refund_that_cannot_be_applied() {
    let cases = [
        (PendingOperation::Refund(try_money(0)), 0),
        (PendingOperation::Refund(try_money(6_001)), 4_000),
        (
            PendingOperation::Refund(Money::new(1_000, Currency::EUR).unwrap()),
            0,
        ),
    ];
    for (operation, refunded) in cases {
        let previous = if refunded == 0 {
            PaymentStatus::Captured
        } else {
            PaymentStatus::PartiallyRefunded
        };
        assert_eq!(
            Payment::restore(unknown_snapshot(operation, previous, refunded)),
            Err(RestoreError::UnknownContextMismatch),
            "{operation:?}"
        );
    }
}

#[test]
fn unknown_refund_checks_refunded_against_previous_status() {
    let snapshot = unknown_snapshot(
        PendingOperation::Refund(try_money(1_000)),
        PaymentStatus::Captured,
        4_000,
    );

    assert_eq!(
        Payment::restore(snapshot),
        Err(RestoreError::RefundInconsistentWithStatus {
            status: PaymentStatus::Unknown
        })
    );
}
