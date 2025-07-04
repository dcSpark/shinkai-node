use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKeySource {
    pub private_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoverySource {
    Mnemonic(String),
    PrivateKey(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub source: RecoverySource,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredWallet {
    pub private_key: String,
    pub public_key: Option<String>,
    pub address: String,
    pub mnemonic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub wallet: RecoveredWallet,
}

pub async fn recover_wallet(input: Input) -> Result<Output, RunError> {
    let code = include_str!("recoverWalletDenoImpl.ts");
    let runner = NonRustCodeRunnerFactory::new("recover_wallet", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(input, None).await
}

#[cfg(test)]
mod tests {
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    use super::*;

    #[tokio::test]
    async fn test_recover_wallet_from_private_key() {
        let _dir = testing_create_tempdir_and_set_env_var();

        // Nothing important, just a random generated wallet
        // privateKey: "0xda1abaf1622435f554d80ba2436dbbfb18a8697ef63c4c26a782baaf82334211",
        // publicKey: "0x03e220eaea3b2006a0bd67a62d44130deaa7b608c976844baedef13ce067fbcec9",
        // address: "0x023251Ef2dF395ed0ad5D3771abfEC23ac40e7cD"

        let input = Input {
            source: RecoverySource::PrivateKey(
                "0xda1abaf1622435f554d80ba2436dbbfb18a8697ef63c4c26a782baaf82334211".to_string(),
            ),
        };
        let result = recover_wallet(input).await.unwrap();
        println!("result: {:?}", result);
        assert_eq!(result.wallet.address, "0x023251Ef2dF395ed0ad5D3771abfEC23ac40e7cD");
        assert_eq!(
            result.wallet.private_key,
            "0xda1abaf1622435f554d80ba2436dbbfb18a8697ef63c4c26a782baaf82334211"
        );
        assert_eq!(result.wallet.public_key, None);
    }

    #[tokio::test]
    async fn test_recover_wallet_from_mnemonic() {
        let _dir = testing_create_tempdir_and_set_env_var();

        // Nothing important, just a random generated wallet
        // privateKey: "0x53840710bca86bcc8e331dd3c2483becea1d5dc65731ade8f3276813a1b2ba04",
        // publicKey: "0x024c3c73ac45e1ecb3dfa269d72cba48e5cf012c6936488b2893379c754593612e",
        // address: "0x84310102F55C513EdB2795A5384bC674521AD6f3",
        // mnemonic: "envelope same educate win over stuff ghost fly exercise tissue reform remember"

        let input = Input {
            source: RecoverySource::Mnemonic(
                "envelope same educate win over stuff ghost fly exercise tissue reform remember".to_string(),
            ),
        };
        let result = recover_wallet(input).await.unwrap();
        println!("result: {:?}", result);
        assert_eq!(result.wallet.address, "0x84310102F55C513EdB2795A5384bC674521AD6f3");
        assert_eq!(
            result.wallet.private_key,
            "0x53840710bca86bcc8e331dd3c2483becea1d5dc65731ade8f3276813a1b2ba04"
        );
        assert_eq!(
            result.wallet.public_key,
            Some("0x024c3c73ac45e1ecb3dfa269d72cba48e5cf012c6936488b2893379c754593612e".to_string())
        );
    }
}
