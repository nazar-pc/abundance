use crate::shader::constants::{MAX_BUCKET_SIZE, PARAM_BC, REDUCED_BUCKET_SIZE};
use crate::shader::find_matches_in_buckets::cpu_tests::Rmap as CorrectRmap;
use crate::shader::find_matches_in_buckets::rmap::{
    NextPhysicalPointer, Rmap, RmapBitPosition, RmapBitPositionExt,
};
use crate::shader::types::{Position, PositionExt, PositionR, R};
use chacha20::ChaCha8Rng;
use rand::prelude::*;
use std::array;

#[test]
fn test_rmap_basic() {
    let mut next_physical_pointer = NextPhysicalPointer::default();
    let mut rmap = Rmap::new();

    unsafe {
        rmap.add(
            RmapBitPosition::new(0),
            Position::from_u32(100),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(0)),
            [Position::from_u32(100), Position::from_u32(0)]
        );

        rmap.add(
            RmapBitPosition::new(0),
            Position::from_u32(101),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(0)),
            [Position::from_u32(100), Position::from_u32(101)]
        );

        // Ignored as duplicate `r`
        rmap.add(
            RmapBitPosition::new(0),
            Position::from_u32(102),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(0)),
            [Position::from_u32(100), Position::from_u32(101)]
        );

        rmap.add(
            RmapBitPosition::new(1),
            Position::from_u32(200),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(1)),
            [Position::from_u32(200), Position::from_u32(0)]
        );
    }
}

#[test]
fn test_rmap_spanning_across_words() {
    let mut next_physical_pointer = NextPhysicalPointer::default();
    let mut rmap = Rmap::new();

    unsafe {
        rmap.add(
            RmapBitPosition::new(24),
            Position::from_u32(300),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(24)),
            [Position::from_u32(300), Position::from_u32(0)]
        );

        rmap.add(
            RmapBitPosition::new(24),
            Position::from_u32(301),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(24)),
            [Position::from_u32(300), Position::from_u32(301)]
        );

        // Ignored as duplicate `r`
        rmap.add(
            RmapBitPosition::new(24),
            Position::from_u32(302),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(24)),
            [Position::from_u32(300), Position::from_u32(301)]
        );
    }
}

#[test]
fn test_rmap_zero_position() {
    let mut next_physical_pointer = NextPhysicalPointer::default();
    let mut rmap = Rmap::new();

    unsafe {
        // Zero position is effectively ignored
        rmap.add(
            RmapBitPosition::new(2),
            Position::from_u32(0),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(2)),
            [Position::from_u32(0), Position::from_u32(0)]
        );

        rmap.add(
            RmapBitPosition::new(2),
            Position::from_u32(400),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(2)),
            [Position::from_u32(400), Position::from_u32(0)]
        );

        // Zero position is effectively ignored
        rmap.add(
            RmapBitPosition::new(2),
            Position::from_u32(0),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(2)),
            [Position::from_u32(400), Position::from_u32(0)]
        );

        rmap.add(
            RmapBitPosition::new(2),
            Position::from_u32(401),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(2)),
            [Position::from_u32(400), Position::from_u32(401)]
        );
    }
}

#[test]
fn test_rmap_zero_when_full() {
    let mut next_physical_pointer = NextPhysicalPointer::default();
    let mut rmap = Rmap::new();

    unsafe {
        rmap.add(
            RmapBitPosition::new(3),
            Position::from_u32(500),
            &mut next_physical_pointer,
        );
        rmap.add(
            RmapBitPosition::new(3),
            Position::from_u32(501),
            &mut next_physical_pointer,
        );
        // Ignored as duplicate `r`
        rmap.add(
            RmapBitPosition::new(3),
            Position::from_u32(0),
            &mut next_physical_pointer,
        );
        assert_eq!(
            rmap.get(RmapBitPosition::new(3)),
            [Position::from_u32(500), Position::from_u32(501)]
        );
    }
}

#[test]
fn test_rmap_against_reference() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let source_bucket = {
        let mut bucket = array::from_fn::<_, MAX_BUCKET_SIZE, _>(|i| PositionR {
            position: i as u32,
            r: unsafe { R::new(rng.random_range(0..u32::from(PARAM_BC))) },
        });
        bucket.shuffle(&mut rng);
        // There should be at most `REDUCED_BUCKET_SIZE` elements (safety invariant)
        bucket[REDUCED_BUCKET_SIZE..].fill(PositionR::SENTINEL);
        // There should be at least one `Position::ZERO`. The implementation is supposed to
        // explicitly ignore it, so let's test that. Reuse `r` from the next element to check
        // whether zero position impacts it or not (it shouldn't)
        bucket[4] = PositionR {
            position: Position::ZERO,
            r: bucket[5].r,
        };
        bucket
    };

    let mut rmap_correct = CorrectRmap::new();
    {
        let mut bucket = source_bucket;
        bucket.sort_by_key(|position_r| position_r.position);

        for position_r in bucket {
            if position_r.position == Position::SENTINEL {
                break;
            }
            unsafe {
                rmap_correct.add(position_r.r.get_inner(), position_r.position);
            }
        }
    }

    let mut rmap_sequential = Rmap::new();
    {
        let mut bucket = source_bucket;
        bucket.sort_by_key(|position_r| position_r.position);

        let mut next_physical_pointer = NextPhysicalPointer::default();
        for position_r in bucket {
            if position_r.position == Position::SENTINEL {
                break;
            }
            unsafe {
                rmap_sequential.add(
                    RmapBitPosition::new(position_r.r.get_inner()),
                    position_r.position,
                    &mut next_physical_pointer,
                );
            }
        }
    }

    let mut rmap_parallel = Rmap::new();
    {
        let mut bucket = source_bucket;
        bucket.sort_by_key(|position_r| (position_r.r, position_r.position));
        unsafe {
            Rmap::update_local_bucket_r_data(0, 1, &mut bucket);
        }

        for position_r in &bucket {
            if position_r.position == Position::SENTINEL {
                break;
            }
            unsafe {
                rmap_parallel.add_with_data_parallel(position_r.r, position_r.position);
            }
        }
    }

    for r in 0..u32::from(PARAM_BC) {
        let rmap_bit_position = unsafe { RmapBitPosition::new(r) };
        assert_eq!(
            unsafe { rmap_correct.get(r) },
            rmap_sequential.get(rmap_bit_position),
            "r={r:?}"
        );
        assert_eq!(
            unsafe { rmap_correct.get(r) },
            rmap_parallel.get(rmap_bit_position),
            "r={r:?}"
        );
    }
}
