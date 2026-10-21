import { recoverTypedDataAddress } from "npm:viem";

// Offline EIP-3009 verification. This code never creates an RPC client or sends a transaction.
async function run(
  _config: never,
  request: any,
): Promise<{ valid: boolean; payer?: string }> {
  const payment = request.paymentPayload;
  const requirements = request.paymentRequirements;
  const authorization = payment?.payload?.authorization;
  const signature = payment?.payload?.signature;
  if (
    !authorization || typeof signature !== "string" ||
    !/^0x[0-9a-fA-F]{130}$/.test(signature)
  ) return { valid: false };
  const expectedPayer = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"; // public Hardhat test mnemonic
  const token = "0x036CbD53842c5426634e7929541eC2318f3dCF7e";
  const recipient = "0xd68b44BcAB515C326226392922fC08c3C4913746";
  if (
    request.x402Version !== 1 || payment.x402Version !== 1 ||
    payment.scheme !== "exact" ||
    payment.network !== "base-sepolia" || requirements?.scheme !== "exact" ||
    requirements.network !== "base-sepolia" ||
    requirements.asset?.toLowerCase() !== token.toLowerCase() ||
    requirements.payTo?.toLowerCase() !== recipient.toLowerCase() ||
    requirements.maxAmountRequired !== "1" ||
    authorization.from?.toLowerCase() !== expectedPayer ||
    authorization.to?.toLowerCase() !== recipient.toLowerCase() ||
    String(authorization.value) !== requirements.maxAmountRequired ||
    !/^0x[0-9a-fA-F]{64}$/.test(authorization.nonce)
  ) {
    return { valid: false };
  }
  const now = BigInt(Math.floor(Date.now() / 1000));
  try {
    if (
      BigInt(authorization.validAfter) > now ||
      BigInt(authorization.validBefore) <= now
    ) return { valid: false };
    const payer = await recoverTypedDataAddress({
      domain: {
        name: "USDC",
        version: "2",
        chainId: 84532,
        verifyingContract: token,
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
        value: BigInt(authorization.value),
        validAfter: BigInt(authorization.validAfter),
        validBefore: BigInt(authorization.validBefore),
      },
      signature,
    });
    return { valid: payer.toLowerCase() === expectedPayer, payer };
  } catch {
    return { valid: false };
  }
}
