#[cfg(test)]
mod tests {
    use bip39::{Language, Mnemonic, Seed};
    use dashmap::DashMap;
    use ethers::core::k256::SecretKey;
    use ethers::signers::LocalWallet;
    use ethers::signers::Signer;
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
    use shinkai_node::payments::execute_transaction::execute_transaction;
    use shinkai_node::payments::payment_manager::{PaymentManager, PaymentManagerError};
    use shinkai_node::payments::payment_methods::CryptoNetwork;
    use shinkai_node::payments::payment_methods::CryptoTokenAmount;
    use shinkai_node::payments::payment_methods::{CryptoPayment, CryptoToken, CryptoWallet, Payment};

    use std::env;
    use std::future::Future;
    use std::pin::Pin;

    // These are mock versions of `execute_transaction` that always return Ok
    fn mock_execute_transaction_bitcoin(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
        _send_amount: CryptoTokenAmount,
        _provider: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    fn mock_execute_transaction_evm(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
        _send_amount: CryptoTokenAmount,
        _provider: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    fn mock_execute_transaction_solana(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
        _send_amount: CryptoTokenAmount,
        _provider: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    fn mock_execute_transaction_cardano(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
        _send_amount: CryptoTokenAmount,
        _provider: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    pub fn create_wallet() -> Result<LocalWallet, Box<dyn std::error::Error>> {
        // Try to read the mnemonic from the environment variable
        match env::var("FROM_WALLET_MNEMONICS") {
            Ok(mnemonic) => {
                // Create a Mnemonic instance from the phrase
                let mnemonic = Mnemonic::from_phrase(&mnemonic, Language::English)?;

                // Generate a Seed from the Mnemonic
                let seed = Seed::new(&mnemonic, "");

                // Generate a `SecretKey<Secp256k1>` from the seed
                let secret_key = SecretKey::from_slice(&seed.as_bytes()[0..32])?;

                // Generate a wallet from the secret key
                let wallet: LocalWallet = LocalWallet::from(secret_key);

                Ok(wallet)
            }
            Err(_) => {
                // If mnemonic is not found, try to read the private key from the environment variable
                match env::var("FROM_WALLET_PRIVATE_KEY") {
                    Ok(private_key) => {
                        // Parse the private key from hex
                        let private_key_bytes = hex::decode(private_key)?;

                        // Generate a `SecretKey<Secp256k1>` from the private key
                        let secret_key = SecretKey::from_slice(&private_key_bytes)?;

                        // Generate a wallet from the secret key
                        let wallet: LocalWallet = LocalWallet::from(secret_key);

                        Ok(wallet)
                    }
                    Err(e) => Err(Box::new(e)),
                }
            }
        }
    }

    #[ignore]
    #[tokio::test]
    async fn test_payment_manager() {
        init_default_tracing(); 
        let sepolia_rpc = "https://public.stackup.sh/api/v1/node/arbitrum-sepolia";
        #[allow(clippy::complexity)]
        let execute_transaction_evm: fn(
            CryptoWallet,
            CryptoWallet,
            CryptoToken,
            CryptoTokenAmount,
            String,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> =
            if std::env::var("USE_MOCK").is_ok() {
                mock_execute_transaction_evm
            } else {
                |from_wallet, to_wallet, token, send_amount, sepolia_rpc| {
                    let from_wallet = from_wallet.clone();
                    let to_wallet = to_wallet.clone();
                    let token = token.clone();
                    let send_amount = send_amount.clone();
                    let sepolia_rpc = sepolia_rpc.clone();
                    Box::pin(execute_transaction(
                        from_wallet,
                        to_wallet,
                        token,
                        send_amount,
                        sepolia_rpc,
                    ))
                }
            };

        let manager = PaymentManager::new(
            mock_execute_transaction_bitcoin,
            execute_transaction_evm,
            mock_execute_transaction_solana,
            mock_execute_transaction_cardano,
        );

        let local_wallet = create_wallet().unwrap();
        eprintln!("Wallet address: {:x}", local_wallet.address());

        let from_wallet = CryptoWallet {
            address: format!("{:x}", local_wallet.address()),
            unsafe_private_key: format!("{:x}", local_wallet.signer().to_bytes()),
            network: CryptoNetwork {
                name: "Ethereum".to_string(),
                chain_id: "11155111".to_string(),
                rpc_url: sepolia_rpc.to_string(),
            },
            tokens: DashMap::new(),
        };

        let to_wallet = CryptoWallet {
            address: "0x3c8cf6ea0461Cf3A5b45068524c61C559ab07233".to_string(),
            unsafe_private_key: "".to_string(),
            network: CryptoNetwork {
                name: "Ethereum".to_string(),
                chain_id: "11155111".to_string(),
                rpc_url: sepolia_rpc.to_string(),
            },
            tokens: DashMap::new(),
        };

        let token = CryptoToken {
            name: "ETH".to_string(),
            symbol: "ETH".to_string(),
            address: None,
            amount: CryptoTokenAmount {
                decimals_places: 18,
                amount: 10u128.pow(18),
            },
        };

        let crypto_payment = CryptoPayment::EVM(from_wallet.clone());
        let payment = Payment::Crypto(crypto_payment.clone());
        let send_token = CryptoTokenAmount {
            decimals_places: 18,
            amount: 10u128.pow(13),
        };

        let result = match payment.clone() {
            Payment::Crypto(crypto) => match crypto.clone() {
                CryptoPayment::EVM(_) => {
                    manager
                        .send_transaction(&crypto, &to_wallet, &token, &send_token, sepolia_rpc.to_string())
                        .await
                }
                _ => Err(PaymentManagerError::UnsupportedNetwork),
            },
            _ => Err(PaymentManagerError::UnsupportedNetwork),
        };

        assert!(result.is_ok());

        // Create SHIN token
        let shin_token = CryptoToken {
            name: "SHIN".to_string(),
            symbol: "SHIN".to_string(),
            address: Some("0xdbed03a7D17FcAA42a34f577d1609101fBce6099".to_string()),
            amount: CryptoTokenAmount {
                decimals_places: 18,
                amount: 10u128.pow(16), // 0.1 SHIN
            },
        };

        // Send SHIN
        let send_shin_token = CryptoTokenAmount {
            decimals_places: 18,
            amount: 10u128.pow(16), // 0.1 SHIN
        };

        let result_shin = match payment {
            Payment::Crypto(crypto) => match crypto.clone() {
                CryptoPayment::EVM(_) => {
                    manager
                        .send_transaction(
                            &crypto,
                            &to_wallet,
                            &shin_token,
                            &send_shin_token,
                            sepolia_rpc.to_string(),
                        )
                        .await
                }
                _ => Err(PaymentManagerError::UnsupportedNetwork),
            },
            _ => Err(PaymentManagerError::UnsupportedNetwork),
        };

        eprintln!("SHIN Result: {:?}", result_shin);
        assert!(result_shin.is_ok());
    }
}
