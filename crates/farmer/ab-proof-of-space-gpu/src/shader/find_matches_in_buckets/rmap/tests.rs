use crate::shader::find_matches_in_buckets::rmap::{
    NextPhysicalPointer, Rmap, RmapBitPosition, RmapBitPositionExt,
};
use crate::shader::types::{Position, PositionExt};

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
