import { HDNodeWallet, Wallet } from "npm:ethers@6.14.3";

type RecoverySource =
  | {
      mnemonic: string;
    }
  | {
      privateKey: string;
    };

type Parameters = {
  source: RecoverySource;
};

type Output = {
  wallet: {
    privateKey: string;
    publicKey?: string;
    address: string;
    mnemonic?: string;
  };
};

export async function run(
  _configurations: {},
  parameters: Parameters
): Promise<Output> {
  let wallet: Wallet | HDNodeWallet;
  if ("mnemonic" in parameters.source) {
    wallet = Wallet.fromPhrase(parameters.source.mnemonic);
  } else {
    wallet = new Wallet(parameters.source.privateKey);
  }
  return {
    wallet: {
      privateKey: wallet.privateKey,
      publicKey: wallet instanceof HDNodeWallet ? wallet.publicKey : undefined,
      address: wallet.address,
      mnemonic: wallet instanceof HDNodeWallet ? wallet.mnemonic?.phrase : undefined,
    },
  };
}
