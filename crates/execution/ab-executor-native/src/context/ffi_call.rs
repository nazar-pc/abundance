use crate::context::{MethodDetails, NativeExecutorContext};
use ab_contracts_common::env::{Env, EnvState, ExecutorContext};
use ab_contracts_common::metadata::decode::{
    ArgumentKind, MethodKind, MethodMetadataDecoder, MethodMetadataItem, MethodsContainerKind,
};
use ab_contracts_common::{ContractError, MAX_TOTAL_METHOD_ARGS};
use ab_core_primitives::address::Address;
use ab_executor_slots::{NestedSlots, SlotIndex, SlotKey};
use ab_system_contract_address_allocator::AddressAllocator;
use arrayvec::ArrayVec;
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::{mem, ptr, slice};
use tracing::{debug, error, warn};

// The worst case is to have a slot with two pointers and two 32-bit size fields:
// address + data + size + capacity
const INTERNAL_ARGS_SIZE: usize =
    usize::from(MAX_TOTAL_METHOD_ARGS) * (size_of::<*mut c_void>() * 2 + size_of::<u32>() * 2);

#[derive(Copy, Clone)]
#[repr(C)]
struct FfiDataSizeCapacityRo {
    data_ptr: NonNull<u8>,
    size: u32,
    capacity: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct FfiDataSizeCapacityRw {
    data_ptr: *mut u8,
    size: u32,
    capacity: u32,
}

/// Read from an external arguments pointer and move it forward.
///
/// # Safety
/// `external_args` must have enough capacity for the read value, and the current offset must
/// have the correct alignment for the type being read.
#[inline(always)]
unsafe fn read_external_args<T>(external_args: &mut NonNull<c_void>) -> T {
    // SAFETY: guaranteed by this function signature
    unsafe {
        let value = external_args.cast::<T>().read();
        *external_args = external_args.byte_add(size_of::<T>());
        value
    }
}

/// Write to an internal arguments pointer and move it forward.
///
/// # Safety
/// `internal_args` must have enough capacity for the written value, and the current offset must
/// have the correct alignment for the type being written.
#[inline(always)]
unsafe fn write_internal_args<T>(internal_args: &mut NonNull<c_void>, value: T) {
    // SAFETY: guaranteed by this function signature
    unsafe {
        internal_args.cast::<T>().write(value);
        *internal_args = internal_args.byte_add(size_of::<T>());
    }
}

/// Stores details about arguments that need to be processed after FFI call
#[derive(Copy, Clone)]
enum PostProcessing {
    Slot {
        /// Offset into `internal_args` where the corresponding slot entry is located.
        ///
        /// NOTE: An assertion above ensures `u8` is large enough to store all possible offsets.
        internal_args_ptr: NonNull<c_void>,
        slot_index: SlotIndex,
        /// Whether a slot written must be non-empty.
        ///
        /// This is the case for state in `#[init]` methods.
        must_be_not_empty: bool,
    },
    Output {
        /// Offset into `InternalArgs` where the guest will store a pointer to potentially updated
        /// output contents.
        ///
        /// NOTE: An assertion above ensures `u8` is large enough to store all possible offsets.
        internal_args_ptr: NonNull<c_void>,
        /// Offset into `ExternalArgs` where the host will potentially update output contents.
        ///
        /// This is strictly smaller than `internal_args_ptr`, hence the same type.
        external_args_ptr: NonNull<c_void>,
    },
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
        // SAFETY: this is the correct original type, and nothing else is referencing it right now
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
    external_args: &'external_args mut NonNull<c_void>,
    env_state: EnvState,
    create_nested_context: CreateNestedContext,
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

    // Allocate a buffer that will contain incrementally built `InternalArgs` that the method
    // expects, according to its metadata
    let mut internal_args =
        MaybeUninit::<[*mut c_void; INTERNAL_ARGS_SIZE / size_of::<*const c_void>()]>::uninit();
    let mut post_processing = ArrayVec::<_, { usize::from(MAX_TOTAL_METHOD_ARGS) }>::new_const();

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

    let internal_args = NonNull::new(internal_args.as_mut_ptr().cast::<c_void>())
        .expect("Taken from non-null instance; qed");
    // This pointer will be moving as the data structure is being constructed, while `internal_args`
    // will keep pointing to the beginning
    let internal_args_cursor = &mut internal_args.clone();
    // This pointer will be moving as the data structure is being read, while `external_args` will
    // keep pointing to the beginning
    let external_args_cursor = &mut external_args.clone();

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

            // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient size above
            // and aligned correctly
            unsafe {
                write_internal_args(
                    internal_args_cursor,
                    FfiDataSizeCapacityRo {
                        data_ptr: NonNull::from_ref(state_bytes.as_slice()).as_non_null_ptr(),
                        size: state_bytes.len(),
                        capacity: state_bytes.len(),
                    },
                );
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

            post_processing.push(PostProcessing::Slot {
                internal_args_ptr: *internal_args_cursor,
                slot_index,
                must_be_not_empty: false,
            });

            // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient size above
            // and aligned correctly
            unsafe {
                write_internal_args(
                    internal_args_cursor,
                    FfiDataSizeCapacityRw {
                        data_ptr: state_bytes.as_mut_ptr(),
                        size: state_bytes.len(),
                        capacity: state_bytes.capacity(),
                    },
                );
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
                // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient size
                // above and aligned correctly
                unsafe {
                    write_internal_args(internal_args_cursor, env_ro);
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

                // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient size
                // above and aligned correctly
                unsafe {
                    write_internal_args(internal_args_cursor, env_rw);
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
                        unsafe { &*read_external_args::<*const Address>(external_args_cursor) },
                        contract,
                    )
                };

                let slot_key = SlotKey {
                    owner: *owner,
                    contract,
                };
                let slot_bytes = slots.use_ro(slot_key).ok_or(ContractError::Forbidden)?;

                // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient size
                // above and aligned correctly
                unsafe {
                    if !tmp {
                        write_internal_args(internal_args_cursor, owner);
                    }
                    write_internal_args(
                        internal_args_cursor,
                        FfiDataSizeCapacityRo {
                            data_ptr: NonNull::from_ref(slot_bytes.as_slice()).as_non_null_ptr(),
                            size: slot_bytes.len(),
                            capacity: slot_bytes.len(),
                        },
                    );
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
                    let address =
                        unsafe { &*read_external_args::<*const Address>(external_args_cursor) };

                    (address, contract, recommended_slot_capacity)
                };

                let slot_key = SlotKey {
                    owner: *owner,
                    contract,
                };
                let (slot_index, slot_bytes) = slots
                    .use_rw(slot_key, capacity)
                    .ok_or(ContractError::Forbidden)?;

                if !tmp {
                    // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient size
                    // above and aligned correctly
                    unsafe {
                        write_internal_args(internal_args_cursor, owner);
                    }
                }

                post_processing.push(PostProcessing::Slot {
                    internal_args_ptr: *internal_args_cursor,
                    slot_index,
                    must_be_not_empty: false,
                });

                // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient size
                // above and aligned correctly
                unsafe {
                    write_internal_args(
                        internal_args_cursor,
                        FfiDataSizeCapacityRw {
                            data_ptr: slot_bytes.as_mut_ptr(),
                            size: slot_bytes.len(),
                            capacity: slot_bytes.capacity(),
                        },
                    );
                }
            }
            ArgumentKind::Input => {
                // SAFETY: `external_args_cursor` must point to an input pointer + size + capacity.
                // `internal_args_cursor`'s memory is allocated with a sufficient size above and
                // aligned correctly.
                unsafe {
                    let data_size_capacity =
                        read_external_args::<FfiDataSizeCapacityRw>(external_args_cursor);
                    write_internal_args(internal_args_cursor, data_size_capacity);
                }
            }
            ArgumentKind::Output | ArgumentKind::Return => {
                let last_argument = argument_index == num_arguments - 1;
                // `#[init]` method returns the state of the contract and needs to be stored
                // accordingly
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

                    if matches!(argument_kind, ArgumentKind::Return) {
                        // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient
                        // size above and aligned correctly
                        unsafe {
                            // The return type is `TrivialType` and doesn't have size/capacity
                            write_internal_args(internal_args_cursor, state_bytes.as_mut_ptr());
                        }
                        // SAFETY: While the data is uninitialized, it will not be read except
                        // through the above pointer until and unless the method returns
                        // successfully, in which case the data will in fact be initialized.
                        // It is more efficient to just set the length here right away than do
                        // explicit post-processing below.
                        unsafe {
                            state_bytes.set_len(state_bytes.capacity());
                        }
                    } else {
                        post_processing.push(PostProcessing::Slot {
                            internal_args_ptr: *internal_args_cursor,
                            slot_index,
                            must_be_not_empty: true,
                        });

                        // SAFETY: `internal_args_cursor`'s memory is allocated with a sufficient
                        // size above and aligned correctly
                        unsafe {
                            write_internal_args(
                                internal_args_cursor,
                                FfiDataSizeCapacityRw {
                                    data_ptr: state_bytes.as_mut_ptr(),
                                    size: 0,
                                    capacity: state_bytes.capacity(),
                                },
                            );
                        }
                    }
                } else {
                    if last_argument && is_allocate_new_address_method {
                        // SAFETY: `external_args_cursor`'s must contain a single pointer for new
                        // address allocation.
                        // `internal_args_cursor`'s memory is allocated with a sufficient size above
                        // and aligned correctly.
                        unsafe {
                            let address = read_external_args::<*mut Address>(external_args_cursor);
                            write_internal_args(internal_args_cursor, address);
                            new_address_ptr.replace(address);
                        }
                    } else if matches!(argument_kind, ArgumentKind::Return) {
                        // SAFETY: `external_args_cursor`'s must contain a single pointer for return
                        // value.
                        // `internal_args_cursor`'s memory is allocated with a sufficient size above
                        // and aligned correctly.
                        unsafe {
                            // The return type is `TrivialType` and doesn't have size/capacity
                            let data = read_external_args::<*mut u8>(external_args_cursor);
                            write_internal_args(internal_args_cursor, data);
                        }
                    } else {
                        post_processing.push(PostProcessing::Output {
                            internal_args_ptr: *internal_args_cursor,
                            external_args_ptr: *external_args_cursor,
                        });

                        // SAFETY: `external_args_cursor`'s must contain an output pointer + size
                        // + capacity.
                        // `internal_args_cursor`'s memory is allocated with a sufficient size above
                        // and aligned correctly.
                        unsafe {
                            let data_size_capacity =
                                read_external_args::<FfiDataSizeCapacityRw>(external_args_cursor);
                            write_internal_args(internal_args_cursor, data_size_capacity);
                        }
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

    let internal_args = internal_args.cast::<c_void>();
    // SAFETY: FFI function was generated at the same time as corresponding `Args` and must match
    // ABI of the fingerprint, or else it wouldn't compile
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
        // Assert that the API has the expected shape
        let _: fn(&mut AddressAllocator, &mut Env<'_>) -> Result<Address, ContractError> =
            AddressAllocator::allocate_address;
        // SAFETY: Method call to address allocator succeeded, so it must have returned an address
        let new_address = unsafe { new_address_ptr.read() };
        if !slots.add_new_contract(new_address) {
            warn!("Failed to add new contract returned by address allocator");
            return Err(ContractError::InternalError);
        }
    }

    for &entry in post_processing.iter() {
        match entry {
            PostProcessing::Slot {
                internal_args_ptr,
                slot_index,
                must_be_not_empty,
            } => {
                // SAFETY: Correct pointer created earlier that is not used for anything else at the
                // moment
                let FfiDataSizeCapacityRw {
                    data_ptr,
                    size,
                    capacity: _,
                } = unsafe { internal_args_ptr.cast().read() };

                if must_be_not_empty && size == 0 {
                    error!(
                        %size,
                        "Contract returned empty size where it is not allowed, likely state of \
                        `#[init]` method"
                    );
                    return Err(ContractError::BadOutput);
                }

                let slot_bytes = slots
                    .access_used_rw(slot_index)
                    .expect("Was used above and must exist since `Slots` was not dropped yet; qed");

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
            PostProcessing::Output {
                internal_args_ptr,
                external_args_ptr,
            } => {
                // SAFETY: Correct pointer created earlier that is not used for anything else at the
                // moment
                let source_size =
                    unsafe { internal_args_ptr.cast::<FfiDataSizeCapacityRw>().read() }.size;
                // SAFETY: Correct pointer created earlier that is not used for anything else at the
                // moment
                let FfiDataSizeCapacityRw {
                    data_ptr: _,
                    size,
                    capacity,
                } = unsafe { external_args_ptr.cast::<FfiDataSizeCapacityRw>().as_mut() };

                if source_size > *capacity {
                    error!(
                        size = %source_size,
                        %capacity,
                        "Contract returned invalid size for output in source allocation"
                    );
                    return Err(ContractError::BadOutput);
                }

                *size = source_size;
            }
        }
    }

    Ok(())
}
