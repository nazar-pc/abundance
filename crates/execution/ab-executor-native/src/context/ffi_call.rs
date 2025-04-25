use crate::context::{MethodDetails, NativeExecutorContext};
use ab_contracts_common::env::{Env, EnvState, ExecutorContext};
use ab_contracts_common::metadata::decode::{
    ArgumentKind, MethodKind, MethodMetadataDecoder, MethodMetadataItem, MethodsContainerKind,
};
use ab_contracts_common::{Address, ContractError, MAX_TOTAL_METHOD_ARGS};
use ab_executor_slots::{NestedSlots, SlotIndex, SlotKey};
use ab_system_contract_address_allocator::AddressAllocator;
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::{mem, ptr, slice};
use tracing::{debug, error, warn};

/// Read a pointer of type `$ty` from `$external` and advance `$external` past it
macro_rules! read_ptr {
    ($external:ident as $ty:ty) => {{
        let ptr = NonNull::<NonNull<c_void>>::cast::<$ty>($external).read();

        $external = $external.offset(1);

        ptr
    }};
}

/// Write a `$src` pointer of type `$ty` into `$internal`, advance `$internal` past written pointer
/// and return pointer to the written location
macro_rules! write_ptr {
    ($src:expr => $internal:ident as $ty:ty) => {{
        let ptr = NonNull::<*mut c_void>::cast::<$ty>($internal);
        ptr.write($src);

        $internal = $internal.offset(1);

        ptr
    }};
}

/// Read a pointer from `$external`, write into `$internal`, advance both `$external` and
/// `$internal` by pointer size and return read pointer
macro_rules! copy_ptr {
    ($external:ident => $internal:ident as $ty:ty) => {{
        let ptr;
        {
            let src = NonNull::<NonNull<c_void>>::cast::<$ty>($external);
            let dst = NonNull::<*mut c_void>::cast::<$ty>($internal);
            ptr = src.read();
            dst.write(ptr);
        }

        $external = $external.offset(1);
        $internal = $internal.offset(1);

        ptr
    }};
}

#[derive(Copy, Clone)]
struct DelayedProcessingSlotReadOnly {
    size: u32,
}

#[derive(Copy, Clone)]
struct DelayedProcessingSlotReadWrite {
    /// Pointer to `InternalArgs` where guest will store a pointer to potentially updated slot
    /// contents
    data_ptr: NonNull<*mut u8>,
    /// Pointer to `InternalArgs` where guest will store potentially updated slot size,
    /// corresponds to `data_ptr`, filled during the second pass through the arguments
    /// (while reading `ExternalArgs`)
    size: u32,
    capacity: u32,
    slot_index: SlotIndex,
    /// Whether slot written must be non-empty.
    ///
    /// This is the case for state in `#[init]` methods.
    must_be_not_empty: bool,
}

/// Stores details about arguments that need to be processed after FFI call.
///
/// It is also more efficient to store length and capacities compactly next to each other in memory.
#[derive(Copy, Clone)]
enum DelayedProcessing {
    SlotReadOnly(DelayedProcessingSlotReadOnly),
    SlotReadWrite(DelayedProcessingSlotReadWrite),
}

struct DelayedProcessingCollection<'a> {
    buffer: &'a [UnsafeCell<MaybeUninit<DelayedProcessing>>; MAX_TOTAL_METHOD_ARGS as usize],
    cursor: usize,
}

impl<'a> DelayedProcessingCollection<'a> {
    #[inline(always)]
    fn from_buffer(
        buffer: &'a [UnsafeCell<MaybeUninit<DelayedProcessing>>; MAX_TOTAL_METHOD_ARGS as usize],
    ) -> Self {
        Self { buffer, cursor: 0 }
    }

    /// Insert new entry and get a shared reference to it, which doesn't inherit stack borrows tag.
    ///
    /// # Safety
    /// Must not insert more entries than [`MAX_TOTAL_METHODS_ARGS`].
    #[inline(always)]
    unsafe fn insert_ro(
        &mut self,
        entry: DelayedProcessingSlotReadOnly,
    ) -> &DelayedProcessingSlotReadOnly {
        // SAFETY: Method contract is that the entry must exist, cursor is always moving forward and
        // doesn't reference the same entry more than once here
        let inserted = unsafe {
            self.buffer
                .get_unchecked(self.cursor)
                .get()
                .as_mut_unchecked()
                .write(DelayedProcessing::SlotReadOnly(entry))
        };
        self.cursor += 1;
        let DelayedProcessing::SlotReadOnly(entry) = inserted else {
            unreachable!("Just inserted `DelayedProcessing::SlotReadOnly` entry; qed");
        };
        entry
    }

    /// Insert new entry and get a mutable reference to it, which doesn't inherit stack borrows tag.
    ///
    /// # Safety
    /// Must not insert more entries than [`MAX_TOTAL_METHODS_ARGS`].
    #[inline(always)]
    unsafe fn insert_rw(
        &mut self,
        entry: DelayedProcessingSlotReadWrite,
    ) -> &mut DelayedProcessingSlotReadWrite {
        // SAFETY: Method contract is that the entry must exist, cursor is always moving forward and
        // doesn't reference the same entry more than once here
        let inserted = unsafe {
            self.buffer
                .get_unchecked(self.cursor)
                .get()
                .as_mut_unchecked()
                .write(DelayedProcessing::SlotReadWrite(entry))
        };
        self.cursor += 1;
        let DelayedProcessing::SlotReadWrite(entry) = inserted else {
            unreachable!("Just inserted `DelayedProcessing::SlotReadWrite` entry; qed");
        };
        entry
    }

    fn iter(&self) -> impl Iterator<Item = &DelayedProcessing> {
        self.buffer.iter().take(self.cursor).map(|entry| {
            // SAFETY: All values were initialized
            unsafe { entry.as_ref_unchecked().assume_init_ref() }
        })
    }
}

/// Special container that allows aliasing of `Env` stored inside it and holds onto slots
enum MaybeEnv<Env, Slots> {
    None(Slots),
    ReadOnly(*mut UnsafeCell<Env>),
    ReadWrite(*mut UnsafeCell<Env>),
}

impl<Env, Slots> Drop for MaybeEnv<Env, Slots> {
    #[inline(always)]
    fn drop(&mut self) {
        match self {
            MaybeEnv::None(_) => {}
            &mut MaybeEnv::ReadOnly(env) | &mut MaybeEnv::ReadWrite(env) => {
                // SAFETY: As `self` is being dropped, we can safely assume any aliasing has ended
                // and drop the original `Box`
                let _ = unsafe { Box::from_raw(env) };
            }
        }
    }
}

impl<'env> MaybeEnv<MaybeUninit<Env<'env>>, ()> {
    /// Insert a new value and get a pointer to it, value must be initialized later with
    /// [`Self::initialize()`]
    #[inline(always)]
    fn insert_ro(&mut self) -> *const MaybeUninit<Env<'env>> {
        let env = Box::into_raw(Box::new(UnsafeCell::new(MaybeUninit::uninit())));
        let env_ptr = {
            // SAFETY: Just initialized, no other references to the value
            let env_ref = unsafe { env.as_ref_unchecked() };
            env_ref.get().cast_const()
        };
        *self = Self::ReadOnly(env);
        env_ptr
    }

    /// Insert a new value and get a pointer to it, value must be initialized later with
    /// [`Self::initialize()`]
    #[inline(always)]
    fn insert_rw(&mut self) -> *mut MaybeUninit<Env<'env>> {
        let env = Box::into_raw(Box::new(UnsafeCell::new(MaybeUninit::uninit())));
        let env_ptr = {
            // SAFETY: Just initialized, no other references to the value
            let env_ref = unsafe { env.as_ref_unchecked() };
            env_ref.get()
        };
        *self = Self::ReadWrite(env);
        env_ptr
    }

    /// # Safety
    /// Nothing must have a live reference to `self` or its internals
    #[inline(always)]
    unsafe fn initialize<'slots, CreateNestedContext>(
        self,
        slots: NestedSlots<'slots>,
        env_state: EnvState,
        create_nested_context: CreateNestedContext,
    ) -> MaybeEnv<Env<'env>, NestedSlots<'env>>
    where
        CreateNestedContext:
            FnOnce(NestedSlots<'slots>, bool) -> &'env mut NativeExecutorContext<'slots>,
        'slots: 'env,
    {
        match self {
            Self::None(()) => MaybeEnv::None(slots),
            Self::ReadOnly(env_ro) => {
                let env =
                    Env::with_executor_context(env_state, create_nested_context(slots, false));
                {
                    // SAFETY: Nothing is accessing `env_ro` right now as per function signature,
                    // and it is guaranteed to be initialized with `Self::insert_ro()` above
                    let env_ro = unsafe { env_ro.as_mut_unchecked() };
                    env_ro.get_mut().write(env);
                }
                // Very explicit cast to the initialized value since it was just written to
                let env_ro = env_ro.cast::<UnsafeCell<Env<'env>>>();

                // Prevent destructor from running and de-allocating `Env`
                mem::forget(self);

                MaybeEnv::ReadOnly(env_ro)
            }
            Self::ReadWrite(env_rw) => {
                let env = Env::with_executor_context(env_state, create_nested_context(slots, true));
                {
                    // SAFETY: Nothing is accessing `env_rw` right now as per function signature,
                    // and it is guaranteed to be initialized with `Self::insert_rw()` above
                    let env_rw = unsafe { env_rw.as_mut_unchecked() };
                    env_rw.get_mut().write(env);
                }
                // Very explicit cast to the initialized value since it was just written to
                let env_rw = env_rw.cast::<UnsafeCell<Env<'env>>>();

                // Prevent destructor from running and de-allocating `Env`
                mem::forget(self);

                MaybeEnv::ReadWrite(env_rw)
            }
        }
    }
}

impl<'env> MaybeEnv<Env<'env>, NestedSlots<'env>> {
    /// # Safety
    /// Nothing must have a live reference to `self` or its internals
    #[inline(always)]
    unsafe fn get_slots_mut<'tmp>(&'tmp mut self) -> &'tmp mut NestedSlots<'env>
    where
        'env: 'tmp,
    {
        let env = match self {
            MaybeEnv::None(slots) => {
                return slots;
            }
            MaybeEnv::ReadOnly(env) | MaybeEnv::ReadWrite(env) => env,
        };
        // SAFETY: Nothing is accessing `env` right now as per function signature
        let env = unsafe { env.as_mut_unchecked() };
        let env = env.get_mut();
        // SAFETY: this is the correct original type and nothing else is referencing it right now
        let context = unsafe {
            &mut *ptr::from_mut::<dyn ExecutorContext + 'tmp>(env.get_mut_executor_context())
                .cast::<NativeExecutorContext<'env>>()
        };
        context.slots.get_mut()
    }
}

#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
pub(super) fn make_ffi_call<'slots, 'external_args, CreateNestedContext>(
    allow_env_mutation: bool,
    is_allocate_new_address_method: bool,
    parent_slots: &'slots mut NestedSlots<'slots>,
    contract: Address,
    method_details: MethodDetails,
    external_args: &'external_args mut NonNull<NonNull<c_void>>,
    env_state: EnvState,
    create_nested_context: CreateNestedContext,
) -> Result<(), ContractError>
where
    CreateNestedContext: FnOnce(NestedSlots<'slots>, bool) -> NativeExecutorContext<'slots>,
{
    // Allocate a buffer that will contain incrementally built `InternalArgs` that method expects,
    // according to its metadata.
    // `* 4` is due the worst case being to have a slot with 4 pointers: address + data + size +
    // capacity.
    let mut internal_args =
        UnsafeCell::new([MaybeUninit::<*mut c_void>::uninit(); MAX_TOTAL_METHOD_ARGS as usize * 4]);
    // Delayed processing of sizes as capacities since knowing them requires processing all
    // arguments first.
    //
    // NOTE: It is important that this is never reallocated as it will invalidate all pointers to
    // elements of this array!
    let delayed_processing_buffer = [
        UnsafeCell::new(MaybeUninit::uninit()),
        UnsafeCell::new(MaybeUninit::uninit()),
        UnsafeCell::new(MaybeUninit::uninit()),
        UnsafeCell::new(MaybeUninit::uninit()),
        UnsafeCell::new(MaybeUninit::uninit()),
        UnsafeCell::new(MaybeUninit::uninit()),
        UnsafeCell::new(MaybeUninit::uninit()),
        UnsafeCell::new(MaybeUninit::uninit()),
    ];

    // Having large-ish `internal_args` and delayed_processing_buffer` in a separate stack frame
    // results in a better data layout for performance
    make_ffi_call_internal(
        allow_env_mutation,
        is_allocate_new_address_method,
        parent_slots,
        contract,
        method_details,
        external_args,
        env_state,
        create_nested_context,
        &delayed_processing_buffer,
        &mut internal_args,
    )
}

#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
fn make_ffi_call_internal<'slots, 'external_args, CreateNestedContext>(
    allow_env_mutation: bool,
    is_allocate_new_address_method: bool,
    parent_slots: &'slots mut NestedSlots<'slots>,
    contract: Address,
    method_details: MethodDetails,
    external_args: &'external_args mut NonNull<NonNull<c_void>>,
    env_state: EnvState,
    create_nested_context: CreateNestedContext,
    delayed_processing_buffer: &[UnsafeCell<MaybeUninit<DelayedProcessing>>;
         MAX_TOTAL_METHOD_ARGS as usize],
    internal_args: &mut UnsafeCell<[MaybeUninit<*mut c_void>; MAX_TOTAL_METHOD_ARGS as usize * 4]>,
) -> Result<(), ContractError>
where
    CreateNestedContext: FnOnce(NestedSlots<'slots>, bool) -> NativeExecutorContext<'slots>,
{
    let MethodDetails {
        recommended_state_capacity,
        recommended_slot_capacity,
        recommended_tmp_capacity,
        mut method_metadata,
        ffi_fn,
    } = method_details;

    let method_metadata_decoder =
        MethodMetadataDecoder::new(&mut method_metadata, MethodsContainerKind::Unknown);
    let (mut arguments_metadata_decoder, method_metadata_item) =
        match method_metadata_decoder.decode_next() {
            Ok(result) => result,
            Err(error) => {
                error!(%error, "Method metadata decoding error");
                return Err(ContractError::InternalError);
            }
        };
    let MethodMetadataItem {
        method_kind,
        num_arguments,
        ..
    } = method_metadata_item;

    let number_of_arguments =
        usize::from(num_arguments) + if method_kind.has_self() { 1 } else { 0 };

    if number_of_arguments > usize::from(MAX_TOTAL_METHOD_ARGS) {
        debug!(%number_of_arguments, "Too many arguments");
        return Err(ContractError::BadInput);
    }

    let internal_args = NonNull::new(internal_args.get().cast::<MaybeUninit<*mut c_void>>())
        .expect("Taken from non-null instance; qed");
    // This pointer will be moving as the data structure is being constructed, while `internal_args`
    // will keep pointing to the beginning
    let mut internal_args_cursor = internal_args.cast::<*mut c_void>();
    // This pointer will be moving as the data structure is being read, while `external_args` will
    // keep pointing to the beginning
    let mut external_args_cursor = *external_args;
    let mut delayed_processing =
        DelayedProcessingCollection::from_buffer(delayed_processing_buffer);

    // `view_only == true` when only `#[view]` method is allowed
    let (view_only, mut slots) = match method_kind {
        MethodKind::Init
        | MethodKind::UpdateStateless
        | MethodKind::UpdateStatefulRo
        | MethodKind::UpdateStatefulRw => {
            if !allow_env_mutation {
                warn!(allow_env_mutation, "Only `#[view]` methods are allowed");
                return Err(ContractError::Forbidden);
            }

            let Some(slots) = parent_slots.new_nested_rw() else {
                error!("Unexpected creation of non-read-only slots from read-only slots");
                return Err(ContractError::InternalError);
            };

            (false, slots)
        }
        MethodKind::ViewStateless | MethodKind::ViewStateful => {
            let slots = parent_slots.new_nested_ro();
            (true, slots)
        }
    };

    let mut maybe_env = MaybeEnv::None(());

    // Handle `&self` and `&mut self`
    match method_kind {
        MethodKind::Init | MethodKind::UpdateStateless | MethodKind::ViewStateless => {
            // No state handling is needed
        }
        MethodKind::UpdateStatefulRo | MethodKind::ViewStateful => {
            let state_bytes = slots
                .use_ro(SlotKey {
                    owner: contract,
                    contract: Address::SYSTEM_STATE,
                })
                .ok_or(ContractError::Forbidden)?;

            if state_bytes.is_empty() {
                warn!("Contract does not have state yet, can't call stateful method before init");
                return Err(ContractError::Forbidden);
            }

            // SAFETY: Number of arguments checked above
            let result = unsafe {
                delayed_processing.insert_ro(DelayedProcessingSlotReadOnly {
                    size: state_bytes.len(),
                })
            };

            // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above and
            // aligned correctly
            unsafe {
                write_ptr!(state_bytes.as_ptr() => internal_args_cursor as *const u8);
                write_ptr!(&result.size => internal_args_cursor as *const u32);
            }
        }
        MethodKind::UpdateStatefulRw => {
            if view_only {
                warn!("Only `#[view]` methods are allowed");
                return Err(ContractError::Forbidden);
            }

            let slot_key = SlotKey {
                owner: contract,
                contract: Address::SYSTEM_STATE,
            };
            let (slot_index, state_bytes) = slots
                .use_rw(slot_key, recommended_state_capacity)
                .ok_or(ContractError::Forbidden)?;

            if state_bytes.is_empty() {
                warn!("Contract does not have state yet, can't call stateful method before init");
                return Err(ContractError::Forbidden);
            }

            let entry = DelayedProcessingSlotReadWrite {
                // Is updated below
                data_ptr: NonNull::dangling(),
                size: state_bytes.len(),
                capacity: state_bytes.capacity(),
                slot_index,
                must_be_not_empty: false,
            };
            // SAFETY: Number of arguments checked above
            let result = unsafe { delayed_processing.insert_rw(entry) };

            // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above and
            // aligned correctly
            unsafe {
                result.data_ptr =
                    write_ptr!(state_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                write_ptr!(&mut result.size => internal_args_cursor as *mut u32);
                write_ptr!(&result.capacity => internal_args_cursor as *const u32);
            }
        }
    }

    let mut new_address_ptr = None;

    // Handle all other arguments one by one
    for argument_index in 0..num_arguments {
        let argument_kind = match arguments_metadata_decoder.decode_next() {
            Some(Ok(item)) => item.argument_kind,
            Some(Err(error)) => {
                error!(%error, "Argument metadata decoding error");
                return Err(ContractError::InternalError);
            }
            None => {
                error!("Argument not found, invalid metadata");
                return Err(ContractError::InternalError);
            }
        };

        match argument_kind {
            ArgumentKind::EnvRo => {
                // Allocate and create a pointer now, the actual value will be inserted towards the
                // end of the function
                let env_ro = maybe_env.insert_ro().cast::<Env<'_>>();
                // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above
                // and aligned correctly
                unsafe {
                    write_ptr!(env_ro => internal_args_cursor as *const Env<'_>);
                }

                // Size for `#[env]` is implicit and doesn't need to be added to `InternalArgs`
            }
            ArgumentKind::EnvRw => {
                if view_only {
                    return Err(ContractError::Forbidden);
                }

                // Allocate and create a pointer now, the actual value will be inserted towards the
                // end of the function
                let env_rw = maybe_env.insert_rw().cast::<Env<'_>>();

                // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above
                // and aligned correctly
                unsafe {
                    write_ptr!(env_rw => internal_args_cursor as *mut Env<'_>);
                }

                // Size for `#[env]` is implicit and doesn't need to be added to `InternalArgs`
            }
            ArgumentKind::TmpRo | ArgumentKind::SlotRo => {
                let tmp = matches!(argument_kind, ArgumentKind::TmpRo);

                let (owner, contract) = if tmp {
                    if view_only {
                        return Err(ContractError::Forbidden);
                    }

                    // Null contact is used implicitly for `#[tmp]` since it is not possible for
                    // this contract to write something there directly
                    (&contract, Address::NULL)
                } else {
                    // SAFETY: `external_args_cursor`'s must contain a valid pointer to address,
                    // moving right past that is safe
                    (
                        unsafe { &*read_ptr!(external_args_cursor as *const Address) },
                        contract,
                    )
                };

                let slot_key = SlotKey {
                    owner: *owner,
                    contract,
                };
                let slot_bytes = slots.use_ro(slot_key).ok_or(ContractError::Forbidden)?;

                // SAFETY: Number of arguments checked above
                let result = unsafe {
                    delayed_processing.insert_ro(DelayedProcessingSlotReadOnly {
                        size: slot_bytes.len(),
                    })
                };

                // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above
                // and aligned correctly
                unsafe {
                    if !tmp {
                        write_ptr!(owner => internal_args_cursor as *const Address);
                    }
                    write_ptr!(slot_bytes.as_ptr() => internal_args_cursor as *const u8);
                    write_ptr!(&result.size => internal_args_cursor as *const u32);
                }
            }
            ArgumentKind::TmpRw | ArgumentKind::SlotRw => {
                if view_only {
                    return Err(ContractError::Forbidden);
                }

                let tmp = matches!(argument_kind, ArgumentKind::TmpRw);

                let (owner, contract, capacity) = if tmp {
                    // Null contact is used implicitly for `#[tmp]` since it is not possible for
                    // this contract to write something there directly
                    (&contract, Address::NULL, recommended_tmp_capacity)
                } else {
                    // SAFETY: `external_args_cursor`'s must contain a valid pointer to address,
                    // moving right past that is safe
                    let address = unsafe { &*read_ptr!(external_args_cursor as *const Address) };

                    (address, contract, recommended_slot_capacity)
                };

                let slot_key = SlotKey {
                    owner: *owner,
                    contract,
                };
                let (slot_index, slot_bytes) = slots
                    .use_rw(slot_key, capacity)
                    .ok_or(ContractError::Forbidden)?;

                let entry = DelayedProcessingSlotReadWrite {
                    // Is updated below
                    data_ptr: NonNull::dangling(),
                    size: slot_bytes.len(),
                    capacity: slot_bytes.capacity(),
                    slot_index,
                    must_be_not_empty: false,
                };
                // SAFETY: Number of arguments checked above
                let result = unsafe { delayed_processing.insert_rw(entry) };

                // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above
                // and aligned correctly
                unsafe {
                    if !tmp {
                        write_ptr!(owner => internal_args_cursor as *const Address);
                    }
                    result.data_ptr =
                        write_ptr!(slot_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                    write_ptr!(&mut result.size => internal_args_cursor as *mut u32);
                    write_ptr!(&result.capacity => internal_args_cursor as *const u32);
                }
            }
            ArgumentKind::Input => {
                // SAFETY: `external_args_cursor`'s must contain a pointers to input + size.
                // `internal_args_cursor`'s memory is allocated with sufficient size above and
                // aligned correctly.
                unsafe {
                    // Input
                    copy_ptr!(external_args_cursor => internal_args_cursor as *const u8);
                    // Size
                    copy_ptr!(external_args_cursor => internal_args_cursor as *const u32);
                }
            }
            ArgumentKind::Output => {
                let last_argument = argument_index == num_arguments - 1;
                // `#[init]` method returns state of the contract and needs to be stored accordingly
                if matches!((method_kind, last_argument), (MethodKind::Init, true)) {
                    if view_only {
                        return Err(ContractError::Forbidden);
                    }

                    let slot_key = SlotKey {
                        owner: contract,
                        contract: Address::SYSTEM_STATE,
                    };
                    let (slot_index, state_bytes) = slots
                        .use_rw(slot_key, recommended_state_capacity)
                        .ok_or(ContractError::Forbidden)?;

                    if !state_bytes.is_empty() {
                        debug!("Can't initialize already initialized contract");
                        return Err(ContractError::Forbidden);
                    }

                    let entry = DelayedProcessingSlotReadWrite {
                        // Is updated below
                        data_ptr: NonNull::dangling(),
                        size: 0,
                        capacity: state_bytes.capacity(),
                        slot_index,
                        must_be_not_empty: true,
                    };
                    // SAFETY: Number of arguments checked above
                    let result = unsafe { delayed_processing.insert_rw(entry) };

                    // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                    // above and aligned correctly
                    unsafe {
                        result.data_ptr =
                            write_ptr!(state_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                        write_ptr!(&mut result.size => internal_args_cursor as *mut u32);
                        write_ptr!(&result.capacity => internal_args_cursor as *const u32);
                    }
                } else {
                    // SAFETY: `external_args_cursor`'s must contain a pointers to input + size
                    // + capacity.
                    // `internal_args_cursor`'s memory is allocated with sufficient size above and
                    // aligned correctly.
                    unsafe {
                        // Output
                        if last_argument && is_allocate_new_address_method {
                            let ptr = copy_ptr!(external_args_cursor => internal_args_cursor as *mut Address);
                            new_address_ptr.replace(ptr);
                        } else {
                            copy_ptr!(external_args_cursor => internal_args_cursor as *mut u8);
                        }
                        // Size (might be a null pointer for trivial types)
                        let size_ptr =
                            copy_ptr!(external_args_cursor => internal_args_cursor as *mut u32);
                        if !size_ptr.is_null() {
                            // Override output size to be zero even if caller guest tried to put
                            // something there
                            size_ptr.write(0);
                        }
                        // Capacity
                        copy_ptr!(external_args_cursor => internal_args_cursor as *const u32);
                    }
                }
            }
        }
    }

    let mut nested_context = None;
    // SAFETY: No live references to `maybe_env`
    let mut maybe_env = unsafe {
        maybe_env.initialize(slots, env_state, |slots, allow_env_mutation| {
            nested_context.insert(create_nested_context(slots, allow_env_mutation))
        })
    };

    // Will only read initialized number of pointers, hence `NonNull<c_void>` even though there is
    // likely slack capacity with uninitialized data
    let internal_args = internal_args.cast::<NonNull<c_void>>();

    // SAFETY: FFI function was generated at the same time as corresponding `Args` and must match
    // ABI of the fingerprint or else it wouldn't compile
    let result = Result::<(), ContractError>::from(unsafe { ffi_fn(internal_args) });

    // SAFETY: No live references to `maybe_env`
    let slots = unsafe { maybe_env.get_slots_mut() };

    if let Err(error) = result {
        slots.reset();

        return Err(error);
    }

    // Catch new address allocation and add it to new contracts in slots for code and other things
    // to become usable for it
    if let Some(new_address_ptr) = new_address_ptr {
        // Assert that the API has expected shape
        let _: fn(&mut AddressAllocator, &mut Env<'_>) -> Result<Address, ContractError> =
            AddressAllocator::allocate_address;
        // SAFETY: Method call to address allocator succeeded, so it must have returned an address
        let new_address = unsafe { new_address_ptr.read() };
        if !slots.add_new_contract(new_address) {
            warn!("Failed to add new contract returned by address allocator");
            return Err(ContractError::InternalError);
        }
    }

    for &entry in delayed_processing.iter() {
        match entry {
            DelayedProcessing::SlotReadOnly { .. } => {
                // No processing is necessary
            }
            DelayedProcessing::SlotReadWrite(DelayedProcessingSlotReadWrite {
                data_ptr,
                size,
                slot_index,
                must_be_not_empty,
                ..
            }) => {
                if must_be_not_empty && size == 0 {
                    error!(
                        %size,
                        "Contract returned empty size where it is not allowed, likely state of \
                        `#[init]` method"
                    );
                    return Err(ContractError::BadOutput);
                }

                // SAFETY: Correct pointer created earlier that is not used for anything else at the
                // moment
                let data_ptr = unsafe { data_ptr.as_ptr().read().cast_const() };
                let slot_bytes = slots.access_used_rw(slot_index).expect(
                    "Was used in `make_ffi_call` and must exist if `Slots` was not dropped \
                    yet; qed",
                );

                // Guest created a different allocation for slot, copy bytes
                if !ptr::eq(data_ptr, slot_bytes.as_ptr()) {
                    if data_ptr.is_null() {
                        error!("Contract returned `null` pointer for slot data");
                        return Err(ContractError::BadOutput);
                    }
                    // SAFETY: For native execution guest behavior is assumed to be trusted and
                    // provide a correct pointer and size
                    let data = unsafe { slice::from_raw_parts(data_ptr, size as usize) };
                    slot_bytes.copy_from_slice(data);
                    continue;
                }

                if size > slot_bytes.capacity() {
                    error!(
                        %size,
                        capacity = %slot_bytes.capacity(),
                        "Contract returned invalid size for slot data in source allocation"
                    );
                    return Err(ContractError::BadOutput);
                }
                // Otherwise, set the size to what guest claims
                //
                // SAFETY: For native execution guest behavior is assumed to be trusted and provide
                // the correct size
                unsafe {
                    slot_bytes.set_len(size);
                }
            }
        }
    }

    Ok(())
}
