use super::payment_methods::{CryptoToken, CryptoTokenAmount, CryptoWallet};
use crate::payments::payment_manager::PaymentManagerError;
use aes_gcm::aead::generic_array::GenericArray;
use ethers::{abi::Abi, core::k256::SecretKey, prelude::*};
use lazy_static::lazy_static;
use std::{convert::TryFrom, sync::Arc};
use std::convert::TryInto;

// {
//     // Get the latest block number
//     let block_number = provider.get_block_number().await.unwrap();
//     eprintln!("Latest block number: {:?}", block_number);

//     // Get the balance
//     eprintln!("Getting balance for address: {}", from_wallet.address);
//     let from_address: ethers::types::Address = from_wallet.address.parse().unwrap();
//     let balance = provider.get_balance(from_address, None).await.unwrap();
//     eprintln!("Current balance: {:?}", balance);
// }

lazy_static! {
    static ref ERC20_ABI: Abi = serde_json::from_str(
        r#"
        [
            {
                "constant": false,
                "inputs": [
                    {
                        "name": "_to",
                        "type": "address"
                    },
                    {
                        "name": "_value",
                        "type": "uint256"
                    }
                ],
                "name": "transfer",
                "outputs": [
                    {
                        "name": "",
                        "type": "bool"
                    }
                ],
                "payable": false,
                "stateMutability": "nonpayable",
                "type": "function"
            }
        ]
    "#
    )
    .unwrap();
}

pub async fn execute_transaction(
    from_wallet: CryptoWallet,
    to_wallet: CryptoWallet,
    token: CryptoToken,
    send_amount: CryptoTokenAmount,
    provider_url: String,
) -> Result<(), PaymentManagerError> {
    let provider = Provider::<Http>::try_from(provider_url).unwrap();
    let chain_id = provider.get_chainid().await.unwrap().low_u64();
    // eprintln!("Chain ID (from provider): {}", chain_id);

    // Parse the private key from the wallet
    let secret_key_bytes = hex::decode(&from_wallet.unsafe_private_key).unwrap();
    let secret_key_bytes = GenericArray::from_slice(&secret_key_bytes);
    let secret_key = SecretKey::from_bytes(secret_key_bytes).unwrap();

    let local_wallet = LocalWallet::from(secret_key).with_chain_id(chain_id);
    let client = SignerMiddleware::new(provider.clone(), local_wallet);

    // Create a transaction
    let mut tx = TransactionRequest::new();
    tx.to = Some(to_wallet.address.parse().unwrap());

    match &token.address {
        None => {
            tx.value = Some(ethers::types::U256::from(send_amount.amount));
        }
        Some(contract_address_str) => {
            // For ERC20, the value is 0 and the data field is used to call the contract's transfer function
            tx.value = Some(ethers::types::U256::from(0));
            let contract_address = contract_address_str.parse::<ethers::types::Address>().unwrap();
            let contract = Contract::new(contract_address, ERC20_ABI.clone(), Arc::new(provider.clone()));
            let amount: u64 = send_amount.amount.try_into().unwrap();
            let call = contract.method::<(ethers::types::Address, u64), bool>("transfer", (to_wallet.address.parse().unwrap(), amount)).unwrap();
            tx = call.tx.into();
        }
    }

    // Set the chain_id
    // eprintln!("Chain ID: {}", from_wallet.network.chain_id);
    // let chain_id = u64::from_str_radix(&from_wallet.network.chain_id, 10).unwrap();
    // eprintln!("Chain ID (parsed): {}", chain_id);
    tx.chain_id = Some(chain_id.into());
    tx.gas = Some(ethers::types::U256::from(100_000));

    let gas_price = provider.get_gas_price().await.unwrap();
    tx.gas_price = Some(gas_price);

    let from_address: ethers::types::Address = from_wallet.address.parse().unwrap();
    let nonce = provider
        .get_transaction_count(from_address, None)
        .await
        .unwrap();
    tx.from = Some(from_address);
    tx.nonce = Some(nonce);

    // eprintln!("Sending transaction: {:?}", tx);

    // Send the transaction and wait for one confirmation
    let pending_tx = client
        .send_transaction(tx, None)
        .await
        .map_err(|err| PaymentManagerError::TransactionError(err.to_string()))?;
    // eprintln!("Pending transaction: {:?}", pending_tx);

    let _receipt = pending_tx
        .confirmations(1)
        .await
        .map_err(|err| PaymentManagerError::TransactionError(err.to_string()))?;
    // eprintln!("Transaction receipt: {:?}", receipt);

    Ok(())
}
