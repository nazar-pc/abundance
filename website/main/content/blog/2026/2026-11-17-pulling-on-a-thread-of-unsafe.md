---
title: Pulling on a thread of `unsafe`
date: 2026-11-17
draft: false
description: Fixing all kinds of bugs from safety to logic to performance
tags: [ status-update ]
authors: [ nazar-pc ]
---

It has been a while since the last update, apologies. I was kind of overwhelmed with things. I initially worked some
more on RISC-V interpreter and then went into UI tests and then started discovering all kinds of bugs and was busy
fixing them for more than a month since. There is light at the end of it all, though, so let me share where we are.

<!--more-->

## core::hint::assert_unchecked

If you do not read the rest of the post, I'd like you to read about [`assert_unchecked`]. Truly magnificent tool for
performance optimizations that is much easier to reason about than most alternatives and with much less `unsafe`.

[`assert_unchecked`]: https://doc.rust-lang.org/stable/core/hint/fn.assert_unchecked.html

Very often in the context of performance optimizations we use `unsafe` to do things like unchecked indexing, for example
when we know the upper bound from a protected internal invariant, which the compiler doesn't necessarily see and inserts
bounds checks by default. I hit this a lot when designing code that guarantees lack of panics at compile time.

Normally, you'd call `get_unchecked()` and write a safety proof. Which, to be fair, is correct and the most direct way
to do it, but not the most efficient from a maintenance point of view. Instead, if you wrap the index in a new type, you
can add `assert_unchecked()` in its force-inlined getter or `impl From<T> for usize`, such that with the hint compiler
can remove bounds checks during optimization passes. This is often much easier to reason about in a single place than
using `unsafe` in a bunch of call sites.

The most extreme example of this that I had was in [PR 826], where a few strategic `assert_unchecked()` calls plus much
better abstractions replaced hundreds of `unsafe` blocks with just a few.

[PR 826]: https://github.com/nazar-pc/abundance/pull/826

Here is an example of this in action:

```rust
/// Register group of elements of width `EEW` that lies within the vector register file.
///
/// Created from a [`VectorConfig`] with `EMUL = LMUL * EEW / SEW`, it holds `VLMAX` elements, the
/// first `vl` of which are the body. This makes the element accesses of
/// [`VectorRegisterFile::read()`] and friends safe, since the group carries the bounds they check
/// indices against.
#[derive(Debug, Clone, Copy)]
pub struct VRegGroup<Hart>
where
    Hart: VectorHartConfig,
{
    base: VReg,
    emul: Vlmul,
    eew: Eew,
    // Invariant: `vl <= vlmax` and `vlmax` elements of `eew` starting at `base` end within the
    // register file
    vl: BoundedVl<Hart>,
    vlmax: Vl,
}

impl<Hart> VRegGroup<Hart>
where
    Hart: VectorHartConfig,
{
    /// Register group starting at `base` with elements of width `eew` under `config`.
    ///
    /// Returns `None` if `eew` exceeds `ELEN`, `EMUL = LMUL * EEW / SEW` is outside `[1/8, 8]` or
    /// `base` is not aligned to `EMUL`, each of which makes an instruction illegal.
    #[inline(always)]
    pub const fn new(config: VectorConfig<Hart>, base: VReg, eew: Eew) -> Option<Self> {
        if !Hart::VECTOR_LENGTHS.elen.supports(eew) {
            cold_path();
            return None;
        }
        let vtype = config.vtype();
        let emul = vtype.vlmul().emul(eew, vtype.vsew())?;
        if !base.is_group_aligned(emul.register_count()) {
            cold_path();
            return None;
        }

        let vl = config.vl();
        let vlmax = config.vlmax();
        // Both always hold for a legal `EMUL`, but are checked here rather than derived from how
        // `VectorConfig` and `Vlmul` compute them, since unsafe code relies on the invariant
        let group_end = u32::from(base.to_bits()) * Hart::VECTOR_LENGTHS.vlen.bytes()
            + u32::from(vlmax) * u32::from(eew.bytes_width());
        if u32::from(vl.get()) > u32::from(vlmax)
            || group_end > 32 * Hart::VECTOR_LENGTHS.vlen.bytes()
        {
            cold_path();
            return None;
        }

        Some(Self {
            base,
            emul,
            eew,
            vl,
            vlmax,
        })
    }

    // ...

    /// Number of body elements, which is `vl`
    #[inline(always)]
    pub const fn vl(self) -> BoundedVl<Hart> {
        // SAFETY: Invariant of `VRegGroup`
        unsafe {
            assert_unchecked(u32::from(self.vl.get()) <= u32::from(self.vlmax));
        }
        self.vl
    }

    /// Number of elements the group holds, which is `VLMAX` and at least [`Self::vl()`]
    #[inline(always)]
    pub const fn vlmax(self) -> Vl {
        // SAFETY: Invariant of `VRegGroup`
        unsafe {
            assert_unchecked(u32::from(self.vl.get()) <= u32::from(self.vlmax));
        }
        self.vlmax
    }
}
```

`vl` and `vlmax` getters are used in places where math can be either substantially optimized or made panic-free or both
with those assertions, but it would have been hard to articulate that at each of MANY call sites. Now this is just a
local invariant assertion.

A simpler example is shift amount for various RISC-V instructions:

```rust
/// Shift amount of an instruction, which is always below the width of `Type` in bits.
///
/// `Type` is the type of the shifted value: the register type for most instructions, and `u32` for
/// instructions that operate on 32-bit words on RV64, like `slliw`. Shifts by this amount never
/// overflow, so execution doesn't need to mask it.
#[derive(Clone, Copy)]
pub struct Shamt<Type> {
    shamt: u8,
    shifted_type: PhantomData<Type>,
}

impl<Type> Shamt<Type>
where
    Type: RegType,
{
    /// Create a new shift amount, returns `None` if it is not below the width of `Type` in bits
    #[inline(always)]
    pub const fn new(shamt: u8) -> Option<Self> {
        if shamt >= Type::BITS {
            return None;
        }

        Some(Self {
            shamt,
            shifted_type: PhantomData,
        })
    }

    /// Get the shift amount, which is below the width of `Type` in bits
    #[inline(always)]
    pub const fn get(self) -> u32 {
        // SAFETY: The field is private to this module and only set by the constructor, which
        // rejects values that are not below the width of `Type` in bits
        unsafe {
            assert_unchecked(self.shamt < Type::BITS);
        }
        u32::from(self.shamt)
    }
}
```

`Type` is essentially a register type as an integer, so asserting that the shift amount is always smaller allows the
compiler to understand there will be no arithmetic overflow/underflow for shifts and can take that into consideration.

## UI tests

I've been looking for other ways to reduce the amount of unsafe since [PR 826] and after having decent success decided
to look at other robustness guarantees.

I've designed a lot of things in misuse-resistant way. This often means the code that is not correct simply doesn't
compile. Trick question: if incorrect code doesn't compile, how do we know it keeps failing to compile going forward? 🤔

As you may guess, the answer is UI tests, the first of which landed in [PR 838] and a bunch more were added since.
Essentially, you write intentionally wrong code and create snapshots of what an error is supposed to look like. When
error disappears or changes you inspect why that happened.

[PR 838]: https://github.com/nazar-pc/abundance/pull/838

## Bugs, bugs everywhere

UI testing is where everything went downhill in terms of workload and in the other direction in terms of code quality.
Both my attempts to reduce `unsafe` and add UI tests to parts of the code base I have not touched in a while forced
Claude to re-analyze a lot of things spotting blemishes here and there. In the end I have resolved hundreds of bugs
ranging from tricky soundness issues to minor logic bugs and performance pitfalls.

I've been chasing them for a long time now, and there are still like 50 items on the list, and I'm not entirely sure if
they are all small items either.

What I am sure about, though, is that the code quality is A LOT higher today. For example, `ab-io-type`, widely used for
low-level consensus data structures, correctly accounted for everything necessary to treat data structures as bytes, but
not necessarily the other way around. One big thing I missed was that invalid enum discriminants can lead to a safety
violation when bytes are converted into an instance. I originally saw that `bool` was problematic, but not enums.

Now that situation is covered from two sides:

* there is an API for such data structures that validates invariants before giving an instance back
* data structures that do not have such invariants implement `AnyBitPattern`, allowing simplified conversion from bytes
  without validation

Now both `bool` and various enums are usable in fairly convenient way. Lots of useful helpers that make parsing code
more expressive and easier to follow. You can even opt out of `AnyBitPattern` and add a custom invariant validation
function if necessary (let's say for a new type that wraps an integer that must be within a certain limited range).

## Performance and other optimizations

### RISC-V Interpreter performance

The last number I can see in [PR 798] is a CoreMark score of ~3046, and I observed a score of over 4000
with [this LLVM patch]. However, because it was generated with an LLM's help and overlapped with some of the ongoing
LLVM design discussions, it didn't get much attention in the past two months since its creation.

[PR 798]: https://github.com/nazar-pc/abundance/pull/798
[this LLVM patch]: https://github.com/llvm/llvm-project/pull/223255

### Archiving performance

I also looked at erasure coding of all things and fixed quadratic time complexity of block archiving in [PR 901] and
improved overall end-to-end archiving performance by ~14% single-threaded and ~33% multithreaded. Not only that, with
pending upstream changes at [AndersTrier/reed-solomon-simd#80] another ~30% erasure coding improvement could be had and
then there are tiling changes I have prototyped that will make it even faster. Honestly crazy numbers considering that
KZG-based implementation in Subspace required 4.5 seconds while burning most of my 64 logical CPU cores 🤯, and now a
single thread is an order of magnitude faster.

[PR 901]: https://github.com/nazar-pc/abundance/pull/901
[AndersTrier/reed-solomon-simd#80]: https://github.com/AndersTrier/reed-solomon-simd/pull/80

### Allocations and direct I/O

I reviewed Zooko's [smalloc] again recently and brainstormed a bit to see if it is a good fit. The conclusion is that an
allocator like that is very niche and in current state to use it well likely requires re-architecting an app around it.
However, reviewing allocations in the code I found a lot of places that could be optimized.

[smalloc]: https://github.com/zooko/smalloc

As the result `ab-aligned-buffer` that only supported 16-byte alignment before was extended to support larger alignments
as well. Then it also gained support of custom per-instance offsets (while retaining 16-byte size) and
`ab-direct-io-file` was re-designed to use it at API boundary. Now all uses of `bytes` crate except for NATS (its API)
are using `ab-aligned-buffer` instead, which is smaller (16 bytes vs 32 bytes) and simpler. Disk layout for files
created by farmer changed to ensure page alignment and now most things that interact with disk or network avoid extra
allocations on both read and write paths. Everything is stored in page-aligned buffers and just sliced and referenced as
needed.

Very happy with these changes!

## Tests

I probably forgot about a lot of things, and honestly it is probably fine. The important thing, though, is that a lot of
code that was not exercised (yet) now has tests, and code that was exercised is much more robust with various edge cases
covered.

While I did not write anything that implements intermediate shards just yet, I certainly got a lot closer because if I
did it earlier, I would have had a lot of "fun" debugging tricky issues that are now resolved.

There are many thousands of lines of tests now, I can't even describe it. I would not have written them myself,
certainly not to this level of detail in any reasonable amount of time. This is truly an incredible time to be an
engineer with LLMs doing so much of the heavy lifting possible.

## Backports

I backported a bunch of things into [Subspace] as well. While code bases are increasingly diverging even in mostly
forked crates, there are still similar patterns and LLMs again make backports almost effortless.

[Subspace]: https://github.com/autonomys/subspace

## Conclusion

This is the first two-three months that I mostly reviewed code rather than wrote it myself, which is crazy!

Sure, I sometimes have LLM re-design the architecture two to four times before I get what I like since it doesn't seem
to have sufficient taste or has superhuman tolerance to API complexity. And I certainly wouldn't merge the first thing
it produces as is. But it is finally at the point where given good instructions and clear constraints I can get work
done faster that way than by writing it myself. At least for mostly bug fixing and optimizations I have been dealing
with.

Lately my primary bottleneck is the ability to comprehend and review all the generated changes plus 1-2 hour iteration
times between some of the changes due to weak CPUs used in sandboxed environments. Certainly a paradigm shift from when
I tried Claude 3.7 that was mostly a joke rather than a serious tool.

I'll certainly spend some more time fixing various issues, but I'll try to get the next update out the door sooner. And
you can expect a blog post with release announcement for various crates in the near future too.

[Zulip] is where you can discuss things with me, see you again in a bit!

[Zulip]: https://abundance.zulipchat.com/
