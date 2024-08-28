use ethers::abi::Abi;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref ERC20_ABI: Abi = serde_json::from_str(
        r#"
        [
            {
                "constant": false,
                "inputs": [
                    {
                        "name": "_to",
                        "type": "address"
                    },
                    {
                        "name": "_value",
                        "type": "uint256"
                    }
                ],
                "name": "transfer",
                "outputs": [
                    {
                        "name": "",
                        "type": "bool"
                    }
                ],
                "payable": false,
                "stateMutability": "nonpayable",
                "type": "function"
            }
        ]
    "#
    )
    .unwrap();
}
