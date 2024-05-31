use x25519_dalek::PublicKey as EncryptionPublicKey;

pub struct ShinkaiProxyBuilderInfo {
    pub proxy_enc_public_key: EncryptionPublicKey,
}