use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Failed to read BWS file: {0}")]
    Bws(#[from] bridge_parsers::BridgeError),
    #[error("Failed to parse PBN file: {0}")]
    Pbn(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AnalysisError>;
