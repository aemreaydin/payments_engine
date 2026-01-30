use thiserror::Error;

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
