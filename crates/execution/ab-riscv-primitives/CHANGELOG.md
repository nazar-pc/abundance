# 0.3.0

Breaking changes:

* `Instruction::alignment()` is replaced by the `Instruction::ALIGNMENT` associated constant
* Changed APIs around vector extensions for better type safety and performance
* Instruction enums are generic over hart configuration (`HartConfig`) instead of the register type, `BasicHart` and
  `BasicVectorHart` cover common cases, `Instruction::Reg` is replaced with `Instruction::Hart`
* `ELEN` and `VLEN` are configured with `VectorHartConfig::VECTOR_LENGTHS` (`VectorHartConfig` is required by vector
  instructions), `SUPPORTED_ELEN_VLEN` is replaced by `VectorLengths::new()`
* `Vtype` is generic over hart configuration instead of `ELEN`/`VLEN`
* `M` inherits `Zmmul` and `Zbc` inherits `Zbkc`, instructions shared by `Zbb`/`Zbkb` and `Zknd`/`Zkne` are in separate
  enums inherited by both
* `RVE` constant moved from `ZcmpRegister` to `Register`
* `Instruction::IMPLEMENTED_EXTENSIONS` includes extensions with some of their instructions ignored or missing due to
  conditions
* `Instruction::IMPLEMENTED_EXTENSIONS` contains `ImplementedExtension` instead of `TypeId`, instruction enums must
  specify their ISA extensions in `Instruction::OWN_ISA_EXTENSIONS`
* `Reg<Type>` implements `From<EReg<Type>>` instead of `From<EReg<u64>>`, which it implemented for any `Type` by
  mistake: RV32E registers convert into RV32I ones, and RV64E registers no longer convert into RV32I ones

New features:

* `Vtype::vlmax()` and `Vtype::eew_register_count()`
* `Instruction::ISA_STRING` with ISA string of an instruction set (as used in `Tag_RISCV_arch` attribute of ELF files)
* Vector loads and stores with `EEW > ELEN` and `vzext.vf8`/`vsext.vf8` with `ELEN < 64` are rejected during decoding,
  Zvbc with `ELEN < 64` and Zve* with `ELEN > 64` don't compile

Fixes:

* `pack rd, rs1, x0` (RV32) and `packw rd, rs1, x0` (RV64) are decoded with Zbkb without Zbb (as `zext.h` with Zbb)
* Hidden `EReg::Phantom` and `Reg::Phantom` variants are now truly uninhabited, previously they could be constructed
  from safe code, resulting in undefined behavior in `Display` implementations and elsewhere
* Ssstrict fixes (all matching Sail):
    * Masked vector instructions that read `v0`, the mask, as a data source are reserved and no longer decode
    * Masked vector instructions that write `v0` with an element width other than 1 are reserved and no longer decode
    * `vtype` with `SEW` above `LMUL * ELEN` for fractional `LMUL` sets `vill`

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
