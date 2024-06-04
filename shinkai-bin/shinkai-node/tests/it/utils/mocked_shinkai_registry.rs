use chrono::{DateTime, Utc};
use dashmap::DashMap;
use ethers::prelude::*;
use shinkai_crypto_identities::{OnchainIdentity, ShinkaiRegistryError, ShinkaiRegistryTrait};
use std::sync::Arc;
use std::time::SystemTime;

pub struct MockedShinkaiRegistry {
    pub cache: Arc<DashMap<String, (SystemTime, OnchainIdentity)>>,
}

impl ShinkaiRegistryTrait for MockedShinkaiRegistry {
    fn new(_url: &str, _contract_address: &str, _abi_path: &str) -> Result<Self, ShinkaiRegistryError> {
        let cache = DashMap::new();

        let identities = vec![
            OnchainIdentity {
                shinkai_identity: "nico.shinkai".to_string(),
                bound_nft: U256::from(1),
                staked_tokens: U256::from(1000),
                encryption_key: "60045bdb15c24b161625cf05558078208698272bfe113f792ea740dbd79f4708".to_string(),
                signature_key: "69fa099bdce516bfeb46d5fc6e908f6cf8ffac0aba76ca0346a7b1a751a2712e".to_string(),
                routing: true,
                address_or_proxy_nodes: vec!["192.168.1.109:8080".to_string()],
                delegated_tokens: U256::from(500),
                last_updated: DateTime::<Utc>::from(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1704927408)),
            },
            OnchainIdentity {
                shinkai_identity: "nico.shinkai".to_string(),
                bound_nft: U256::from(2),
                staked_tokens: U256::from(1000),
                encryption_key: "912fed05e286af45f44580d6a87da61e1f9a0946237dd29f7bc2d3cbeba0857f".to_string(),
                signature_key: "389fb4bbb3d382a2f2f23cdfa5614ed288975bc4f4a0448876efba108dc2c583".to_string(),
                routing: true,
                address_or_proxy_nodes: vec!["192.168.1.233:8081".to_string()],
                delegated_tokens: U256::from(500),
                last_updated: DateTime::<Utc>::from(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1704927408)),
            },
            OnchainIdentity {
                shinkai_identity: "nico.shinkai".to_string(),
                bound_nft: U256::from(3),
                staked_tokens: U256::from(1000),
                encryption_key: "3273d113e401a215e429e3272352186a7370cf7edf1e2d68aa7ef87a20237371".to_string(),
                signature_key: "63dd3953fe0b9e3212503fc1de9be9b46008615a4522facf271f0c2b3585c3e6".to_string(),
                routing: true,
                address_or_proxy_nodes: vec!["127.0.0.1:8082".to_string()],
                delegated_tokens: U256::from(500),
                last_updated: DateTime::<Utc>::from(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1704927408)),
            },
            OnchainIdentity {
                shinkai_identity: "nico.shinkai".to_string(),
                bound_nft: U256::from(4),
                staked_tokens: U256::from(1000),
                encryption_key: "60045bdb15c24b161625cf05558078208698272bfe113f792ea740dbd79f4708".to_string(),
                signature_key: "69fa099bdce516bfeb46d5fc6e908f6cf8ffac0aba76ca0346a7b1a751a2712e".to_string(),
                routing: true,
                address_or_proxy_nodes: vec!["127.0.0.1:8080".to_string()],
                delegated_tokens: U256::from(500),
                last_updated: DateTime::<Utc>::from(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1704927408)),
            },
            OnchainIdentity {
                shinkai_identity: "nico.shinkai".to_string(),
                bound_nft: U256::from(5),
                staked_tokens: U256::from(1000),
                encryption_key: "912fed05e286af45f44580d6a87da61e1f9a0946237dd29f7bc2d3cbeba0857f".to_string(),
                signature_key: "389fb4bbb3d382a2f2f23cdfa5614ed288975bc4f4a0448876efba108dc2c583".to_string(),
                routing: true,
                address_or_proxy_nodes: vec!["127.0.0.1:8081".to_string()],
                delegated_tokens: U256::from(500),
                last_updated: DateTime::<Utc>::from(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1704927408)),
            },
        ];

        for (i, identity) in identities.into_iter().enumerate() {
            cache.insert(format!("identity{}", i+1), (SystemTime::now(), identity));
        }

        Ok(Self {
            cache: Arc::new(cache),
        })
    }

    fn get_identity_record(&self, identity: String) -> Result<OnchainIdentity, ShinkaiRegistryError> {
        match self.cache.get(&identity) {
            Some(value) => Ok(value.value().1.clone()),
            None => Err(ShinkaiRegistryError::CustomError("Identity not found in mock".to_string())),
        }
    }

    fn get_cache_time(&self, identity: &str) -> Option<SystemTime> {
        self.cache.get(identity).map(|value| value.value().0)
    }
}