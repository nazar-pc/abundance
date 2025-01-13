# Status quo

> [!NOTE]
> State-of-the-art blockchains are yet to resolve the Blockchain Trilemma

Bitcoin and Ethereum de-facto lost decentralization due to their consensus algorithm, they are also very slow.

Majority of modern blockchains today use a variation of Proof-of-Stake consensus, meaning they are premissioned and
inherently not scalable to unbounded number of participants. Those that are based on Proof-of-Work are arguably not
secure to begin with, regardless of what algorithm they are using due to how cheap and easy it is to purchase compute in
large quantities today to attach the blockchain.

There are many blockchains that claim to be scalable, but all of them have a glass ceiling and unable to scale
bandwidth, storage and compute sub-linearly with unbounded number of participants. Those that do not support sharding
are not scalable by definition, those that are sharded have other inherent limitations that prevent unbounded growth.

Only security is the property that is arguably achieved by many blockchains, but it is also debatable how real or useful
it is without two other properties.

# 🚧 Project Abundance 🚧

The goal here is to find a way to remove existing bottlenecks and unleash the full power of the blockchain.

We need **ABUNDANCE**:

* Abundance of consensus participation
    * Without practical limits
* Abundance of bandwidth, storage and compute
    * With increased consensus participation it must be possible to process more transactions, persist larger history
      and do more computation without practical limits
* Abundance of developers
    * Standard programming languages with familiar tooling, standard and efficient execution environment, infrastructure
      that allows for a single app to scale to the capacity of the blockchain itself
