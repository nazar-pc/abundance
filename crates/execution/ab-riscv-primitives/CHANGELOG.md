# 0.3.0

Breaking changes:

* `Instruction::alignment()` is replaced by the `Instruction::ALIGNMENT` associated constant

# 0.2.0

New features:

* Implemented new extensions (pass all ACT4 tests):
    * Zifencei
    * Ssstrict
* Added `MCsr::Mconfigptr`, `Mcycle`, `Minstret`, `Mcycleh`, `Minstreth`, `Menvcfg`, `Menvcfgh`, `Mseccfg` and
  `Mseccfgh` constants (mandatory M-mode CSRs previously missing from `MCsr`)

Fixes:

* Ssstrict fixes:
    * Reject `vmv.v.v`/`vmv.v.i`/`vmv.v.x` encodings with a nonzero `vs2` - per spec `vs2` is fixed to `v0` for these
      (the unmasked forms of `vmerge.vvm`/`vmerge.vim`/`vmerge.vxm`), and any other value is reserved
    * `vror.vi` now decodes its full 6-bit immediate (0-63, needed for SEW=64 rotate amounts) - the low bit of funct6
      extends the 5-bit field in `vs1`, but was previously required to be zero, incorrectly rejecting half of the valid
      encoding space as illegal instead of decoding it
    * Reject `vid.v` encodings with a nonzero `vs2` - the field is reserved (must be `v0`) since `vid.v` has no source
      vector operand, and any other value is reserved
    * RV32 `rori` now correctly requires the full 7-bit funct7 (`0b0110000`) instead of only its top 6 bits - unlike
      RV64 (which legitimately needs a 6-bit shamt, with bit 25 as shamt[5]), RV32's shamt is only 5 bits, so bit 25
      isn't part of the immediate and must be checked; this fixed a copy-paste bug from the RV64 decoder that accepted a
      range of reserved encodings as `rori`

# 0.1.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

New features:

* Implemented new extensions (pass all ACT4 tests):
    * A
    * Zaamo
    * Zabha
    * Zacas
    * Zalrsc
    * Zawrs
    * Zkr
    * Zvbb
    * Zvbc
    * Zvkb
* Implemented new extensions (in good shape, but ACT4 tests are currently non-existing):
    * Zalasr
* Completely panic-free implementation of everything
* `const` implementations of essentially all APIs and many derives
* Support for indirect threading execution (not const) in addition to `match` loop for even higher performance

Improvements:

* Major API improvements around type safety, correctness, ergonomics and performance
* Improved documentation
* Improved performance
* Zve64x extension was refactored into generic ZveXx that can represent both Zve64x and Zve32x on both RV32 and RV64

Fixes:

* ZveXx (Zve64x, etc.) extension saw numerous fixes and now passes all ACT4 tests

# 0.0.4

New features:

* Implement `c.unimp` pseudo-instruction

Improvements:

* `Registers` removed from primitives as it is very implementation-specific
* Make `Register` trait safe

Fixes:

* Fix Zcmp instruction decoding, it now works with real-world binaries

# 0.0.3

New features:

* Implemented new extensions (pass all ACT4 tests):
    * Zbkb
    * Zbkx
    * Zca
    * Zcb
    * Zicond
    * Zkn
    * Zknd
    * Zkne
* Implemented new extensions (in good shape, but ACT4 tests are currently non-existing):
    * Zcmp

Improvements:

* Added prelude module with re-export of everything for much more manageable imports

Fixes:

* Fix various Zve64x issues (most likely still buggy though)

# 0.0.2

New features:

* Zicsr extension support
* Experimental Zve32x/Zve64x extension support (known to be buggy)
* RV32 support, including all extensions previously supported on RV64

Improvements:

* Improved API and generics on GPRs with more operations
* RISC-V Architectural Certification Tests pass successfully for everything except vector extensions

Fixes:

* Fixed Zba/Zbb instruction decoding
* Fixed `fence.tso` instruction decoding

# 0.0.1

Initial release
