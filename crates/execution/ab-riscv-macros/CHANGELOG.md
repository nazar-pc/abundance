# 0.3.0

Breaking changes:

* `#[instruction]` now takes instruction alignment from an `ALIGNMENT` associated constant instead of an `alignment()`
  method
* Execution changes mirror API changes in `ab-riscv-interpreter`

Improvements:

* Support instruction macros in non-src directories (tests, examples, etc.)

# 0.1.1

Improvements:

* Always use `extern "C"` for indirect threading handlers on non-x86_64 targets
* Suppress warnings generated for extensions with a single instruction

# 0.1.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

New features:

* Major changes in supported syntax and generated code for better performance
* Support for panic-free implementations
* Support for `const` implementation of instruction execution
* Support for indirect threading dispatch code generation

# 0.0.4

New features:

* Support for compressed instructions (composition of size and alignment methods)

Improvements:

* Combine `where` predicates on instruction decoding implementation

Fixes:

* Add support for `const unsafe trait`

# 0.0.3

New features:

* Support for compressed instructions (composition of size and alignment methods)

Improvements:

* Improve pre- and post-processing of code to support more syntactic constructs
* Improve decoding-related code generation by referencing the original definition during composition rather than
  higher-level pre-composed code
    * This opens doors for more features down the road

Fixes:

* Fix handling of pending instructions to fix dependencies failing to resolve sometime
* Fix handling of some variants of execution `match` blocks

# 0.0.2

New features:

* Implement support for new `ExecutableInstruction::prepare_csr_read()`/`ExecutableInstruction::prepare_csr_write()`
  methods

Improvements:

* Retain original documentation attributes on enum definition
* Automatically combine generics when composing instructions

Fixes:

* Fix handling of match blocks in `#[instruction_execution]` macro in certain cases

# 0.0.1

Initial release
