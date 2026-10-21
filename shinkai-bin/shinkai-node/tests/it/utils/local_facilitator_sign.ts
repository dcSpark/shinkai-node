import { mnemonicToAccount } from "npm:viem/accounts";
async function run(_config: never, _parameters: never) {
  const account = mnemonicToAccount(
    "test test test test test test test test test test test junk",
  );
  const authorization = {
    from: account.address,
    to: "0xd68b44BcAB515C326226392922fC08c3C4913746",
    value: "1",
    validAfter: "0",
    validBefore: String(Math.floor(Date.now() / 1000) + 300),
    nonce: `0x${"12".repeat(32)}`,
  };
  const signature = await account.signTypedData({
    domain: {
      name: "USDC",
      version: "2",
      chainId: 84532,
      verifyingContract: "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
    },
    primaryType: "TransferWithAuthorization",
    types: {
      TransferWithAuthorization: [
        { name: "from", type: "address" },
        { name: "to", type: "address" },
        { name: "value", type: "uint256" },
        { name: "validAfter", type: "uint256" },
        { name: "validBefore", type: "uint256" },
        { name: "nonce", type: "bytes32" },
      ],
    },
    message: {
      ...authorization,
      value: 1n,
      validAfter: 0n,
      validBefore: BigInt(authorization.validBefore),
    },
  });
  const request = {
    x402Version: 1,
    paymentPayload: {
      x402Version: 1,
      scheme: "exact",
      network: "base-sepolia",
      payload: { signature, authorization },
    },
    paymentRequirements: {
      scheme: "exact",
      network: "base-sepolia",
      asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
      payTo: authorization.to,
      maxAmountRequired: "1",
    },
  };
  return request;
}
