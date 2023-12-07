Shinkai Smart Contract protocol overview
https://shinkai-contracts.pages.dev/

Key features/requirements:
User can register an identity (e.g. nico.shinkai) by staking amount of SHIN tokens proportionally inverse to the length of identity name (function)
User can stake more than what is needed to buy that specific identity, and should be able to partially withdraw that extra difference without losing the identity
Identity registration requires submitting an address that will be the owner of the identity
User can unstake SHIN tokens, losing the ownership of the identity
Staked tokens are isolated to identities, meaning staking extra for first identity does not carry over to any discount for the second
User can transfer the identity along with the staked SHIN tokens to another address
Identity hold information like node address/proxy nodes, encryption key, signature key, and a “metadata” hashmap
When a user registers an identity they'll be able to choose one of two options:
Supply their own dns/ip:port (node address)
Specify one or more identities which will be their "proxy nodes"
Users can batch register many identities in one transaction, batch transfer identities, batch unregister (unstake) identities
SHIN token
Inflationary 
No forced global inflation rate, so nothing like "5% yearly inflation no matter what"
Inflation applies to staked tokens so there is a set target inflation rate with regards to estimated staking rate
Able to be staked, giving payouts in 2:1 ratio of <user rewards>:<delegated rewards>
After user registers identity, they can set delegation (array of identities and delegated tokens)
Owner of an identity can claim (withdraw) personal staking rewards + delegated rewards together
Read endpoint for amount of tokens delegated from one delegating identity -> target identity
Read endpoint that adds up all delegations yearly rewards from all identities owned by a specific address towards a target identity
Read endpoint that given an identity returns ip:port list of the identity/it's proxys
From start only ".shinkai" namespace will be available, but prepare for more in the future
Contracts need to be upgradeable
Contracts
RegistryControlled
Helper contract that others will inherit. Inherits standard Ownable.
address registry variable that holds the address that will be permitted to do certain actions
onlyRegistry modifier that reverts if msg.sender is not registry
setRegistry(address registry_) function the owner can call to set registry
ShinkaiToken
Standard ERC20 token that inherits RegistryControlled
Name: Shinkai
Symbol: SHIN
Initial supply: ???
Has a public mint function that is protected with the onlyRegistry modifier
ShinkaiNFT
ERC721A (Azuki standard), that inherits RegistryControlled
Name: Shinkai NFT
Symbol: SHINKAI
Has a public mint and burn functions that are protected with the onlyRegistry modifier
Has transferFromBatched and safeTransferFromBatched batched functions variants
_beforeTokenTransfers hook is implemented to ensure claiming rewards before transferring the NFT
ShinkaiRegistry
Declarations
Data structure IdentityRecord
uint256 boundNft
uint256 stakedTokens - how many tokens are staked for this identity in the registry
string encryptionKey - data for node
string signatureKey - data for node
bool routing - true if routing should be used (addressOrProxyNodes contains proxy nodes identities), else false (addressOrProxyNodes contains just one node address)
string[] addressOrProxyNodes - either node DNS/IP:PORT or one or more identities
uint256 delegatedTokens - Amount of tokens this identity has got delegated from others
Name + Namespace  = Identity
"nico" + ".shinkai" = "nico.shinkai"
Basic identity functions
All read/write functions that are interacting with an identity specify the identity by using the whole identity as an input argument, except for two: claimIdentity and decreaseStake. These two functions need to use identityStakeRequirement(string name, uint256 namespace) and for that, name and namespace needs to be separate.

Write functions:
claimIdentity(string name, uint256 namespace, uint256 stakeAmount, address owner)
Claim an available identity specified by name and namespace (for self or any other address). Namespace 0 means ".shinkai", other values are rejected now.
Will mint owner one Shinkai NFT which this identity will be bound to.
This function will transfer a stakeAmount of SHIN tokens to the registry. stakeAmount must be at least the stake requirement of the identity. Identity stake requirement is determined by function identityStakeRequirement (mathematical function to be defined).
User can transfer (stake) more than identityStakeRequirement if he wishes to do so.
claimIdentityBatched(string[] names, uint256[] namespaces, uint256[] stakeAmounts, address[] owners)
Batch function for claimIdentity
decreaseStake(string name, uint256 namespace, uint256 amount)
Identity owner can partially withdraw stake if that identity has more staked SHIN than what its stake requirement is.
Resulting staked amount must remain greater or equal to identity stake requirement.
Resulting staked amount must remain greater or equal to total amount of delegated tokens by the identity.
increaseStake(string identity, uint256 amount)
Any user can increase stake for specified identity.
unclaimIdentity(string identity) 
Identity owner can unstake (withdraw all) staked tokens for an identity, giving up the ownership of that identity and erasing its data. It burns the Shinkai NFT bound to that identity.
unclaimIdentityBatched(string[] identities) 
Batch function for unclaimIdentity
resetIdentityData(string identity)
Identity owner can use this function to reset identity keys, node addresses, and delegations
setNodeAddress, setProxyNodes, setKeys
Identity owner can use these to set respective data values for an identity.
setNodeAddress sets routing to false and sets addressOrProxyNodes
setProxyNodes sets routing to true and sets addressOrProxyNodes
updateMetadata(string identity, string[] keys, string[] values)
Identity owner can update key-value pairs of identity's metadata mapping.
deleteMetadata(string identity, string[] keys, string[] values)
Identity owner can update key-value pairs of identity's metadata mapping.
readMetadata(string identity, string[] keys, string[] values)
Read functions:
getIdentityRecord(string identity)
Returns the IdentityRecord for specified identity
ownerOf(string identity)
Shortcut function, returns the owner of the NFT bound to the identity
stakedTokensOf(string identity)
Shortcut function, returns the stakedTokens of identity
getIdentityNodeAddresses(string identity)
If specified identity has routing disabled, then returns directly its addressOrProxyNodes (array containing node address), else it will recursively call this function on each identity in addressOrProxyNodes and return array of results
getDelegatedTokensTowards(string fromIdentity, string toIdentity)
Returns the amount of tokens delegated by fromIdentity towards toIdentity
Staking
uint baseRewardsRate
Means amount of SHIN tokens to be emitted per one staked SHIN token, per block.
Is meant as a base rate, and if we want to use 2:1 ratio for 'staking rewards':'delegation rewards', it means that staking rewards will use 2x this
struct RewardsState 
uint224 index - protocol's last updated rewards state index - index is a cumulative sum of SHIN tokens per staked SHIN token accrued
uint32 block - block number when the index was last updated at
RewardsState rewardsState
Holds information about cumulative sum of rewards pertaining to specific time. This is used to calculate user's staking and delegation rewards.
mapping(string => uint) identityStakingIndex
The staking rewards index for identity as of the last time they accrued staking rewards
Needs to be set to updated rewardsState.index when claiming an identity
internal function updateRewardsState()
Will update rewardsState.index to add (block.number - rewardsState.block) * baseRewardsRate, and update rewardsState.block to block.number
function setBaseRewardsRate(uint rate)
Owner of Registry contract can set baseRewardsRate. Must execute updateRewardsState before setting the new rate to accumulate rewards for prior rate.
function claimStakingRewards(string identity)
Firstly, execute updateRewardsState to have up-to-date rewardsState. Then calculate how many tokens the identity owner has accrued since last time with (rewardsState.index - identityStakingIndex(identity)) * 2 * stakedTokensOf(identity)and mint that amount of tokens to the identity owner. Finally, update identityStakedIndex(identity) to rewardsState.index.
This function has to be executed also everytime stakedTokensOf balance changes (so when increasing/decreasing stake or unstaking)
function claimStakingRewardsBatched(string[] identities)
Batched version of claimStakingRewards

Delegation
Design is similar to staking rewards system, but we want to keep track of accrued delegation rewards for identities, because the amount of delegated tokens are volatile by actions of delegators (changing delegations, resetting identity data, unclaiming identity), and we don't want to mint tokens to delegatees with each such action.
From user perspective, they are setting delegations by simply specifying a subset amount of their staked tokens to some other identities. For example if I have an identity with 100 staked SHIN, I can delegate 70 SHIN to identity A and 20 SHIN to identity B. I would still have 10 available tokens to delegate from this identity, and I cannot decrease this identity's stake by more than 10 tokens (need to explicitly change delegations first).
The burden of calculating how much $ does "100 delegated tokens" mean in terms of e.g. yearly income for some service, is left to off-chain calculation, as the blockchain specifics (block time etc.) are volatile and so such calculations wouldn't be practical in the smart contract.
mapping(string => uint) identityDelegationIndex
The delegation rewards index for identity as of the last time they accrued delegation rewards
mapping(string => uint) identityDelegationAccrued
The delegation rewards accrued but not yet transferred to each identity
mapping(string => mapping(string => uint256)) identityDelegations
Delegator => Delegatee => Delegation amount
For having record of delegations of a specific identity when they will need to be unset.
mapping(string => string[]) public identityDelegatees
For being able to iterate through identityDelegations (delegatees of an identity)
function getAvailableTokensForDelegation(string identity)
Returns the difference between the amount of identity's staked tokens and total sum of tokens delegated by the identity.
function getDelegatedTokensTowards(string fromIdentity, string toIdentity)
Returns the amount of tokens delegated from one identity to another.
function setDelegations(string identity, Delegation[] delegations)
Identity owner can set new delegations. This will unset any previous delegations (accruing previous delegatees' rewards beforehand) and set the new ones (also accruing the new delegatees' rewards beforehand).
function accrueDelegationRewards(string identity)
Firstly, execute updateRewardsState to have up-to-date rewardsState. Then calculate how many tokens the user has accrued since last time with (rewardsState.index - identityDelegationIndex(identity)) * delegatedTokensOf(identity)and add that amount to the identityDelegationAccrued of this identity. Finally, update identityDelegationIndex(identity) to rewardsState.index.
This function has to be executed also everytime delegatedTokensOf balance changes (so with each change to a delegation to an identity, including changes to the staked amount of delegator)
function claimDelegationRewards(string identity)
Firstly, execute accrueDelegationRewards to have up-to-date identityDelegationAccrued. Then mint that amount of tokens to the user. Finally, update identityDelegationAccrued[identity] to zero.
function claimDelegationRewardsBatched(string[] identities)
Batched version of claimDelegationRewards
Events
IdentityClaim(string indexed identity, uint256 nftTokenId)
Emitted in claimIdentity, claimIdentityBatched
IdentityUnclaim(string indexed identity, uint256 nftTokenId)
Emitted in unclaimIdentity, unclaimIdentityBatched
StakeUpdate(string indexed identity, uint256 newStake)
Emitted when the amount of staked SHIN tokens for a specified identity changes, so in claimIdentity, claimIdentityBatched, unclaimIdentity, unclaimIdentityBatched, increaseStake, decreaseStake
KeysUpdate(string indexed identity, string encryptionKey, string signatureKey)
Emitted when identity keys are changed, so in setKeys, resetIdentityData, unclaimIdentity, unclaimIdentityBatched
AddressOrProxyNodesUpdate(string indexed identity, bool routing, string[] addressOrProxyNodes)
Emitted when the addressOrProxyNodes of the identity is changed, so in setNodeAddress, setProxyNodes, resetIdentityData, unclaimIdentity, unclaimIdentityBatched
MetadataUpdate(string indexed identity, string[] keys, string[] values)
Emitted when metadata of the identity is changed, so in updateMetadata
DelegationsUpdate(string indexed identity, Delegation[] delegations)
Emitted when delegations of the identity are changed, so in setDelegations, resetIdentityData
MetadataRemoval(string indexed identity, string[] keys)
Emitted when metadata of the identity is removed, so in removeMetadata
BaseRewardsRateUpdate(uint256 newRate)
Emitted when baseRewardsRate is updated, so in setBaseRewardsRate
StakingRewardsClaim(string indexed identity, uint256 rewards)
Emitted when staking rewards are claimed, so in decreaseStake, increaseStake, unclaimIdentity, unclaimIdentityBatched
DelegationRewardsAccrual(string indexed identity, uint256 rewards)
Emitted when delegation rewards are accrued, so in accrueDelegationRewards, claimDelegationRewards, setDelegations, resetIdentityData, unclaimIdentity, unclaimIdentityBatched
DelegationRewardsClaim(string indexed identity, uint256 rewards)
Emitted when delegation rewards are claimed, so in claimDelegationRewards
DelegatedTokensUpdate(string indexed identity, uint256 newDelegatedTokens)
Emitted when delegatedTokens of an identity is updated, so in setDelegations, resetIdentityData, unclaimIdentity, unclaimIdentityBatched


Identity stake requirement
Formula:
req = 1000 / (length * length)


Shinkai NFT metadata generation and flow (not implemented yet)
NFTs by ERC721 standard's definition are tracked by numerical token IDs. Metadata consumers (like NFT marketplaces, wallets, etc.) are fetching metadata for specific token by calling function tokenURI(uint256 tokenId) that returns a URI. That URI points to a metadata JSON.

We would like our Shinkai NFTs to have the following metadata structure and information:
{
"name": "<identity>",
"description": "An identity on the Shinkai Network of length: <identity name length>",
"image": "<image URI>"
}

Since our Shinkai NFTs representing identities cannot logically have their metadata generated and stored prior to minting, we need to create the following flow:
Have a server listening to minting events of our NFT contract (technical note: it's actually Transfer event from zero address to non-zero address)
When a minting event (event contains the token ID) is caught: 
Get the identity from the token ID by calling function tokenIdToIdentity(uint256 tokenId) on the ShinkaiRegistry
Generate an image from the identity with some deterministic algorithm.
Store that image somewhere, e.g. on IPFS. 
Save the image IPFS URI for the specific identity into a database
Have a server that responds to a GET request (e.g. https://nft-metadata.shinkai.com/matej.shinkai) for a specific identity with the metadata JSON of above mentioned structure for that specific identity with image URI fetched from the database

And we will set the base address of the server mentioned in point 3 as the base URI of Shinkai NFT before we deploy the contract. 
So when a wallet asks the NFT contract for tokenURI(1), it will get https://nft-metadata.shinkai.com/matej.shinkai, which upon GET request returns JSON 
{
"name": "matej.shinkai",
"description": "An identity on the Shinkai Network of length: 5",
"image": "ipfs:\/\/QmPZWJytgo9JNnKfrqoLmjQ4fyHW5yVo8S8B2Ckh79mALT"
}

Current status:
We have https://nft-metadata.shinkai.com/<identity> that responds with JSON without the image.
https://github.com/dcSpark/shinkai-contracts/tree/main/metadata-server


Shinkai backend indexer & API
There are two non-trivial functions performed by the Shinkai dApp: 
getting a list of all existing shinkai identities
getting a list of connected account's shinkai identities. 
Since this information is not tracked in any gettable array within the smart contracts (would be a waste of resources), we have to get them in a more complicated manner.

Initial naive solution uses a viem's multicall (calling smart contract that can perform multiple read calls to other contracts in one call) for the first function and alchemy-sdk for the second function. However, both of these have certain limits: First has a direct EVM gas limit (so with scale it still has to be divided into multiple calls) and the second has a maximum page size of 100 defined by alchemy (so if user has more than 100 identities, we have to do multiple calls as well). That means that at scale our call counts would be ever-so increasing, which could create problems with rate limiting of the alchemy provider.

Ideal solution is to build a simple backend indexing service with API for the front-end to get values for the two functions. What we need for that is essentially just keeping a track of ownership records in a DB.
But since we will be implementing that, we will extend the API with endpoints for all the remaining data that front-end needs, so that the front-end will not need to query the blockchain for anything.

The service will listen to events described in the Events section and react to them by updating the DB.

https://github.com/dcSpark/shinkai-contracts/tree/main/backend

API endpoints:
/identities?owner=<owner>&filter=<filter>&size=<size>&page=<page>
array of shinkai identities (owned by the owner, if argument used), paginated, filterable
/identity?identity=<identity>
identity data to be displayed on the identity detail page, such as owner, NFT tokenId, staked tokens, keys and node address, delegations, etc.
/claim-history?identity=<identity>&types[]=<type>&size=<size>&page=<page>
history of rewards claims of an identity, paginated, filterable (type: Staking | Delegation)
/base-rewards-rate
protocol's base rewards rate

Tech stack wise this can be done the same way as for Milkomeda Liquid Staking Backend: express, mongoose

This indexer can afterwards be extended with indexing also the staking and delegation information, should we want to display stuff like global statistics of delegations etc.
Backlog
Change naming: routerNodes -> proxyNodes
BE: Track base rewards rate change
BE: Track total amount of tokens delegated towards a specific identity
BE: Change all properties that are of type Date to block height
BE: Validate proxy nodes and delegations are valid (claimed) identities
BE: Put a maximum limit of 100 on page size
BE: Merge all-identities and identities-of into one query with an optional owner parameter. Add ability to substring search identity.
BE: Separate identity delegations information into its own paginated endpoint, add ability to search.
BE: Return only the needed data when querying for list of identities
BE: Track rewards claim history
FE: Pagination for the identities
FE: Search function for identities
FE: Fetch base rewards rate from BE
FE: Validate proxy nodes and delegations are valid (claimed) identities when editing
FE: Rewards section - Add staked/delegated tokens amount and their respective SHIN yearly
