//! `VariableElements` and `MaybeData` reject zero-sized types in generic code in crates with
//! `generic_const_args` too, which doesn't have to propagate the bound, the check is evaluated for
//! every instance instead. Each case uses a different type since an error is only reported once
//! per type.

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_io_type::IoType;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_elements::VariableElements;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

fn elements_from_buffer<Element: TrivialType>(buffer: &[Element]) {
    let _ = VariableElements::<Element>::from_buffer(buffer, &0);
}

fn elements_from_buffer_mut<Element: TrivialType>(buffer: &mut [Element]) {
    let mut size = 0;
    let _ = VariableElements::<Element>::from_buffer_mut(buffer, &mut size);
}

fn elements_from_uninit<Element: TrivialType>(uninit: &mut [MaybeUninit<Element>]) {
    let mut size = 0;
    let _ = VariableElements::<Element>::from_uninit(uninit, &mut size);
}

fn elements_from_ptr<Element: TrivialType>(ptr: &NonNull<Element>) {
    // SAFETY: Size and capacity are zero, nothing is accessed through the pointer
    let _ = unsafe { <VariableElements<Element> as IoType>::from_ptr(ptr, &0, 0) };
}

fn elements_from_mut_ptr<Element: TrivialType>(ptr: &mut NonNull<Element>) {
    let mut size = 0;
    // SAFETY: Size and capacity are zero, nothing is accessed through the pointer
    let _ = unsafe { <VariableElements<Element> as IoType>::from_mut_ptr(ptr, &mut size, 0) };
}

// Methods don't create instances, so the check is evaluated with the layout instead
fn elements_get_initialized<Element: TrivialType>()
-> for<'a> fn(&'a VariableElements<Element>) -> &'a [Element] {
    VariableElements::<Element>::get_initialized
}

fn data_from_ref<Data: TrivialType>() {
    let _ = MaybeData::<Data>::from_ref(None);
}

fn data_from_mut<Data: TrivialType>(buffer: &mut Data) {
    let mut size = 0;
    let _ = MaybeData::<Data>::from_mut(buffer, &mut size);
}

fn data_from_uninit<Data: TrivialType>(uninit: &mut MaybeUninit<Data>) {
    let mut size = 0;
    let _ = MaybeData::<Data>::from_uninit(uninit, &mut size);
}

fn data_from_ptr<Data: TrivialType>(ptr: &NonNull<Data>) {
    // SAFETY: Size and capacity are zero, nothing is accessed through the pointer
    let _ = unsafe { <MaybeData<Data> as IoType>::from_ptr(ptr, &0, 0) };
}

fn data_from_mut_ptr<Data: TrivialType>(ptr: &mut NonNull<Data>) {
    let mut size = 0;
    // SAFETY: Size is zero, nothing is accessed through the pointer
    let _ = unsafe { <MaybeData<Data> as IoType>::from_mut_ptr(ptr, &mut size, Data::SIZE) };
}

fn data_get<Data: TrivialType>() -> for<'a> fn(&'a MaybeData<Data>) -> Option<&'a Data> {
    MaybeData::<Data>::get
}

fn main() {
    elements_from_buffer::<[u8; 0]>(&[]);
    elements_from_buffer_mut::<[u16; 0]>(&mut []);
    elements_from_uninit::<[u32; 0]>(&mut []);
    elements_from_ptr::<[u64; 0]>(&NonNull::dangling());
    elements_from_mut_ptr::<[u128; 0]>(&mut NonNull::dangling());
    elements_get_initialized::<[i8; 0]>();
    data_from_ref::<()>();
    data_from_mut::<[i16; 0]>(&mut []);
    data_from_uninit::<[i32; 0]>(&mut MaybeUninit::uninit());
    data_from_ptr::<[i64; 0]>(&NonNull::dangling());
    data_from_mut_ptr::<[i128; 0]>(&mut NonNull::dangling());
    data_get::<[(); 0]>();
}
