//! Opaque helpers for ZveXx extension

/// Size of an instruction in bytes.
///
/// All instructions here are the same size.
#[doc(hidden)]
pub const INSTRUCTION_SIZE: u8 = size_of::<u32>() as u8;
