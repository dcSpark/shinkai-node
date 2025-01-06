use ed25519_dalek::{ed25519::signature::SignerMut, SigningKey, VerifyingKey};

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};

#[derive(Debug, Deserialize)]
struct ChallengeResponse {
    challenge: String,
}

#[derive(Deserialize)]
struct EmailResponse {
    email: String,
}

#[derive(Debug)]
pub struct ShinkaiEmails {
    pub api_url: String,
    pub verifying_key: VerifyingKey,
    pub signing_key: SigningKey,
}

impl ShinkaiEmails {
    pub fn new(api_url: String, verifying_key: VerifyingKey, signing_key: SigningKey) -> Self {
        ShinkaiEmails {
            api_url,
            verifying_key,
            signing_key,
        }
    }

    async fn get_challenge(&self, verifying_key: &str) -> Result<ChallengeResponse, Box<dyn std::error::Error>> {
        shinkai_log(
            ShinkaiLogOption::Email,
            ShinkaiLogLevel::Debug,
            &format!("getting challenge for verifying key: {}", verifying_key),
        );
        let client = Client::new();
        let response = match client
            .post(format!("{}/api/v1/challenge", self.api_url))
            .header("Content-Type", "application/json")
            .json(&json!({ "verifyingKey": verifying_key }))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Email,
                    ShinkaiLogLevel::Error,
                    &format!("Error getting challenge: {}", e),
                );
                return Err(Box::new(e));
            }
        };

        let response_body: ChallengeResponse = response.json().await?;

        shinkai_log(
            ShinkaiLogOption::Email,
            ShinkaiLogLevel::Debug,
            &format!("successfully received challenge: {}", response_body.challenge),
        );
        Ok(response_body)
    }

    fn get_auth_headers(
        &self,
        verifying_key: &str,
        challenge: &str,
        signed_challenge: &str,
    ) -> reqwest::header::HeaderMap {
        shinkai_log(
            ShinkaiLogOption::Email,
            ShinkaiLogLevel::Debug,
            "greating authentication headers",
        );
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("X-AUTH-CHALLENGE-VERIFYING-KEY", verifying_key.parse().unwrap());
        headers.insert("X-AUTH-CHALLENGE-CHALLENGE", challenge.parse().unwrap());
        headers.insert("X-AUTH-CHALLENGE-SIGNED-CHALLENGE", signed_challenge.parse().unwrap());
        headers
    }

    async fn get_authenticated_client(&self) -> Result<Client, Box<dyn std::error::Error>> {
        shinkai_log(
            ShinkaiLogOption::Email,
            ShinkaiLogLevel::Debug,
            "getting authenticated client",
        );
        // Get the challenge from the server
        let challenge = self
            .get_challenge(&hex::encode(self.verifying_key.to_bytes()))
            .await?
            .challenge;
        // Sign the challenge
        let mut signing_key = self.signing_key.clone();
        let signed_challenge = signing_key.sign(challenge.as_bytes());

        // Prepare the authentication headers
        let headers = self.get_auth_headers(
            &hex::encode(self.verifying_key.to_bytes()),
            &challenge,
            &hex::encode(signed_challenge.to_bytes()),
        );
        // Send the authentication request
        let client = Client::builder().default_headers(headers).build()?;
        shinkai_log(
            ShinkaiLogOption::Email,
            ShinkaiLogLevel::Debug,
            "successfully created authenticated client",
        );
        Ok(client)
    }

    async fn create_email(&self, password: &str) -> Result<String, Box<dyn std::error::Error>> {
        shinkai_log(ShinkaiLogOption::Email, ShinkaiLogLevel::Info, "creating new email");
        let client = self.get_authenticated_client().await?;
        let response = match client
            .post(format!("{}/api/v1/email", self.api_url))
            .header("Content-Type", "application/json")
            .json(&json!({ "password": password }))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Email,
                    ShinkaiLogLevel::Error,
                    &format!("failed to create email: {}", e),
                );
                return Err(Box::new(e));
            }
        };

        let response_body: EmailResponse = response.json().await?;
        shinkai_log(
            ShinkaiLogOption::Email,
            ShinkaiLogLevel::Info,
            &format!("successfully created email: {}", response_body.email),
        );
        Ok(response_body.email)
    }

    pub async fn initialize_email(&self, password: &str) -> Result<String, Box<dyn std::error::Error>> {
        shinkai_log(ShinkaiLogOption::Email, ShinkaiLogLevel::Info, "initializing email");
        let email = self.create_email(password).await?;
        shinkai_log(
            ShinkaiLogOption::Email,
            ShinkaiLogLevel::Info,
            &format!("successfully initialized email: {}", email),
        );
        Ok(email)
    }
}

#[tokio::test]
async fn initialize_email() {
    let secret_key_bytes: [u8; 32] = hex::decode("209dd64296d9b3673b9e07c64d0047f0977372a4a60f0a80fe6f3f381bf49178")
        .unwrap()
        .try_into()
        .unwrap();
    let secret_key = SigningKey::from_bytes(&secret_key_bytes);
    let public_key = secret_key.verifying_key();
    println!("secret key: {}", hex::encode(secret_key.to_bytes()));
    println!("public key: {}", hex::encode(public_key.to_bytes()));

    let email = ShinkaiEmails::new(
        "https://shinkai-emails-302883622007.us-central1.run.app".to_string(),
        public_key,
        secret_key,
    )
    .initialize_email("passwordpassword")
    .await;
    println!("email: {:?}", email);
    assert!(email.is_ok());
    println!("email: {}", email.unwrap());
}
