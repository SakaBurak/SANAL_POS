use payos_core::{Money, MoneyError};
use time::{OffsetDateTime, UtcOffset};

use crate::error::{PaymentError, RestoreError};
use crate::merchant_id::MerchantId;
use crate::payment_id::PaymentId;
use crate::status::PaymentStatus;

/// Banka çağrısının sonucu belirsiz kaldığında hangi işlemin beklediği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingOperation {
    Authorize,
    Capture,
    Void,
    Refund(Money),
}

/// Belirsiz bir işlemin sorgu veya mutabakatla öğrenilen sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankOutcome {
    Approved,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnknownContext {
    operation: PendingOperation,
    previous: PaymentStatus,
}

/// `Unknown` durumundaki bir ödemenin bekleyen işlemi ve önceki durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownSnapshot {
    pub operation: PendingOperation,
    pub previous_status: PaymentStatus,
}

/// `Payment`'ın kalıcı katman için düz hali. Bir snapshot'tan `Payment`
/// yalnızca `Payment::restore` ile, domain kuralları yeniden doğrulanarak elde
/// edilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaymentSnapshot {
    pub id: PaymentId,
    pub merchant_id: MerchantId,
    pub amount: Money,
    pub status: PaymentStatus,
    pub refunded: Money,
    pub unknown: Option<UnknownSnapshot>,
    pub version: u32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Ödeme aggregate'i. Durum yalnızca komut metotlarıyla değişir; başarısız bir
/// komut nesneyi değiştirmez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payment {
    id: PaymentId,
    merchant_id: MerchantId,
    amount: Money,
    status: PaymentStatus,
    refunded: Money,
    unknown: Option<UnknownContext>,
    version: u32,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl Payment {
    pub fn new(id: PaymentId, merchant_id: MerchantId, amount: Money, now: OffsetDateTime) -> Self {
        let now = normalize_time(now);
        Self {
            id,
            merchant_id,
            amount,
            status: PaymentStatus::Created,
            refunded: Money::zero(amount.currency()),
            unknown: None,
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    /// Kalıcı kayıttan ödemeyi yeniden kurar. Komutlarla ulaşılamayacak bir
    /// durum (tutarsız iade, eksik `Unknown` bağlamı vb.) reddedilir.
    pub fn restore(snapshot: PaymentSnapshot) -> Result<Self, RestoreError> {
        if snapshot.version == 0 {
            return Err(RestoreError::InvalidVersion);
        }
        let created_at = normalize_time(snapshot.created_at);
        let updated_at = normalize_time(snapshot.updated_at);
        if updated_at < created_at {
            return Err(RestoreError::UpdatedBeforeCreated);
        }
        let amount = snapshot.amount;
        let refunded = snapshot.refunded;
        if refunded.currency() != amount.currency() {
            return Err(RestoreError::CurrencyMismatch);
        }
        let remaining = amount
            .checked_sub(refunded)
            .map_err(|_| RestoreError::RefundExceedsAmount)?;

        let unknown = match (snapshot.status, snapshot.unknown) {
            (PaymentStatus::Unknown, Some(ctx)) => {
                Some(restore_unknown_context(ctx, amount, refunded)?)
            }
            (PaymentStatus::Unknown, None) | (_, Some(_)) => {
                return Err(RestoreError::UnknownContextMismatch);
            }
            (_, None) => None,
        };

        let effective = unknown.map_or(snapshot.status, |ctx| ctx.previous);
        let consistent = match effective {
            PaymentStatus::Created
            | PaymentStatus::RequiresAction
            | PaymentStatus::Authorized
            | PaymentStatus::Captured
            | PaymentStatus::Failed
            | PaymentStatus::Voided => refunded.is_zero(),
            PaymentStatus::PartiallyRefunded => !refunded.is_zero() && !remaining.is_zero(),
            PaymentStatus::Refunded => !refunded.is_zero() && remaining.is_zero(),
            PaymentStatus::Disputed => refunded.is_zero() || !remaining.is_zero(),
            PaymentStatus::Unknown => false,
        };
        if !consistent {
            return Err(RestoreError::RefundInconsistentWithStatus {
                status: snapshot.status,
            });
        }

        Ok(Self {
            id: snapshot.id,
            merchant_id: snapshot.merchant_id,
            amount,
            status: snapshot.status,
            refunded,
            unknown,
            version: snapshot.version,
            created_at,
            updated_at,
        })
    }

    pub fn snapshot(&self) -> PaymentSnapshot {
        PaymentSnapshot {
            id: self.id,
            merchant_id: self.merchant_id,
            amount: self.amount,
            status: self.status,
            refunded: self.refunded,
            unknown: self.unknown.map(|ctx| UnknownSnapshot {
                operation: ctx.operation,
                previous_status: ctx.previous,
            }),
            version: self.version,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub const fn id(&self) -> PaymentId {
        self.id
    }

    pub const fn merchant_id(&self) -> MerchantId {
        self.merchant_id
    }

    pub const fn amount(&self) -> Money {
        self.amount
    }

    pub const fn status(&self) -> PaymentStatus {
        self.status
    }

    pub const fn refunded(&self) -> Money {
        self.refunded
    }

    pub fn pending_operation(&self) -> Option<PendingOperation> {
        self.unknown.map(|ctx| ctx.operation)
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub const fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    pub fn require_action(&mut self, now: OffsetDateTime) -> Result<(), PaymentError> {
        self.simple_transition(PaymentStatus::RequiresAction, now)
    }

    pub fn authorize(&mut self, now: OffsetDateTime) -> Result<(), PaymentError> {
        self.simple_transition(PaymentStatus::Authorized, now)
    }

    pub fn fail(&mut self, now: OffsetDateTime) -> Result<(), PaymentError> {
        self.simple_transition(PaymentStatus::Failed, now)
    }

    /// Tam tutar capture.
    pub fn capture(&mut self, now: OffsetDateTime) -> Result<(), PaymentError> {
        self.simple_transition(PaymentStatus::Captured, now)
    }

    pub fn void(&mut self, now: OffsetDateTime) -> Result<(), PaymentError> {
        self.simple_transition(PaymentStatus::Voided, now)
    }

    pub fn dispute(&mut self, now: OffsetDateTime) -> Result<(), PaymentError> {
        self.simple_transition(PaymentStatus::Disputed, now)
    }

    pub fn refund(&mut self, amount: Money, now: OffsetDateTime) -> Result<(), PaymentError> {
        if !self.is_refundable() {
            return Err(PaymentError::InvalidTransition {
                from: self.status,
                to: PaymentStatus::Refunded,
            });
        }
        let (to, refunded) = self.refund_outcome(amount)?;
        let version = self.check_command(to, now)?;
        self.refunded = refunded;
        self.apply(to, version, now);
        Ok(())
    }

    /// Banka çağrısı zaman aşımına uğradığında veya yanıt belirsiz kaldığında
    /// çağrılır. `Unknown` durumundayken yalnızca `resolve_unknown` çalışır;
    /// işlem tekrar gönderilemez.
    pub fn mark_unknown(
        &mut self,
        operation: PendingOperation,
        now: OffsetDateTime,
    ) -> Result<(), PaymentError> {
        let allowed = match operation {
            PendingOperation::Authorize => matches!(
                self.status,
                PaymentStatus::Created | PaymentStatus::RequiresAction
            ),
            PendingOperation::Capture | PendingOperation::Void => {
                self.status == PaymentStatus::Authorized
            }
            PendingOperation::Refund(_) => self.is_refundable(),
        };
        if !allowed {
            return Err(PaymentError::InvalidTransition {
                from: self.status,
                to: PaymentStatus::Unknown,
            });
        }
        let version = self.check_command(PaymentStatus::Unknown, now)?;
        if let PendingOperation::Refund(amount) = operation {
            self.refund_outcome(amount)?;
        }
        self.unknown = Some(UnknownContext {
            operation,
            previous: self.status,
        });
        self.apply(PaymentStatus::Unknown, version, now);
        Ok(())
    }

    pub fn resolve_unknown(
        &mut self,
        outcome: BankOutcome,
        now: OffsetDateTime,
    ) -> Result<(), PaymentError> {
        let ctx = match (self.status, self.unknown) {
            (PaymentStatus::Unknown, Some(ctx)) => ctx,
            _ => {
                return Err(PaymentError::NotInUnknownState {
                    status: self.status,
                });
            }
        };

        let mut refunded = self.refunded;
        let to = match (ctx.operation, outcome) {
            (PendingOperation::Authorize, BankOutcome::Approved) => PaymentStatus::Authorized,
            (PendingOperation::Authorize, BankOutcome::Declined) => PaymentStatus::Failed,
            (PendingOperation::Capture, BankOutcome::Approved) => PaymentStatus::Captured,
            (PendingOperation::Void, BankOutcome::Approved) => PaymentStatus::Voided,
            (PendingOperation::Refund(amount), BankOutcome::Approved) => {
                let (to, total) = self.refund_outcome(amount)?;
                refunded = total;
                to
            }
            (
                PendingOperation::Capture | PendingOperation::Void | PendingOperation::Refund(_),
                BankOutcome::Declined,
            ) => ctx.previous,
        };

        if !PaymentStatus::Unknown.can_transition_to(to) {
            return Err(PaymentError::InvalidTransition {
                from: PaymentStatus::Unknown,
                to,
            });
        }
        let version = self.next_version(now)?;
        self.refunded = refunded;
        self.unknown = None;
        self.apply(to, version, now);
        Ok(())
    }

    fn simple_transition(
        &mut self,
        to: PaymentStatus,
        now: OffsetDateTime,
    ) -> Result<(), PaymentError> {
        let version = self.check_command(to, now)?;
        self.apply(to, version, now);
        Ok(())
    }

    /// `Unknown` durumundayken hiçbir komut kabul edilmez; çıkış yalnızca
    /// `resolve_unknown` ile olur.
    fn check_command(&self, to: PaymentStatus, now: OffsetDateTime) -> Result<u32, PaymentError> {
        if self.status == PaymentStatus::Unknown || !self.status.can_transition_to(to) {
            return Err(PaymentError::InvalidTransition {
                from: self.status,
                to,
            });
        }
        self.next_version(now)
    }

    /// `updated_at` geriye gidemez: `now` son güncellemeden önce olamaz.
    fn next_version(&self, now: OffsetDateTime) -> Result<u32, PaymentError> {
        let now = normalize_time(now);
        if now < self.updated_at {
            return Err(PaymentError::TimestampBeforeLastUpdate {
                updated_at: self.updated_at,
                now,
            });
        }
        self.version
            .checked_add(1)
            .ok_or(PaymentError::VersionOverflow)
    }

    const fn is_refundable(&self) -> bool {
        matches!(
            self.status,
            PaymentStatus::Captured | PaymentStatus::PartiallyRefunded
        )
    }

    /// İade sonrası hedef durumu ve yeni toplam iade tutarını hesaplar.
    fn refund_outcome(&self, amount: Money) -> Result<(PaymentStatus, Money), PaymentError> {
        if amount.is_zero() {
            return Err(PaymentError::ZeroRefund);
        }
        let total = self.refunded.checked_add(amount)?;
        let remaining = self.amount.checked_sub(total).map_err(|err| match err {
            MoneyError::InsufficientAmount => PaymentError::RefundExceedsCaptured,
            other => PaymentError::Money(other),
        })?;
        let to = if remaining.is_zero() {
            PaymentStatus::Refunded
        } else {
            PaymentStatus::PartiallyRefunded
        };
        Ok((to, total))
    }

    fn apply(&mut self, to: PaymentStatus, version: u32, now: OffsetDateTime) {
        self.status = to;
        self.version = version;
        self.updated_at = normalize_time(now);
    }
}

/// `mark_unknown`'ın kabul ettiği işlem/önceki durum çiftlerini doğrular.
fn restore_unknown_context(
    ctx: UnknownSnapshot,
    amount: Money,
    refunded: Money,
) -> Result<UnknownContext, RestoreError> {
    let previous = ctx.previous_status;
    let valid = match ctx.operation {
        PendingOperation::Authorize => matches!(
            previous,
            PaymentStatus::Created | PaymentStatus::RequiresAction
        ),
        PendingOperation::Capture | PendingOperation::Void => previous == PaymentStatus::Authorized,
        PendingOperation::Refund(pending) => {
            matches!(
                previous,
                PaymentStatus::Captured | PaymentStatus::PartiallyRefunded
            ) && !pending.is_zero()
                && refunded
                    .checked_add(pending)
                    .and_then(|total| amount.checked_sub(total))
                    .is_ok()
        }
    };
    if !valid {
        return Err(RestoreError::UnknownContextMismatch);
    }
    Ok(UnknownContext {
        operation: ctx.operation,
        previous,
    })
}

/// Domain zaman çözünürlüğü: UTC ve mikrosaniye. PostgreSQL `timestamptz`
/// mikrosaniye tuttuğu için kaydedilip geri okunan zaman bire bir aynı kalır.
fn normalize_time(time: OffsetDateTime) -> OffsetDateTime {
    let utc = time.checked_to_offset(UtcOffset::UTC).unwrap_or(time);
    utc.replace_microsecond(utc.microsecond()).unwrap_or(utc)
}
