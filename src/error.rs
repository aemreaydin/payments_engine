use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("account {0} is frozen")]
    AccountFrozen(u16),

    #[error("missing amount for transaction {0}")]
    MissingAmount(u32),

    #[error("duplicate transaction id {0}")]
    DuplicateTransaction(u32),

    #[error("invalid amount {1} for transaction {0}")]
    InvalidAmount(u32, Decimal),

    #[error("insufficient funds for client {0}: need {1}, have {2}")]
    InsufficientFunds(u16, Decimal, Decimal),

    #[error("transaction {0} not found")]
    TransactionNotFound(u32),

    #[error("transaction {0} is already under dispute")]
    AlreadyUnderDispute(u32),

    #[error("transaction {0} is not under dispute")]
    NotUnderDispute(u32),

    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
