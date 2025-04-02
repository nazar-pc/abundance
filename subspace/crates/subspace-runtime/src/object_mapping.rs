//! Object mapping extraction code.
//! This code must be kept in sync with `test/subspace-test-runtime/src/lib.rs`.

use crate::{Block, Runtime, RuntimeCall};
use parity_scale_codec::{Compact, CompactLen, Encode};
use sp_std::prelude::*;
use subspace_core_primitives::hashes;
use subspace_core_primitives::objects::{BlockObject, BlockObjectMapping};
use subspace_runtime_primitives::MAX_CALL_RECURSION_DEPTH;

/// Extract the nested `pallet-utility` object mappings from `call`.
///
/// Object mappings are currently ignored if nested within unprivileged multisig `RuntimeCalls`,
/// privileged sudo, collective, and scheduler `RuntimeCall`s, and democracy nested `BoundedCall`s.
// TODO:
// - add start and end nesting closures to nested_call_iter(), so we can modify and restore
//   the base offset
// - convert this code to use nested_call_iter(), and remove the recursion depth limit
// - extract other nested calls: sudo, collective, multisig, scheduler, democracy (BoundedCall)
pub(crate) fn extract_utility_block_object_mapping(
    mut base_offset: u32,
    objects: &mut Vec<BlockObject>,
    call: &pallet_utility::Call<Runtime>,
    mut recursion_depth_left: u16,
) {
    if recursion_depth_left == 0 {
        return;
    }

    recursion_depth_left -= 1;

    // Add enum variant to the base offset.
    base_offset += 1;

    match call {
        pallet_utility::Call::batch { calls }
        | pallet_utility::Call::batch_all { calls }
        | pallet_utility::Call::force_batch { calls } => {
            base_offset += Compact::compact_len(&(calls.len() as u32)) as u32;

            for call in calls {
                extract_call_block_object_mapping(base_offset, objects, call, recursion_depth_left);

                base_offset += call.encoded_size() as u32;
            }
        }
        pallet_utility::Call::as_derivative { index, call } => {
            base_offset += index.encoded_size() as u32;

            extract_call_block_object_mapping(
                base_offset,
                objects,
                call.as_ref(),
                recursion_depth_left,
            );
        }
        pallet_utility::Call::dispatch_as { as_origin, call } => {
            base_offset += as_origin.encoded_size() as u32;

            extract_call_block_object_mapping(
                base_offset,
                objects,
                call.as_ref(),
                recursion_depth_left,
            );
        }
        pallet_utility::Call::with_weight { call, .. } => {
            extract_call_block_object_mapping(
                base_offset,
                objects,
                call.as_ref(),
                recursion_depth_left,
            );
        }
        pallet_utility::Call::__Ignore(_, _) => {
            // Ignore.
        }
    }
}

/// Extract the object mappings from `call`.
pub(crate) fn extract_call_block_object_mapping(
    mut base_offset: u32,
    objects: &mut Vec<BlockObject>,
    call: &RuntimeCall,
    recursion_depth_left: u16,
) {
    // Add RuntimeCall enum variant to the base offset.
    base_offset += 1;

    match call {
        // Extract the actual object mappings.
        RuntimeCall::System(frame_system::Call::remark { remark }) => {
            objects.push(BlockObject {
                hash: hashes::blake3_hash(remark),
                // Add frame_system::Call enum variant to the base offset.
                offset: base_offset + 1,
            });
        }
        RuntimeCall::System(frame_system::Call::remark_with_event { remark }) => {
            objects.push(BlockObject {
                hash: hashes::blake3_hash(remark),
                // Add frame_system::Call enum variant to the base offset.
                offset: base_offset + 1,
            });
        }

        // Recursively extract object mappings for the call.
        RuntimeCall::Utility(call) => {
            extract_utility_block_object_mapping(base_offset, objects, call, recursion_depth_left)
        }
        // Other calls don't contain object mappings.
        _ => {}
    }
}

/// Extract the object mappings from `block`.
pub(crate) fn extract_block_object_mapping(block: Block) -> BlockObjectMapping {
    let mut block_object_mapping = BlockObjectMapping::default();
    let mut base_offset =
        block.header.encoded_size() + Compact::compact_len(&(block.extrinsics.len() as u32));
    for extrinsic in block.extrinsics {
        let preamble_size = extrinsic.preamble.encoded_size();
        // Extrinsic starts with vector length followed by preamble and
        // `function` encoding.
        let base_extrinsic_offset = base_offset
            + Compact::compact_len(&((preamble_size + extrinsic.function.encoded_size()) as u32))
            + preamble_size;

        extract_call_block_object_mapping(
            base_extrinsic_offset as u32,
            block_object_mapping.objects_mut(),
            &extrinsic.function,
            MAX_CALL_RECURSION_DEPTH as u16,
        );

        base_offset += extrinsic.encoded_size();
    }

    block_object_mapping
}
