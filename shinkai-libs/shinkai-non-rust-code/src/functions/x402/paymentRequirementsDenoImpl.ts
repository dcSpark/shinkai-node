import {
  FacilitatorConfig,
  PaymentRequirements,
  Price,
  Network,
} from "npm:x402/types";
import { processPriceToAtomicAmount } from "npm:x402/shared";

type Parameters = {
  price: Price;
  network: Network;
  payTo: string;
  x402Version: number;
  facilitator: FacilitatorConfig;
};

type Output = {
  paymentRequirements: PaymentRequirements[];
};

// deno-lint-ignore no-unused-vars
async function run(
  _configurations: never,
  parameters: Parameters
): Promise<Output> {
  const atomicAmountForAsset = processPriceToAtomicAmount(
    parameters.price,
    parameters.network
  );

  if ("error" in atomicAmountForAsset) {
    throw new Error(atomicAmountForAsset.error);
  }

  const { maxAmountRequired, asset } = atomicAmountForAsset;

  const paymentRequirements: PaymentRequirements[] = [
    {
      scheme: "exact",
      network: parameters.network,
      maxAmountRequired,
      resource: "https://shinkai.com",
      description: "",
      mimeType: "",
      payTo: parameters.payTo,
      maxTimeoutSeconds: 300,
      asset: asset?.address ?? "",
      outputSchema: {},
      extra: asset?.eip712,
    },
  ];

  return {
    paymentRequirements,
  };
} 