use super::payment_methods::{CryptoToken, CryptoTokenAmount, CryptoWallet};
use crate::payments::payment_manager::PaymentManagerError;
use aes_gcm::aead::generic_array::GenericArray;
use ethers::{core::k256::SecretKey, prelude::*};
use std::convert::TryFrom;

pub async fn execute_transaction(
    from_wallet: CryptoWallet,
    to_wallet: CryptoWallet,
    token: CryptoToken,
    send_amount: CryptoTokenAmount,
    provider_url: String,
) -> Result<(), PaymentManagerError> {
    let provider = Provider::<Http>::try_from(provider_url).unwrap();
    let chain_id = provider.get_chainid().await.unwrap().low_u64();
    eprintln!("Chain ID (from provider): {}", chain_id);

    {
        // Get the latest block number
        let block_number = provider.get_block_number().await.unwrap();
        eprintln!("Latest block number: {:?}", block_number);

        // Get the balance
        eprintln!("Getting balance for address: {}", from_wallet.address);
        let from_address: ethers::types::Address = from_wallet.address.parse().unwrap();
        let balance = provider.get_balance(from_address, None).await.unwrap();
        eprintln!("Current balance: {:?}", balance);
    }

    // Parse the private key from the wallet
    let secret_key_bytes = hex::decode(&from_wallet.unsafe_private_key).unwrap();
    let secret_key_bytes = GenericArray::from_slice(&secret_key_bytes);
    let secret_key = SecretKey::from_bytes(secret_key_bytes).unwrap();

    let local_wallet = LocalWallet::from(secret_key).with_chain_id(chain_id);
    let client = SignerMiddleware::new(provider.clone(), local_wallet);

    // TODO(Nico): this is just for a PoC. Expand to read the token to it supports ERC20 and others.

    // Create a transaction
    let mut tx = TransactionRequest::new();
    tx.to = Some(to_wallet.address.parse().unwrap());
    tx.value = Some(ethers::types::U256::from(send_amount.amount));

    // Set the chain_id
    eprintln!("Chain ID: {}", from_wallet.network.chain_id);
    let chain_id = u64::from_str_radix(&from_wallet.network.chain_id, 10).unwrap();
    eprintln!("Chain ID (parsed): {}", chain_id);
    tx.chain_id = Some(chain_id.into());
    tx.gas = Some(ethers::types::U256::from(100_000));

    let gas_price = provider.get_gas_price().await.unwrap();
    tx.gas_price = Some(gas_price);

    let from_address: ethers::types::Address = from_wallet.address.parse().unwrap();
    let nonce = provider.get_transaction_count(from_address.clone(), None).await.unwrap();
    tx.from = Some(from_address);
    tx.nonce = Some(nonce);

    eprintln!("Sending transaction: {:?}", tx);

    // Send the transaction
    client
        .send_transaction(tx, None)
        .await
        .map_err(|err| PaymentManagerError::TransactionError(err.to_string()))?;

    Ok(())
}
