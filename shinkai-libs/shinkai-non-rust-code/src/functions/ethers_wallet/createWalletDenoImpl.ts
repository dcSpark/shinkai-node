import { Wallet } from "npm:ethers@6.14.3";

type Parameters = {};

type Output = {
  wallet: {
    privateKey: string;
    publicKey: string;
    address: string;
  };
};

export async function run(
  _configurations: {},
  _parameters: Parameters
): Promise<Output> {
  const wallet = Wallet.createRandom();
  return {
    wallet: {
      privateKey: wallet.privateKey,
      publicKey: wallet.publicKey,
      address: wallet.address,
    },
  };
}
