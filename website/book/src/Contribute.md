# How can you help?

There are a number of things that are being worked on, but there are also things that we'd really like some help with or
to collaborate on. If any of this is interesting to you, join our [Zulip] chat and let's discuss it. The list will be
expanded over time.

[Zulip]: https://abundance.zulipchat.com/

If you have ideas that are not mentioned below, feel free to reach out and share them.

There may or may not be funding available for these things.

## High-performance RISC-V VM

> [!NOTE]
> Mostly engineering

We need a RISC-V VM. There is already a basic interpreter, but more performance is still needed. The basic requirements
are as follows:

* Able to run RV64E code with popular extensions (current interpreter supports RV64EMBZbc, adding vector and
  cryptographic extensions afterward, C extension is explicitly not supported directly for simplicity and performance
  reasons)
* Runs in a secure minimal sandbox (like seccomp, possibly in a hardware-accelerated VM)
* Cross-platform (Linux, macOS and Windows) deterministic execution
* Has low overhead gas metering
* High performance (~50% of native speed is highly desirable, which includes gas metering)
* Low memory usage
* Quick instantiations to support cheap and frequent cross-contract calls

[PolkaVM](https://github.com/paritytech/polkavm) satisfies some requirements above, but not all and doesn't 100% align
in its design goals, but there might be an opportunity for collaboration.

## State database

> [!NOTE]
> Research informed by engineering constraints

We need a special state database. Since consensus nodes do not store the state itself (it is not mandatory), but rather
store hashes, one per account, we have quite unique database requirements, and it must be possible to exploit them for
better performance.

Here are the key requirements:

* Key-value database
* All keys are monotonically increasing 128-bit unsigned integers
* It is extremely likely that many keys will be close to each other (most will be exactly next to each other)
* All values are tiny, constant size, less than 100 bytes
* Verifiable, entries form a Sparse Merkle Tree, whose root is the state root
* Efficiently updatable (including update of the Merkle Root)
* Reasonably efficiently provable
* Optimized for modern NVMe SSDs (concurrent random reads, sequential batch writes)
* Should use low to moderate amount of RAM
* Nice if supports versioning, but not mandatory
* Very, very fast

By very fast, I mean that it should ideally support hundreds of thousands of operations on modern SSDs, productively
leveraging IOPS available.

Specifically, note that we don't need variable sized keys, values are constant size as well. Keys are not uniformly and
randomly distributed throughout the key space, instead a lot (most) of them will be right next to each other.

Also, most blockchains support state versioning at various depths, here we just need 100 blocks max. I suspect we can
store the diff in RAM or apply directly, while keeping separate "rollback details" rather than supporting proper
versions of the whole tree be readily available at any time.

With such constraints, I think there should be something really simple and efficient out there either implemented (less
likely) or at least designed in a paper or something (more likely).

## P2P networking stack

> [!NOTE]
> Research/engineering

We need a P2P networking stack. There is a prototype already, but it'll need to be expanded significantly with sharding
and blockchain needs in mind. Some requirements:

* TCP-based
* Likely pea2pea-based with Yamux and Noise (strictly speaking, not a hard requirement)
* Low overhead and high performance
* Zero-copy whenever possible
* Support for custom gossip protocols (block and transaction propagation, proof-of-time notifications)
* Support for both structured (for distributed storage network) and unstructured (for blockchain) architecture

There is a networking stack inherited from [Subspace reference implementation], which is focused on distributed storage
network needs. It generally works, though bootstrapping time and downloading speeds can be improved. It will likely need
to be upgraded with support for various blockchain needs that were previously provided by [Substrate] framework, but
will have to be reimplemented differently.

[Subspace reference implementation]: https://github.com/autonomys/subspace

[Substrate]: https://github.com/paritytech/polkadot-sdk/tree/master/substrate

## Funding model for core contributors

> [!NOTE]
> Research

It would be really nice to find a sustainable funding model for core contributors and valuable community members. Here
are some requirements:

* Fully on chain without legal entities and jurisdiction constraints
* Ability to discover and reward valuable contributions without relying on committees/governance
* Ideally P2P or using small DAOs

The first thing that comes to mind is having multiple rewards addresses specified during farming, with a specified
portion of rewards probabilistically ending up in a wallet of the developer/contributor that farmer wants to support.
This doesn't solve the problem of discoverability though. One way or another, there should not be a big
treasury/governance structure that is responsible for managing funds, it should be more direct and more distributed.

## GPU plotting performance improvements

> [!NOTE]
> Engineering

GPU plotting was [re-written in Rust] using [rust-gpu] with the goal of running on Vulkan 1.2-capable devices (plus
Metal on macOS). It works and has decent performance, but there must still be significant optimization opportunities
left to explore, some of which might require improving rust-gpu itself first.

[re-written in Rust]: https://github.com/nazar-pc/abundance/tree/2862d4ae59b60000e020bcbf38c4dcbd9a74f10e/crates/farmer/ab-proof-of-space-gpu

[rust-gpu]: https://github.com/Rust-GPU/rust-gpu

