use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("error communicating with Pathfinder database")]
    Unknown(String),
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Error::Unknown(value.to_string())
    }
}
