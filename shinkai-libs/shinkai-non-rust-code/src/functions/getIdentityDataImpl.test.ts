import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { run } from "./getIdentityDataImpl.ts";

Deno.test(
  "getIdentityDataImpl - should return identity data for a valid identity",
  async () => {
    console.log("Current location:", Deno.cwd());
    const result = await run(
      {
        rpc_urls: [
          "https://base-sepolia.blockpi.network/v1/rpc/public",
          "https://sepolia.base.org",
          "https://base-sepolia-rpc.publicnode.com",
          "https://base-sepolia.gateway.tenderly.co",
        ],
        contract_address: "0x425Fb20ba3874e887336aAa7f3fab32D08135BA9",
        contract_abi: await Deno.readTextFile(
          "shinkai-libs/shinkai-crypto-identities/src/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json"
        ),
        timeout_rpc_request_ms: 5000,
      },
      {
        identityId: "official.sep-shinkai",
      }
    );

    assertEquals(result, {
      identityData: {
        boundNft: "4n",
        stakedTokens: "165000000000000000000n",
        encryptionKey:
          "9d89af22de24fcc621ed47a08e98f1c52fada3e49b98462cb02c48237940c85b",
        signatureKey:
          "1ffbfa5d90e7b79b395d034f81ec07ea0c7eabd6c9a510014173c6e5081411d1",
        routing: true,
        addressOrProxyNodes: [],
        delegatedTokens: "0n",
        lastUpdated: 1715000000,
      },
    });
  }
);
