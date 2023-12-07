
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tokio::runtime::Runtime;

//     #[test]
//     fn test_get_identity_record() {
//         let mut rt = Runtime::new().unwrap();

//         rt.block_on(async {
//             let registry = ShinkaiRegistry::new(
//                 "https://eth-sepolia.g.alchemy.com/v2/demo",
//                 "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
//                 "/src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json",
//             ).await.unwrap();

//             let record = registry.get_identity_record("identity_to_fetch".to_string()).await.unwrap();

//             println!("{:?}", record);
//         });
//     }
// }