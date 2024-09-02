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
pub struct CoinbaseMPCWallet {
    pub id: String,
    pub network: Network,
    pub address: Address,
    // pub wallet: Wallet<SigningKey>,
    // pub provider: LocalWalletProvider,

    // Note: do we need access to ToolRouter? (maybe not, since we can call the Coinbase SDK directly)
    // Should we create a new manager that calls the Coinbase MPC SDK directly? (Probably)
    // So we still need access to lancedb so we can get the code for each tool
    // If we use lancedb each time (it's slightly slower) but we can have everything in sync

    // We could have an UI in Settings, where we can select the Coinbase Wallet or the Ethers Local Wallet

    // Note: maybe we should create a new struct that holds the information about Config + Params + Results (for each tool)
    // based on what we have in the typescript tools
}

// List of required tools
// 1- shinkai-tool-coinbase-create-wallet
// 2- shinkai-tool-coinbase-get-my-address
// 3- shinkai-tool-coinbase-get-balance
// 4- shinkai-tool-coinbase-get-transactions
// 5- shinkai-tool-coinbase-send-tx
