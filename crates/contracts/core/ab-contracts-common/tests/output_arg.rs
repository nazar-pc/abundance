//! Tests of `OutputArg` for memory whose contents accept every bit pattern, used the way an
//! executor does it: a call writes contents through the pointer and the size it reports is set
//! afterward

use ab_contracts_common::output::OutputArg;
use ab_io_type::IoType;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::variable_elements::VariableElements;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

/// Write `bytes` to the memory of `output` and report their number as the new size, like a
/// successful call does
fn call<Target, Output>(output: &mut Output, bytes: &[u8])
where
    Target: IoType,
    Output: OutputArg<Target>,
{
    let (ptr, _size, capacity) = output.as_raw_output();
    let size = u32::try_from(bytes.len()).expect("Small outputs in tests; qed");
    assert!(size <= capacity);
    // SAFETY: The pointer is valid for writes of the capacity (`OutputArg` contract), which `bytes`
    // fit into, and `output` isn't used until after the write
    unsafe {
        ptr.cast::<u8>()
            .copy_from_nonoverlapping(NonNull::from_ref(bytes).cast::<u8>(), bytes.len());
    }
    // SAFETY: Tests write a valid size for the target, with that many bytes initialized above
    unsafe {
        output.set_output_size(size);
    }
}

/// Give a container the method received to a call
fn forward(output: &mut MaybeData<'_, u64>, bytes: &[u8]) {
    call(output, bytes);
}

#[test]
fn plain_value() {
    let mut value = 0u64;
    call(&mut value, &7u64.to_le_bytes());
    assert_eq!(value, 7);

    // The type of an integer literal is inferred from the target
    let mut inferred = 0;
    call::<u32, _>(&mut inferred, &9u32.to_le_bytes());
    assert_eq!(inferred, 9);
}

#[test]
fn maybe_data() {
    let mut data = 0u64;
    let mut size = 0;
    call(
        &mut MaybeData::from_mut(&mut data, &mut size).unwrap(),
        &5u64.to_le_bytes(),
    );
    // The size reaches the memory the view describes
    assert_eq!(size, 8);
    assert_eq!(data, 5);

    let mut uninit = MaybeUninit::<u16>::uninit();
    let mut size = 0;
    {
        let mut maybe_data = MaybeData::from_uninit(&mut uninit, &mut size).unwrap();
        call(&mut maybe_data, &3u16.to_le_bytes());
        assert_eq!(maybe_data.get(), Some(&3));
    }
    assert_eq!(size, 2);

    // The type of an integer literal is inferred from the target
    let mut inferred = 0;
    let mut size = 0;
    call::<MaybeData<'_, u8>, _>(
        &mut MaybeData::from_mut(&mut inferred, &mut size).unwrap(),
        &[4],
    );
    assert_eq!((inferred, size), (4, 1));
}

#[test]
fn variable_elements() {
    let mut uninit = [MaybeUninit::<u16>::uninit(); 4];
    let mut size = 0;
    {
        let mut elements = VariableElements::<u16>::from_uninit(&mut uninit, &mut size).unwrap();
        call(&mut elements, &[1, 0, 2, 0, 3, 0]);
        assert_eq!(elements.get_initialized(), [1, 2, 3]);
    }
    assert_eq!(size, 6);
}

#[test]
fn forwarded_container() {
    let mut data = 0u64;
    let mut size = 0;
    forward(
        &mut MaybeData::from_mut(&mut data, &mut size).unwrap(),
        &6u64.to_le_bytes(),
    );
    assert_eq!((data, size), (6, 8));
}
