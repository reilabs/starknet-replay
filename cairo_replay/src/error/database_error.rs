use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DatabaseError {
    #[error("error communicating with Pathfinder database")]
    Error(String),
}

impl From<anyhow::Error> for DatabaseError {
    fn from(value: anyhow::Error) -> Self {
        DatabaseError::Error(value.to_string())
    }
}
