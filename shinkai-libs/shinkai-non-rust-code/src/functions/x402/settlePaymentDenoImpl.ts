import { useFacilitator } from "npm:x402/verify";
import {
FacilitatorConfig,
  PaymentPayload,
  PaymentRequirements,

  settleResponseHeader,
} from "npm:x402/types";
import {
  findMatchingPaymentRequirements,
} from "npm:x402/shared";

type Parameters = {
  facilitator: FacilitatorConfig;
  payment: PaymentPayload;
  accepts: PaymentRequirements[];
};

type Output = {
  invalid?: {
    error: string;
    accepts: PaymentRequirements[];
    x402Version: number;
  };
  valid?: {
    paymentResponse: string;
  };
};

// deno-lint-ignore no-unused-vars
async function run(
  _configurations: never,
  parameters: Parameters
): Promise<Output> {
  const { settle } = useFacilitator(parameters.facilitator);
  
  const selectedPaymentRequirements = findMatchingPaymentRequirements(
    parameters.accepts,
    parameters.payment,
  );
  if (!selectedPaymentRequirements) {
    return {
      invalid: {
        error: "Unable to find matching payment requirements",
        accepts: parameters.accepts,
        x402Version: parameters.payment.x402Version,
      },
    };
  }

  try {
    const settlement = await settle(parameters.payment, selectedPaymentRequirements);
    if (settlement.success) {
      const responseHeader = settleResponseHeader(settlement);
      return {
        valid: {
          paymentResponse: responseHeader,
        },
      };
    } else {
      throw new Error(settlement.errorReason);
    }
  } catch (error) {
    return {
      invalid: {
        error: `Failed to settle payment - error: ${error}`,
        accepts: parameters.accepts,
        x402Version: parameters.payment.x402Version,
      },
    };
  }
}
