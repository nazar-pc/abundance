---
title: System contracts, trait support and more
date: 2025-01-21
draft: false
description: Updates to contracts infrastructure during last week
tags: [ status-update ]
authors: [ nazar-pc ]
---

Last week was busy on various improvements for contracts infrastructure, trying to clarify existing API and ensuring
everything that might be built is actually possible. First system contracts were introduced, trait support was added and
more, below is a recap of key updates.

<!--more-->

[Environment improvements](https://github.com/nazar-pc/abundance/pull/13) changed the way calls into extension traits
are made, statically guaranteeing that `#[view]` methods can't recursively call methods that potentially `#[update]`
slot contents, this is because `#[view]` methods are supposed to not modify anything and be callable from non-block
context, so it'd be strange for them to be able to access API that is capable of altering persistent data. Now this is
expressed on type system level.

[`#[tmp]` arguments](https://github.com/nazar-pc/abundance/pull/14) were introduced as a bit of ephemeral state that
only lives for a duration of a single transaction processing. This will allow to, for example, approve transfer of a
specific amount of a specific token just for the duration of a single transaction and nothing else, which makes it
possible to make the least privileged contract calls instead of allowing to do everything on behalf of the user by
default. Also, safe contructors for `VariableBytes` and `MaybeData` were added that make calls into smart contracts more
convenient.

[Contacts overview] ([rendered]) was added to the book with some diagrams detailing how storage is organized and
"Everything is a contract" approach that is being tested right now. This was followed up in [PR 19] and [PR 27] with
introduction of system contracts `ab-system-contract-address-allocator`, `ab-system-contract-code` and
`ab-system-contract-state` that implement some core fundamental capabilities. This means that in contrast to older
revisions of the code base "code" and "state" are no longer separate types of storage, they are just slots stored by
system contracts, which happens to be known by the host so it can read/write those when needed. Address allocator is the
contract which allocates addresses to contracts that are about to be deployed and is used by code contract during
deployment of new contracts. There is still a lot more work left around system contracts, but so far the concept "
Everything is a contract" seems to be working reasonably well.

[Contacts overview]: https://github.com/nazar-pc/abundance/pull/16

[rendered]: https://abundance.build/book/Execution_environment/Contracts_overview.html

[PR 19]: https://github.com/nazar-pc/abundance/pull/19

[PR 27]: https://github.com/nazar-pc/abundance/pull/27

There were some metadata improvements ([PR 20], [PR 21], [PR 22]) that massaged metadata information about the contract
and its methods, followed by more `#[contract]` macro refactoring ([PR 23], [PR 24]) that finally led to [trait support]
being implemented. Trait support allows to define things like fungible token as a trait and for other contract to
implement it. Then contracts can rely on just trait definition to be able to interact with any contract that has
implemented that trait. Simple `Fungible` trait was added to the codebase just to demonstrate how it could work. More
traits will be added over time, for example it is likely that some kind of "Wallet" trait will be defined to unify
interation with contracts from the user side.

[PR 20]: https://github.com/nazar-pc/abundance/pull/20

[PR 21]: https://github.com/nazar-pc/abundance/pull/21

[PR 22]: https://github.com/nazar-pc/abundance/pull/22

[PR 23]: https://github.com/nazar-pc/abundance/pull/23

[PR 24]: https://github.com/nazar-pc/abundance/pull/24

[trait support]: https://github.com/nazar-pc/abundance/pull/25

There were some smaller changes here and there as well, but if you're interested in that you better go read numerous PRs
directly instead.

## Upcoming plans

The next steps will involve implementing some kind of test environment for contract execution, such that it is possible
to combine a couple of contracts, deploy them and see them interacting with each other. This will be an important
milestone in showcasing developer experience and will hopefully help to collect some developer feedback.
