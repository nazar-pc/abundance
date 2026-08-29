# 0.2.0

Breaking changes:

* `SystemInstructionHandler::handle_ebreak()` now takes `program_counter: &mut PC` (replacing the previous by-value
  `pc`) and an `instruction_size`, and returns `Result<ControlFlow<()>, ExecutionError<Reg::Type>>`, matching
  `handle_ecall()` - needed so an implementation can dispatch a Breakpoint trap (redirect `program_counter`) instead of
  only observing where the breakpoint fired

New features:

* Implemented new extensions (pass all ACT4 tests):
    * Zifencei
    * Ssstrict

Fixes:

* Ssstrict fixes:
    * Reject unaligned addresses in Zalrsc implementation
    * Reject AMOs whose misaligned access crosses the atomicity granule
    * AMO instructions (`Zaamo`, `Zabha`, `Zacas`) now report a Store/AMO fault instead of a Load fault when their read
      half faults out of bounds - the two halves of an AMO's read-modify-write access are a single atomic operation and
      must be classified identically regardless of which physical half a fault surfaces on
    * `sc`/`sc.d` now check their target address for a Store/AMO access fault even when the reservation check itself
      fails, matching a successful `sc`'s own fault behavior - only the write is skipped on failure, not the fault check
    * Segment vector loads/stores (`vlseg*`/`vsseg*` and their strided/indexed/fault-only-first variants) now reject
      encodings where `NFIELDS * EMUL > 8`, per spec - previously only the `vd`/`vs3` register-group-fits-in-32 bound
      was checked, silently accepting reserved encodings that exceed the maximum allowed field-group size
    * Zvbc's `vclmul`/`vclmulh` now reject any SEW other than 64 - per spec these carry-less multiply instructions are
      only defined at SEW=64, and any other SEW is a reserved encoding
    * Narrowing shift/clip instructions (`vnsra`/`vnsrl`/`vnclip`/`vnclipu`) now reject a destination that overlaps only
      the high part of the wide source register group - only aliasing the low part (`vd == vs2`) is legal per spec; any
      other overlap was previously accepted instead of rejected as reserved

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
* `const` implementations of almost all APIs and execution of most extensions (except vector)

Improvements:

* Many API improvements around type safety and correctness
* Improved documentation

Fixes:

* ZveXx (Zve64x, etc.) extension saw numerous fixes and now passes all ACT4 tests

# 0.0.4

New features:

* Implement `c.unimp` pseudo-instruction
* Introduce `RegisterFile` trait that allows customizing registers data structure with `BasicRegisters` that contains
  implementation that previously existed in primitives

Improvements:

* Replace `state` argument in `ExecutableInstruction::execute()` with explicit arguments, opening paths for further
  optimizations in the future, `InterpreterState` moved to `basic::BasicInterpreterState`
* Modify some code that prevented inlining in the interpreter with explicit loops
* Better `xperm4`/`xperm8` helpers with SIMD

Fixes:

* Make `BasicInstructionFetcher` support 16-bit instruction at the very end of the address space

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

* Used hardware intrinsics for RV32 and RV64 in many more cases
* Added prelude module with re-export of everything for much more manageable imports

Fixes:

* Fixed hardware intrinsics support for RV32 and RV64 (some are now checked in CI)

# 0.0.2

New features:

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
