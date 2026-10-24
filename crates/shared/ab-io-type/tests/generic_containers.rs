//! Methods of `MaybeData` and `VariableElements` called from generic code of a crate that doesn't
//! enable the nightly features of `ab-io-type`, with only `IoType` bounds on the containers

use ab_io_type::IoType;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_elements::VariableElements;
use core::mem::MaybeUninit;

fn maybe_data_replace<Data>(uninit: &mut MaybeUninit<Data>, data: Data) -> Data
where
    Data: TrivialType,
    for<'a> MaybeData<'a, Data>: IoType,
{
    let mut size = 0;
    let mut maybe_data = MaybeData::from_uninit(uninit, &mut size).expect("Size is zero; qed");
    assert!(maybe_data.get().is_none());
    maybe_data.replace(data);
    *maybe_data.get().expect("Just replaced; qed")
}

fn maybe_data_get_mut<Data>(data: &mut Data) -> bool
where
    Data: TrivialType,
    for<'a> MaybeData<'a, Data>: IoType,
{
    let mut size = Data::SIZE;
    let mut maybe_data = MaybeData::from_mut(data, &mut size).expect("Size of data; qed");
    maybe_data.get_mut().is_some()
}

fn elements_append<Element>(uninit: &mut [MaybeUninit<Element>], elements: &[Element]) -> usize
where
    Element: TrivialType,
    for<'a> VariableElements<'a, Element>: IoType,
{
    let mut size = 0;
    let mut variable_elements =
        VariableElements::<Element>::from_uninit(uninit, &mut size).expect("Size is zero; qed");
    assert!(variable_elements.get_initialized().is_empty());
    assert!(variable_elements.append(elements));
    variable_elements.get_initialized().len()
}

fn elements_from_buffer<Element>(buffer: &[Element]) -> u32
where
    Element: TrivialType,
    for<'a> VariableElements<'a, Element>: IoType,
{
    let size = u32::try_from(size_of_val(buffer)).expect("Small buffer in tests; qed");
    VariableElements::<Element>::from_buffer(buffer, &size)
        .expect("Size of the buffer; qed")
        .count()
}

#[test]
fn maybe_data() {
    assert_eq!(maybe_data_replace(&mut MaybeUninit::<u64>::uninit(), 7), 7);
    assert!(maybe_data_get_mut(&mut 5u16));
}

#[test]
fn variable_elements() {
    assert_eq!(
        elements_append(&mut [MaybeUninit::<u16>::uninit(); 3], &[1, 2]),
        2
    );
    assert_eq!(elements_from_buffer(&[1u32, 2, 3]), 3);
}
