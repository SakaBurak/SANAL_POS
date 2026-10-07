use payos_core::{Currency, Money, MoneyError};
use payos_domain::{
    BankOutcome, MerchantId, Payment, PaymentError, PaymentId, PaymentStatus, PendingOperation,
};
use time::OffsetDateTime;
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
fn new_payment() -> Payment {
    let uuid = Builder::from_unix_timestamp_millis(1_700_000_000_000, &[1; 10]).into_uuid();
    let id = PaymentId::from_uuid(uuid).unwrap();
    let merchant_id = MerchantId::from_uuid(Uuid::from_u128(42));
    Payment::new(id, merchant_id, try_money(TOTAL), at(0))
}

#[allow(clippy::unwrap_used)]
fn payment_in(status: PaymentStatus) -> Payment {
    let mut p = new_payment();
    match status {
        PaymentStatus::Created => {}
        PaymentStatus::RequiresAction => p.require_action(at(1)).unwrap(),
        PaymentStatus::Authorized => p.authorize(at(1)).unwrap(),
        PaymentStatus::Failed => p.fail(at(1)).unwrap(),
        PaymentStatus::Captured => {
            p.authorize(at(1)).unwrap();
            p.capture(at(2)).unwrap();
        }
        PaymentStatus::Voided => {
            p.authorize(at(1)).unwrap();
            p.void(at(2)).unwrap();
        }
        PaymentStatus::PartiallyRefunded => {
            p.authorize(at(1)).unwrap();
            p.capture(at(2)).unwrap();
            p.refund(try_money(4_000), at(3)).unwrap();
        }
        PaymentStatus::Refunded => {
            p.authorize(at(1)).unwrap();
            p.capture(at(2)).unwrap();
            p.refund(try_money(TOTAL), at(3)).unwrap();
        }
        PaymentStatus::Disputed => {
            p.authorize(at(1)).unwrap();
            p.capture(at(2)).unwrap();
            p.dispute(at(3)).unwrap();
        }
        PaymentStatus::Unknown => p.mark_unknown(PendingOperation::Authorize, at(1)).unwrap(),
    }
    assert_eq!(p.status(), status);
    p
}

fn invalid(from: PaymentStatus, to: PaymentStatus) -> Result<(), PaymentError> {
    Err(PaymentError::InvalidTransition { from, to })
}

// --- Oluşturma ve mutlu yollar ---

#[test]
fn new_payment_starts_created_at_version_one() {
    let p = new_payment();

    assert_eq!(p.status(), PaymentStatus::Created);
    assert_eq!(p.version(), 1);
    assert_eq!(p.amount(), try_money(TOTAL));
    assert!(p.refunded().is_zero());
    assert_eq!(p.refunded().currency(), Currency::TRY);
    assert_eq!(p.pending_operation(), None);
    assert_eq!(p.created_at(), at(0));
    assert_eq!(p.updated_at(), at(0));
}

#[test]
fn three_ds_happy_path_reaches_captured_and_bumps_version() {
    let mut p = new_payment();

    p.require_action(at(1)).unwrap();
    assert_eq!(
        (p.status(), p.version()),
        (PaymentStatus::RequiresAction, 2)
    );

    p.authorize(at(2)).unwrap();
    assert_eq!((p.status(), p.version()), (PaymentStatus::Authorized, 3));

    p.capture(at(3)).unwrap();
    assert_eq!((p.status(), p.version()), (PaymentStatus::Captured, 4));

    assert_eq!(p.created_at(), at(0));
    assert_eq!(p.updated_at(), at(3));
}

#[test]
fn frictionless_payment_goes_from_created_to_authorized() {
    let mut p = new_payment();

    p.authorize(at(1)).unwrap();

    assert_eq!(p.status(), PaymentStatus::Authorized);
    assert_eq!(p.version(), 2);
}

#[test]
fn declined_authorization_fails_payment() {
    let mut p = payment_in(PaymentStatus::RequiresAction);

    p.fail(at(2)).unwrap();

    assert_eq!(p.status(), PaymentStatus::Failed);
}

#[test]
fn authorized_payment_can_be_voided() {
    let mut p = payment_in(PaymentStatus::Authorized);

    p.void(at(2)).unwrap();

    assert_eq!(p.status(), PaymentStatus::Voided);
}

// --- İade ---

#[test]
fn partial_then_remaining_refund_reaches_refunded() {
    let mut p = payment_in(PaymentStatus::Captured);

    p.refund(try_money(4_000), at(3)).unwrap();
    assert_eq!(p.status(), PaymentStatus::PartiallyRefunded);
    assert_eq!(p.refunded(), try_money(4_000));

    p.refund(try_money(1_000), at(4)).unwrap();
    assert_eq!(p.status(), PaymentStatus::PartiallyRefunded);
    assert_eq!(p.refunded(), try_money(5_000));

    p.refund(try_money(5_000), at(5)).unwrap();
    assert_eq!(p.status(), PaymentStatus::Refunded);
    assert_eq!(p.refunded(), try_money(TOTAL));
}

#[test]
fn full_refund_at_once_reaches_refunded() {
    let mut p = payment_in(PaymentStatus::Captured);

    p.refund(try_money(TOTAL), at(3)).unwrap();

    assert_eq!(p.status(), PaymentStatus::Refunded);
}

#[test]
fn rejects_refund_exceeding_captured_amount() {
    let mut p = payment_in(PaymentStatus::Captured);
    assert_eq!(
        p.refund(try_money(TOTAL + 1), at(3)),
        Err(PaymentError::RefundExceedsCaptured)
    );

    let mut p = payment_in(PaymentStatus::PartiallyRefunded);
    assert_eq!(
        p.refund(try_money(6_001), at(4)),
        Err(PaymentError::RefundExceedsCaptured)
    );
}

#[test]
fn rejects_zero_refund() {
    let mut p = payment_in(PaymentStatus::Captured);

    assert_eq!(
        p.refund(Money::zero(Currency::TRY), at(3)),
        Err(PaymentError::ZeroRefund)
    );
}

#[test]
fn rejects_refund_in_different_currency() {
    let mut p = payment_in(PaymentStatus::Captured);
    let usd = Money::new(100, Currency::USD).unwrap();

    assert_eq!(
        p.refund(usd, at(3)),
        Err(PaymentError::Money(MoneyError::CurrencyMismatch))
    );
}

// --- Yasak geçişler ---

#[test]
fn captured_cannot_go_back_to_authorized() {
    let mut p = payment_in(PaymentStatus::Captured);

    assert_eq!(
        p.authorize(at(9)),
        invalid(PaymentStatus::Captured, PaymentStatus::Authorized)
    );
}

#[test]
fn final_states_cannot_be_captured() {
    for status in [
        PaymentStatus::Refunded,
        PaymentStatus::Failed,
        PaymentStatus::Voided,
        PaymentStatus::Created,
        PaymentStatus::Captured,
    ] {
        let mut p = payment_in(status);
        assert_eq!(
            p.capture(at(9)),
            invalid(status, PaymentStatus::Captured),
            "{status}"
        );
    }
}

#[test]
fn cannot_refund_before_capture() {
    for status in [
        PaymentStatus::Created,
        PaymentStatus::Authorized,
        PaymentStatus::Voided,
    ] {
        let mut p = payment_in(status);
        assert!(
            matches!(
                p.refund(try_money(100), at(9)),
                Err(PaymentError::InvalidTransition { from, .. }) if from == status
            ),
            "{status}"
        );
    }
}

#[test]
fn failed_command_leaves_payment_unchanged() {
    let mut p = payment_in(PaymentStatus::Authorized);
    let before = p.clone();

    assert!(p.refund(try_money(100), at(9)).is_err());
    assert!(p.authorize(at(9)).is_err());
    assert!(p.mark_unknown(PendingOperation::Authorize, at(9)).is_err());
    assert!(p.resolve_unknown(BankOutcome::Approved, at(9)).is_err());

    assert_eq!(p, before);
}

#[test]
fn dispute_is_allowed_after_capture_and_partial_refund() {
    for status in [PaymentStatus::Captured, PaymentStatus::PartiallyRefunded] {
        let mut p = payment_in(status);
        p.dispute(at(9)).unwrap();
        assert_eq!(p.status(), PaymentStatus::Disputed);
    }

    let mut p = payment_in(PaymentStatus::Authorized);
    assert_eq!(
        p.dispute(at(9)),
        invalid(PaymentStatus::Authorized, PaymentStatus::Disputed)
    );
}

// --- Unknown: işlem-bilinçli çözüm ---

#[allow(clippy::unwrap_used)]
fn resolve(
    start: PaymentStatus,
    operation: PendingOperation,
    outcome: BankOutcome,
) -> (Payment, Payment) {
    let mut p = payment_in(start);
    let before = p.clone();
    p.mark_unknown(operation, at(10)).unwrap();
    assert_eq!(p.status(), PaymentStatus::Unknown);
    assert_eq!(p.pending_operation(), Some(operation));
    assert_eq!(Some(p.version()), before.version().checked_add(1));

    p.resolve_unknown(outcome, at(11)).unwrap();
    assert_eq!(p.pending_operation(), None);
    assert_eq!(Some(p.version()), before.version().checked_add(2));
    assert_eq!(p.updated_at(), at(11));
    (before, p)
}

#[test]
fn unknown_authorize_resolves_to_authorized_or_failed() {
    for start in [PaymentStatus::Created, PaymentStatus::RequiresAction] {
        let (_, p) = resolve(start, PendingOperation::Authorize, BankOutcome::Approved);
        assert_eq!(p.status(), PaymentStatus::Authorized);

        let (_, p) = resolve(start, PendingOperation::Authorize, BankOutcome::Declined);
        assert_eq!(p.status(), PaymentStatus::Failed);
    }
}

#[test]
fn unknown_capture_resolves_to_captured_or_back_to_authorized() {
    let (_, p) = resolve(
        PaymentStatus::Authorized,
        PendingOperation::Capture,
        BankOutcome::Approved,
    );
    assert_eq!(p.status(), PaymentStatus::Captured);

    let (_, p) = resolve(
        PaymentStatus::Authorized,
        PendingOperation::Capture,
        BankOutcome::Declined,
    );
    assert_eq!(p.status(), PaymentStatus::Authorized);
}

#[test]
fn unknown_void_resolves_to_voided_or_back_to_authorized() {
    let (_, p) = resolve(
        PaymentStatus::Authorized,
        PendingOperation::Void,
        BankOutcome::Approved,
    );
    assert_eq!(p.status(), PaymentStatus::Voided);

    let (_, p) = resolve(
        PaymentStatus::Authorized,
        PendingOperation::Void,
        BankOutcome::Declined,
    );
    assert_eq!(p.status(), PaymentStatus::Authorized);
}

#[test]
fn approved_unknown_refund_applies_refund() {
    let (_, p) = resolve(
        PaymentStatus::Captured,
        PendingOperation::Refund(try_money(3_000)),
        BankOutcome::Approved,
    );
    assert_eq!(p.status(), PaymentStatus::PartiallyRefunded);
    assert_eq!(p.refunded(), try_money(3_000));

    let (_, p) = resolve(
        PaymentStatus::PartiallyRefunded,
        PendingOperation::Refund(try_money(6_000)),
        BankOutcome::Approved,
    );
    assert_eq!(p.status(), PaymentStatus::Refunded);
    assert_eq!(p.refunded(), try_money(TOTAL));
}

#[test]
fn declined_unknown_refund_restores_previous_state() {
    for start in [PaymentStatus::Captured, PaymentStatus::PartiallyRefunded] {
        let (before, p) = resolve(
            start,
            PendingOperation::Refund(try_money(1_000)),
            BankOutcome::Declined,
        );
        assert_eq!(p.status(), start);
        assert_eq!(p.refunded(), before.refunded());
    }
}

#[test]
fn commands_are_rejected_while_unknown() {
    let mut p = payment_in(PaymentStatus::Authorized);
    p.mark_unknown(PendingOperation::Capture, at(10)).unwrap();
    let before = p.clone();
    let unknown = PaymentStatus::Unknown;

    assert_eq!(p.capture(at(11)), invalid(unknown, PaymentStatus::Captured));
    assert_eq!(p.void(at(11)), invalid(unknown, PaymentStatus::Voided));
    assert_eq!(
        p.authorize(at(11)),
        invalid(unknown, PaymentStatus::Authorized)
    );
    assert_eq!(p.fail(at(11)), invalid(unknown, PaymentStatus::Failed));
    assert_eq!(p.dispute(at(11)), invalid(unknown, PaymentStatus::Disputed));
    assert!(matches!(
        p.refund(try_money(100), at(11)),
        Err(PaymentError::InvalidTransition {
            from: PaymentStatus::Unknown,
            ..
        })
    ));
    assert_eq!(
        p.mark_unknown(PendingOperation::Capture, at(11)),
        invalid(unknown, unknown)
    );

    assert_eq!(p, before);
}

#[test]
fn resolve_requires_unknown_state() {
    let mut p = payment_in(PaymentStatus::Authorized);

    assert_eq!(
        p.resolve_unknown(BankOutcome::Approved, at(9)),
        Err(PaymentError::NotInUnknownState {
            status: PaymentStatus::Authorized
        })
    );
}

#[test]
fn mark_unknown_requires_operation_valid_for_current_state() {
    let cases = [
        (PaymentStatus::Created, PendingOperation::Capture),
        (PaymentStatus::Created, PendingOperation::Void),
        (PaymentStatus::Authorized, PendingOperation::Authorize),
        (PaymentStatus::Captured, PendingOperation::Capture),
        (
            PaymentStatus::Authorized,
            PendingOperation::Refund(try_money(100)),
        ),
    ];

    for (status, operation) in cases {
        let mut p = payment_in(status);
        assert_eq!(
            p.mark_unknown(operation, at(9)),
            invalid(status, PaymentStatus::Unknown),
            "{status} {operation:?}"
        );
    }
}

#[test]
fn mark_unknown_refund_validates_amount() {
    let mut p = payment_in(PaymentStatus::Captured);
    let before = p.clone();

    assert_eq!(
        p.mark_unknown(PendingOperation::Refund(try_money(TOTAL + 1)), at(9)),
        Err(PaymentError::RefundExceedsCaptured)
    );
    assert_eq!(
        p.mark_unknown(PendingOperation::Refund(Money::zero(Currency::TRY)), at(9)),
        Err(PaymentError::ZeroRefund)
    );
    assert_eq!(p, before);
}

#[test]
fn terminal_states_cannot_enter_unknown() {
    let operations = [
        PendingOperation::Authorize,
        PendingOperation::Capture,
        PendingOperation::Void,
        PendingOperation::Refund(try_money(100)),
    ];

    for status in PaymentStatus::ALL.into_iter().filter(|s| s.is_terminal()) {
        for operation in operations {
            let mut p = payment_in(status);
            assert_eq!(
                p.mark_unknown(operation, at(9)),
                invalid(status, PaymentStatus::Unknown),
                "{status} {operation:?}"
            );
        }
    }
}
