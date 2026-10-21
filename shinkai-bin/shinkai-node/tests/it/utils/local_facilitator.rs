use serde::Deserialize;
use serde_json::{json, Value};
use shinkai_message_primitives::schemas::x402_types::FacilitatorConfig;
use shinkai_non_rust_code::{NonRustCodeRunnerFactory, NonRustRuntime};
use std::{collections::HashSet, convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use warp::{http::StatusCode, Filter, Reply};

#[derive(Default)]
struct Payments {
    verified: HashSet<String>,
    settled: HashSet<String>,
}

/// A local protocol fixture, not a blockchain simulator. It verifies real signed
/// EIP-3009 authorizations but never submits a transaction or calls a chain RPC.
pub struct LocalFacilitator {
    pub config: FacilitatorConfig,
    payments: Arc<Mutex<Payments>>,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Deserialize)]
struct Verification {
    valid: bool,
    payer: Option<String>,
}

impl LocalFacilitator {
    pub fn start() -> Self {
        let payments = Arc::new(Mutex::new(Payments::default()));
        let state = payments.clone();
        let routes = warp::post()
            .and(warp::path::param::<String>())
            .and(warp::path::end())
            .and(warp::body::content_length_limit(32 * 1024))
            .and(warp::body::json())
            .and_then(move |operation: String, request: Value| {
                let state = state.clone();
                async move {
                    if operation != "verify" && operation != "settle" {
                        return Ok::<_, Infallible>(warp::reply::with_status(warp::reply::json(&json!({"error":"Unknown operation"})), StatusCode::NOT_FOUND));
                    }
                    let runner = NonRustCodeRunnerFactory::new("local_facilitator_verify", include_str!("local_facilitator_verify.ts"), vec![])
                        .with_runtime(NonRustRuntime::Deno).create_runner(json!({}));
                    let validation = runner.run::<_, Verification>(request.clone(), Some(Duration::from_secs(30))).await;
                    let Ok(Verification { valid: true, payer: Some(payer) }) = validation else {
                        return Ok(warp::reply::with_status(warp::reply::json(&json!({"isValid":false,"invalidReason":"Invalid signed authorization","success":false,"errorReason":"Invalid signed authorization"})), StatusCode::OK));
                    };
                    let authorization = &request["paymentPayload"]["payload"]["authorization"];
                    let identity = serde_json::to_string(authorization).unwrap();
                    let mut state = state.lock().await;
                    let response = if operation == "verify" {
                        state.verified.insert(identity);
                        json!({"isValid":true,"payer":payer})
                    } else if state.verified.contains(&identity) && state.settled.insert(identity) {
                        json!({"success":true,"payer":payer,"network":"base-sepolia","transaction":format!("0x{}", "ab".repeat(32))})
                    } else {
                        json!({"success":false,"errorReason":"Authorization was not verified or was already settled"})
                    };
                    Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
                }
            });
        let (address, server) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));
        Self {
            config: FacilitatorConfig {
                url: format!("http://{address}"),
            },
            payments,
            task: tokio::spawn(server),
        }
    }

    pub async fn assert_paid_flow(&self) {
        let payments = self.payments.lock().await;
        assert!(
            !payments.verified.is_empty(),
            "The local facilitator did not verify a payment"
        );
        assert_eq!(
            payments.verified, payments.settled,
            "Every verified payment must settle locally"
        );
    }
}

impl Drop for LocalFacilitator {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_facilitator_validates_signed_payments_and_settles_only_once() {
        let facilitator = LocalFacilitator::start();
        let runner = NonRustCodeRunnerFactory::new(
            "local_facilitator_sign",
            include_str!("local_facilitator_sign.ts"),
            vec![],
        )
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
        let request: Value = runner.run(json!({}), Some(Duration::from_secs(30))).await.unwrap();
        let client = reqwest::Client::new();
        let send = |operation: &str, request: Value| {
            let client = client.clone();
            let endpoint = format!("{}/{operation}", facilitator.config.url);
            async move {
                client
                    .post(endpoint)
                    .json(&request)
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap()
            }
        };
        // A valid signature alone cannot settle before the verify step.
        assert_eq!(send("settle", request.clone()).await["success"], false);
        let mut tampered = request.clone();
        tampered["paymentPayload"]["payload"]["authorization"]["value"] = json!("2");
        assert_eq!(send("verify", tampered).await["isValid"], false);
        let mut bad_signature = request.clone();
        bad_signature["paymentPayload"]["payload"]["signature"] = json!(format!("0x{}", "00".repeat(65)));
        assert_eq!(send("verify", bad_signature).await["isValid"], false);
        assert_eq!(send("verify", request.clone()).await["isValid"], true);
        assert_eq!(send("settle", request.clone()).await["success"], true);
        assert_eq!(send("settle", request).await["success"], false);
        facilitator.assert_paid_flow().await;
    }
}
