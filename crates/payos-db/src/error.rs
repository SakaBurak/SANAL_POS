use payos_domain::PaymentId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("payment {0} already exists")]
    AlreadyExists(PaymentId),
    #[error("payment {0} not found")]
    NotFound(PaymentId),
    /// Ödeme okunduktan sonra başka bir yazma tarafından değiştirilmiş.
    #[error("payment {id} version conflict: expected {expected}, actual {actual}")]
    VersionConflict {
        id: PaymentId,
        expected: u32,
        actual: u32,
    },
    /// Ödemenin sürümü beklenen sürümden büyük değil; yazılacak değişiklik yok.
    #[error("payment {id} has no changes after version {expected}")]
    NothingToUpdate { id: PaymentId, expected: u32 },
    /// Satır veritabanı kısıtlarını geçmiş ama domain kurallarına uymuyor.
    #[error("corrupt payment row: {0}")]
    CorruptRow(String),
}
