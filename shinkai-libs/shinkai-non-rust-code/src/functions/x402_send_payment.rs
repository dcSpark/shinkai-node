use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Serialize)]
pub struct Configurations {
    pub private_key: String,
    pub network: String,
}

#[derive(Serialize)]
pub struct Input {
    pub url: String,
    pub amount: String,
    pub token_address: Option<String>,
    pub invoice_id: String,
}

#[derive(Deserialize)]
pub struct Output {
    pub tx_hash: String,
}

pub async fn send_payment(
    private_key: String,
    network: String,
    url: String,
    amount: String,
    token_address: Option<String>,
    invoice_id: String,
) -> Result<Output, RunError> {
    let code = r#"
        import { createWalletClient, http } from 'npm:viem';
        import { privateKeyToAccount } from 'npm:viem/accounts';
        import { withPaymentInterceptor } from 'npm:x402-axios';
        import axios from 'npm:axios';
        import { base, baseSepolia } from 'npm:viem/chains';

        async function run(configurations, parameters) {
            const chain = configurations.network === 'base' ? base : baseSepolia;
            const account = privateKeyToAccount(configurations.private_key);
            const client = createWalletClient({ account, chain, transport: http() });
            const api = withPaymentInterceptor(axios.create({ baseURL: parameters.url }), client);
            const res = await api.post('/', { amount: parameters.amount, token: parameters.token_address, invoiceId: parameters.invoice_id });
            return { tx_hash: res.data?.hash ?? '' };
        }
    "#;

    let configs = Configurations { private_key, network };
    let runner = NonRustCodeRunnerFactory::new("x402_send_payment", code.to_string(), vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(configs);
    runner
        .run::<_, Output>(Input { url, amount, token_address, invoice_id })
        .await
}
