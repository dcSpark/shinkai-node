import { assertEquals } from "https://deno.land/std@0.201.0/assert/mod.ts";
import { run } from "./createWalletDenoImpl.ts";

Deno.test("createWalletDenoImpl should create a valid wallet", async () => {
  const result = await run({}, {});
  console.log(result);
  const { wallet } = result;

  // Check that wallet has expected properties
  assertEquals(typeof wallet.address, "string");
  assertEquals(wallet.address.startsWith("0x"), true);
  assertEquals(wallet.address.length, 42);

  assertEquals(typeof wallet.privateKey, "string");
  assertEquals(wallet.privateKey.startsWith("0x"), true);
  assertEquals(wallet.privateKey.length, 66);

  assertEquals(typeof wallet.mnemonic, "string");
});
