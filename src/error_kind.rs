#[derive(thiserror::Error, Debug)]
pub enum SandboxError {
    #[error("{0}")]
    SandboxConfigError(#[from] SandboxConfigError),

    #[error("{0}")]
    TcpError(#[from] TcpError),

    #[error("Error while performing r/w operations on the file: {0}")]
    FileError(std::io::Error),

    #[error("Runtime error: {0}")]
    RuntimeError(std::io::Error),

    #[error("Timeout: Sandbox didn't start within provided timeout")]
    TimeoutError,

    #[error("Error resolving binary: {0}")]
    BinaryError(String),

    #[error("Download error: {0}")]
    DownloadError(String),

    #[error("Install error: {0}")]
    InstallError(String),

    #[error("Verification error: {0}")]
    SandboxVerificationError(String),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatformError(String),
}

#[derive(thiserror::Error, Debug)]
pub enum SandboxRpcError {
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Unexpected response from the RPC")]
    UnexpectedResponse,

    #[error("Sandbox RPC error: {0}")]
    SandboxRpcError(String),
}

#[derive(thiserror::Error, Debug)]
pub enum TcpError {
    #[error("Error while binding listener to a port {0}: {1}")]
    BindError(u16, std::io::Error),

    #[error("Error while getting local address: {0}")]
    LocalAddrError(std::io::Error),

    #[error("Error while locking port file: {0}")]
    LockingError(std::io::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum SandboxConfigError {
    #[error("Error while performing r/w on config file: {0}")]
    FileError(std::io::Error),

    #[error("Error while parsing config file: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Invalid environment variables: {0}")]
    EnvParseError(String),
}
