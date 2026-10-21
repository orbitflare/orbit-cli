use crate::error::CliResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub endpoints: EndpointsConfig,
    #[serde(default)]
    pub preferences: PreferencesConfig,
    #[serde(default)]
    pub templates: TemplatesConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, AuthProfile>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AuthProfile {
    #[serde(default)]
    pub auth_method: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub wallet_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_network")]
    pub default: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub region: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            default: default_network(),
            region: String::new(),
        }
    }
}

fn default_network() -> String {
    "mainnet".into()
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EndpointsConfig {
    #[serde(default)]
    pub rpc: RpcEndpoint,
    #[serde(default)]
    pub jetstream: SimpleEndpoint,
    #[serde(default)]
    pub grpc: GrpcEndpoint,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SimpleEndpoint {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub fallback_url: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcEndpoint {
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_rpc_devnet_url")]
    pub devnet_url: String,
    #[serde(default)]
    pub devnet_fallback_url: Vec<String>,
    #[serde(default)]
    pub fallback_url: Vec<String>,
}

impl Default for RpcEndpoint {
    fn default() -> Self {
        Self {
            url: String::new(),
            devnet_url: default_rpc_devnet_url(),
            devnet_fallback_url: vec!["https://api.devnet.solana.com".into()],
            fallback_url: vec![],
        }
    }
}

fn default_rpc_devnet_url() -> String {
    crate::regions::DEVNET_RPC_URL.into()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrpcEndpoint {
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_grpc_devnet_url")]
    pub devnet_url: String,
    #[serde(default)]
    pub devnet_fallback_url: Vec<String>,
    #[serde(default)]
    pub fallback_url: Vec<String>,
}

impl Default for GrpcEndpoint {
    fn default() -> Self {
        Self {
            url: String::new(),
            devnet_url: default_grpc_devnet_url(),
            devnet_fallback_url: vec![],
            fallback_url: vec![],
        }
    }
}

fn default_grpc_devnet_url() -> String {
    crate::regions::DEVNET_GRPC_URL.into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreferencesConfig {
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default = "default_true")]
    pub color: bool,
    #[serde(default = "default_commitment")]
    pub commitment: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for PreferencesConfig {
    fn default() -> Self {
        Self {
            output_format: default_output_format(),
            color: true,
            commitment: default_commitment(),
            theme: default_theme(),
        }
    }
}

fn default_theme() -> String {
    "ocean".into()
}

fn default_output_format() -> String {
    "pretty".into()
}
fn default_true() -> bool {
    true
}
fn default_commitment() -> String {
    "confirmed".into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplatesConfig {
    #[serde(default = "default_templates_repo")]
    pub repo: String,
}

impl Default for TemplatesConfig {
    fn default() -> Self {
        Self {
            repo: default_templates_repo(),
        }
    }
}

fn default_templates_repo() -> String {
    "https://github.com/orbitflare/templates".into()
}

impl Config {
    pub fn dir() -> PathBuf {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".orbitflare")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.yml")
    }

    pub fn load() -> CliResult<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = serde_yml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self) -> CliResult<()> {
        let dir = Self::dir();
        std::fs::create_dir_all(&dir)?;
        let contents = serde_yml::to_string(self)?;
        std::fs::write(Self::path(), contents)?;
        Ok(())
    }
}
