use super::payment_methods::{CryptoWallet, CryptoToken};
use aes_gcm::aead::generic_array::GenericArray;
use ethers::{core::k256::SecretKey, prelude::*};
use std::convert::TryFrom;

async fn execute_transaction(
    from_wallet: &CryptoWallet,
    to_wallet: &CryptoWallet,
    token: &CryptoToken,
) -> Result<(), &'static str> {
    // Here you would add the logic to send a transaction based on the wallet details.
    // For example, if you're sending an EVM transaction:
    let provider = Provider::<Http>::try_from("http://localhost:8545").unwrap();

    // Parse the private key from the wallet
    let secret_key_bytes = hex::decode(&from_wallet.unsafe_private_key).unwrap();
    let secret_key_bytes = GenericArray::from_slice(&secret_key_bytes);
    let secret_key = SecretKey::from_bytes(secret_key_bytes).unwrap();

    let local_wallet = LocalWallet::from(secret_key);
    let client = SignerMiddleware::new(provider, local_wallet.clone());

    // Create a transaction
    let mut tx = TransactionRequest::new();
    tx.to = Some(to_wallet.address.parse().unwrap());
    tx.value = Some(ethers::utils::parse_ether(&token.amount.to_string()).unwrap());

    // Send the transaction
    let _tx_hash = client.send_transaction(tx, None).await.unwrap();

    Ok(())
}