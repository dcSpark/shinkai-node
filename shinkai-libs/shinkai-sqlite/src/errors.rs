use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqliteManagerError {
    #[error("Tool already exists with key: {0}")]
    ToolAlreadyExists(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    #[error("Embedding generation error: {0}")]
    EmbeddingGenerationError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Tool not found with key: {0}")]
    ToolNotFound(String),
    #[error("ToolPlayground already exists with job_id: {0}")]
    ToolPlaygroundAlreadyExists(String),
    #[error("ToolPlayground not found with job_id: {0}")]
    ToolPlaygroundNotFound(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Tool offering not found with key: {0}")]
    ToolOfferingNotFound(String),
    #[error("DateTime parse error: {0}")]
    DateTimeParseError(String),
    #[error("Subscription not found with id: {0}")]
    SubscriptionNotFound(String),
    #[error("Wallet manager not found")]
    WalletManagerNotFound,
    #[error("Data not found")]
    DataNotFound,
    #[error("Data already exists")]
    DataAlreadyExists,
    #[error("Invalid identity name: {0}")]
    InvalidIdentityName(String),
    #[error("Invoice not found with id: {0}")]
    InvoiceNotFound(String),
    #[error("Network error not found with id: {0}")]
    InvoiceNetworkErrorNotFound(String),
    #[error("Profile does not exist: {0}")]
    ProfileNotFound(String),
    #[error("Profile name already exists")]
    ProfileNameAlreadyExists,
    #[error("Invalid profile name: {0}")]
    InvalidProfileName(String),
    #[error("Invalid attribute name: {0}")]
    InvalidAttributeName(String),
    #[error("Registration code does not exist")]
    CodeNonExistent,
    #[error("Registration code already used")]
    CodeAlreadyUsed,
    #[error("Error: {0}")]
    SomeError(String),
    #[error("Missing value: {0}")]
    MissingValue(String),
    #[error("Inbox not found: {0}")]
    InboxNotFound(String),
    #[error("Lock error")]
    LockError,
    #[error("Invalid data")]
    InvalidData,
    #[error("Failed fetching value")]
    FailedFetchingValue,
    #[error("Query error: {query}, source: {source}")]
    QueryError {
        query: String,
        source: rusqlite::Error,
    },
    #[error("Directory not empty")]
    DirectoryNotEmpty,
    #[error("Directory not found")]
    DirectoryNotFound,
    #[error("Unsupported embedding length: {0}")]
    UnsupportedEmbeddingLength(usize),
    #[error("Deserialization error")]
    DeserializationError,
    #[error("Chrono parse error: {0}")]
    ChronoParseError(chrono::ParseError),
    #[error("Version Converson Error: {0}")]
    VersionConversionError(String),
    #[error("Tool key not found: {0}")]
    ToolKeyNotFound(String),
    #[error("Version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },
    #[error("Version parse error: {0}")]
    VersionParseError(String),
    // Add other error variants as needed
}

impl From<&str> for SqliteManagerError {
    fn from(err: &str) -> SqliteManagerError {
        SqliteManagerError::SomeError(err.to_string())
    }
}

impl From<chrono::ParseError> for SqliteManagerError {
    fn from(err: chrono::ParseError) -> SqliteManagerError {
        SqliteManagerError::ChronoParseError(err)
    }
}

impl From<String> for SqliteManagerError {
    fn from(err: String) -> SqliteManagerError {
        SqliteManagerError::SomeError(err)
    }
}
