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
