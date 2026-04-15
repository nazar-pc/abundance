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

* Used hardware intrinsics for RV32 and RV64 in many more cases
* Added prelude module with re-export of everything for much more manageable imports

Fixes:

* Fixed hardware intrinsics support for RV32 and RV64 (some are now checked in CI)

# 0.0.2

Features:

* Zicsr extension support
* Experimental Zve32x/Zve64x extension support (known to be buggy)
* Extensible state infrastructure that allowed to support CSRs, vector extensions and can be used to introduce floating
  point support and other features in the future, while keeping it zero cost to those who don't need it
* RV32 support, including all extensions previously supported on RV64

Improvements:

* Customizable handlers for fence instructions (was hardcoded to no-op before)
* Substantially simplified error handling for common cases
* Extended virtual memory API to support vector extensions
* Improved developer experience with helper modules for reusable parts of the implementation (more improvements coming
  later)
* Slightly improved performance
* RISC-V Architectural Certification Tests pass successfully for everything except vector extensions

Fixes:

* Fixed Zbc instruction execution

# 0.0.1

Initial release
