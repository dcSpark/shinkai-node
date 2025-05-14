use chrono::DateTime;
use chrono::Utc;
use dashmap::DashMap;
use ed25519_dalek::VerifyingKey;
use lazy_static::lazy_static;
use shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use shinkai_message_primitives::shinkai_utils::signatures::string_to_signature_public_key;
use shinkai_non_rust_code::functions::get_identity_data::get_identity_data;
use std::fmt;
use std::fs;
use std::net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;
use std::time::{Duration, UNIX_EPOCH};
use tokio::net::lookup_host;
use tokio::task;
use trust_dns_resolver::config::*;
use trust_dns_resolver::TokioAsyncResolver;
use x25519_dalek::PublicKey;

lazy_static! {
    static ref CACHE_TIME: Duration = Duration::from_secs(60 * 30);
    static ref CACHE_NO_UPDATE: Duration = Duration::from_secs(60 * 15);
}

#[derive(Debug)]
pub enum ShinkaiRegistryError {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    CustomError(String),
    SystemTimeError(std::time::SystemTimeError),
    AddressParseError(AddrParseError),
    IdentityNotFound(String),
    IdentityFetchError(String),
}

impl fmt::Display for ShinkaiRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiRegistryError::IoError(err) => write!(f, "IO Error: {}", err),
            ShinkaiRegistryError::JsonError(err) => write!(f, "JSON Error: {}", err),
            ShinkaiRegistryError::CustomError(err) => write!(f, "Custom Error: {}", err),
            ShinkaiRegistryError::SystemTimeError(err) => write!(f, "System Time Error: {}", err),
            ShinkaiRegistryError::AddressParseError(err) => write!(f, "Address Parse Error: {}", err),
            ShinkaiRegistryError::IdentityNotFound(err) => write!(f, "Identity Not Found: {}", err),
            ShinkaiRegistryError::IdentityFetchError(err) => write!(f, "Identity Fetch Error: {}", err),
        }
    }
}

impl From<AddrParseError> for ShinkaiRegistryError {
    fn from(err: AddrParseError) -> ShinkaiRegistryError {
        ShinkaiRegistryError::AddressParseError(err)
    }
}

impl From<std::time::SystemTimeError> for ShinkaiRegistryError {
    fn from(err: std::time::SystemTimeError) -> ShinkaiRegistryError {
        ShinkaiRegistryError::SystemTimeError(err)
    }
}

impl From<serde_json::Error> for ShinkaiRegistryError {
    fn from(err: serde_json::Error) -> ShinkaiRegistryError {
        ShinkaiRegistryError::JsonError(err)
    }
}

impl From<std::io::Error> for ShinkaiRegistryError {
    fn from(err: std::io::Error) -> ShinkaiRegistryError {
        ShinkaiRegistryError::IoError(err)
    }
}

impl std::error::Error for ShinkaiRegistryError {}

#[derive(Debug, PartialEq, Clone)]
pub struct OnchainIdentity {
    pub shinkai_identity: String,
    pub bound_nft: String, // id of the nft
    pub staked_tokens: String,
    pub encryption_key: String,
    pub signature_key: String,
    pub routing: bool,
    pub address_or_proxy_nodes: Vec<String>,
    pub delegated_tokens: String,
    pub last_updated: DateTime<Utc>,
}

impl OnchainIdentity {
    pub async fn first_address(&self) -> Result<SocketAddr, ShinkaiRegistryError> {
        shinkai_log(
            ShinkaiLogOption::CryptoIdentity,
            ShinkaiLogLevel::Info,
            format!(
                "Getting first address for identity: {} with addresses: {:?}",
                self.shinkai_identity, self.address_or_proxy_nodes
            )
            .as_str(),
        );
        let default_value = "localhost:9550";
        let first_address = self
            .address_or_proxy_nodes
            .iter()
            .filter(|addr| !addr.is_empty())
            .collect::<Vec<_>>()
            .first()
            .map_or(default_value, |addr| addr.as_str());
        let address = Self::validate_address(first_address)?;

        // Try to parse the address directly first
        match address.parse::<SocketAddr>() {
            Ok(addr) => Ok(addr),
            Err(_) => {
                // Attempt a normal DNS lookup first
                if let Ok(mut addrs) = lookup_host(address.clone()).await {
                    if let Some(addr) = addrs.next() {
                        return Ok(addr);
                    }
                }

                // Split the address into host and port parts
                let (host, port) = match address.rsplit_once(':') {
                    Some((h, p)) => (h, p.parse().unwrap_or(9552)),
                    None => (address.as_str(), 9552),
                };

                // Configure resolver with Google DNS and relaxed options
                let mut opts = ResolverOpts::default();
                opts.validate = false; // Disable strict validation
                opts.use_hosts_file = false; // Don't check hosts file

                let resolver = TokioAsyncResolver::tokio(
                    ResolverConfig::from_parts(
                        None,
                        vec![],
                        NameServerConfigGroup::from_ips_clear(
                            &[
                                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), // Cloudflare DNS primary
                                IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)), // Cloudflare DNS secondary
                            ],
                            53,
                            true,
                        ),
                    ),
                    opts,
                );

                let resolved_addresses = resolver
                    .lookup_ip(host)
                    .await
                    .map_err(|e| ShinkaiRegistryError::CustomError(format!("DNS resolution error: {}", e)))?;

                resolved_addresses
                    .iter()
                    .next()
                    .map(|ip| SocketAddr::new(ip, port))
                    .ok_or_else(|| ShinkaiRegistryError::CustomError("No address resolved".to_string()))
            }
        }
    }

    fn validate_address(first_address: &str) -> Result<String, ShinkaiRegistryError> {
        let address = first_address.replace("http://", "").replace("https://", "");

        let address = if address.starts_with("localhost:") {
            address.replacen("localhost", "127.0.0.1", 1)
        } else {
            address.to_string()
        };

        // Append default ports if missing
        let address = if !address.contains(':') {
            if first_address.starts_with("https://") {
                format!("{}:443", address)
            } else if first_address.starts_with("http://") {
                format!("{}:80", address)
            } else {
                address
            }
        } else {
            address
        };

        Ok(address)
    }

    pub fn encryption_public_key(&self) -> Result<PublicKey, ShinkaiRegistryError> {
        string_to_encryption_public_key(&self.encryption_key)
            .map_err(|err| ShinkaiRegistryError::CustomError(err.to_string()))
    }

    pub fn signature_verifying_key(&self) -> Result<VerifyingKey, ShinkaiRegistryError> {
        string_to_signature_public_key(&self.signature_key)
            .map_err(|err| ShinkaiRegistryError::CustomError(err.to_string()))
    }
}

pub trait ShinkaiRegistryTrait {
    fn new(url: &str, contract_address: &str, abi_path: &str) -> Result<Self, ShinkaiRegistryError>
    where
        Self: Sized;
    fn get_identity_record(&self, identity: String) -> Result<OnchainIdentity, ShinkaiRegistryError>;
    fn get_cache_time(&self, identity: &str) -> Option<SystemTime>;
}

#[derive(Debug, Clone)]
pub struct ShinkaiRegistry {
    pub cache: Arc<DashMap<String, (SystemTime, OnchainIdentity)>>,
    pub rpc_endpoints: Vec<String>, // TODO: needs to be updated for mainnet -- also depends on the network
    pub abi_file_content: String,
    pub contract_address: String,
}

impl ShinkaiRegistry {
    pub async fn new(
        url: &str,
        contract_address: &str,
        abi_path: Option<String>,
    ) -> Result<Self, ShinkaiRegistryError> {
        let abi_file_content = match abi_path {
            Some(path) => fs::read_to_string(path).map_err(ShinkaiRegistryError::IoError)?,
            None => {
                shinkai_log(
                    ShinkaiLogOption::CryptoIdentity,
                    ShinkaiLogLevel::Info,
                    "Using default ABI",
                );
                include_str!("./abi/ShinkaiRegistry.sol/ShinkaiRegistry.json").to_string()
            }
        };

        let rpc_endpoints = vec![
            url.to_string(),
            "https://sepolia.base.org".to_string(),
            "https://base-sepolia-rpc.publicnode.com".to_string(),
            "https://base-sepolia.gateway.tenderly.co".to_string(),
        ];

        Ok(Self {
            abi_file_content,
            contract_address: contract_address.to_string(),
            cache: Arc::new(DashMap::new()),
            rpc_endpoints,
        })
    }

    pub async fn get_identity_record(
        &self,
        identity: String,
        force_refresh: Option<bool>,
    ) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        let identity = if identity.starts_with("@@") {
            identity.trim_start_matches("@@").to_string()
        } else {
            identity
        };

        let force_refresh = force_refresh.unwrap_or(false);
        let now = SystemTime::now();

        // Skip cache check if force_refresh is true
        if !force_refresh {
            // If the cache is up-to-date, return the cached value
            if let Some(value) = self.cache.get(&identity) {
                let (last_updated, record) = value.value().clone();
                if now.duration_since(last_updated)? < *CACHE_NO_UPDATE {
                    return Ok(record);
                } else if now.duration_since(last_updated)? < *CACHE_TIME {
                    // Spawn a new task to update the cache in the background
                    let identity_clone = identity.clone();
                    let cache_clone = self.cache.clone();
                    let rpc_endpoints_clone = self.rpc_endpoints.clone();
                    let contract_address_clone = self.contract_address.clone();
                    let abi_file_content_clone = self.abi_file_content.clone();
                    task::spawn(async move {
                        if let Err(e) = Self::update_cache(
                            rpc_endpoints_clone,
                            contract_address_clone,
                            abi_file_content_clone,
                            &cache_clone,
                            identity_clone,
                        )
                        .await
                        {
                            // Log the error
                            shinkai_log(
                                ShinkaiLogOption::CryptoIdentity,
                                ShinkaiLogLevel::Error,
                                format!("Error updating cache: {}", e).as_str(),
                            );
                        }
                    });

                    return Ok(record);
                }
            }
        }

        // Otherwise, update the cache
        let record = Self::update_cache(
            self.rpc_endpoints.clone(),
            self.contract_address.clone(),
            self.abi_file_content.clone(),
            &self.cache,
            identity.clone(),
        )
        .await?;
        Ok(record.clone())
    }

    async fn update_cache(
        rpc_endpoints: Vec<String>,
        contract_address: String,
        contract_abi: String,
        cache: &DashMap<String, (SystemTime, OnchainIdentity)>,
        identity: String,
    ) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        // Fetch the identity record from the contract
        let record =
            Self::fetch_identity_record(rpc_endpoints, contract_address, contract_abi, identity.clone()).await?;

        // Update the cache and the timestamp
        cache.insert(identity.clone(), (SystemTime::now(), record.clone()));

        Ok(record)
    }

    pub fn get_cache_time(&self, identity: &str) -> Option<SystemTime> {
        self.cache.get(identity).map(|value| value.value().0)
    }

    pub async fn fetch_identity_record(
        rpc_endpoints: Vec<String>,
        contract_address: String,
        contract_abi: String,
        identity: String,
    ) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        let identity_data = get_identity_data(rpc_endpoints, contract_address, contract_abi, identity.clone())
            .await
            .map_err(|e| ShinkaiRegistryError::IdentityFetchError(e.to_string()))?
            .identity_data;

        if identity_data.is_none() {
            return Err(ShinkaiRegistryError::IdentityNotFound(format!(
                "identity '{}' not found",
                identity.clone()
            )));
        }

        let identity_data = identity_data.unwrap();

        let last_updated = UNIX_EPOCH + Duration::from_secs(identity_data.last_updated);
        let last_updated = DateTime::<Utc>::from(last_updated);

        let onchain_identity = OnchainIdentity {
            shinkai_identity: identity.clone(),
            bound_nft: identity_data.bound_nft,
            staked_tokens: identity_data.staked_tokens,
            encryption_key: identity_data.encryption_key,
            signature_key: identity_data.signature_key,
            routing: identity_data.routing,
            address_or_proxy_nodes: identity_data.address_or_proxy_nodes,
            delegated_tokens: identity_data.delegated_tokens,
            last_updated,
        };

        eprintln!("fetch identity result: {:?}", onchain_identity);

        Ok(onchain_identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_address() {
        let cases = vec![
            ("http://localhost:8080", "127.0.0.1:8080"),
            ("https://localhost", "localhost:443"),
            ("http://example.com", "example.com:80"),
            ("https://example.com", "example.com:443"),
            ("example.com:1234", "example.com:1234"),
            (
                "https://hosting.shinkai.com/by/4G60_4564a10178_node",
                "hosting.shinkai.com/by/4G60_4564a10178_node:443",
            ),
        ];

        for (input, expected) in cases {
            let result = OnchainIdentity::validate_address(input).unwrap();
            assert_eq!(result, expected);
        }
    }

    #[tokio::test]
    async fn test_get_identity_record() {
        use std::env;
        use std::path::PathBuf;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

        env::set_var(
            "SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH",
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/debug/shinkai-tools-runner-resources/deno")
                .to_string_lossy()
                .to_string(),
        );

        env::set_var(
            "SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH",
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/debug/shinkai-tools-runner-resources/uv")
                .to_string_lossy()
                .to_string(),
        );

        let registry = ShinkaiRegistry::new(
            "https://sepolia.base.org",
            "0x425fb20ba3874e887336aaa7f3fab32d08135ba9",
            None, // ABI path is optional
        )
        .await
        .unwrap();

        let identity = "node1_test.sep-shinkai".to_string();

        let record = registry.get_identity_record(identity.clone(), None).await.unwrap();

        let expected_record = OnchainIdentity {
            shinkai_identity: "node1_test.sep-shinkai".to_string(),
            bound_nft: "9n".to_string(),
            staked_tokens: "55000000000000000000n".to_string(),
            encryption_key: "60045bdb15c24b161625cf05558078208698272bfe113f792ea740dbd79f4708".to_string(),
            signature_key: "69fa099bdce516bfeb46d5fc6e908f6cf8ffac0aba76ca0346a7b1a751a2712e".to_string(),
            routing: false,
            address_or_proxy_nodes: vec!["127.0.0.1:8080".to_string()],
            delegated_tokens: "0n".to_string(),
            last_updated: chrono::DateTime::<chrono::Utc>::from(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1738389678),
            ),
        };
        assert!(expected_record.first_address().await.is_ok());
        assert_eq!(record, expected_record);
    }
}
