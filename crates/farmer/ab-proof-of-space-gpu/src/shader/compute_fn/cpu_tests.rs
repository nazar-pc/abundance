// use crate::shader::compute_fn::{compute_fn_impl, metadata_size_bits, y_size_bits};
// use crate::shader::constants::K;
// use ab_chacha8::{ChaCha8Block, ChaCha8State};
// use ab_core_primitives::pos::PosProof;
//
// // TODO: Reuse code from `ab-proof-of-space`, right now this is copy-pasted from there
// pub(super) fn compute_fn<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
//     y: u32,
//     left_metadata: u128,
//     right_metadata: u128,
// ) -> (u32, u128) {
//     let left_metadata = u128::from(left_metadata);
//     let right_metadata = u128::from(right_metadata);
//
//     let parent_metadata_bits = metadata_size_bits(K, PARENT_TABLE_NUMBER);
//
//     // Only supports `K` from 15 to 25 (otherwise math will not be correct when concatenating y,
//     // left metadata and right metadata)
//     let hash = {
//         // Take only bytes where bits were set
//         let num_bytes_with_data =
//             (y_size_bits(K) + metadata_size_bits(K, PARENT_TABLE_NUMBER) * 2).div_ceil(u8::BITS);
//
//         // Collect `K` most significant bits of `y` at the final offset of eventual `input_a`
//         let y_bits = u128::from(y) << (u128::BITS - y_size_bits(K));
//
//         // Move bits of `left_metadata` at the final offset of eventual `input_a`
//         let left_metadata_bits =
//             left_metadata << (u128::BITS - parent_metadata_bits - y_size_bits(K));
//
//         // Part of the `right_bits` at the final offset of eventual `input_a`
//         let y_and_left_bits = y_size_bits(K) + parent_metadata_bits;
//         let right_bits_start_offset = u128::BITS - parent_metadata_bits;
//
//         // If `right_metadata` bits start to the left of the desired position in `input_a` move
//         // bits right, else move left
//         if right_bits_start_offset < y_and_left_bits {
//             let right_bits_pushed_into_input_b = y_and_left_bits - right_bits_start_offset;
//             // Collect bits of `right_metadata` that will fit into `input_a` at the final offset in
//             // eventual `input_a`
//             let right_bits_a = right_metadata >> right_bits_pushed_into_input_b;
//             let input_a = y_bits | left_metadata_bits | right_bits_a;
//             // Collect bits of `right_metadata` that will spill over into `input_b`
//             let input_b = right_metadata << (u128::BITS - right_bits_pushed_into_input_b);
//
//             let input = [input_a.to_be_bytes(), input_b.to_be_bytes()];
//             let input_len = size_of::<u128>() + right_bits_pushed_into_input_b.div_ceil(u8::BITS);
//             ab_blake3::single_block_hash(&input.as_flattened()[..input_len])
//                 .expect("Exactly a single block worth of bytes; qed")
//         } else {
//             let right_bits_a = right_metadata << (right_bits_start_offset - y_and_left_bits);
//             let input_a = y_bits | left_metadata_bits | right_bits_a;
//
//             ab_blake3::single_block_hash(&input_a.to_be_bytes()[..num_bytes_with_data])
//                 .expect("Less than a single block worth of bytes; qed")
//         }
//     };
//
//     let y_output =
//         u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) >> (u32::BITS - y_size_bits(K));
//
//     let metadata_size_bits = metadata_size_bits(K, TABLE_NUMBER);
//
//     let metadata = if TABLE_NUMBER < 4 {
//         (left_metadata << parent_metadata_bits) | right_metadata
//     } else if metadata_size_bits > 0 {
//         // For K up to 25 it is guaranteed that metadata + bit offset will always fit into u128.
//         // We collect bytes necessary, potentially with extra bits at the start and end of the bytes
//         // that will be taken care of later.
//         let metadata = u128::from_be_bytes(
//             hash[y_size_bits(K) / u8::BITS..][..size_of::<u128>()]
//                 .try_into()
//                 .expect("Always enough bits for any K; qed"),
//         );
//         // Remove extra bits at the beginning
//         let metadata = metadata << (y_size_bits(K) % u8::BITS);
//         // Move bits into the correct location
//         metadata >> (u128::BITS - metadata_size_bits)
//     } else {
//         0
//     };
//
//     (y_output, metadata)
// }
//
// #[test]
// fn compute_fn_cpu() {
//     let seed = [1; 32];
//     let num_x = 100;
//
//     // Calculate the necessary number of ChaCha8 blocks
//     let keystream_length_blocks =
//         (num_x * u32::from(PosProof::K)).div_ceil(size_of::<ChaCha8Block>() as u32 * u8::BITS);
//     let initial_state = ChaCha8State::init(&seed, &[0; _]);
//
//     let chacha8_keystream = (0..keystream_length_blocks)
//         .map(|counter| initial_state.compute_block(counter))
//         .collect::<Vec<_>>();
//
//     for x in 0..num_x {
//         assert_eq!(
//             compute_fn::<{ PosProof::K }>(x, &seed),
//             compute_fn_impl(x, chacha8_keystream.as_flattened()),
//             "X={x}"
//         );
//     }
// }
