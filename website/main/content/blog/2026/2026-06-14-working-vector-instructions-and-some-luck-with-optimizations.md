---
title: Working vector instructions and some luck with optimizations
date: 2026-06-14
draft: false
description: Vector instructions are passing ACT4 and faster interpreter overall
tags: [ status-update ]
authors: [ nazar-pc ]
---

The last few weeks were a confusing mix of both some decent work and procrastination with way too many blockers faced.
It didn't feel productive at all despite some data proving the opposite. One thing I can say for sure is I didn't make
any progress on the blockchain itself this time, just wasn't in the mood for it.

<!--more-->

## Unfinished Rust nightly features are not guaranteed to stay around

Shortly after the previous update, Rust nightly `nightly-2026-05-03` became the last nightly usable with the current
state of the code base. The reason is unstable **and incomplete** feature `generic_const_exprs`. It was getting
increasingly difficult to use over the years, but still possible, until it wasn't. `nightly-2026-05-04` introduced
changes that broke it to a completely unusable state, and [it is not going to be fixed] like ever (the feature will be
removed).

[it is not going to be fixed]: https://github.com/rust-lang/rust/issues/156296

It is not all completely lost, though, there is a new feature called `generic_const_args`, which is slightly different,
but achieves most of the same goals. There are many issues with transitioning to it, though.

First, it is unusable in `nightly-2026-05-03` or even `nightly-2026-05-04`, so I can't convert the code base
incrementally, there is something like a week or more of a gap before the feature is usable enough to compile something
useful. This means the migration is "all or nothing".

Second, the latest version of the feature requires `-Znext-solver=globally` flag to work, which needs to be propagated
across the stack, and on top of that multiple crates are now hitting compiler recursion limits, so also need
`-Zmin-recursion-limit=256` flag on top of all that. Unfortunately, it gets quite ugly, and ultimately still impossible
[to compile certain things under Miri] since the flag doesn't propagate all the way down.

[to compile certain things under Miri]: https://github.com/rust-lang/miri/issues/5101

And if that wasn't enough, `-Znext-solver=globally` causes [rustdoc ICE] that was known for many months and still
unresolved.

[rustdoc ICE]: https://github.com/rust-lang/rust/issues/156487

This means I can't transition and get a green CI, this also means I can't publish crates using either
`generic_const_exprs` or `generic_const_args` to crates.io and expect for docs.rs to generate the documentation
successfully. Very frustrating situation to be in. While I can probably hack something for Miri specifically, `rustdoc`
issue is a hard blocker.

I periodically update [PR 698] with newer Rust versions and changes from `main` to keep things up to date, but unless at
least `rustdoc` is fixed, it can't be merged.

[PR 698]: https://github.com/nazar-pc/abundance/pull/698

## Interpreter performance improvements

Well, from unfortunate news only something more positive. I've been working on and off on interpreter performance with
many more things not working, I found a few that did bring substantial performance improvements.

First, sprinkling compiler hints about cold paths (mostly error handling) in [PR 668] I was able to convince LLVM to
generate much better machine code:

```
blake3_hash_chunk/interpreter/eager
                        time:   [28.951 µs 29.196 µs 29.403 µs]
                        thrpt:  [33.214 MiB/s 33.449 MiB/s 33.732 MiB/s]
                 change:
                        time:   [−17.891% −17.043% −16.186%] (p = 0.00 < 0.05)
                        thrpt:  [+19.312% +20.544% +21.789%]
                        Performance has improved.

ed25519_verify/interpreter/eager
                        time:   [1.6695 ms 1.6754 ms 1.6806 ms]
                        thrpt:  [595.02  elem/s 596.88  elem/s 599.00  elem/s]
                 change:
                        time:   [−10.558% −9.1931% −7.6390%] (p = 0.00 < 0.05)
                        thrpt:  [+8.2708% +10.124% +11.805%]
                        Performance has improved.
```

[PR 668]: https://github.com/nazar-pc/abundance/pull/668

This was the first substantial encouraging performance improvement in a while despite LLMs claiming I was hitting the
wall of the architecture.

I then spent way too much time trying to understand why LLVM keeps generating bad code for the main execution loop. I
even played with PGO and turned out PGO results in substantially **worse** code every time, which as I later discovered
happens essentially every time when the function is large.

With the acquired knowledge and the anticipation for the total number of instructions exceeding `u8` soon I went to
refactor immediates of some instructions from 32 bits to 24 bits in [PR 670] (so that total instruction enum size bits
in 64 bits even with `u16` discriminant). Then renamed some (arguably confusing) `r1s`/`r2s` operands to `rs1`/`rs2`
in [PR 671] for consistency with the majority of other instructions. And finally in [PR 675] I refactored instruction
layout to always include `rs1`/`rs2` operands (fake `x0` in case instruction didn't have it) and execution to fetch
`rs1`/`rs2` values before matching on the instruction.

[PR 670]: https://github.com/nazar-pc/abundance/pull/670
[PR 671]: https://github.com/nazar-pc/abundance/pull/671
[PR 675]: https://github.com/nazar-pc/abundance/pull/675

This removed a lot of duplication in generated code since the number of places where GPRs are fetched and stored reduced
dramatically. In concrete numbers, on x86-64 the execution function size reduced from ~65 kiB to ~7 kiB, which finally
made PGO effective and producing positive results. With all that, I was able to achieve another satisfying performance
improvement (in contrast to previous numbers, this is with PGO):

```
blake3_hash_chunk/interpreter/eager
                        time:   [26.966 µs 27.033 µs 27.118 µs]
                        thrpt:  [36.012 MiB/s 36.125 MiB/s 36.215 MiB/s]
                 change:
                        time:   [−9.0588% −8.6867% −8.3353%] (p = 0.00 < 0.05)
                        thrpt:  [+9.0933% +9.5131% +9.9612%]
                        Performance has improved.

ed25519_verify/interpreter/eager
                        time:   [1.5143 ms 1.5177 ms 1.5218 ms]
                        thrpt:  [657.13  elem/s 658.91  elem/s 660.38  elem/s]
                 change:
                        time:   [−11.374% −11.001% −10.609%] (p = 0.00 < 0.05)
                        thrpt:  [+11.868% +12.361% +12.834%]
                        Performance has improved.
```

Perf stats before:

```
             0      context-switches:u               #      0,0 cs/sec  cs_per_second     
             0      cpu-migrations:u                 #      0,0 migrations/sec  migrations_per_second
             0      page-faults:u                    #      0,0 faults/sec  page_faults_per_second
      1 001,24 msec task-clock:u                     #      1,0 CPUs  CPUs_utilized       
    10 194 173      branch-misses:u                  #      0,4 %  branch_miss_rate         (50,34%)
 2 727 518 292      branches:u                       #   2724,2 M/sec  branch_frequency     (50,04%)
 4 889 904 052      cpu-cycles:u                     #      4,9 GHz  cycles_frequency       (66,44%)
18 967 503 459      instructions:u                   #      3,9 instructions  insn_per_cycle  (49,66%)
    93 970 856      stalled-cycles-frontend:u        #     0,02 frontend_cycles_idle        (49,96%)
```

And after:

```
             0      context-switches:u               #      0,0 cs/sec  cs_per_second     
             0      cpu-migrations:u                 #      0,0 migrations/sec  migrations_per_second
             0      page-faults:u                    #      0,0 faults/sec  page_faults_per_second
      1 001,16 msec task-clock:u                     #      1,0 CPUs  CPUs_utilized       
     1 938 966      branch-misses:u                  #      0,1 %  branch_miss_rate         (50,25%)
 3 330 192 405      branches:u                       #   3326,3 M/sec  branch_frequency     (49,85%)
 4 856 438 773      cpu-cycles:u                     #      4,9 GHz  cycles_frequency       (66,44%)
22 767 555 823      instructions:u                   #      4,7 instructions  insn_per_cycle  (49,75%)
    27 528 902      stalled-cycles-frontend:u        #     0,01 frontend_cycles_idle        (50,15%)
```

The results are pretty great to be honest. However, sometimes these numbers are quite confusing. I was getting 5.3
instructions per cycle on my Zen 4 CPU (which maxes out at about 6), but that sometimes meant it was executing more
"useless" instructions and end-to-end performance wasn't actually getting better. But with these changes the improvement
was definitely real.

I also looked into RISC-V code generated for my custom target and got confused when I saw a lot of `auipc + jalr`
instruction pairs and no `jal` instructions. `jal` is perfectly fine for small relative jumps and should have been
sufficient for small contract files, yet compiler stubbornly didn't use them. Eventually, I discovered that adding
`+relax` to target features finally achieves this ([PR 684]), which both reduced benchmarking contract size from 22.3 kB
to 21.7 kB (jumps now need instruction instead of two) and slightly improved performance as the result since there were
fewer instructions to execute:

[PR 684]: https://github.com/nazar-pc/abundance/pull/684

```
blake3_hash_chunk/interpreter/eager
                        time:   [26.029 µs 26.057 µs 26.076 µs]
                        thrpt:  [37.451 MiB/s 37.477 MiB/s 37.519 MiB/s]

ed25519_verify/interpreter/eager
                        time:   [1.4453 ms 1.4467 ms 1.4479 ms]
                        thrpt:  [690.64  elem/s 691.25  elem/s 691.89  elem/s]
```

Then I discovered frankly a bit surprising optimization in [PR 685] Storing a byte offset to the instruction instead of
its index results in a better performance due to skipping the bit shifting since memory reads do need byte offset in the
end. Net result is a solid improvement from a single instruction change:

[PR 685]: https://github.com/nazar-pc/abundance/pull/685

```
blake3_hash_chunk/interpreter/eager
                        time:   [24.732 µs 24.779 µs 24.866 µs]
                        thrpt:  [39.273 MiB/s 39.410 MiB/s 39.487 MiB/s]
                 change:
                        time:   [−5.5264% −5.2876% −4.9948%] (p = 0.00 < 0.05)
                        thrpt:  [+5.2574% +5.5828% +5.8496%]
                        Performance has improved.

ed25519_verify/interpreter/eager
                        time:   [1.3732 ms 1.3784 ms 1.3828 ms]
                        thrpt:  [723.16  elem/s 725.50  elem/s 728.20  elem/s]
                 change:
                        time:   [−5.4184% −5.1885% −4.9165%] (p = 0.00 < 0.05)
                        thrpt:  [+5.1707% +5.4725% +5.7288%]
                        Performance has improved.
```

This stuff can only really be seen when studying generated assembly, and thankfully those of us who can't really read
assembly productively, especially x86-64, we now have LLMs to interrogate about it. Reminded me of
[another single-instruction change in Proof-of-Time] that resulted in 10% difference.

[another single-instruction change in Proof-of-Time]: https://github.com/autonomys/subspace/issues/1754#issuecomment-1665960569

I've mentioned LLVM's SROA a few times before, but so far I failed to apply the changes that trigger it and improve
performance in a targeted way. Until [PR 686]. I've tried different permutations of things until I found something that
finally clicked. In that PR I refactored execution to move the instruction fetcher to the stack temporarily, such that
LLVM SROA can promote some of its fields to native registers, and due to the use of it for every single instruction it
was finally something that improved performance:

```
blake3_hash_chunk/interpreter/eager
                        time:   [23.421 µs 23.484 µs 23.520 µs]
                        thrpt:  [41.520 MiB/s 41.584 MiB/s 41.696 MiB/s]
                 change:
                        time:   [−6.2576% −6.0281% −5.8153%] (p = 0.00 < 0.05)
                        thrpt:  [+6.1743% +6.4148% +6.6754%]
                        Performance has improved.

ed25519_verify/interpreter/eager
                        time:   [1.3011 ms 1.3039 ms 1.3057 ms]
                        thrpt:  [765.89  elem/s 766.94  elem/s 768.60  elem/s]
                 change:
                        time:   [−7.7839% −7.2270% −6.7192%] (p = 0.00 < 0.05)
                        thrpt:  [+7.2032% +7.7900% +8.4410%]
                        Performance has improved.
```

[PR 686]: https://github.com/nazar-pc/abundance/pull/686

So it wasn't the architecture limit after all, and I think there are still more optimizations left to discover in a sea
of many more failed attempts (some of which I have already done since). I'm happy with the cumulative performance
improvements over the last month or so.

## More capabilities to express RISC-V extensions

One thing that bothered me for a while since I discovered it is the ability to express instruction dependencies. There
are some extensions in RISC-V specification and even individual instructions that are only present when some other
extension is also present.

For example, `C` (compressed instructions) extension includes `Zcf` extension (compressed floating point instructions),
but only when `F` extension is also present (meaning floating point instructions are supported at all). Or `Zcb`'s
`c.sext.b` only present when `Zbb` extension is also available.

With [PR 687] such dependencies can now be expressed with `#[instruction]` macro on both extension and individual
instruction levels. There are more capabilities of that kind currently missing. Some that I know about are conflicts
between `Zcmp`/`Zcmpt` and `Zcd` (they reuse conflicting instruction encodings), also various EEW/SEW restrictions in
vector instructions that depend on the presence of `V` extension and similar. We'll get there soon.

[PR 687]: https://github.com/nazar-pc/abundance/pull/687

## Vector instructions

Continuing the topic of changes related to RISC-V, [ACT4] finally supports (somewhat incomplete) tests for vector
instructions, which allowed me to run implementation against them and fix a bunch of bugs in [PR 694] and [PR 696].
Everything they have so far instruction decoder and interpreter now pass.

[ACT4]: https://github.com/riscv/riscv-arch-test
[PR 694]: https://github.com/nazar-pc/abundance/pull/694
[PR 696]: https://github.com/nazar-pc/abundance/pull/696

Not only that, with the tests present, I was able to add support for `Zvkb` and `Zvbb` extensions in [PR 710] and then
`Zvbc` in [PR 711], which all also pass ACT4 tests.

[PR 710]: https://github.com/nazar-pc/abundance/pull/710
[PR 711]: https://github.com/nazar-pc/abundance/pull/711

Enabling vector instructions in RISC-V contracts reduces contract size (depending on the minimum vector length
configured), but unfortunately, it drops the performance substantially and makes it even worse once PGO is involved. The
clear sign of the issue is the large amount of code LLVM is dealing with. I have made some attempts to deal with that,
but not particularly successful so far. Vector instructions are very complex, relatively speaking, and maybe
auto-vectorization in LLVM just isn't that good yet (though I know work is being done in that direction upstream).

It is possible that vector instructions may end up being useless in contracts from the interpreter performance point of
view, but for now I'm hopeful.

## Surprising tiny performance win

When working on migration to `generic_const_args` I [hit an ICE] that I worked around in [PR 690] by replacing
`impl Iterator` with a concrete implementation that unexpectedly improved performance in a non-negligible way:

[PR 690]: https://github.com/nazar-pc/abundance/pull/690

[hit an ICE]: https://github.com/rust-lang/rust/issues/156744

```
2/balanced/all-proofs   time:   [1.2343 ns 1.2347 ns 1.2353 ns]
                        change: [−17.073% −16.003% −15.116%] (p = 0.00 < 0.05)
                        Performance has improved.

4/balanced/all-proofs   time:   [1.2339 ns 1.2344 ns 1.2349 ns]
                        change: [−15.688% −15.443% −15.099%] (p = 0.00 < 0.05)
                        Performance has improved.

256/balanced/all-proofs time:   [1.2345 ns 1.2349 ns 1.2353 ns]
                        change: [−15.633% −15.423% −15.103%] (p = 0.00 < 0.05)
                        Performance has improved.

32768/balanced/all-proofs
                        time:   [1.2333 ns 1.2336 ns 1.2341 ns]
                        change: [−18.464% −17.211% −15.945%] (p = 0.00 < 0.05)
                        Performance has improved.

65536/balanced/all-proofs
                        time:   [1.2331 ns 1.2372 ns 1.2414 ns]
                        change: [−15.431% −15.040% −14.571%] (p = 0.00 < 0.05)
                        Performance has improved.
```

## Conclusion

As you can see, there were a lot of things happening and even more that I didn't mention, but ultimately it doesn't seem
like anything in particular was actually completed. I really wanted to release 0.1 version of the RISC-V crates to
crates.io, but now with inability to generate documentation I am blocked.

`-Znext-solver=globally` is supposed to be stable and default in Q3 of this year, so hopefully those issues are not
going to haunt me for long 🤞.

## Upcoming plans

I do have a high-level plan for sharding-related changes, it just needs actual engineering to be done. Hopefully soon,
it is just really boring and tedious work.

I'll be on [Zulip] if you have anything to discuss, otherwise I'll be back with another update some time soon.

[Zulip]: https://abundance.zulipchat.com/
