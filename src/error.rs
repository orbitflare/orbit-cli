use thiserror::Error;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Yaml(#[from] serde_yml::Error),

    #[error("{0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("{0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("{0}")]
    Keyring(#[from] keyring::Error),

    #[error("{method} {endpoint} — HTTP {status}: {message}")]
    Api {
        method: String,
        endpoint: String,
        status: u16,
        message: String,
    },

    #[error("wallet_not_linked")]
    WalletNotLinked,

    #[error(transparent)]
    Sdk(#[from] orbitflare_sdk::Error),

    #[error(transparent)]
    Apex(#[from] orbitflare_apex::Error),

    #[error(transparent)]
    ApexRpc(#[from] orbitflare_apex::rpc::RpcError),

    #[error("{0}")]
    Other(String),
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError::Other(s)
    }
}

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        CliError::Other(s.to_string())
    }
}
