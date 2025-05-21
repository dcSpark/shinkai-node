import { useFacilitator } from "npm:x402/verify";
import {
  FacilitatorConfig,
  PaymentPayload,
  PaymentRequirements,
  // Price, // Not needed at top level of Parameters
  // Network, // Not needed at top level of Parameters
} from "npm:x402/types";
import {
  findMatchingPaymentRequirements,
  // processPriceToAtomicAmount, // Not needed if requirements are passed in
} from "npm:x402/shared";
import { exact } from "npm:x402/schemes";

type Parameters = {
  payment?: string;
  paymentRequirements: PaymentRequirements[];
  contentId: string; // Added from Rust Input
  buyerId?: string; // Added from Rust Input
  sellerId: string; // Added from Rust Input
  expectedSellerId?: string; // Added from Rust Input
  facilitatorConfig?: FacilitatorConfig; // Changed from facilitator and made optional
  x402Version: number;
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
  const { verify } = useFacilitator(parameters.facilitatorConfig || {}); // Use facilitatorConfig or default

  // If no payment is provided, return the requirements from the input
  if (!parameters.payment) {
    return {
      invalid: {
        error: "No payment provided",
        accepts: parameters.paymentRequirements, // Use directly from input
        x402Version: parameters.x402Version,
      },
    };
  }

  // Verify payment
  let decodedPayment: PaymentPayload;
  try {
    // Assuming exact.evm.decodePayment is a generic way to decode or it needs to be adapted
    // if payment types other than EVM are expected based on paymentRequirements.
    // For now, keeping it as is. The x402Version is part of the JWT itself.
    decodedPayment = exact.evm.decodePayment(parameters.payment);
    // decodedPayment.x402Version = parameters.x402Version; // x402Version is part of the decoded JWT
  } catch (error) {
    return {
      invalid: {
        error: `Invalid or malformed payment header - error: ${error}`,
        accepts: parameters.paymentRequirements, // Use directly from input
        x402Version: parameters.x402Version, // Fallback to this if decode fails early
      },
    };
  }

  const selectedPaymentRequirements = findMatchingPaymentRequirements(
    parameters.paymentRequirements, // Use directly from input
    decodedPayment
  );

  if (!selectedPaymentRequirements) {
    return {
      invalid: {
        error: "Unable to find matching payment requirements",
        accepts: parameters.paymentRequirements, // Use directly from input
        x402Version: decodedPayment.x402Version || parameters.x402Version,
      },
    };
  }

  // Additional checks for contentId, buyerId, sellerId if necessary.
  // The x402 library's verify function might handle some of these internally
  // based on the decodedPayment and selectedPaymentRequirements.
  // For example, `decodedPayment.aud` (audience) should match `contentId`.
  // `decodedPayment.iss` (issuer) should match `buyerId`.
  // `decodedPayment.sub` (subject) or a custom claim should relate to `sellerId`.

  if (decodedPayment.aud !== parameters.contentId) {
    return {
        invalid: {
            error: `Payment audience (aud: ${decodedPayment.aud}) does not match expected contentId (${parameters.contentId})`,
            accepts: parameters.paymentRequirements,
            x402Version: decodedPayment.x402Version || parameters.x402Version,
        },
    };
  }

  if (parameters.buyerId && decodedPayment.iss !== parameters.buyerId) {
    return {
        invalid: {
            error: `Payment issuer (iss: ${decodedPayment.iss}) does not match expected buyerId (${parameters.buyerId})`,
            accepts: parameters.paymentRequirements,
            x402Version: decodedPayment.x402Version || parameters.x402Version,
        },
    };
  }
  
  // `seller_id` in Rust maps to `sub` (subject) in the JWT typically.
  // `expected_seller_id` would also map to `sub`.
  if (decodedPayment.sub !== parameters.sellerId) {
     return {
        invalid: {
            error: `Payment subject (sub: ${decodedPayment.sub}) does not match expected sellerId (${parameters.sellerId})`,
            accepts: parameters.paymentRequirements,
            x402Version: decodedPayment.x402Version || parameters.x402Version,
        },
    };
  }
  if (parameters.expectedSellerId && decodedPayment.sub !== parameters.expectedSellerId) {
    return {
        invalid: {
            error: `Payment subject (sub: ${decodedPayment.sub}) does not match expectedSellerId (${parameters.expectedSellerId})`,
            accepts: parameters.paymentRequirements,
            x402Version: decodedPayment.x402Version || parameters.x402Version,
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
        error: `Unhandled error verifying payment - error: ${error instanceof Error ? error.message : error} - status: 500 - ${JSON.stringify(decodedPayment)}`,
        accepts: parameters.paymentRequirements, // Use directly from input
        x402Version: decodedPayment.x402Version || parameters.x402Version,
      },
    };
  }

  if (!verification.isValid) {
    return {
      invalid: {
        error: `Invalid payment - ${verification.invalidReason}`,
        accepts: parameters.paymentRequirements, // Use directly from input
        payer: verification.payer,
        x402Version: decodedPayment.x402Version || parameters.x402Version,
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
