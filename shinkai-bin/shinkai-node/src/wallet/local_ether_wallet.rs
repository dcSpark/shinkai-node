use aes_gcm::aead::generic_array::GenericArray;
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic, Seed};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{Address as EthersAddress, NameOrAddress};
use ethers::utils::{format_units, hex, to_checksum};
use ethers::{core::k256::SecretKey, prelude::*};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

use ethers::signers::LocalWallet as EthersLocalWallet;
use futures::TryFutureExt;

use crate::wallet::erc20_abi::ERC20_ABI;
use crate::wallet::wallet_error::WalletError;

use super::mixed::{self, Address, AddressBalanceList, Asset, AssetType, Balance, Network, PublicAddress};
use super::wallet_traits::{CommonActions, IsWallet, PaymentWallet, ReceivingWallet, SendActions, TransactionHash};

pub type LocalWalletProvider = Provider<Http>;

#[derive(Debug, Clone)]
pub struct LocalEthersWallet {
    pub id: String,
    pub network: Network,
    pub address: Address,
    pub wallet: Wallet<SigningKey>,
    pub provider: LocalWalletProvider,
}

impl Serialize for LocalEthersWallet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let wallet_bytes = self.wallet.signer().to_bytes();
        let provider_url = self.provider.url().to_string();

        let mut state = serializer.serialize_struct("LocalEthersWallet", 5)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("network", &self.network)?;
        state.serialize_field("address", &self.address)?;
        state.serialize_field("wallet_private_key", &hex::encode(wallet_bytes))?;
        state.serialize_field("provider_url", &provider_url)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for LocalEthersWallet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LocalEthersWalletData {
            id: String,
            network: Network,
            address: Address,
            wallet_private_key: String,
            provider_url: String,
        }

        let data = LocalEthersWalletData::deserialize(deserializer)?;
        let wallet_bytes = hex::decode(data.wallet_private_key).map_err(serde::de::Error::custom)?;
        let wallet_secret_key =
            SecretKey::from_bytes(GenericArray::from_slice(&wallet_bytes)).map_err(serde::de::Error::custom)?;
        let wallet = EthersLocalWallet::from(wallet_secret_key).with_chain_id(data.network.chain_id);

        let provider = Provider::<Http>::try_from(data.provider_url).map_err(serde::de::Error::custom)?;

        Ok(LocalEthersWallet {
            id: data.id,
            network: data.network,
            address: data.address,
            wallet,
            provider,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletSource {
    Mnemonic(String),
    PrivateKey(String),
}

impl LocalEthersWallet {
    pub fn create_wallet(network: Network) -> Result<Self, WalletError> {
        // TODO: instead of using a random number, we should randomly generate a mnemonic and use that
        let wallet = EthersLocalWallet::new(&mut rand::thread_rng()).with_chain_id(network.chain_id);
        let address = format!("0x{:x}", wallet.address());

        let provider =
            Provider::<Http>::try_from(network.default_rpc()).map_err(|e| WalletError::InvalidRpcUrl(e.to_string()))?;

        Ok(LocalEthersWallet {
            id: Uuid::new_v4().to_string(),
            network: network.clone(),
            provider,
            wallet,
            address: Address {
                wallet_id: Uuid::new_v4().to_string(),
                network_id: network.id,
                public_key: None,
                address_id: address,
            },
        })
    }

    pub fn recover_wallet(network: Network, source: WalletSource) -> Result<Self, WalletError> {
        let wallet = match source {
            WalletSource::Mnemonic(mnemonic) => {
                let mnemonic = Mnemonic::from_phrase(&mnemonic, Language::English)
                    .map_err(|e| WalletError::Bip39Error(e.to_string()))?;
                let seed = Seed::new(&mnemonic, "");
                let xprv = XPrv::new(&seed).map_err(|e| WalletError::Bip39Error(e.to_string()))?;
                let derivation_path =
                    DerivationPath::from_str("m/44'/60'/0'/0/0").map_err(|e| WalletError::Bip39Error(e.to_string()))?;
                let child_xprv = derivation_path
                    .into_iter()
                    .fold(Ok(xprv), |acc, child| acc.and_then(|key| key.derive_child(child)))
                    .map_err(|e| WalletError::Bip39Error(e.to_string()))?;
                let secret_key = SecretKey::from_slice(child_xprv.private_key().to_bytes().as_slice())?;

                EthersLocalWallet::from(secret_key).with_chain_id(network.chain_id)
            }
            WalletSource::PrivateKey(private_key) => {
                let private_key_bytes = hex::decode(private_key)?;
                let secret_key = SecretKey::from_slice(&private_key_bytes)?;
                EthersLocalWallet::from(secret_key).with_chain_id(network.chain_id)
            }
        };

        let address = to_checksum(&wallet.address(), None);
        println!("recovered wallet's address: {}", address);
        let provider =
            Provider::<Http>::try_from(network.default_rpc()).map_err(|e| WalletError::InvalidRpcUrl(e.to_string()))?;

        Ok(LocalEthersWallet {
            id: Uuid::new_v4().to_string(),
            network: network.clone(),
            provider,
            wallet,
            address: Address {
                wallet_id: Uuid::new_v4().to_string(),
                network_id: network.id,
                public_key: None,
                address_id: address,
            },
        })
    }

    pub async fn prepare_transaction_request(
        from_wallet: &LocalEthersWallet,
        to_wallet: PublicAddress,
        token: Option<Asset>,
        send_amount: U256,
        provider_url: String,
        invoice_id: String,
    ) -> Result<TransactionRequest, WalletError> {
        let provider =
            Provider::<Http>::try_from(provider_url).map_err(|e| WalletError::InvalidRpcUrl(e.to_string()))?;
        let chain_id = provider
            .get_chainid()
            .await
            .map_err(|e| WalletError::ProviderError(e.to_string()))?
            .low_u64();

        let mut tx = TransactionRequest::new();
        tx.to = Some(NameOrAddress::Address(
            EthersAddress::from_str(&to_wallet.address_id).map_err(|e| WalletError::InvalidAddress(e.to_string()))?,
        ));

        if let Some(token) = token {
            let contract_address = token
                .contract_address
                .ok_or_else(|| WalletError::MissingContractAddress(token.asset_id.clone()))?
                .parse::<EthersAddress>()
                .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
            let contract = Contract::new(contract_address, ERC20_ABI.clone(), Arc::new(provider.clone()));
            let call = contract
                .method::<(EthersAddress, U256), bool>(
                    "transfer",
                    (
                        EthersAddress::from_str(&to_wallet.address_id)
                            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?,
                        send_amount,
                    ),
                )
                .map_err(|e| WalletError::ContractError(e.to_string()))?;
            tx = call.tx.into();
        } else {
            tx.value = Some(send_amount);
        }

        tx.chain_id = Some(chain_id.into());
        let from_address = from_wallet.wallet.address();
        let nonce = provider
            .get_transaction_count(from_address, None)
            .await
            .map_err(|e| WalletError::ProviderError(e.to_string()))?;
        tx.from = Some(from_address);
        tx.nonce = Some(nonce);

        let gas_price = provider
            .get_gas_price()
            .await
            .map_err(|e| WalletError::ProviderError(e.to_string()))?;
        tx.gas_price = Some(gas_price);

        // Add "kai:" prefix to the invoice ID and include it in the transaction data
        let data_with_prefix = format!("kai:{}", invoice_id);
        tx.data = Some(data_with_prefix.into_bytes().into());

        // Convert TransactionRequest to TypedTransaction
        let typed_tx: TypedTransaction = tx.clone().into();

        // Estimate gas instead of hardcoding
        let gas_estimate = provider
            .estimate_gas(&typed_tx, None)
            .await
            .map_err(|e| WalletError::ProviderError(e.to_string()))?;
        tx.gas = Some(gas_estimate);
        eprintln!("Tx: {:?}", tx);

        Ok(tx)
    }

    pub async fn internal_sign_transaction(&self, tx_request: TypedTransaction) -> Result<Signature, WalletError> {
        let typed_tx: TypedTransaction = tx_request.into();
        let signature = self
            .wallet
            .sign_transaction(&typed_tx)
            .map_err(|e| WalletError::SigningError(e.to_string()))
            .await?;

        Ok(signature)
    }

    pub async fn internal_send_transaction(
        &self,
        to_wallet: PublicAddress,
        token: Option<Asset>,
        send_amount: U256,
        invoice_id: String,
    ) -> Result<H256, WalletError> {
        let tx_request = Self::prepare_transaction_request(
            self,
            to_wallet,
            token,
            send_amount,
            self.provider.url().to_string(),
            invoice_id,
        )
        .await?;

        let signer = SignerMiddleware::new(self.provider.clone(), self.wallet.clone());

        let pending_tx = signer
            .send_transaction(tx_request, None)
            .await
            .map_err(|e| WalletError::ProviderError(e.to_string()))?;

        let receipt = pending_tx
            .await
            .map_err(|e| WalletError::ProviderError(e.to_string()))?;

        if let Some(receipt) = receipt {
            let tx_hash = receipt.transaction_hash;
            Ok(tx_hash)
        } else {
            Err(WalletError::MissingTransactionReceipt)
        }
    }
}

impl IsWallet for LocalEthersWallet {}

impl PaymentWallet for LocalEthersWallet {
    // No additional methods needed, as they are covered by SendActions and CommonActions
}

impl ReceivingWallet for LocalEthersWallet {
    // No additional methods needed, as they are covered by SendActions and CommonActions
}

impl SendActions for LocalEthersWallet {
    fn send_transaction(
        &self,
        to_wallet: PublicAddress,
        token: Option<Asset>,
        send_amount: String,
        invoice_id: String,
    ) -> Pin<Box<dyn Future<Output = Result<TransactionHash, WalletError>> + Send + 'static>> {
        let send_amount_u256 = U256::from_dec_str(&send_amount);
        let send_amount_u256 = match send_amount_u256 {
            Ok(amount) => amount,
            Err(e) => return Box::pin(async move { Err(WalletError::ConversionError(e.to_string())) }),
        };

        let self_clone = self.clone();
        let fut = async move {
            let tx_hash = self_clone
                .internal_send_transaction(to_wallet, token, send_amount_u256, invoice_id)
                .await?;
            Ok(tx_hash.to_string())
        };

        Box::pin(fut)
    }

    fn sign_transaction(
        &self,
        tx: mixed::Transaction,
    ) -> Pin<Box<dyn Future<Output = Result<String, WalletError>> + Send + 'static>> {
        let self_clone = self.clone();
        let fut = async move {
            let typed_tx: TypedTransaction = tx.into();
            let signature = self_clone.internal_sign_transaction(typed_tx).await?;
            Ok(signature.to_string())
        };

        Box::pin(fut)
    }
}

// Implement conversion from mixed::Transaction to TypedTransaction
impl From<mixed::Transaction> for TypedTransaction {
    fn from(tx: mixed::Transaction) -> Self {
        let mut typed_tx = TypedTransaction::default();
        typed_tx.set_to(NameOrAddress::Address(
            EthersAddress::from_str(&tx.to_address_id.unwrap()).unwrap(),
        ));
        typed_tx.set_data(tx.unsigned_payload.into_bytes().into());
        typed_tx
    }
}

impl CommonActions for LocalEthersWallet {
    fn get_payment_address(&self) -> PublicAddress {
        self.address.clone().into()
    }

    fn get_address(&self) -> Address {
        self.address.clone()
    }

    fn get_balance(&self) -> Pin<Box<dyn Future<Output = Result<f64, WalletError>> + Send + 'static>> {
        let self_clone = self.clone();
        Box::pin(async move {
            let balance_wei = self_clone
                .provider
                .get_balance(self_clone.wallet.address(), None)
                .await
                .map_err(|e| WalletError::ProviderError(e.to_string()))?;

            // Convert balance from wei to ETH using ethers::utils::format_units
            let balance_eth = format_units(balance_wei, "ether")
                .map_err(|e| WalletError::ConversionError(e.to_string()))?
                .parse::<f64>()
                .map_err(|e| WalletError::ConversionError(e.to_string()))?;

            Ok(balance_eth)
        })
    }

    fn check_balances(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<AddressBalanceList, WalletError>> + Send + 'static>> {
        let self_clone = self.clone();
        Box::pin(async move {
            let mut balances = Vec::new();

            // Check ETH balance
            let eth_balance_wei = self_clone
                .provider
                .get_balance(self_clone.wallet.address(), None)
                .await
                .map_err(|e| WalletError::ProviderError(e.to_string()))?;
            let eth_balance = Balance {
                amount: eth_balance_wei.to_string(),
                decimals: Some(18),
                asset: Asset::new(AssetType::ETH, &self_clone.network.id).ok_or_else(|| {
                    WalletError::UnsupportedAssetForNetwork("ETH".to_string(), self_clone.network.id.to_string())
                })?,
            };
            balances.push(eth_balance);

            // Check USDC balance
            if let Some(usdc_asset) = Asset::new(AssetType::USDC, &self_clone.network.id) {
                let usdc_contract_address = usdc_asset
                    .contract_address
                    .clone()
                    .ok_or_else(|| WalletError::MissingContractAddress(usdc_asset.asset_id.clone()))?;
                let usdc_contract = Contract::new(
                    usdc_contract_address
                        .parse::<EthersAddress>()
                        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?,
                    ERC20_ABI.clone(),
                    Arc::new(self_clone.provider.clone()),
                );
                let usdc_balance: U256 = usdc_contract
                    .method::<EthersAddress, U256>("balanceOf", self_clone.wallet.address())
                    .map_err(|e| WalletError::ContractError(e.to_string()))?
                    .call()
                    .await
                    .map_err(|e| WalletError::ProviderError(e.to_string()))?;
                let usdc_balance = Balance {
                    amount: usdc_balance.to_string(),
                    decimals: Some(6),
                    asset: usdc_asset,
                };
                balances.push(usdc_balance);
            }

            // Check KAI balance (if applicable)
            if let Some(kai_asset) = Asset::new(AssetType::KAI, &self_clone.network.id) {
                let kai_contract_address = kai_asset
                    .contract_address
                    .clone()
                    .ok_or_else(|| WalletError::MissingContractAddress(kai_asset.asset_id.clone()))?;
                let kai_contract = Contract::new(
                    kai_contract_address
                        .parse::<EthersAddress>()
                        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?,
                    ERC20_ABI.clone(),
                    Arc::new(self_clone.provider.clone()),
                );
                let kai_balance: U256 = kai_contract
                    .method::<EthersAddress, U256>("balanceOf", self_clone.wallet.address())
                    .map_err(|e| WalletError::ContractError(e.to_string()))?
                    .call()
                    .await
                    .map_err(|e| WalletError::ProviderError(e.to_string()))?;
                let kai_balance = Balance {
                    amount: kai_balance.to_string(),
                    decimals: Some(18),
                    asset: kai_asset,
                };
                balances.push(kai_balance);
            }

            Ok(AddressBalanceList {
                data: balances.clone(),
                has_more: false,
                next_page: "".to_string(),
                total_count: balances.len() as u32,
            })
        })
    }
    //     fn check_balance(&self) -> Result<AddressBalanceList, String> {
    //         // Implement balance checking logic
    //     }

    //     fn check_transaction_history(&self) -> Result<Vec<Transaction>, String> {
    //         // Implement transaction history checking logic
    //     }

    //     fn restore_wallet(&self, seed_phrase: String) -> Result<(), String> {
    //         // Implement wallet restoration logic
    //     }

    //     fn create_wallet(&self) -> Result<LocalWallet, String> {
    //         // Implement wallet creation logic
    //     }

    //     fn get_balance(&self) -> Result<Balance, String> {
    //         // Implement balance retrieval logic
    //     }

    //     fn get_block(&self, block_number: u64) -> Result<String, String> {
    //         // Implement block retrieval logic
    //     }

    //     fn get_block_number(&self) -> Result<u64, String> {
    //         // Implement block number retrieval logic
    //     }

    //     fn verify_message(&self, message: String, signature: String) -> Result<bool, String> {
    //         // Implement message verification logic
    //     }

    //     fn get_transaction(&self, tx_hash: String) -> Result<Transaction, String> {
    //         // Implement transaction retrieval logic
    //     }

    //     fn get_transaction_confirmations(&self, tx_hash: String) -> Result<u64, String> {
    //         // Implement transaction confirmations retrieval logic
    //     }

    //     fn get_transaction_receipt(&self, tx_hash: String) -> Result<Transaction, String> {
    //         // Implement transaction receipt retrieval logic
    //     }

    //     fn wait_for_transaction_receipt(&self, tx_hash: String) -> Result<Transaction, String> {
    //         // Implement waiting for transaction receipt logic
    //     }

    //     fn watch_pending_transactions(&self) -> Result<Vec<Transaction>, String> {
    //         // Implement watching pending transactions logic
    //     }

    //     fn get_addresses(&self) -> Result<AddressList, String> {
    //         // Implement addresses retrieval logic
    //     }

    //     fn send_raw_transaction(&self, raw_tx: String) -> Result<(), String> {
    //         // Implement raw transaction sending logic
    //     }

    //     fn prepare_transaction_request(
    //         &self,
    //         request: CreateTransferRequest,
    //     ) -> Result<Transfer, String> {
    //         // Implement transaction request preparation logic
    //     }

    //     fn request_addresses(&self) -> Result<AddressList, String> {
    //         // Implement addresses request logic
    //     }

    //     fn load_from_private_key(&self, private_key: String) -> Result<LocalWallet, String> {
    //         // Implement loading from private key logic
    //     }

    //     fn load_from_mnemonic(&self, mnemonic: String) -> Result<LocalWallet, String> {
    //         // Implement loading from mnemonic logic
    //     }

    //     fn load_from_coinbase_mpc_cred(&self, cred: String) -> Result<LocalWallet, String> {
    //         // Implement loading from Coinbase MPC credentials logic
    //     }

    //     fn get_address(&self) -> Result<Address, String> {
    //         // Implement address retrieval logic
    //     }
}

#[cfg(test)]
mod tests {

    use crate::wallet::wallet_manager::WalletManager;

    use super::super::mixed::{Address, Asset, Network};
    use super::*;
    use ethers::utils::Anvil;
    use mixed::{NetworkIdentifier, NetworkProtocolFamilyEnum};

    fn create_test_network() -> Network {
        Network {
            id: NetworkIdentifier::Anvil,
            display_name: "Anvil".to_string(),
            chain_id: 31337,
            protocol_family: NetworkProtocolFamilyEnum::Evm,
            is_testnet: true,
            native_asset: Asset {
                network_id: "Anvil".to_string(),
                asset_id: "ETH".to_string(),
                decimals: Some(18),
                contract_address: None,
            },
        }
    }

    #[test]
    fn test_create_wallet() {
        let network = create_test_network();
        let wallet = LocalEthersWallet::create_wallet(network.clone()).unwrap();
        assert_eq!(wallet.network.id, network.id);
        assert_eq!(wallet.network.display_name, network.display_name);
        assert!(!wallet.id.is_empty());
        assert!(!wallet.address.address_id.is_empty());
    }

    #[test]
    fn test_recover_wallet_from_mnemonic() {
        let network = create_test_network();
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let wallet =
            LocalEthersWallet::recover_wallet(network.clone(), WalletSource::Mnemonic(mnemonic.to_string())).unwrap();
        assert_eq!(wallet.network.id, network.id);
        assert_eq!(wallet.network.display_name, network.display_name);
        assert!(!wallet.id.is_empty());
        assert!(!wallet.address.address_id.is_empty());
        assert_eq!(wallet.address.address_id, "0xea6e8f7525e8af0669546ac6c5b8318fd2c6d7b6");
    }

    #[test]
    fn test_recover_wallet_from_private_key() {
        let network = create_test_network();
        let private_key = "0000000000000000000000000000000000000000000000000000000000000001";
        let wallet =
            LocalEthersWallet::recover_wallet(network.clone(), WalletSource::PrivateKey(private_key.to_string()))
                .unwrap();
        assert_eq!(wallet.network.id, network.id);
        assert_eq!(wallet.network.display_name, network.display_name);
        assert!(!wallet.id.is_empty());
        assert!(!wallet.address.address_id.is_empty());
        assert_eq!(wallet.address.address_id, "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf");
    }

    #[tokio::test]
    async fn test_anvil_current_block() {
        eprintln!("Starting test_anvil_current_block");
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        // Start Anvil instance with block time set to 1 second and the specified mnemonic
        let anvil = Anvil::new().block_time(1u64).port(62582u16).mnemonic(mnemonic).spawn();
        eprintln!("Anvil endpoint {}", anvil.endpoint());

        let provider = Provider::<Http>::try_from(anvil.endpoint()).unwrap();

        // Recover the wallet using the mnemonic
        let network = create_test_network();
        let wallet =
            LocalEthersWallet::recover_wallet(network.clone(), WalletSource::Mnemonic(mnemonic.to_string())).unwrap();

        eprintln!("Wallet address: {}", wallet.address.address_id);

        // Set initial balance for the wallet address
        let initial_balance = U256::from(10u64.pow(18)); // 1 ETH in wei
        provider
            .request::<_, ()>("anvil_setBalance", (wallet.address.address_id.clone(), initial_balance))
            .await
            .unwrap();
        eprintln!("Initial balance set");

        // Retrieve the chain ID directly from Anvil
        let chain_id: u64 = provider.request::<_, U256>("eth_chainId", ()).await.unwrap().as_u64();
        eprintln!("Chain ID: {}", chain_id);

        // Send 1 ETH to the target address
        let target_address = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf";
        eprintln!("Network id: {}", network.id);

        let send_amount = U256::from(10u64.pow(17)); // 0.1 ETH in wei
        let invoice_id = "123";
        let tx_hash = wallet
            .internal_send_transaction(
                PublicAddress {
                    network_id: network.id,
                    address_id: target_address.to_string(),
                },
                None,
                send_amount,
                invoice_id.to_string(),
            )
            .await
            .unwrap();

        eprintln!("Transaction hash: {:x}", tx_hash);

        // Check the balance of the recovered wallet
        let balance = wallet.get_balance().await.unwrap();
        println!("Wallet balance: {} ETH", balance);

        // Retrieve the transaction and check the data field
        let tx = provider.get_transaction(tx_hash).await.unwrap().unwrap();
        let expected_data = format!("kai:{}", invoice_id).into_bytes();
        assert_eq!(tx.input.0, expected_data);

        // // Assert that the balance is 0 (initial balance)
        // assert_eq!(balance, U256::zero());
    }

    #[test]
    fn test_serialize_deserialize_wallet() {
        let network = create_test_network();
        let wallet = LocalEthersWallet::create_wallet(network.clone()).unwrap();

        // Serialize the wallet
        let serialized_wallet = serde_json::to_string(&wallet).unwrap();

        // Deserialize the wallet
        let deserialized_wallet: LocalEthersWallet = serde_json::from_str(&serialized_wallet).unwrap();

        // Compare the original and deserialized wallets
        assert_eq!(wallet.id, deserialized_wallet.id);
        assert_eq!(wallet.network, deserialized_wallet.network);
        assert_eq!(wallet.address, deserialized_wallet.address);
        assert_eq!(wallet.wallet.address(), deserialized_wallet.wallet.address());
        assert_eq!(wallet.provider.url(), deserialized_wallet.provider.url());
    }
}