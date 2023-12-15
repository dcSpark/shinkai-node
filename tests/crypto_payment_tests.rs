#[cfg(test)]
mod tests {
    use shinkai_node::payments::execute_transaction::execute_transaction;
    use shinkai_node::payments::payment_manager::{PaymentManager, PaymentManagerError};
    use shinkai_node::payments::payment_methods::{Crypto, CryptoToken, CryptoWallet, Payment};

    use std::future::Future;
    use std::pin::Pin;

    // These are mock versions of `execute_transaction` that always return Ok
    fn mock_execute_transaction_bitcoin(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    fn mock_execute_transaction_evm(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    fn mock_execute_transaction_solana(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    fn mock_execute_transaction_cardano(
        _from_wallet: CryptoWallet,
        _to_wallet: CryptoWallet,
        _token: CryptoToken,
    ) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    #[tokio::test]
    async fn test_payment_manager() {
        let execute_transaction_evm: fn(
            CryptoWallet,
            CryptoWallet,
            CryptoToken,
        )
            -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>> =
            if std::env::var("USE_MOCK").is_ok() {
                mock_execute_transaction_evm
            } else {
                |from_wallet, to_wallet, token| {
                    let from_wallet = from_wallet.clone();
                    let to_wallet = to_wallet.clone();
                    let token = token.clone();
                    Box::pin(execute_transaction(from_wallet, to_wallet, token))
                }
            };

        let manager = PaymentManager::new(
            mock_execute_transaction_bitcoin,
            execute_transaction_evm,
            mock_execute_transaction_solana,
            mock_execute_transaction_cardano,
        );
        let from_wallet = CryptoWallet {
            address: "0x123".to_string(),
            network: "EVM".to_string(),
            unsafe_private_key: std::env::var("FROM_WALLET_PRIVATE_KEY").expect("FROM_WALLET_PRIVATE_KEY must be set"),
        };

        let to_wallet = CryptoWallet {
            address: "0x456".to_string(),
            network: "EVM".to_string(),
            unsafe_private_key: "".to_string(),
        };

        let token = CryptoToken {
            name: "ETH".to_string(),
            symbol: "ETH".to_string(),
            amount: 1.0,
            address: None,
        };

        let payment = Payment::Crypto(Crypto::EVM(from_wallet.clone()));

        let result = match payment {
            Payment::Crypto(crypto) => match crypto {
                Crypto::EVM(wallet) => manager.send_transaction(&wallet, &to_wallet, &token).await,
                _ => Err(PaymentManagerError::UnsupportedNetwork),
            },
            _ => Err(PaymentManagerError::UnsupportedNetwork),
        };

        assert!(result.is_ok());
    }
}
