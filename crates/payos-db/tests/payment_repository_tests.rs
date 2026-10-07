use std::error::Error;

use payos_core::{Currency, Money};
use payos_db::{PaymentRepository, RepositoryError};
use payos_domain::{MerchantId, Payment, PaymentId, PaymentStatus, PendingOperation};
use sqlx::{PgConnection, PgPool, Row};
use time::{Duration, OffsetDateTime};
use uuid::{Builder, Uuid};

type TestResult = Result<(), Box<dyn Error>>;

const TOTAL: i64 = 10_000;

fn at(seconds: i64) -> Result<OffsetDateTime, Box<dyn Error>> {
    let unix = 1_700_000_000_i64
        .checked_add(seconds)
        .ok_or("timestamp overflow")?;
    Ok(OffsetDateTime::from_unix_timestamp(unix)?)
}

fn money(amount: i64) -> Result<Money, Box<dyn Error>> {
    Ok(Money::new(amount, Currency::TRY)?)
}

fn payment_id(seed: u8) -> Result<PaymentId, Box<dyn Error>> {
    let uuid = Builder::from_unix_timestamp_millis(1_700_000_000_000, &[seed; 10]).into_uuid();
    Ok(PaymentId::from_uuid(uuid)?)
}

fn new_payment(seed: u8) -> Result<Payment, Box<dyn Error>> {
    Ok(Payment::new(
        payment_id(seed)?,
        MerchantId::from_uuid(Uuid::from_u128(42)),
        money(TOTAL)?,
        at(0)?,
    ))
}

/// Komutlarla ulaşılabilen her durum ve her `Unknown` işlem türü; her biri
/// farklı id ile.
fn reachable_payments() -> Result<Vec<Payment>, Box<dyn Error>> {
    let mut payments = Vec::new();

    payments.push(new_payment(1)?);

    let mut p = new_payment(2)?;
    p.require_action(at(1)?)?;
    payments.push(p);

    let mut p = new_payment(3)?;
    p.authorize(at(1)?)?;
    payments.push(p);

    let mut p = new_payment(4)?;
    p.fail(at(1)?)?;
    payments.push(p);

    let mut p = new_payment(5)?;
    p.authorize(at(1)?)?;
    p.capture(at(2)?)?;
    payments.push(p);

    let mut p = new_payment(6)?;
    p.authorize(at(1)?)?;
    p.void(at(2)?)?;
    payments.push(p);

    let mut p = new_payment(7)?;
    p.authorize(at(1)?)?;
    p.capture(at(2)?)?;
    p.refund(money(4_000)?, at(3)?)?;
    payments.push(p);

    let mut p = new_payment(8)?;
    p.authorize(at(1)?)?;
    p.capture(at(2)?)?;
    p.refund(money(TOTAL)?, at(3)?)?;
    payments.push(p);

    let mut p = new_payment(9)?;
    p.authorize(at(1)?)?;
    p.capture(at(2)?)?;
    p.dispute(at(3)?)?;
    payments.push(p);

    let mut p = new_payment(10)?;
    p.require_action(at(1)?)?;
    p.mark_unknown(PendingOperation::Authorize, at(2)?)?;
    payments.push(p);

    let mut p = new_payment(11)?;
    p.authorize(at(1)?)?;
    p.mark_unknown(PendingOperation::Capture, at(2)?)?;
    payments.push(p);

    let mut p = new_payment(12)?;
    p.authorize(at(1)?)?;
    p.mark_unknown(PendingOperation::Void, at(2)?)?;
    payments.push(p);

    let mut p = new_payment(13)?;
    p.authorize(at(1)?)?;
    p.capture(at(2)?)?;
    p.refund(money(4_000)?, at(3)?)?;
    p.mark_unknown(PendingOperation::Refund(money(6_000)?), at(4)?)?;
    payments.push(p);

    Ok(payments)
}

async fn load(conn: &mut PgConnection, id: PaymentId) -> Result<Payment, Box<dyn Error>> {
    Ok(PaymentRepository::find_by_id(conn, id)
        .await?
        .ok_or("payment not found")?)
}

// --- Round-trip ---

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn every_reachable_payment_round_trips(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let payments = reachable_payments()?;
    for status in PaymentStatus::ALL {
        assert!(
            payments.iter().any(|p| p.status() == status),
            "{status} is not covered"
        );
    }

    for payment in &payments {
        PaymentRepository::insert(&mut conn, payment).await?;
    }
    for payment in &payments {
        assert_eq!(&load(&mut conn, payment.id()).await?, payment);
    }
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn sub_microsecond_timestamps_round_trip(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let created = at(0)?
        .checked_add(Duration::nanoseconds(123_456_789))
        .ok_or("timestamp overflow")?;
    let payment = Payment::new(
        payment_id(1)?,
        MerchantId::from_uuid(Uuid::from_u128(42)),
        money(TOTAL)?,
        created,
    );

    PaymentRepository::insert(&mut conn, &payment).await?;

    assert_eq!(load(&mut conn, payment.id()).await?, payment);
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn find_by_id_returns_none_for_missing_payment(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;

    let found = PaymentRepository::find_by_id(&mut conn, payment_id(1)?).await?;

    assert_eq!(found, None);
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn inserting_same_id_twice_is_rejected(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let payment = new_payment(1)?;
    PaymentRepository::insert(&mut conn, &payment).await?;

    let result = PaymentRepository::insert(&mut conn, &payment).await;

    assert!(
        matches!(result, Err(RepositoryError::AlreadyExists(id)) if id == payment.id()),
        "{result:?}"
    );
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn repository_works_inside_a_transaction(pool: PgPool) -> TestResult {
    let payment = new_payment(1)?;

    let mut tx = pool.begin().await?;
    PaymentRepository::insert(&mut tx, &payment).await?;
    assert_eq!(load(&mut tx, payment.id()).await?, payment);
    tx.rollback().await?;

    let mut conn = pool.acquire().await?;
    assert_eq!(
        PaymentRepository::find_by_id(&mut conn, payment.id()).await?,
        None
    );
    Ok(())
}

// --- Update ve optimistic locking ---

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn update_persists_changes_and_keeps_immutable_columns(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let original = new_payment(1)?;
    PaymentRepository::insert(&mut conn, &original).await?;

    let mut payment = load(&mut conn, original.id()).await?;
    payment.authorize(at(1)?)?;
    payment.capture(at(2)?)?;
    payment.refund(money(4_000)?, at(3)?)?;
    payment.mark_unknown(PendingOperation::Refund(money(1_000)?), at(4)?)?;
    PaymentRepository::update(&mut conn, &payment, original.version()).await?;

    let stored = load(&mut conn, original.id()).await?;
    assert_eq!(stored, payment);
    assert_eq!(stored.version(), 5);

    let row = sqlx::query(
        "SELECT merchant_id, amount_minor, currency, created_at FROM payments WHERE id = $1",
    )
    .bind(original.id().as_uuid())
    .fetch_one(&mut *conn)
    .await?;
    assert_eq!(
        row.try_get::<Uuid, _>("merchant_id")?,
        *original.merchant_id().as_uuid()
    );
    assert_eq!(row.try_get::<i64, _>("amount_minor")?, TOTAL);
    assert_eq!(row.try_get::<&str, _>("currency")?, "TRY");
    assert_eq!(
        row.try_get::<OffsetDateTime, _>("created_at")?,
        original.created_at()
    );
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn concurrent_update_with_stale_version_conflicts(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let payment = new_payment(1)?;
    PaymentRepository::insert(&mut conn, &payment).await?;

    let mut first = load(&mut conn, payment.id()).await?;
    let mut second = load(&mut conn, payment.id()).await?;
    first.authorize(at(1)?)?;
    second.fail(at(1)?)?;

    PaymentRepository::update(&mut conn, &first, 1).await?;
    let result = PaymentRepository::update(&mut conn, &second, 1).await;

    assert!(
        matches!(
            result,
            Err(RepositoryError::VersionConflict { id, expected: 1, actual: 2 })
                if id == payment.id()
        ),
        "{result:?}"
    );
    let stored = load(&mut conn, payment.id()).await?;
    assert_eq!(stored, first);
    assert_eq!(stored.status(), PaymentStatus::Authorized);
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn updating_missing_payment_is_not_found(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let mut payment = new_payment(1)?;
    payment.authorize(at(1)?)?;

    let result = PaymentRepository::update(&mut conn, &payment, 1).await;

    assert!(
        matches!(result, Err(RepositoryError::NotFound(id)) if id == payment.id()),
        "{result:?}"
    );
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn updating_unchanged_payment_is_rejected(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let payment = new_payment(1)?;
    PaymentRepository::insert(&mut conn, &payment).await?;

    let result = PaymentRepository::update(&mut conn, &payment, payment.version()).await;

    assert!(
        matches!(
            result,
            Err(RepositoryError::NothingToUpdate { expected: 1, .. })
        ),
        "{result:?}"
    );
    Ok(())
}

// --- Veritabanı savunması ---

async fn insert_raw(
    conn: &mut PgConnection,
    status: &str,
    amount_minor: i64,
    refunded_minor: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO payments (
             id, merchant_id, amount_minor, currency, status, refunded_minor,
             version, created_at, updated_at
         ) VALUES ($1, $2, $3, 'TRY', $4, $5, 1, now(), now())",
    )
    .bind(Uuid::from_u128(7))
    .bind(Uuid::from_u128(42))
    .bind(amount_minor)
    .bind(status)
    .bind(refunded_minor)
    .execute(conn)
    .await
    .map(|_| ())
}

fn is_check_violation(result: &Result<(), sqlx::Error>) -> bool {
    matches!(result, Err(sqlx::Error::Database(err)) if err.is_check_violation())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn check_constraints_reject_invalid_rows(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;

    let negative = insert_raw(&mut conn, "created", -1, 0).await;
    assert!(is_check_violation(&negative), "{negative:?}");

    let unknown_without_pending = insert_raw(&mut conn, "unknown", TOTAL, 0).await;
    assert!(
        is_check_violation(&unknown_without_pending),
        "{unknown_without_pending:?}"
    );

    let unknown_status = insert_raw(&mut conn, "settled", TOTAL, 0).await;
    assert!(is_check_violation(&unknown_status), "{unknown_status:?}");
    Ok(())
}

#[sqlx::test(migrator = "payos_db::MIGRATOR")]
async fn domain_inconsistent_row_is_reported_as_corrupt(pool: PgPool) -> TestResult {
    let mut conn = pool.acquire().await?;
    let id = payment_id(1)?;
    sqlx::query(
        "INSERT INTO payments (
             id, merchant_id, amount_minor, currency, status, refunded_minor,
             version, created_at, updated_at
         ) VALUES ($1, $2, $3, 'TRY', 'refunded', 0, 1, now(), now())",
    )
    .bind(id.as_uuid())
    .bind(Uuid::from_u128(42))
    .bind(TOTAL)
    .execute(&mut *conn)
    .await?;

    let result = PaymentRepository::find_by_id(&mut conn, id).await;

    assert!(
        matches!(result, Err(RepositoryError::CorruptRow(_))),
        "{result:?}"
    );
    Ok(())
}
