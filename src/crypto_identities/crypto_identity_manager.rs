use ethers::abi::Abi;
use ethers::prelude::*;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use std::convert::TryFrom;
use std::fmt;
use std::fs;
use std::sync::Arc;

#[derive(Debug)]
pub enum ShinkaiRegistryError {
    UrlParseError(rustube::url::ParseError),
    ProviderError(ethers::providers::ProviderError),
    ContractAbiError(ethers::contract::AbiError),
    AbiError(ethers::abi::Error),
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    CustomError(String),
}

impl fmt::Display for ShinkaiRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiRegistryError::UrlParseError(err) => write!(f, "URL Parse Error: {}", err),
            ShinkaiRegistryError::ProviderError(err) => write!(f, "Provider Error: {}", err),
            // ShinkaiRegistryError::ContractError(err) => write!(f, "Contract Error: {}", err),
            ShinkaiRegistryError::ContractAbiError(err) => write!(f, "Contract ABI Error: {}", err),
            ShinkaiRegistryError::AbiError(err) => write!(f, "ABI Error: {}", err),
            ShinkaiRegistryError::IoError(err) => write!(f, "IO Error: {}", err),
            ShinkaiRegistryError::JsonError(err) => write!(f, "JSON Error: {}", err),
            ShinkaiRegistryError::CustomError(err) => write!(f, "Custom Error: {}", err),
        }
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

impl From<ethers::providers::ProviderError> for ShinkaiRegistryError {
    fn from(err: ethers::providers::ProviderError) -> ShinkaiRegistryError {
        ShinkaiRegistryError::ProviderError(err)
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

impl From<rustube::url::ParseError> for ShinkaiRegistryError {
    fn from(err: rustube::url::ParseError) -> ShinkaiRegistryError {
        ShinkaiRegistryError::UrlParseError(err)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct OnchainIdentity {
    pub bound_nft: U256, // id of the nft
    pub staked_tokens: U256,
    pub encryption_key: String,
    pub signature_key: String,
    pub routing: bool,
    pub address_or_proxy_nodes: Vec<String>,
    pub delegated_tokens: U256,
}

#[derive(Debug, Clone)]
pub struct ShinkaiRegistry {
    pub contract: ContractInstance<Arc<Provider<Http>>, Provider<Http>>,
    // identity_records: HashMap<String, IdentityRecord>,
    // ... add other fields as needed
}

impl ShinkaiRegistry {
    // Initialize a new ShinkaiRegistry
    pub async fn new(url: &str, contract_address: &str, abi_path: &str) -> Result<Self, ShinkaiRegistryError> {
        let provider = Provider::<Http>::try_from(url).map_err(ShinkaiRegistryError::UrlParseError)?;
        let contract_address: Address = contract_address.parse().map_err(|e| {
            shinkai_log(ShinkaiLogOption::CryptoIdentity, ShinkaiLogLevel::Error, format!("Error parsing contract address: {}", e).as_str());
            ShinkaiRegistryError::AbiError(ethers::abi::Error::InvalidData)
        })?;

        let abi = fs::read_to_string(abi_path).map_err(ShinkaiRegistryError::IoError)?;
        let abi: Abi = serde_json::from_str(&abi).map_err(ShinkaiRegistryError::JsonError)?;

        let contract = Contract::new(contract_address, abi, Arc::new(provider));
        Ok(Self { contract })
    }

    pub async fn get_identity_record(&self, identity: String) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        let function_call = match self
            .contract
            .method::<_, (U256, U256, String, String, bool, Vec<String>, U256)>("getIdentityRecord", (identity,))
        {
            Ok(call) => call,
            Err(err) => {
                shinkai_log(ShinkaiLogOption::CryptoIdentity, ShinkaiLogLevel::Error, format!("Error creating function call: {}", err).as_str());
                return Err(ShinkaiRegistryError::ContractAbiError(err));
            }
        };

        let result: (U256, U256, String, String, bool, Vec<String>, U256) = match function_call.call().await {
            Ok(res) => res,
            Err(e) => {
                shinkai_log(ShinkaiLogOption::CryptoIdentity, ShinkaiLogLevel::Error, format!("Error calling contract: {}", e).as_str());
                return Err(ShinkaiRegistryError::CustomError("Contract Error".to_string()));
            }
        };
        shinkai_log(ShinkaiLogOption::CryptoIdentity, ShinkaiLogLevel::Debug, format!("get_identity_record result: {:?}", result).as_str());

        Ok(OnchainIdentity {
            bound_nft: result.0,
            staked_tokens: result.1,
            encryption_key: result.2,
            signature_key: result.3,
            routing: result.4,
            address_or_proxy_nodes: result.5,
            delegated_tokens: result.6,
        })
    }
}

/*
    - Create Identity Manager (Reader)
    - it should have an indexer for caching
    - it should be able to check if the indexer is up to date easily (maybe with a timestamp)
    - if indexer not up to date, it should be able to update it and (next line)
    - it should be able to do individual request to an external or local node api
    - it should be able to read current information associated with the user (delegation, info of stake pools, etc)

    // Should we have a different identity manager for writing? It feels like we should.
    // add local mnemonics
    // rpc (local or external) to create new identities

    // some values:
    TESTNET_RPC=https://eth-sepolia.g.alchemy.com/v2/demo
    MAINNET_RPC=https://eth.llamarpc.com
*/
