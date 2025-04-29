#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(
    array_chunks,
    generic_arg_infer,
    generic_const_exprs,
    maybe_uninit_slice,
    maybe_uninit_uninit_array_transpose,
    ptr_as_ref_unchecked,
    trusted_len
)]
#![no_std]

pub mod balanced_hashed;

use blake3::KEY_LEN;

/// Used as a key in keyed blake3 hash for inner nodes of Merkle Trees.
///
/// This value is a blake3 hash of as string `merkle-tree-inner-node`.
// TODO: Replace with hashing once https://github.com/BLAKE3-team/BLAKE3/issues/440 is resolved
pub const INNER_NODE_DOMAIN_SEPARATOR: [u8; KEY_LEN] = [
    0x53, 0x11, 0x7d, 0x4d, 0xa8, 0x1a, 0x34, 0x35, 0x0b, 0x1a, 0x30, 0xd4, 0x28, 0x6d, 0x7e, 0x5a,
    0x1e, 0xb0, 0xa2, 0x0f, 0x5e, 0x5e, 0x26, 0x94, 0x47, 0x4b, 0x4f, 0xbd, 0x86, 0xc3, 0xc0, 0x7e,
];
