import { JsonRpcProvider, Contract, formatUnits } from "npm:ethers@6.14.3";

// Standard ERC-20 ABI for the functions we need
const ERC20_ABI = [
  "function balanceOf(address owner) view returns (uint256)",
  "function decimals() view returns (uint8)",
  "function symbol() view returns (string)",
  "function name() view returns (string)",
];

type Parameters = {
  tokenAddress?: string;
  walletAddress: string;
  rpcUrl: string;
};

type TokenInfo = {
  name: string;
  symbol: string;
  decimals: number;
};

type Output = {
  balance: string;
  formattedBalance: string;
  tokenInfo: TokenInfo;
};

export async function run(
  _configurations: {},
  parameters: Parameters
): Promise<Output> {
  const provider = new JsonRpcProvider(parameters.rpcUrl);

  console.log("getting balance of ", parameters.walletAddress);

  if (parameters.tokenAddress) {
    const contract = new Contract(parameters.tokenAddress, ERC20_ABI, provider);
    // Get token info using Promise.all for parallel execution
    const [balance, decimals, symbol, name] = await Promise.all([
      contract.balanceOf(parameters.walletAddress),
      contract.decimals(),
      contract.symbol(),
      contract.name(),
    ]);
    provider.destroy();

    // Format the balance using the token's decimals
    const formattedBalance = formatUnits(balance, decimals);

    console.log("balance", balance);
    console.log("formattedBalance", formattedBalance);
    console.log("tokenInfo", {
      name,
      symbol,
      decimals: Number(decimals),
    });
    return {
      balance: balance.toString(),
      formattedBalance,
      tokenInfo: {
        name,
        symbol,
        decimals: Number(decimals),
      },
    };
  } else {
    const balance = await provider.getBalance(parameters.walletAddress);
    provider.destroy();
    const decimals = 18;
    const symbol = "ETH";
    const name = "Ether";
    const formattedBalance = formatUnits(balance, decimals);
    console.log("balance", balance);
    console.log("formattedBalance", formattedBalance);
    console.log("tokenInfo", { name, symbol, decimals });
    return {
      balance: balance.toString(),
      formattedBalance,
      tokenInfo: {
        name,
        symbol,
        decimals,
      },
    };
  }
}
