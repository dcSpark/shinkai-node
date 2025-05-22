import {
  ChainIdToNetwork,
  PaymentRequirements,
  PaymentRequirementsSchema,
} from "npm:x402/types";
import { evm } from "npm:x402/types";
import {
  createPaymentHeader,
  selectPaymentRequirements,
} from "npm:x402/client";
import { privateKeyToAccount } from "npm:viem/accounts";
import { Account } from "npm:viem";

type Parameters = {
  accepts: PaymentRequirements[];
  x402Version: number;
  privateKey: `0x${string}`;
};

type Output = {
  payment: string;
};

async function run(
  _configurations: never,
  parameters: Parameters
): Promise<Output> {
  const walletClient: typeof evm.SignerWallet | Account = privateKeyToAccount(
    parameters.privateKey
  );

  const parsed = parameters.accepts.map((x) =>
    PaymentRequirementsSchema.parse(x)
  );

  const chainId = evm.isSignerWallet(walletClient)
    ? walletClient.chain?.id
    : evm.isAccount(walletClient)
    ? walletClient.client?.chain?.id
    : undefined;

  const selectedPaymentRequirements = selectPaymentRequirements(
    parsed,
    chainId ? ChainIdToNetwork[chainId] : undefined,
    "exact"
  );
  const paymentHeader = await createPaymentHeader(
    walletClient,
    parameters.x402Version,
    selectedPaymentRequirements
  );

  return {
    payment: paymentHeader,
  };
}
