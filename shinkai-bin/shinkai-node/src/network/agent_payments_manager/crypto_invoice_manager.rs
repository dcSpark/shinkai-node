// use async_trait::async_trait;
// use std::sync::Arc;

// use super::external_agent_offerings_manager::AgentOfferingManagerError;

// #[async_trait]
// pub trait CryptoInvoiceManagerTrait: Send + Sync {
//     fn new(provider_url: &str) -> Result<Self, AgentOfferingManagerError>
//     where
//         Self: Sized;

//     async fn verify_transaction(
//         &self,
//         tx_hash: &str,
//         expected_message: &str,
//     ) -> Result<bool, AgentOfferingManagerError>;
// }

// pub struct CryptoInvoiceManager {
//     provider: Arc<Provider<Http>>,
// }

// #[async_trait]
// impl CryptoInvoiceManagerTrait for CryptoInvoiceManager {
//     fn new(provider_url: &str) -> Result<Self, AgentOfferingManagerError> {
//         let provider = Provider::<Http>::try_from(provider_url)
//             .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to create provider: {:?}", e)))?;

//         Ok(Self {
//             provider: Arc::new(provider),
//         })
//     }

//     async fn verify_transaction(
//         &self,
//         tx_hash: &str,
//         expected_message: &str,
//     ) -> Result<bool, AgentOfferingManagerError> {
//         let tx_hash: H256 = tx_hash.parse().map_err(|e| {
//             AgentOfferingManagerError::OperationFailed(format!("Failed to parse transaction hash: {:?}", e))
//         })?;
//         let tx =
//             self.provider.get_transaction(tx_hash).await.map_err(|e| {
//                 AgentOfferingManagerError::OperationFailed(format!("Failed to fetch transaction: {:?}", e))
//             })?;

//         if let Some(transaction) = tx {
//             let tx_data = transaction.input.0.to_vec();
//             let tx_message = String::from_utf8(tx_data).map_err(|e| {
//                 AgentOfferingManagerError::OperationFailed(format!("Failed to decode transaction data: {:?}", e))
//             })?;

//             if tx_message == expected_message {
//                 return Ok(true);
//             }
//         }

//         Ok(false)
//     }
// }

// // Mock implementation
// pub struct MockCryptoInvoiceManager {
//     pub verify_transaction_result: Result<bool, AgentOfferingManagerError>,
// }

// #[async_trait]
// impl CryptoInvoiceManagerTrait for MockCryptoInvoiceManager {
//     fn new(_provider_url: &str) -> Result<Self, AgentOfferingManagerError> {
//         Ok(Self {
//             verify_transaction_result: Ok(false), // Default value
//         })
//     }

//     #[allow(unused_variables)]
//     async fn verify_transaction(
//         &self,
//         tx_hash: &str,
//         expected_message: &str,
//     ) -> Result<bool, AgentOfferingManagerError> {
//         self.verify_transaction_result.clone()
//     }
// }
