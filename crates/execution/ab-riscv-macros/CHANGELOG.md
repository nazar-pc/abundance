# 0.0.3

Features:

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

Features:

* Implement support for new `ExecutableInstruction::prepare_csr_read()`/`ExecutableInstruction::prepare_csr_write()`
  methods

Improvements:

* Retain original documentation attributes on enum definition
* Automatically combine generics when composing instructions

Fixes:

* Fix handling of match blocks in `#[instruction_execution]` macro in certain cases

# 0.0.1

Initial release
