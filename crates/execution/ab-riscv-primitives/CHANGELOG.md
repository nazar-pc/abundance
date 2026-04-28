# 0.0.4

Features:

* Implement `c.unimp` pseudo-instruction

Improvements:

* `Registers` removed from primitives as it is very implementation-specific
* Make `Register` trait safe

Fixes:

* Fix Zcmp instruction decoding, it now works with real-world binaries

# 0.0.3

Features:

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

Features:

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
