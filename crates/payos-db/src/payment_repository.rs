use std::fmt;

use payos_core::{Currency, Money};
use payos_domain::{
    MerchantId, Payment, PaymentId, PaymentSnapshot, PaymentStatus, PendingOperation,
    UnknownSnapshot,
};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::error::RepositoryError;

/// `payments` tablosuna erişim. Metotlar `&mut PgConnection` aldığı için hem
/// havuzdan alınan bağlantıyla hem de bir transaction içinde (`&mut *tx`)
/// kullanılabilir.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaymentRepository;

impl PaymentRepository {
    pub async fn insert(conn: &mut PgConnection, payment: &Payment) -> Result<(), RepositoryError> {
        let snapshot = payment.snapshot();
        let pending = PendingColumns::encode(snapshot.unknown);
        let result = sqlx::query(
            "INSERT INTO payments (
                 id, merchant_id, amount_minor, currency, status, refunded_minor,
                 pending_operation, pending_refund_minor, unknown_previous_status,
                 version, created_at, updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(snapshot.id.as_uuid())
        .bind(snapshot.merchant_id.as_uuid())
        .bind(snapshot.amount.amount().value())
        .bind(snapshot.amount.currency().code())
        .bind(snapshot.status.as_str())
        .bind(snapshot.refunded.amount().value())
        .bind(pending.operation)
        .bind(pending.refund_minor)
        .bind(pending.previous_status)
        .bind(i64::from(snapshot.version))
        .bind(snapshot.created_at)
        .bind(snapshot.updated_at)
        .execute(conn)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
                Err(RepositoryError::AlreadyExists(snapshot.id))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn find_by_id(
        conn: &mut PgConnection,
        id: PaymentId,
    ) -> Result<Option<Payment>, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, merchant_id, amount_minor, currency, status, refunded_minor,
                    pending_operation, pending_refund_minor, unknown_previous_status,
                    version, created_at, updated_at
               FROM payments
              WHERE id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(conn)
        .await?;

        row.as_ref().map(payment_from_row).transpose()
    }

    /// Ödemeyi yalnızca DB'deki sürüm hâlâ `expected_version` ise yazar
    /// (optimistic locking). Değişmeyen alanlar (`merchant_id`, tutar, para
    /// birimi, `created_at`) güncellenmez.
    pub async fn update(
        conn: &mut PgConnection,
        payment: &Payment,
        expected_version: u32,
    ) -> Result<(), RepositoryError> {
        let snapshot = payment.snapshot();
        if snapshot.version <= expected_version {
            return Err(RepositoryError::NothingToUpdate {
                id: snapshot.id,
                expected: expected_version,
            });
        }
        let pending = PendingColumns::encode(snapshot.unknown);
        let result = sqlx::query(
            "UPDATE payments
                SET status = $3,
                    refunded_minor = $4,
                    pending_operation = $5,
                    pending_refund_minor = $6,
                    unknown_previous_status = $7,
                    version = $8,
                    updated_at = $9
              WHERE id = $1 AND version = $2",
        )
        .bind(snapshot.id.as_uuid())
        .bind(i64::from(expected_version))
        .bind(snapshot.status.as_str())
        .bind(snapshot.refunded.amount().value())
        .bind(pending.operation)
        .bind(pending.refund_minor)
        .bind(pending.previous_status)
        .bind(i64::from(snapshot.version))
        .bind(snapshot.updated_at)
        .execute(&mut *conn)
        .await?;

        if result.rows_affected() > 0 {
            return Ok(());
        }

        let actual: Option<i64> = sqlx::query_scalar("SELECT version FROM payments WHERE id = $1")
            .bind(snapshot.id.as_uuid())
            .fetch_optional(&mut *conn)
            .await?;
        match actual {
            None => Err(RepositoryError::NotFound(snapshot.id)),
            Some(actual) => Err(RepositoryError::VersionConflict {
                id: snapshot.id,
                expected: expected_version,
                actual: u32::try_from(actual).map_err(corrupt)?,
            }),
        }
    }
}

/// `Unknown` bağlamının sütun karşılığı. Bu kodlama bir persistence detayıdır.
struct PendingColumns {
    operation: Option<&'static str>,
    refund_minor: Option<i64>,
    previous_status: Option<&'static str>,
}

impl PendingColumns {
    fn encode(unknown: Option<UnknownSnapshot>) -> Self {
        let Some(ctx) = unknown else {
            return Self {
                operation: None,
                refund_minor: None,
                previous_status: None,
            };
        };
        let (operation, refund_minor) = match ctx.operation {
            PendingOperation::Authorize => ("authorize", None),
            PendingOperation::Capture => ("capture", None),
            PendingOperation::Void => ("void", None),
            PendingOperation::Refund(amount) => ("refund", Some(amount.amount().value())),
        };
        Self {
            operation: Some(operation),
            refund_minor,
            previous_status: Some(ctx.previous_status.as_str()),
        }
    }
}

fn decode_unknown(
    operation: Option<&str>,
    refund_minor: Option<i64>,
    previous_status: Option<&str>,
    currency: Currency,
) -> Result<Option<UnknownSnapshot>, RepositoryError> {
    let (operation, previous_status) = match (operation, previous_status) {
        (None, None) if refund_minor.is_none() => return Ok(None),
        (Some(operation), Some(previous_status)) => (operation, previous_status),
        _ => return Err(corrupt("incomplete pending operation columns")),
    };
    let operation = match (operation, refund_minor) {
        ("authorize", None) => PendingOperation::Authorize,
        ("capture", None) => PendingOperation::Capture,
        ("void", None) => PendingOperation::Void,
        ("refund", Some(minor)) => {
            PendingOperation::Refund(Money::new(minor, currency).map_err(corrupt)?)
        }
        (other, _) => return Err(corrupt(format!("invalid pending operation `{other}`"))),
    };
    Ok(Some(UnknownSnapshot {
        operation,
        previous_status: previous_status.parse::<PaymentStatus>().map_err(corrupt)?,
    }))
}

fn payment_from_row(row: &PgRow) -> Result<Payment, RepositoryError> {
    let id = PaymentId::from_uuid(row.try_get::<Uuid, _>("id")?).map_err(corrupt)?;
    let merchant_id = MerchantId::from_uuid(row.try_get::<Uuid, _>("merchant_id")?);
    let currency = row
        .try_get::<&str, _>("currency")?
        .parse::<Currency>()
        .map_err(corrupt)?;
    let amount = Money::new(row.try_get("amount_minor")?, currency).map_err(corrupt)?;
    let refunded = Money::new(row.try_get("refunded_minor")?, currency).map_err(corrupt)?;
    let status = row
        .try_get::<&str, _>("status")?
        .parse::<PaymentStatus>()
        .map_err(corrupt)?;
    let unknown = decode_unknown(
        row.try_get("pending_operation")?,
        row.try_get("pending_refund_minor")?,
        row.try_get("unknown_previous_status")?,
        currency,
    )?;
    let version = u32::try_from(row.try_get::<i64, _>("version")?).map_err(corrupt)?;

    Payment::restore(PaymentSnapshot {
        id,
        merchant_id,
        amount,
        status,
        refunded,
        unknown,
        version,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
    .map_err(|err| corrupt(format!("payment {id}: {err}")))
}

fn corrupt(reason: impl fmt::Display) -> RepositoryError {
    RepositoryError::CorruptRow(reason.to_string())
}
