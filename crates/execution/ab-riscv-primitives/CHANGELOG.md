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
