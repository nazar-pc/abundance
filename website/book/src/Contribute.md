# How can you help?

There are a number of things that are being worked on, but there are also things that we'd really like some help with or
to collaborate on. If any of this is interesting to you, join our [Zulip] chat and let's discuss it. The list will be
expanded over time.

[Zulip]: https://abundance.zulipchat.com/

If you have ideas that are not mentioned below, feel free to reach out and share them.

There may or may not be funding available for these things.

## RISC-V VM

We need a RISC-V VM. The basic requirements are as follows:

* Supports an ELF shared library as its input format, must be able to run it straight after compiler without any
  additional processing
* Able to run RV64E code with popular extensions (probably RV64EMAC to start, adding vector and cryptographic extensions
  afterward)
* Runs in a secure minimal sandbox (like seccomp, possibly in a hardware-accelerated VM)
* Cross-platform (Linux, macOS and Windows) deterministic execution
* Has low overhead gas metering
* High performance (~50% of native speed is highly desirable, which includes gas metering)
* Low memory usage
* Quick instantiations to support cheap and frequent cross-contract calls

[PolkaVM](https://github.com/paritytech/polkavm) satisfies some requirements above, but not all and doesn't 100% align
in its design goals, but there might be an opportunity for collaboration.

## P2P networking stack

We need a P2P networking stack. There is a prototype already, but it'll need to be expanded significantly with sharding
and blockchain needs in mind. Some requirements:

* TCP-based
* Likely libp2p-based (strictly speaking, not a hard requirement, but very desirable for interoperability)
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

It would be really nice to find a sustainable funding model for core contributors and valuable community members. Here
are some requirements:

* Fully on chain without legal entities and jurisdiction constraints
* Ability to discover and reward valuable contributions without relying on committees/governance
* Ideally P2P or using small DAOs

The first thing that comes to mind is having multiple rewards addresses specified during farming, with a specified
portion of rewards probabilistically ending up in a wallet of the developer/contributor that farmer wants to support.
This doesn't solve the problem of discoverability though. One way or another there should not be a big
treasury/governance structure that is responsible for managing funds, it should be more direct and more distributed.

## GPU plotting

GPU plotting was inherited from [Subspace reference implementation], but due to getting rid of KZG it is temporarily a
bit broken.

Fixing it is an immediate priority. That said, rewriting the whole thing with Rust GPU is also a very desirable thing,
such that a wider range of hardware (basically anything Vulkan-capable) can be supported, including iGPUs of desktop
CPUs and various SBCs.
