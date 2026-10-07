use payos_core::{Money, MoneyError};
use time::OffsetDateTime;

use crate::error::PaymentError;
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
        let version = self.check_command(to)?;
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
        let version = self.check_command(PaymentStatus::Unknown)?;
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
        let version = self.next_version()?;
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
        let version = self.check_command(to)?;
        self.apply(to, version, now);
        Ok(())
    }

    /// `Unknown` durumundayken hiçbir komut kabul edilmez; çıkış yalnızca
    /// `resolve_unknown` ile olur.
    fn check_command(&self, to: PaymentStatus) -> Result<u32, PaymentError> {
        if self.status == PaymentStatus::Unknown || !self.status.can_transition_to(to) {
            return Err(PaymentError::InvalidTransition {
                from: self.status,
                to,
            });
        }
        self.next_version()
    }

    fn next_version(&self) -> Result<u32, PaymentError> {
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
        self.updated_at = now;
    }
}
