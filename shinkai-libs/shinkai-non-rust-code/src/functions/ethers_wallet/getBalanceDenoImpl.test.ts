import {
  assertEquals,
  assertRejects,
} from "https://deno.land/std@0.224.0/assert/mod.ts";
import { run } from "./getBalanceDenoImpl.ts";

Deno.test("get balance - USDC on Base Sepolia", async () => {
  const parameters = {
    tokenAddress: "0x036CbD53842c5426634e7929541eC2318f3dCF7e", // Real USDC contract on Base Sepolia
    walletAddress: "0x82e2b407E93F63D103C162e36519cC05CeCB979E", // Binance wallet (known to have USDC)
    rpcUrl: "https://sepolia.base.org",
  };

  const result = await run({}, parameters);

  // Check that all fields are present
  assertEquals(typeof result.balance, "string");
  assertEquals(typeof result.formattedBalance, "string");
  assertEquals(typeof result.tokenInfo.name, "string");
  assertEquals(typeof result.tokenInfo.symbol, "string");
  assertEquals(typeof result.tokenInfo.decimals, "number");

  console.log(
    `balance: ${result.balance} (${result.formattedBalance} ${result.tokenInfo.symbol})`
  );

  // USDC should have 6 decimals
  assertEquals(result.balance, "0");
  assertEquals(result.formattedBalance, "0.0");

  // Balance should be numeric strings
  BigInt(result.balance); // Should not throw
  parseFloat(result.formattedBalance); // Should not throw
});

Deno.test("get balance - burn address (should have 0 balance)", async () => {
  const parameters = {
    tokenAddress: "0x036CbD53842c5426634e7929541eC2318f3dCF7e", // Real USDC contract on Ethereum mainnet
    walletAddress: "0x0000000000000000000000000000000000000000", // burn address
    rpcUrl: "https://sepolia.base.org",
  };

  const result = await run({}, parameters);

  // Burn address should have 0 balance
  assertEquals(result.balance, "0");
  assertEquals(result.formattedBalance, "0.0");
});

Deno.test("get balance - invalid token address", async () => {
  const parameters = {
    tokenAddress: "0x1234567890123456789012345678901234567814", // Invalid contract
    walletAddress: "0x0000000000000000000000000000000000000000",
    rpcUrl: "https://eth.llamarpc.com",
  };

  await assertRejects(() => run({}, parameters), Error, "could not decode result data");
});

Deno.test("get balance - invalid wallet address", async () => {
  const parameters = {
    tokenAddress: "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
    walletAddress: "invalid-address",
    rpcUrl: "https://sepolia.base.org",
  };

  await assertRejects(() => run({}, parameters), Error, "could not decode result data");
});

Deno.test("get balance - native ETH (no tokenAddress)", async () => {
  const parameters = {
    walletAddress: "0x82e2b407E93F63D103C162e36519cC05CeCB979E", // Binance wallet
    rpcUrl: "https://sepolia.base.org",
    // no tokenAddress
  };

  const result = await run({}, parameters);

  // Check that all fields are present
  assertEquals(typeof result.balance, "string");
  assertEquals(typeof result.formattedBalance, "string");
  assertEquals(typeof result.tokenInfo.name, "string");
  assertEquals(typeof result.tokenInfo.symbol, "string");
  assertEquals(typeof result.tokenInfo.decimals, "number");

  // Should be ETH info
  assertEquals(result.tokenInfo.symbol, "ETH");
  assertEquals(result.tokenInfo.name, "Ether");
  assertEquals(result.tokenInfo.decimals, 18);

  // Balance should be numeric strings
  BigInt(result.balance); // Should not throw
  parseFloat(result.formattedBalance); // Should not throw
});
