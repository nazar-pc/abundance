# Consensus

While [Proof-of-Work] and [Proof-of-Stake] have solidified as primary ways to achieve consensus in blockchains, they
both inevitably lead to centralization and potentially to reduction of security.

[Proof-of-Work]: https://wikipedia.org/wiki/Proof_of_work

[Proof-of-Stake]: https://wikipedia.org/wiki/Proof_of_stake

With Proof-of-Work, this is primarily driven by the need to access to exotic hardware and cheap electricity, which
limits the number of people who can participate. With the growth of the network and difficulty increase, it also becomes
impractical for small miners to participate without mining pools due to infrequent and unpredictable rewards. Another
complication is the existence of services like Nicehash that allow to buy large amounts of compute for limited amounts
of time on an open market, which makes it very practical to own the network and in case of anything that is smaller than
[Bitcoin] arguably fairly inexpensive.

[Bitcoin]: https://bitcoin.org

> [!WARNING]
> As the result majority of Proof-of-Work blockchains are neither decentralized nor secure in practice.

With Proof-of-Stake currently staked owners of tokens get richer every day, which arguably makes it a permissioned
system. Due to being permissioned and requiring on-chain registration before participation, most Proof-of-Stake
implementations have to substantially limit the number of consensus participants by imposing things like minimum stake
amount as well as only selecting a subset of validators to be active at any time. Due to the nature of consensus
implementation, it is also important for consensus nodes to stay online, so those unable to are typically being punished
for it on top of simply having their tokens locked and not receiving rewards. This also leads to pooling and
centralization. Blockchains like [Polkadot] support nominated staking that improves scalability of consensus
participation
to some degree, but it is not a full replacement for being able to participate in consensus individually.

[Polkadot]: https://polkadot.com

> [!WARNING]
> As the result majority of Proof-of-Stake networks arguably are not really that decentralized in a sense of supporting
> millions or even billions of consensus participants.

## Proof-of-Space

The alternative to above that is not talked about quite as much is [Proof-of-Space]. There are different variations of
it, but a shared trait between them all is permissionless participation with low energy usage requirements, while also
using the resource that is generic, widely distributed and abundant: disk storage.

[Proof-of-Space]: https://en.wikipedia.org/wiki/Proof_of_space

The most prominent example of Proof-of-Space consensus is probably [Chia]. Chia is essentially an energy efficient
version of Bitcoin that wastes disk space to store random data just like Bitcoin wastes compute to calculate hashes. It
also happens to suffer, just like almost every other Proof-of-Space blockchain from [Farmer's dilemma], which makes
incentive compatibility a challenge.

While this is an improvement over Proof-of-Work, it turns out it could be even better.

[Chia]: https://www.chia.net/

[Farmer's dilemma]: https://academy.autonomys.xyz/subspace-protocol/advancing-blockchain

## Proof-of-Archival-Storage

[Proof-of-Archival-Storage] is a flavor of Proof-of-Space, more specifically Proof-of-Storage, that instead of filling
disks with random data stores the history of the blockchain itself. This not only resolves Farmer's dilemma, but allows
for a few interesting side effects:

[Proof-of-Archival-Storage]: https://academy.autonomys.xyz/subspace-protocol/consensus

* On-chain storage cost no longer needs to be hardcoded, it can be driven by on-chain Automated Market Maker thanks to
  ability to measure not only demand for storage (size of blockchain history), but also supply (space pledged to the
  network by farmers), resulting in approximation of real-world price of the hardware, regardless of the token price
* The history of the blockchain can't be lost regardless of how large it becomes, in an incentive-compatible and
  sustainable way, as long as the blockchain is operational
* Blockchain effectively becomes a Distributed Storage Network since any piece of data uploaded to the network can later
  be retrieved from the network of farmers

To make pools even more interesting to farmers [Autonomys Network] was deployed with a voting mechanism built-in that
increases the frequency of rewards 10x comparing to just having block rewards, making it possible to receive weekly
rewards for even relatively small farmers.

[Autonomys Network]: https://www.autonomys.xyz/

> [!IMPORTANT]
> As a result, Proof-of-Archival-Storage seems to be the closest ideal consensus mechanism that is both permissionless,
> distributed and secure.
