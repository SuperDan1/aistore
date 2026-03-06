use std::fmt;

#[derive(Debug)]
pub enum UndoError {
    Io(std::io::Error),
    SegmentNotFound(u32),
    SegmentFull,
    OffsetOutOfBounds,
    CorruptedRecord,
    Other(String),
}

impl fmt::Display for UndoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UndoError::Io(e) => write!(f, "IO error: {}", e),
            UndoError::SegmentNotFound(id) => write!(f, "Undo segment {} not found", id),
            UndoError::SegmentFull => write!(f, "Undo segment is full"),
            UndoError::OffsetOutOfBounds => write!(f, "Offset out of bounds"),
            UndoError::CorruptedRecord => write!(f, "Corrupted undo record"),
            UndoError::Other(msg) => write!(f, "Undo error: {}", msg),
        }
    }
}

impl std::error::Error for UndoError {}

impl From<std::io::Error> for UndoError {
    fn from(err: std::io::Error) -> Self {
        UndoError::Io(err)
    }
}

pub type UndoResult<T> = Result<T, UndoError>;
