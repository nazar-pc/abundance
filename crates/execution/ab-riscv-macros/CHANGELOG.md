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
