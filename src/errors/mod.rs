use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum TransactionError {
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Invalid currency: {0}")]
    InvalidCurrency(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid merchant ID: {0}")]
    InvalidMerchantId(String),
}

#[derive(Error, Debug, PartialEq)]
pub enum RouteError {
    #[error("Invalid PSP configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid fee structure: {0}")]
    InvalidFeeStructure(String),

    #[error("Invalid limits: {0}")]
    InvalidLimits(String),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    FileReadError(String),

    #[error("Failed to parse JSON: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Failed to parse YAML: {0}")]
    YamlParseError(#[from] serde_yaml::Error),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("File watch error: {0}")]
    WatchError(String),
}
