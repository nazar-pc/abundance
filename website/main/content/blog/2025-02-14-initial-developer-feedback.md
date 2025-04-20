---
title: Initial developer feedback
date: 2025-02-14
draft: false
description: Updates from last week, including feedback from developer interviews
tags: [ status-update ]
authors: [ nazar-pc ]
---

Last week felt a bit less productive with a lot of time spent thinking about how to approach slots conflict resolution
in the native execution environment, but still managed to land a few improvements, especially on the documentation side.
Also conducted four separate interviews.

<!--more-->

The fist developer interview was with my past colleague Liu-Cheng, from which I've collected a bunch of notes. As a
result, I have updated documentation on [Contracts overview] page in [PR 57], expanding on some topics and providing
more analogies for developers to connect with. Then [PR 58] renamed existing "example" contract into "playground"
because it was really there to try all kinds of advanced APIs that are not used in most cases, which made it hard to
understand. To make things easier to comprehend, two contracts were introduced: flipper and a fungible token.

[PR 57]: https://github.com/nazar-pc/abundance/pull/57

[Contracts overview]: /book/Execution_environment/Contracts_overview.html

[PR 58]: https://github.com/nazar-pc/abundance/pull/58

Fipper is one of the simplest contracts possible that simply flip a boolean value stored in the state to the opposite
value. It doesn't deal with slots and is there as a gentle introduction, accompanied by an integration test.

Fungible token is more advanced, supports minting and transferring tokens with balances stored in slots. It also
showcases how to work with traits by implementing `Fungible` trait and including examples of how to use it in
integration test.

I also looked at the API provided by the execution environment and simplified a few things prior to the second developer
interview with another past colleague Shamil. Shamil had a bunch more comments about various things from documentation
to APIs that will take some time to incorporate into the code, but it will make things better relatively soon.

For example, it became clear from the interview with Shamil that the distinction between `#[result]`, `#[output]` and
return type is blurry, which I agree with. It'd be nice to unify all 3 into a single concept of an "output." The primary
place where it plays a significant role is `#[init]` method that is expected to return initial state of the contract,
but I think it is possible to simply treat the last output as the state for this purpose, regardless of whether it is a
return type or an explicit argument.

Also default return types provided by the `ab-contracts-common` may not be enough regardless of how extensive the
selection is and developers would want to provide a custom variants that are application-specific. This might be a bit
of a challenge for metadata generation, but likely worth doing anyway.

Huge thanks to both Liu-Cheng and Shamil for spending almost 2 hours with me and going through the very early version of
something completely new at the very early stages of development. The key takeaway so far is that the whole concept of
"slots" is quite different from other execution environments even if justified and will be a learning curve to people.
Developers typically like working with dynamic data structures whenever possible and thinking how to model the system
with (ideally) fixed size slots is a slightly hostile environment. I believe the majority of it is needed for optimal
performance and code size, but there is certainly room for simplification and documentation improvements. Like with
fungible token example that demonstrates that one can live without a global hashmap for balances in a fairly ergonomic
way.

As mentioned in the last update, the check for calling `#[update]` or `#[init]` from `#[view]` methods on execution
environment level, which was implemented in [PR 59]. It also included a bunch of refactoring as preparation for handling
of conflicting storage updates, but the handling itself wasn't ready.

[PR 59]: https://github.com/nazar-pc/abundance/pull/59

In fact, I have spent quite some time thinking and trying a few approaches to efficiently handle update conflicts. After
many failed attempts that quickly blew up in terms of complexity, making it hard to read, and I think I found one that
should work, need to do yet another attempt to implement it 🤞.

Spent some time looking into RISC-V specs trying to understand how it will fit into the architecture, asked a bunch of
[questions at PolkaVM repository]. I must admit, I do not 100% like what I understood about PolkaVM so far. Not sure
what the linking cost is, but my understanding right now is that linker result is what is supposed to be used as a
potential contract rather than "linking" it on the fly. The problem with that is that PolkaVM linker produces something
that isn't quite vanilla RISC-V, it has some custom instructions. This isn't inherently bad, but I imagined a system
where something like a minimal ELF file is uploaded as a contract with minimal (ideally none) API that is not the
standard RISC-V instruction set produced by Rust/LLVM.

[questions at PolkaVM repository]: https://github.com/paritytech/polkavm/discussions/266

For example, PolkaVM uses custom `ecalli` instruction for host function calls instead of the standard `ecall`
instruction (which is explicitly forbidden) due to the need to do static analysis of the program, which I do not see as
a requirement for this project. I'd be ideal to maybe use the same syscalls as Linux for memory allocation and add a
single one for host method calls (or find something from Linux syscalls that matches). Then it would be possible to
literally take a normal static C library compiled for Linux target and use it as a contract. This is not a blocker and
instead more of an exploration and education for me. I hope that there will be a way to take advantage of PolkaVM in a
more generic way and collaborate with them on the project.

Lastly, I had two more researcher interviews with some really nice folks, who unfortunately (for me) are not available
for full-time engagement right now. So the search continues, but I believe there will be opportunities to engage them at
some point of the project.

## Upcoming plans

I'm planning to finally crack the conflicting storage updates situation, further improve update documentation and,
hopefully, simply API based on received developer feedback.

Also, hopefully more hiring interviews.

See you in about a week with more updates!
