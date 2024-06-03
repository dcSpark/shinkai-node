use dashmap::DashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Payment {
    CC(CCPayment),
    Crypto(CryptoPayment),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CryptoPayment {
    BitcoinVM(CryptoWallet),
    EVM(CryptoWallet),
    SolanaVM(CryptoWallet),
    CardanoVM(CryptoWallet),
}

#[derive(Clone, Debug)]
pub struct CryptoWallet {
    pub address: String,
    pub network: CryptoNetwork,
    pub tokens: DashMap<String, CryptoToken>,
    // TODO(Nico): this is just for a PoC
    // The plan is to have a 2-of-2 multisig wallet
    // So even if this is compromised, the funds are safe
    pub unsafe_private_key: String,
}

impl PartialEq for CryptoWallet {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
            && self.network == other.network
            && self.unsafe_private_key == other.unsafe_private_key
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CryptoNetwork {
    pub name: String,
    pub chain_id: String,
    pub rpc_url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CryptoToken {
    pub name: String,
    pub symbol: String,
    pub amount: CryptoTokenAmount,
    pub address: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CryptoTokenAmount {
    pub amount: u128,
    pub decimals_places: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CCPayment {
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