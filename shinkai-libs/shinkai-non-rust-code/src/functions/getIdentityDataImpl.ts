import { ethers, FetchRequest } from "npm:ethers@6.14.1";

type Configurations = {
  rpc_urls: string[];
  contract_address: string;
  contract_abi: string;
  timeout_rpc_request_ms: number;
};

type Parameters = {
  identityId: string;
};

type IdentityData = [
  boundNft: string,
  stakedTokens: number,
  encryptionKey: string,
  signatureKey: string,
  routing: boolean,
  addressOrProxyNodes: string[],
  delegatedTokens: number,
  lastUpdated: number
];

export async function run(
  configurations: Configurations,
  parameters: Parameters
) {
  let identityData: IdentityData | null = null;

  for (const url of configurations.rpc_urls) {
    const rpcRequest = new FetchRequest(url);
    rpcRequest.timeout = configurations.timeout_rpc_request_ms;
    const provider = new ethers.JsonRpcProvider(rpcRequest);

    const contract = new ethers.Contract(
      configurations.contract_address,
      configurations.contract_abi,
      provider
    );
    console.log("trying to call getIdentityData", parameters.identityId);
    try {
      const identityDataResult = await contract.getIdentityData(
        parameters.identityId
      );
      if (identityDataResult) {
        const [
          boundNft,
          stakedTokens,
          encryptionKey,
          signatureKey,
          routing,
          addressOrProxyNodes,
          delegatedTokens,
          lastUpdated,
        ] = identityDataResult;

        // Default data is equivalent to unexisting identity
        if (
          boundNft == 0 &&
          stakedTokens == 0 &&
          encryptionKey === "" &&
          signatureKey === "" &&
          !routing &&
          addressOrProxyNodes.length === 0 &&
          delegatedTokens == 0 &&
          lastUpdated == 0
        ) {
          console.log("identityData is empty");
          continue;
        }

        identityData = {
          boundNft: boundNft.toString() + "n",
          stakedTokens: stakedTokens.toString() + "n",
          encryptionKey,
          signatureKey,
          routing,
          addressOrProxyNodes,
          delegatedTokens: delegatedTokens.toString() + "n",
          lastUpdated: parseInt(lastUpdated.toString()),
        };
        console.log("identityData", identityData);
        break;
      } else {
        console.log("identityData is empty");
      }
    } catch (error) {
      console.log(`getIdentityData failed for rpc:${url} with error:${error}`);
    }
  }
  return { identityData };
}
