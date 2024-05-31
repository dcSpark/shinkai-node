use chrono::DateTime;
use chrono::Utc;
use dashmap::DashMap;
use ed25519_dalek::VerifyingKey;
use ethers::abi::Abi;
use ethers::prelude::*;
use lazy_static::lazy_static;
use shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use shinkai_message_primitives::shinkai_utils::signatures::string_to_signature_public_key;
use std::convert::TryFrom;
use std::fmt;
use std::fs;
use std::net::{AddrParseError, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;
use std::time::{Duration, UNIX_EPOCH};
use tokio::net::lookup_host;
use tokio::task;
use x25519_dalek::PublicKey;

lazy_static! {
    static ref CACHE_TIME: Duration = Duration::from_secs(60 * 10);
}

#[derive(Debug)]
pub enum ShinkaiRegistryError {
    ContractAbiError(ethers::contract::AbiError),
    AbiError(ethers::abi::Error),
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    CustomError(String),
    SystemTimeError(std::time::SystemTimeError),
    AddressParseError(AddrParseError),
}

impl fmt::Display for ShinkaiRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiRegistryError::ContractAbiError(err) => write!(f, "Contract ABI Error: {}", err),
            ShinkaiRegistryError::AbiError(err) => write!(f, "ABI Error: {}", err),
            ShinkaiRegistryError::IoError(err) => write!(f, "IO Error: {}", err),
            ShinkaiRegistryError::JsonError(err) => write!(f, "JSON Error: {}", err),
            ShinkaiRegistryError::CustomError(err) => write!(f, "Custom Error: {}", err),
            ShinkaiRegistryError::SystemTimeError(err) => write!(f, "System Time Error: {}", err),
            ShinkaiRegistryError::AddressParseError(err) => write!(f, "Address Parse Error: {}", err),
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

impl From<ethers::contract::AbiError> for ShinkaiRegistryError {
    fn from(err: ethers::contract::AbiError) -> ShinkaiRegistryError {
        ShinkaiRegistryError::ContractAbiError(err)
    }
}

impl From<serde_json::Error> for ShinkaiRegistryError {
    fn from(err: serde_json::Error) -> ShinkaiRegistryError {
        ShinkaiRegistryError::JsonError(err)
    }
}

impl From<ethers::abi::Error> for ShinkaiRegistryError {
    fn from(err: ethers::abi::Error) -> ShinkaiRegistryError {
        ShinkaiRegistryError::AbiError(err)
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
    pub bound_nft: U256, // id of the nft
    pub staked_tokens: U256,
    pub encryption_key: String,
    pub signature_key: String,
    pub routing: bool,
    pub address_or_proxy_nodes: Vec<String>,
    pub delegated_tokens: U256,
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
        if let Some(first_address) = self.address_or_proxy_nodes.first() {
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

            // Try to parse the address directly first
            match address.parse::<SocketAddr>() {
                Ok(addr) => Ok(addr),
                Err(_) => {
                    // If direct parsing fails, attempt DNS resolution
                    let resolved_addresses = lookup_host(address)
                        .await
                        .map_err(|e| ShinkaiRegistryError::CustomError(format!("DNS resolution error: {}", e)))?;
                    resolved_addresses
                        .into_iter()
                        .next()
                        .ok_or_else(|| ShinkaiRegistryError::CustomError("No address resolved".to_string()))
                }
            }
        } else {
            Err(ShinkaiRegistryError::CustomError("No address available".to_string()))
        }
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
    pub contract: ContractInstance<Arc<Provider<Http>>, Provider<Http>>,
    pub cache: Arc<DashMap<String, (SystemTime, OnchainIdentity)>>,
}

impl ShinkaiRegistry {
    pub async fn new(url: &str, contract_address: &str, abi_path: Option<String>) -> Result<Self, ShinkaiRegistryError> {
        let provider =
            Provider::<Http>::try_from(url).map_err(|err| ShinkaiRegistryError::CustomError(err.to_string()))?;
        let contract_address: Address = contract_address.parse().map_err(|e| {
            shinkai_log(
                ShinkaiLogOption::CryptoIdentity,
                ShinkaiLogLevel::Error,
                format!("Error parsing contract address: {}", e).as_str(),
            );
            ShinkaiRegistryError::AbiError(ethers::abi::Error::InvalidData)
        })?;

        let abi_json = match abi_path {
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
        let abi: Abi = serde_json::from_str(&abi_json).map_err(ShinkaiRegistryError::JsonError)?;

        let contract = Contract::new(contract_address, abi, Arc::new(provider));
        Ok(Self {
            contract,
            cache: Arc::new(DashMap::new()),
        })
    }

    pub async fn get_identity_record(&self, identity: String) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        let identity = if identity.starts_with("@@") {
            identity.trim_start_matches("@@").to_string()
        } else {
            identity
        };
        
        // eprintln!("Getting identity record for: {}", identity);
        let now = SystemTime::now();

        // If the cache is up-to-date, return the cached value
        if let Some(value) = self.cache.get(&identity) {
            let (last_updated, record) = value.value().clone();
            if now.duration_since(last_updated)? < *CACHE_TIME {
                // Spawn a new task to update the cache in the background
                let identity_clone = identity.clone();
                let contract_clone = self.contract.clone();
                let cache_clone = self.cache.clone();
                task::spawn(async move {
                    if let Err(e) = Self::update_cache(&contract_clone, &cache_clone, identity_clone).await {
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

        // Otherwise, update the cache
        let record = Self::update_cache(&self.contract, &self.cache, identity.clone()).await?;
        Ok(record.clone())
    }

    async fn update_cache(
        contract: &ContractInstance<Arc<Provider<Http>>, Provider<Http>>,
        cache: &DashMap<String, (SystemTime, OnchainIdentity)>,
        identity: String,
    ) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        // Fetch the identity record from the contract
        let record = Self::fetch_identity_record(contract, identity.clone()).await?;

        // Update the cache and the timestamp
        cache.insert(identity.clone(), (SystemTime::now(), record.clone()));

        Ok(record)
    }

    pub fn get_cache_time(&self, identity: &str) -> Option<SystemTime> {
        self.cache.get(identity).map(|value| value.value().0)
    }

    pub async fn fetch_identity_record(
        contract: &ContractInstance<Arc<Provider<Http>>, Provider<Http>>,
        identity: String,
    ) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        let function_call = match contract.method::<_, (U256, U256, String, String, bool, Vec<String>, U256, U256)>(
            "getIdentityData",
            (identity.clone(),),
        ) {
            Ok(call) => call,
            Err(err) => {
                shinkai_log(
                    ShinkaiLogOption::CryptoIdentity,
                    ShinkaiLogLevel::Error,
                    format!("Error creating function call: {}", err).as_str(),
                );
                return Err(ShinkaiRegistryError::ContractAbiError(err));
            }
        };

        let result: (U256, U256, String, String, bool, Vec<String>, U256, U256) = match function_call.call().await {
            Ok(res) => res,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::CryptoIdentity,
                    ShinkaiLogLevel::Error,
                    format!("Error calling contract: {}", e).as_str(),
                );
                return Err(ShinkaiRegistryError::CustomError("Contract Error".to_string()));
            }
        };

        let last_updated = UNIX_EPOCH + Duration::from_secs(result.7.low_u64());
        let last_updated = DateTime::<Utc>::from(last_updated);

        Ok(OnchainIdentity {
            shinkai_identity: identity,
            bound_nft: result.0,
            staked_tokens: result.1,
            encryption_key: result.2,
            signature_key: result.3,
            routing: result.4,
            address_or_proxy_nodes: result.5,
            delegated_tokens: result.6,
            last_updated,
        })
    }
}
