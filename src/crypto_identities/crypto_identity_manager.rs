// use std::collections::HashMap;
// use std::convert::TryFrom;
// use alloy_rs::prelude::*;
// use std::fs;

// #[derive(Debug)]
// pub struct OnchainIdentity {
//     pub encryption_key: String,
//     pub signature_key: String,
//     pub staked_tokens: u64,
//     pub bound_nft: u64, // id of the nft
//     pub routing: bool,
//     pub address_or_proxy_nodes: Vec<String>,
//     pub delegated_tokens: u64,
// }

// struct ShinkaiRegistry {
//     contract: Contract<Http>,
//     // identity_records: HashMap<String, IdentityRecord>,
//     // ... add other fields as needed
// }

// impl ShinkaiRegistry {
//     // Initialize a new ShinkaiRegistry
//     pub async fn new(url: &str, contract_address: &str, abi_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
//         let provider = Provider::<Http>::try_from(url)?;
//         let contract_address: Address = contract_address.parse()?;

//         // Read the ABI from the file
//         let abi = fs::read(abi_path)?;

//         let contract = Contract::from_json(
//             provider,
//             contract_address,
//             &abi,
//         )?;

//         Ok(Self { contract })
//     }

//     pub async fn get_identity_record(&self, identity: String) -> Result<IdentityRecord, Box<dyn std::error::Error>> {
//         let result: (String, String, u64, u64, bool, Vec<String>, u64) = self.contract
//             .query("getIdentityRecord", (identity,), None, Options::default(), None)
//             .await?;

//         Ok(IdentityRecord {
//             encryption_key: result.0,
//             signature_key: result.1,
//             staked_tokens: result.2,
//             bound_nft: result.3,
//             routing: result.4,
//             address_or_proxy_nodes: result.5,
//             delegated_tokens: result.6,
//         })
//     }
// }


// /*
//     - Create Identity Manager (Reader)
//     - it should have an indexer for caching
//     - it should be able to check if the indexer is up to date easily (maybe with a timestamp)
//     - if indexer not up to date, it should be able to update it and (next line)
//     - it should be able to do individual request to an external or local node api
//     - it should be able to read current information associated with the user (delegation, info of stake pools, etc)

//     // Should we have a different identity manager for writing? It feels like we should.
//     // add local mnemonics
//     // rpc (local or external) to create new identities

//     // some values:
//     TESTNET_RPC=https://eth-sepolia.g.alchemy.com/v2/demo
//     MAINNET_RPC=https://eth.llamarpc.com
// */