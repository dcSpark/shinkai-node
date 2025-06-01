import { useFacilitator } from "npm:x402/verify";
import {
  FacilitatorConfig,
  PaymentPayload,
  PaymentRequirements,
  Price,
  Network,
} from "npm:x402/types";
import {
  findMatchingPaymentRequirements,
  processPriceToAtomicAmount,
} from "npm:x402/shared";
import { exact } from "npm:x402/schemes";

type Parameters = {
  price: Price;
  network: Network;
  payTo: string;
  payment?: string;
  x402Version: number;
  facilitator: FacilitatorConfig;
};

type Output = {
  invalid?: {
    error: string;
    accepts: PaymentRequirements[];
    x402Version: number;
    payer?: string;
  };
  valid?: {
    decodedPayment: PaymentPayload;
    selectedPaymentRequirements: PaymentRequirements;
  };
};

// deno-lint-ignore no-unused-vars
async function run(
  _configurations: never,
  parameters: Parameters
): Promise<Output> {
  const { verify } = useFacilitator(parameters.facilitator);
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

  // Verify payment
  let decodedPayment: PaymentPayload;
  if (!parameters.payment) {
    return {
      invalid: {
        error: "No payment provided",
        accepts: paymentRequirements,
        x402Version: parameters.x402Version,
      },
    };
  }
  try {
    decodedPayment = exact.evm.decodePayment(parameters.payment);
    decodedPayment.x402Version = parameters.x402Version;
  } catch (error) {
    return {
      invalid: {
        error: `Invalid or malformed payment header - error: ${error}`,
        accepts: paymentRequirements,
        x402Version: parameters.x402Version,
      },
    };
  }

  const selectedPaymentRequirements = findMatchingPaymentRequirements(
    paymentRequirements,
    decodedPayment
  );
  if (!selectedPaymentRequirements) {
    return {
      invalid: {
        error: "Unable to find matching payment requirements",
        accepts: paymentRequirements,
        x402Version: parameters.x402Version,
      },
    };
  }

  let verification;

  try {
    verification = await verify(
      decodedPayment,
      selectedPaymentRequirements
    );
  } catch (error) {
    return {
      invalid: {
        error: `unhandled error verifying payment - error: ${error instanceof Error ? error.message : error} - status: 500 - ${JSON.stringify(decodedPayment)}`,
        accepts: paymentRequirements,
        x402Version: parameters.x402Version,
      },
    };
  }

  if (!verification.isValid) {
    return {
      invalid: {
        error: `Invalid payment - ${verification.invalidReason}`,
        accepts: paymentRequirements,
        payer: verification.payer,
        x402Version: parameters.x402Version,
      },
    };
  }

  return {
    valid: {
      decodedPayment,
      selectedPaymentRequirements,
    },
  };
}
