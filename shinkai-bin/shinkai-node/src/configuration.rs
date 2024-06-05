pub struct Configuration {
    pub global_identity_name: String,
    pub identity_secret_key: SignatureStaticKey,
    pub identity_public_key: SignaturePublicKey,
    pub encryption_secret_key: EncryptionStaticKey,
    pub encryption_public_key: EncryptionPublicKey,
    pub listen_address: SocketAddr,
    pub api_listen_address: SocketAddr,
    pub ping_interval: u64,
}

impl Configuration {
    pub fn new() -> Self {
        // Initialization and fetching of environment variables go here
        // ...
    }
}

