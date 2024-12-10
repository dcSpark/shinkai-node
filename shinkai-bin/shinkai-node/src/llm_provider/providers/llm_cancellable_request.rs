use anyhow::Error;
use reqwest::Client;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

pub struct CancellableRequest {
    cancellation_token: CancellationToken,
}

impl CancellableRequest {
    pub fn new(cancellation_token: CancellationToken) -> Self {
        Self { cancellation_token }
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

pub fn make_cancellable_request(
    client: &Client,
    url: String,
    payload: Value,
) -> (
    CancellableRequest,
    impl std::future::Future<Output = Result<reqwest::Response, Error>>,
) {
    let cancellation_token = CancellationToken::new();
    let child_token = cancellation_token.child_token();

    let request = client.post(url.clone()).json(&payload);

    let future = async move {
        tokio::select! {
            result = request.send() => {
                result.map_err(Error::from)
            },
            _ = child_token.cancelled() => {
                let err = std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "Request cancelled"
                );
                Err(Error::from(err))
            }
        }
    };

    (CancellableRequest::new(cancellation_token), future)
}
