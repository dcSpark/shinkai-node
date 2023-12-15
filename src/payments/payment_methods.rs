#[derive(Clone, Debug, PartialEq)]
pub enum Payment {
    CC(CC),
    Crypto(Crypto),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Crypto {
    Bitcoin(CryptoWallet),
    EVM(CryptoWallet),
    Solana(CryptoWallet),
    Cardano(CryptoWallet),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CryptoWallet {
    pub address: String,
    // pub amount: f64,
    pub network: String,
    // TODO(Nico): this is just for a PoC
    // The plan is to have a 2-of-2 multisig wallet
    // So even if this is compromised, the funds are safe
    pub unsafe_private_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CryptoToken {
    pub name: String,
    pub symbol: String,
    pub amount: f64,
    pub address: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CC {
    pub number: String,
    pub cvv: String,
    pub exp: String,
    pub amount: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CCBilling {
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
}