//! Tests of `IoTypeMut`, which gives generic code exclusive access to both `IoType` values and the
//! values that constructors of containers over exclusive memory return

use ab_io_type::maybe_data::MaybeData;
use ab_io_type::variable_elements::VariableElements;
use ab_io_type::{IoType, IoTypeMut};
use core::mem::MaybeUninit;

fn size_of_target<A>(value: &mut A) -> u32
where
    A: IoTypeMut,
{
    value.io_type_mut().size()
}

#[test]
fn plain_value() {
    let mut value = 5u64;
    assert_eq!(size_of_target(&mut value), 8);
}

#[test]
fn maybe_data() {
    let mut value = 5u64;
    let mut size = 8;
    assert_eq!(
        size_of_target(&mut MaybeData::from_mut(&mut value, &mut size).unwrap()),
        8
    );
    let mut size = 0;
    assert_eq!(
        size_of_target(&mut MaybeData::from_mut(&mut value, &mut size).unwrap()),
        0
    );

    let mut uninit = MaybeUninit::<u64>::uninit();
    let mut size = 0;
    {
        let mut maybe_data = MaybeData::from_uninit(&mut uninit, &mut size).unwrap();
        assert_eq!(size_of_target(&mut maybe_data), 0);
        // Writes through the exclusive reference reach the memory of the view
        maybe_data.io_type_mut().replace(7);
        assert_eq!(size_of_target(&mut maybe_data), 8);
    }
    assert_eq!(size, 8);

    // A value obtained by dereferencing a view
    let mut size = 8;
    let mut maybe_data = MaybeData::from_mut(&mut value, &mut size).unwrap();
    assert_eq!(size_of_target(&mut *maybe_data), 8);
}

#[test]
fn variable_elements() {
    let mut buffer = [1u32, 2];
    let mut size = 8;
    assert_eq!(
        size_of_target(
            &mut VariableElements::<u32>::from_buffer_mut(&mut buffer, &mut size).unwrap()
        ),
        8
    );

    let mut uninit = [MaybeUninit::<u32>::uninit(); 2];
    let mut size = 0;
    {
        let mut elements = VariableElements::<u32>::from_uninit(&mut uninit, &mut size).unwrap();
        assert_eq!(size_of_target(&mut elements), 0);
        assert!(elements.append(&[7]));
        assert_eq!(size_of_target(&mut elements), 4);
    }
    assert_eq!(size, 4);
}
