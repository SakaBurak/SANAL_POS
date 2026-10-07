//! PostgreSQL erişimi ve migration'lar.

pub mod error;
pub mod payment_repository;

pub use error::RepositoryError;
pub use payment_repository::PaymentRepository;

/// `migrations/` klasörü derleme sırasında ikiliye gömülür.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
