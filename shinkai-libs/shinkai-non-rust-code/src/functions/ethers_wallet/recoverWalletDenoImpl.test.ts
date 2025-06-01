import { assertEquals } from "https://deno.land/std@0.201.0/assert/mod.ts";
import { run } from "./recoverWalletDenoImpl.ts";

Deno.test(
  "recoverWalletDenoImpl should recover a valid wallet with private key",
  async () => {
    // Nothing important, just a random generated wallet
    // privateKey: "0xda1abaf1622435f554d80ba2436dbbfb18a8697ef63c4c26a782baaf82334211",
    // publicKey: "0x03e220eaea3b2006a0bd67a62d44130deaa7b608c976844baedef13ce067fbcec9",
    // address: "0x023251Ef2dF395ed0ad5D3771abfEC23ac40e7cD"
    const result = await run(
      {},
      {
        source: {
          privateKey:
            "0xda1abaf1622435f554d80ba2436dbbfb18a8697ef63c4c26a782baaf82334211",
        },
      }
    );
    console.log(result);
    const { wallet } = result;
    assertEquals(
      wallet.address,
      "0x023251Ef2dF395ed0ad5D3771abfEC23ac40e7cD",
      "Address should be the same"
    );
    assertEquals(
      wallet.privateKey,
      "0xda1abaf1622435f554d80ba2436dbbfb18a8697ef63c4c26a782baaf82334211",
      "Private key should be the same"
    );
    assertEquals(wallet.publicKey, undefined);
  }
);

Deno.test(
  "recoverWalletDenoImpl should recover a valid wallet with mnemonic",
  async () => {
    // Nothing important, just a random generated wallet
    // privateKey: "0x53840710bca86bcc8e331dd3c2483becea1d5dc65731ade8f3276813a1b2ba04",
    // publicKey: "0x024c3c73ac45e1ecb3dfa269d72cba48e5cf012c6936488b2893379c754593612e",
    // address: "0x84310102F55C513EdB2795A5384bC674521AD6f3",
    // mnemonic: "envelope same educate win over stuff ghost fly exercise tissue reform remember"
    const result = await run(
      {},
      {
        source: {
          mnemonic:
            "envelope same educate win over stuff ghost fly exercise tissue reform remember",
        },
      }
    );
    console.log(result);
    const { wallet } = result;
    assertEquals(
      wallet.address,
      "0x84310102F55C513EdB2795A5384bC674521AD6f3",
      "Address should be the same"
    );
    assertEquals(
      wallet.privateKey,
      "0x53840710bca86bcc8e331dd3c2483becea1d5dc65731ade8f3276813a1b2ba04",
      "Private key should be the same"
    );
    assertEquals(
      wallet.publicKey,
      "0x024c3c73ac45e1ecb3dfa269d72cba48e5cf012c6936488b2893379c754593612e",
      "Public key should be the same"
    );
  }
);
