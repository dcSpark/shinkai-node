pub enum Payment {
    CC(CC),
    Crypto(Crypto),
}

pub enum Crypto {
    Bitcoin(CryptoWallet),
    EVM(CryptoWallet),
    Solana(CryptoWallet),
    Cardano(CryptoWallet),
}

pub struct CryptoWallet {
    pub address: String,
    pub amount: f64,
    pub network: String,
}

pub struct CryptoToken {
    pub name: String,
    pub symbol: String,
    pub amount: f64,
    pub address: Option<String>,
}

pub struct CC {
    pub number: String,
    pub cvv: String,
    pub exp: String,
    pub amount: f64,
}

pub struct CCBilling {
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
}