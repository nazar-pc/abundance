//! `#[contract]` macro will process *public* methods annotated with following attributes:
//! * `#[init]` - method that can be called to produce an initial state of the contract,
//!   called once during contacts lifetime
//! * `#[update]` - method that can read and/or modify state and/or slots of the contact, may be
//!   called by user transaction directly or by another contract
//! * `#[view]` - method that can only read blockchain data, can read state or slots of the
//!   contract, but can't modify their contents
//!
//! Each argument (except `self`) of these methods has to be annotated with one of the following
//! attributes (must be in this order):
//! * `#[env]` - environment variable, used to access ephemeral execution environment, call methods
//!   on other contracts, etc.
//! * `#[tmp]` - temporary ephemeral value to store auxiliary data while processing a transaction
//! * `#[slot]` - slot corresponding to this contract
//! * `#[input]` - method input coming from user transaction or invocation from another contract
//! * `#[output]` - method output
//! * `#[result]` - a single optional method result as an alternative to returning values from a
//!   function directly, useful to reduce stack usage
//!
//! # For struct implementation
//!
//! ## #\[init]
//!
//! Initializer's purpose is to produce the initial state of the contract.
//!
//! Following arguments are supported by this method (must be in this order):
//! * `#[env]` read-only and read-write
//! * `#[tmp]`
//! * `#[slot]` read-only and read-write
//! * `#[input]`
//! * `#[output]`
//! * `#[result]`
//!
//! `self` argument is not supported in any way in this context since state of the contract is just
//! being created.
//!
//! ## #\[update]
//!
//! Generic method contract that can both update contract's own state and contents of slots.
//!
//! Following arguments are supported by this method (must be in this order):
//! * `&self` or `&mut self` depending on whether state reads and/or modification are required
//! * `#[env]` read-only and read-write
//! * `#[tmp]`
//! * `#[slot]` read-only and read-write
//! * `#[input]`
//! * `#[output]`
//! * `#[result]`
//!
//! ## #\[view]
//!
//! Similar to `#[update]`, but can only access read-only view of the state and slots, can be called
//! outside of block context and can only call other `#[view]` methods.
//!
//! Following arguments are supported by this method (must be in this order):
//! * `&self`
//! * `#[env]` read-only
//! * `#[slot]` read-only
//! * `#[input]`
//! * `#[output]`
//! * `#[result]`
//!
//! # For trait definition and trait implementation
//!
//! ## #\[update]
//!
//! Generic method contract that can (in case of trait indirectly) both update contract's own state
//! and contents of slots.
//!
//! Following arguments are supported by this method in trait context (must be in this order):
//! * `#[env]` read-only and read-write
//! * `#[input]`
//! * `#[output]`
//! * `#[result]`
//!
//! ## #\[view]
//!
//! Similar to `#[update]`, but can only access (in case of trait indirectly) read-only view of the
//! state and slots, can be called outside of block context and can only call other `#[view]`
//! methods.
//!
//! Following arguments are supported by this method in trait context (must be in this order):
//! * `#[env]` read-only
//! * `#[input]`
//! * `#[output]`
//! * `#[result]`

#[doc(hidden)]
pub mod __private;

pub use ab_contracts_macros_impl::contract;
