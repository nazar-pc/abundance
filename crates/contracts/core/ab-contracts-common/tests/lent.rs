//! Tests of `Lent`, used through `OutputArg` the way an executor does it: a call writes contents
//! through the pointer and, if it succeeds, the size it reports is set afterward

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_common::output::{Lent, OutputArg};
use ab_io_type::IoType;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_elements::VariableElements;
use core::borrow::BorrowMut;
use core::mem;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(u8)]
enum Mode {
    A,
    B,
}

static VALIDATIONS: AtomicUsize = AtomicUsize::new(0);

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "`validate` functions take a reference"
)]
fn count_validation(_value: &Counted) -> bool {
    VALIDATIONS.fetch_add(1, Ordering::SeqCst);
    true
}

/// Accepts every value, but counts how many times it was parsed
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(C)]
#[trivial_type(validate = count_validation)]
struct Counted {
    value: u8,
}

struct Pair {
    mode: Mode,
    counter: u8,
}

/// Write `bytes` to the memory of `output` like a call does, and set the size it reports if it
/// succeeds (`Some`)
fn call<Target, Output>(output: &mut Output, bytes: &[u8], size: Option<u32>)
where
    Target: IoType,
    Output: OutputArg<Target>,
{
    let (ptr, _size, capacity) = output.as_raw_output();
    assert!(bytes.len() <= capacity as usize);
    // SAFETY: The pointer is valid for writes of the capacity (`OutputArg` contract), which `bytes`
    // fit into, and `output` isn't used until after the write
    unsafe {
        ptr.cast::<u8>()
            .copy_from_nonoverlapping(NonNull::from_ref(bytes).cast::<u8>(), bytes.len());
    }
    if let Some(size) = size {
        // SAFETY: Tests report a valid size for the target, with that many bytes initialized
        unsafe {
            output.set_output_size(size);
        }
    }
}

/// Size of the contents, from code that only knows that `T` implements `IoType`
fn contents_size<T>(lent: &mut Lent<'_, T>) -> Result<u32, ContractError>
where
    T: IoType,
{
    Ok(lent.get_mut()?.borrow_mut().size())
}

/// Other fields of an `#[output]` stay accessible after one of its fields was lent
#[expect(dead_code, reason = "Only checks that it compiles")]
fn lend_output_field<'a>(env: &Env<'a>, output: &'a mut Pair) -> Result<(), ContractError> {
    let mut lent = env.lend_output(&mut output.mode);
    call(&mut lent, &[1], Some(1));
    output.counter = 1;
    *lent.get_mut()? = Mode::B;
    Ok(())
}

#[test]
fn invalid_value_until_set() {
    let mut lent = Lent::new(Mode::A);
    assert_eq!(*lent.get_mut().unwrap(), Mode::A);
    call(&mut lent, &[7], Some(1));
    assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
    // Still invalid
    assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
    assert_eq!(*lent.set(Mode::B), Mode::B);
    assert_eq!(*lent.get_mut().unwrap(), Mode::B);
}

#[test]
fn moved_after_failed_call() {
    let mut lent = Lent::new(Mode::A);
    // A failed call leaves an invalid value behind and reports no size
    call(&mut lent, &[9], None);
    assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
    // Moved while the value is invalid and lent again, the pointer must follow the value
    let mut moved = Box::new(lent);
    call(&mut *moved, &[1], Some(1));
    assert_eq!(*moved.get_mut().unwrap(), Mode::B);
}

#[test]
fn parsed_once_after_each_call() {
    let mut lent = Lent::new(Counted { value: 1 });
    let before = VALIDATIONS.load(Ordering::SeqCst);
    // Not parsed before a call
    assert_eq!(lent.get_mut().unwrap().value, 1);
    assert_eq!(VALIDATIONS.load(Ordering::SeqCst) - before, 0);

    call(&mut lent, &[5], Some(1));
    assert_eq!(lent.get_mut().unwrap().value, 5);
    lent.get_mut().unwrap().value = 6;
    assert_eq!(lent.get_mut().unwrap().value, 6);
    assert_eq!(VALIDATIONS.load(Ordering::SeqCst) - before, 1);

    // Two calls without access in between, parsed once more
    call(&mut lent, &[7], Some(1));
    call(&mut lent, &[8], Some(1));
    assert_eq!(lent.get_mut().unwrap().value, 8);
    assert_eq!(VALIDATIONS.load(Ordering::SeqCst) - before, 2);
}

#[test]
fn maybe_data_from_uninit() {
    let mut uninit = MaybeUninit::<Mode>::uninit();
    let mut size = 0;
    {
        let mut lent = Lent::<MaybeData<'_, Mode>>::from_uninit(&mut uninit, &mut size).unwrap();
        assert_eq!(lent.get_mut().unwrap().get(), None);
        call(&mut lent, &[1], Some(1));
        assert_eq!(lent.get_mut().unwrap().get(), Some(&Mode::B));
        call(&mut lent, &[9], Some(1));
        assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
        // Size `0` is valid regardless of the contents
        call(&mut lent, &[], Some(0));
        assert_eq!(lent.get_mut().unwrap().get(), None);
        call(&mut lent, &[0], Some(1));
        assert_eq!(lent.get_mut().unwrap().get(), Some(&Mode::A));
    }
    // The size reaches the memory the container describes
    assert_eq!(size, 1);

    let mut size = 1;
    assert!(Lent::<MaybeData<'_, Mode>>::from_uninit(&mut uninit, &mut size).is_none());
}

#[test]
fn variable_elements_from_uninit() {
    let mut uninit = [MaybeUninit::<Mode>::uninit(); 4];
    let mut size = 0;
    {
        let mut lent =
            Lent::<VariableElements<'_, Mode>>::from_uninit(&mut uninit, &mut size).unwrap();
        call(&mut lent, &[0, 1, 1], Some(3));
        assert_eq!(
            lent.get_mut().unwrap().get_initialized(),
            [Mode::A, Mode::B, Mode::B]
        );
        // Changes made through the parsed value are given to the next call
        assert!(lent.get_mut().unwrap().truncate(1));
        assert_eq!(lent.as_raw_output().1, 1);
        assert_eq!(lent.get_mut().unwrap().get_initialized(), [Mode::A]);

        call(&mut lent, &[0, 2], Some(2));
        assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
    }
    // The size reaches the memory the container describes even if the contents are invalid
    assert_eq!(size, 2);

    let mut size = 1;
    assert!(Lent::<VariableElements<'_, Mode>>::from_uninit(&mut uninit, &mut size).is_none());
}

/// `get_mut()` returns a new view of a container, so replacing it doesn't change the memory that
/// is lent to calls, neither for a container `Lent` owns nor for one it borrows
#[test]
fn replaced_view() {
    let mut typed = [Mode::A];
    let mut typed_size = Mode::SIZE;

    let mut uninit = [MaybeUninit::<Mode>::uninit(); 1];
    let mut size = 0;
    let mut lent = Lent::<VariableElements<'_, Mode>>::from_uninit(&mut uninit, &mut size).unwrap();
    {
        let mut view = VariableElements::from_buffer_mut(&mut typed, &mut typed_size).unwrap();
        let mut lent_view = lent.get_mut().unwrap();
        mem::swap(&mut lent_view, &mut view);
    }
    // A call writes an invalid value into the lent memory
    call(&mut lent, &[7], Some(1));
    assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
    assert_eq!(typed, [Mode::A]);
    assert_eq!(typed_size, Mode::SIZE);

    let mut uninit = [MaybeUninit::<Mode>::uninit(); 1];
    let mut size = 0;
    let mut original = VariableElements::<Mode>::from_uninit(&mut uninit, &mut size).unwrap();
    {
        // SAFETY: The contents of `original` are not accessed afterward
        let mut lent = unsafe { Lent::from_mut_unchecked(&mut original) };
        let mut view = VariableElements::from_buffer_mut(&mut typed, &mut typed_size).unwrap();
        let mut lent_view = lent.get_mut().unwrap();
        mem::swap(&mut lent_view, &mut view);
        call(&mut lent, &[7], Some(1));
        assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
    }
    assert_eq!(typed, [Mode::A]);
    assert_eq!(typed_size, Mode::SIZE);
    // The size reaches the container that was lent
    assert_eq!(original.size(), 1);
}

/// `get_mut()` returns `&mut T` for a plain value and a new view by value for a container, generic
/// code gets `&mut T` from both
#[test]
fn get_mut_generic() {
    let mut lent = Lent::new(Mode::A);
    let mode: &mut Mode = lent.get_mut().unwrap();
    *mode = Mode::B;
    assert_eq!(contents_size(&mut lent), Ok(Mode::SIZE));
    assert_eq!(lent.get_mut(), Ok(&mut Mode::B));

    let mut uninit = [MaybeUninit::<Mode>::uninit(); 2];
    let mut size = 0;
    let mut lent = Lent::<VariableElements<'_, Mode>>::from_uninit(&mut uninit, &mut size).unwrap();
    assert_eq!(contents_size(&mut lent), Ok(0));
    call(&mut lent, &[0, 1], Some(2));
    assert_eq!(contents_size(&mut lent), Ok(2));
}

#[test]
fn size_set_without_call() {
    let mut uninit = MaybeUninit::<Mode>::uninit();
    let mut size = 0;
    let mut lent = Lent::<MaybeData<'_, Mode>>::from_uninit(&mut uninit, &mut size).unwrap();
    // A call writes an invalid value, but reports that the output is empty
    call(&mut lent, &[9], Some(0));
    assert_eq!(lent.get_mut().unwrap().get(), None);
    // The byte stays initialized, so the size can be set to include it, but the contents must
    // still be parsed before they are accessed
    // SAFETY: The size of the data, with that many bytes initialized by the call above
    unsafe {
        lent.set_output_size(1);
    }
    assert_eq!(lent.get_mut().unwrap_err(), ContractError::BadOutput);
}

#[test]
fn from_mut_unchecked_local() {
    let mut local = Mode::A;
    {
        // SAFETY: `local` is only accessed after a successful `get_mut()`
        let mut lent = unsafe { Lent::from_mut_unchecked(&mut local) };
        call(&mut lent, &[1], Some(1));
        assert_eq!(*lent.get_mut().unwrap(), Mode::B);
    }
    assert_eq!(local, Mode::B);
}

#[test]
fn from_mut_unchecked_container() {
    let mut data = Mode::A;
    let mut size = 0;
    let mut maybe_data = MaybeData::from_mut(&mut data, &mut size).unwrap();
    {
        // SAFETY: The contents of `maybe_data` are only accessed after they are parsed below
        let mut lent = unsafe { Lent::from_mut_unchecked(&mut maybe_data) };
        call(&mut lent, &[1], Some(1));
    }
    // The size reaches the container without `get_mut()`
    assert_eq!(maybe_data.size(), 1);
    let mut ptr = maybe_data.as_mut_ptr();
    let mut size = maybe_data.size();
    let capacity = maybe_data.capacity();
    // SAFETY: Pointer, size and capacity of a container view whose contents were initialized by
    // the call above, which isn't accessed otherwise while the parsed view exists
    let parsed = unsafe { MaybeData::<Mode>::try_from_mut_ptr(&mut ptr, &mut size, capacity) };
    assert_eq!(parsed.unwrap().get(), Some(&Mode::B));
}
