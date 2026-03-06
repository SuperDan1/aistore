use crate::heap::HeapError;
use std::fmt;

#[derive(Debug)]
pub enum MvccError {
    RowNotVisible,
    RowAlreadyDeleted,
    RowNotFound,
    UndoNotFound,
    TransactionNotFound,
    TransactionNotActive,
    Other(String),
}

impl fmt::Display for MvccError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MvccError::RowNotVisible => write!(f, "Row not visible to transaction"),
            MvccError::RowAlreadyDeleted => write!(f, "Row already deleted"),
            MvccError::RowNotFound => write!(f, "Row not found"),
            MvccError::UndoNotFound => write!(f, "Undo record not found"),
            MvccError::TransactionNotFound => write!(f, "Transaction not found"),
            MvccError::TransactionNotActive => write!(f, "Transaction not active"),
            MvccError::Other(msg) => write!(f, "MVCC error: {}", msg),
        }
    }
}

impl std::error::Error for MvccError {}

impl From<HeapError> for MvccError {
    fn from(e: HeapError) -> Self {
        MvccError::Other(e.to_string())
    }
}

pub type MvccResult<T> = Result<T, MvccError>;
