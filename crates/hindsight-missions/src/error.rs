use std::fmt;

#[derive(Debug)]
pub enum MissionError {
    Rejected(String),
    Storage(String),
    Query(String),
    NotFound(String),
}

impl fmt::Display for MissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MissionError::Rejected(msg) => write!(f, "fact rejected: {}", msg),
            MissionError::Storage(msg) => write!(f, "storage error: {}", msg),
            MissionError::Query(msg) => write!(f, "query error: {}", msg),
            MissionError::NotFound(msg) => write!(f, "not found: {}", msg),
        }
    }
}

impl std::error::Error for MissionError {}
