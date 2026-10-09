use crate::context::{MethodDetails, NativeExecutorContext};
use ab_contracts_common::env::{Env, EnvState};
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

// Only pointers and the structs below are written into `InternalArgs`. A single argument takes at
// most `INTERNAL_ARGS_SIZE / MAX_TOTAL_METHOD_ARGS` bytes, and since sizes of written values are
// multiples of the pointer alignment, which their alignment doesn't exceed, a cursor that starts at
// the beginning of a buffer of pointers stays correctly aligned after every write.
const _: () = {
    assert!(
        size_of::<*const Address>() + size_of::<FfiDataSizeCapacityRo>()
            <= INTERNAL_ARGS_SIZE / usize::from(MAX_TOTAL_METHOD_ARGS)
    );
    assert!(
        size_of::<*const Address>() + size_of::<FfiDataSizeCapacityRw>()
            <= INTERNAL_ARGS_SIZE / usize::from(MAX_TOTAL_METHOD_ARGS)
    );
    assert!(size_of::<FfiDataSizeCapacityRo>().is_multiple_of(align_of::<*mut c_void>()));
    assert!(size_of::<FfiDataSizeCapacityRw>().is_multiple_of(align_of::<*mut c_void>()));
    assert!(align_of::<FfiDataSizeCapacityRo>() <= align_of::<*mut c_void>());
    assert!(align_of::<FfiDataSizeCapacityRw>() <= align_of::<*mut c_void>());
};

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
    data_ptr: *mut u128,
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
    // SAFETY: Guaranteed by the function contract
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
    // SAFETY: Guaranteed by the function contract
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

/// Special container that allows aliasing of `Env` stored inside it and tracks the nested context
/// holding slots.
///
/// The nested context is tracked here even when it is given to `Env`, so that the host never reads
/// anything back from `Env` that the guest had exclusive access to (and could have replaced).
///
/// Variants with pointers are only created by `Self::insert_ro()`, `Self::insert_rw()` and
/// `Self::initialize()`, which `Drop` and `Self::into_slots()` rely on.
enum MaybeEnv<Env, Context> {
    None(Context),
    ReadOnly(*mut UnsafeCell<Env>, Context),
    ReadWrite(*mut UnsafeCell<Env>, Context),
}

impl<Env, Context> Drop for MaybeEnv<Env, Context> {
    #[inline(always)]
    fn drop(&mut self) {
        match self {
            MaybeEnv::None(_) => {}
            &mut (MaybeEnv::ReadOnly(env, _) | MaybeEnv::ReadWrite(env, _)) => {
                // SAFETY: Created with `Box::into_raw()` in `insert_ro()`/`insert_rw()` and only
                // freed here. The guest gets a pointer to it only for the duration of the FFI call,
                // during which `self` (owned by `make_ffi_call()`) can't be dropped.
                let _: Box<_> = unsafe { Box::from_raw(env) };
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
            // SAFETY: Valid pointer from `Box::into_raw()` right above, nothing else references
            // the value yet
            let env_ref = unsafe { env.as_ref_unchecked() };
            env_ref.get().cast_const()
        };
        *self = Self::ReadOnly(env, ());
        env_ptr
    }

    /// Insert a new value and get a pointer to it, value must be initialized later with
    /// [`Self::initialize()`]
    #[inline(always)]
    fn insert_rw(&mut self) -> *mut MaybeUninit<Env<'env>> {
        let env = Box::into_raw(Box::new(UnsafeCell::new(MaybeUninit::uninit())));
        let env_ptr = {
            // SAFETY: Valid pointer from `Box::into_raw()` right above, nothing else references
            // the value yet
            let env_ref = unsafe { env.as_ref_unchecked() };
            env_ref.get()
        };
        *self = Self::ReadWrite(env, ());
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
    ) -> MaybeEnv<Env<'env>, NonNull<NativeExecutorContext<'slots>>>
    where
        CreateNestedContext:
            FnOnce(NestedSlots<'slots>, bool) -> &'env mut NativeExecutorContext<'slots>,
        'slots: 'env,
    {
        // Only `#[view]` methods can be called through `&Env`, and without `Env` the nested context
        // isn't used for calls at all and only holds onto slots
        let allow_env_mutation = matches!(self, Self::ReadWrite(..));
        let mut context = NonNull::from_mut(create_nested_context(slots, allow_env_mutation));

        match self {
            Self::None(()) => MaybeEnv::None(context),
            Self::ReadOnly(env_ro, ()) => {
                // SAFETY: Created from an exclusive reference above. The reference given to
                // `Env` is derived from this pointer, the pointer itself is only used again in
                // `Self::into_slots()`, after the guest returned.
                let env = Env::with_executor_context(env_state, unsafe { context.as_mut() });
                {
                    // SAFETY: Valid pointer from `Box::into_raw()` in `Self::insert_ro()`, which is
                    // only freed when `self` is dropped. Only the pointer to it was given out so
                    // far, and nothing has a live reference to it as per function contract.
                    let env_ro = unsafe { env_ro.as_mut_unchecked() };
                    env_ro.get_mut().write(env);
                }
                // Very explicit cast to the initialized value since it was just written to
                let env_ro = env_ro.cast::<UnsafeCell<Env<'env>>>();

                // Prevent destructor from running and de-allocating `Env`
                mem::forget(self);

                MaybeEnv::ReadOnly(env_ro, context)
            }
            Self::ReadWrite(env_rw, ()) => {
                // SAFETY: Created from an exclusive reference above. The reference given to
                // `Env` is derived from this pointer, the pointer itself is only used again in
                // `Self::into_slots()`, after the guest returned.
                let env = Env::with_executor_context(env_state, unsafe { context.as_mut() });
                {
                    // SAFETY: Valid pointer from `Box::into_raw()` in `Self::insert_rw()`, which is
                    // only freed when `self` is dropped. Only the pointer to it was given out so
                    // far, and nothing has a live reference to it as per function contract.
                    let env_rw = unsafe { env_rw.as_mut_unchecked() };
                    env_rw.get_mut().write(env);
                }
                // Very explicit cast to the initialized value since it was just written to
                let env_rw = env_rw.cast::<UnsafeCell<Env<'env>>>();

                // Prevent destructor from running and de-allocating `Env`
                mem::forget(self);

                MaybeEnv::ReadWrite(env_rw, context)
            }
        }
    }
}

impl<'env, 'slots> MaybeEnv<Env<'env>, NonNull<NativeExecutorContext<'slots>>> {
    /// Free `Env` (if present) and get slots of the nested context
    #[inline(always)]
    fn into_slots(self) -> &'env mut NestedSlots<'slots> {
        let mut context = match self {
            MaybeEnv::None(context)
            | MaybeEnv::ReadOnly(_, context)
            | MaybeEnv::ReadWrite(_, context) => context,
        };
        // The host doesn't use `Env` anymore
        drop(self);
        // SAFETY: Created in `Self::initialize()` from an exclusive reference to the nested context
        // that is borrowed for `'env`. The only reference derived from it was given to the guest
        // through `Env`, and the guest can only use it during the FFI call, while `self` (owned by
        // `make_ffi_call()`) can't be consumed. Consuming `self` also ensures the host doesn't
        // access `Env` or create another reference to the nested context afterward.
        unsafe { context.as_mut() }.slots.get_mut()
    }
}

/// Call method of a contract through its FFI function.
///
/// # Safety
/// `external_args` must point to arguments laid out as
/// [`ExternalArgs`](ab_contracts_common::method::ExternalArgs) of the method described by
/// `method_details`, with all pointers in them valid for the whole call according to the method
/// signature. The native executor trusts contracts and their callers to uphold this, see
/// [`NativeExecutor`](crate::NativeExecutor).
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
pub(super) unsafe fn make_ffi_call<'slots, CreateNestedContext>(
    allow_env_mutation: bool,
    is_allocate_new_address_method: bool,
    parent_slots: &'slots mut NestedSlots<'slots>,
    contract: Address,
    method_details: MethodDetails,
    external_args: NonNull<c_void>,
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
    // expects, according to its metadata.
    //
    // Writes into it are in bounds and aligned: there are at most `MAX_TOTAL_METHOD_ARGS`
    // arguments including `self` (checked below), each written in one go, which is within bounds
    // and keeps the cursor aligned as asserted next to `INTERNAL_ARGS_SIZE`.
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
    #[expect(
        clippy::rest_pattern_accessible_field,
        reason = "Do not need other fields"
    )]
    let MethodMetadataItem {
        method_kind,
        num_arguments,
        ..
    } = method_metadata_item;

    let number_of_arguments = usize::from(num_arguments) + usize::from(method_kind.has_self());

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

            // SAFETY: In bounds of `internal_args` and aligned, see its allocation
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

            // SAFETY: In bounds of `internal_args` and aligned, see its allocation
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
                // SAFETY: In bounds of `internal_args` and aligned, see its allocation
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

                // SAFETY: In bounds of `internal_args` and aligned, see its allocation
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
                    // SAFETY: Arguments before this one were read in metadata order, so the
                    // cursor points to this slot's field in `ExternalArgs`, which is a pointer to
                    // the address, valid for reads during the call as per function contract
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

                // SAFETY: In bounds of `internal_args` and aligned, see its allocation
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
                    // SAFETY: Arguments before this one were read in metadata order, so the
                    // cursor points to this slot's field in `ExternalArgs`, which is a pointer to
                    // the address, valid for reads during the call as per function contract
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
                    // SAFETY: In bounds of `internal_args` and aligned, see its allocation
                    unsafe {
                        write_internal_args(internal_args_cursor, owner);
                    }
                }

                post_processing.push(PostProcessing::Slot {
                    internal_args_ptr: *internal_args_cursor,
                    slot_index,
                    must_be_not_empty: false,
                });

                // SAFETY: In bounds of `internal_args` and aligned, see its allocation
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
                // SAFETY: Arguments before this one were read in metadata order, so the cursor
                // points to this input's pointer, size and capacity in `ExternalArgs` as per
                // function contract. Writing is in bounds of `internal_args` and aligned, see its
                // allocation.
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
                        // SAFETY: In bounds of `internal_args` and aligned, see its allocation
                        unsafe {
                            // The return type is `TrivialType` and doesn't have size/capacity
                            write_internal_args(internal_args_cursor, state_bytes.as_mut_ptr());
                        }
                        // SAFETY: The return type is the state type (enforced by `#[contract]` for
                        // `#[init]`), which is `TrivialType`, so its recommended capacity in
                        // metadata is its size, and `use_rw()` above ensured at least that much
                        // capacity. The bytes are not read until the method returns, and on
                        // success it has written the whole state through the pointer above. On
                        // error slots are reset and the bytes are discarded. It is more efficient
                        // to just set the length here right away than do explicit post-processing
                        // below.
                        unsafe {
                            state_bytes.set_len(recommended_state_capacity);
                        }
                    } else {
                        post_processing.push(PostProcessing::Slot {
                            internal_args_ptr: *internal_args_cursor,
                            slot_index,
                            must_be_not_empty: true,
                        });

                        // SAFETY: In bounds of `internal_args` and aligned, see its allocation
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
                        // SAFETY: Arguments before this one were read in metadata order, so the
                        // cursor points to the return value pointer in `ExternalArgs` of
                        // `AddressAllocator::allocate_address()` as per function contract. Writing
                        // is in bounds of `internal_args` and aligned, see its allocation.
                        unsafe {
                            let address = read_external_args::<*mut Address>(external_args_cursor);
                            write_internal_args(internal_args_cursor, address);
                            new_address_ptr.replace(address);
                        }
                    } else if matches!(argument_kind, ArgumentKind::Return) {
                        // SAFETY: Arguments before this one were read in metadata order, so the
                        // cursor points to the return value pointer in `ExternalArgs` as per
                        // function contract. Writing is in bounds of `internal_args` and aligned,
                        // see its allocation.
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

                        // SAFETY: Arguments before this one were read in metadata order, so the
                        // cursor points to this output's pointer, size and capacity in
                        // `ExternalArgs` as per function contract. Writing is in bounds of
                        // `internal_args` and aligned, see its allocation.
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
    // SAFETY: `internal_args` only holds a raw pointer to `Env` and no references exist
    let maybe_env = unsafe {
        maybe_env.initialize(slots, env_state, |slots, allow_env_mutation| {
            nested_context.insert(create_nested_context(slots, allow_env_mutation))
        })
    };

    let internal_args = internal_args.cast::<c_void>();
    // SAFETY: `internal_args` was built according to `method_metadata`, which `#[contract]`
    // generated together with `ffi_fn` for the same method (the native executor trusts contracts,
    // including manual `Contract` implementations, to pair them correctly, see `NativeExecutor`).
    // Pointers in it stay valid for the whole call:
    // * state and slot buffers belong to `slots` and are registered as accessed there, so nested
    //   calls can't modify or reallocate them
    // * `Env` is allocated in `maybe_env` and only freed when it is consumed after the call, the
    //   nested context lives in `nested_context`, which outlives the call
    // * the rest comes from `external_args`, valid during the call as per function contract
    let result = Result::<(), ContractError>::from(unsafe { ffi_fn(internal_args) });

    let slots = maybe_env.into_slots();

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
        // SAFETY: `new_address_ptr` is the return value pointer of
        // `AddressAllocator::allocate_address()` (checked above), which the guest initialized since
        // the call succeeded
        let new_address = unsafe { new_address_ptr.read() };
        if !slots.add_new_contract(new_address) {
            warn!("Failed to add new contract returned by address allocator");
            return Err(ContractError::InternalError);
        }
    }

    for &entry in &post_processing {
        match entry {
            PostProcessing::Slot {
                internal_args_ptr,
                slot_index,
                must_be_not_empty,
            } => {
                // SAFETY: Points to the `FfiDataSizeCapacityRw` written into `internal_args` for
                // this slot above, which is still alive, and the guest no longer accesses it
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
                    // SAFETY: The guest replaced the slot contents with a different allocation,
                    // whose first `size` bytes are initialized as required by `IoType` (the native
                    // executor trusts contracts to uphold it, see `NativeExecutor`)
                    let data =
                        unsafe { slice::from_raw_parts(data_ptr.cast::<u8>(), size as usize) };
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
                // SAFETY: Checked to be within capacity above, and the guest initialized the first
                // `size` bytes as required by `IoType` (the native executor trusts contracts to
                // uphold it, see `NativeExecutor`)
                unsafe {
                    slot_bytes.set_len(size);
                }
            }
            PostProcessing::Output {
                internal_args_ptr,
                external_args_ptr,
            } => {
                // SAFETY: Points to the `FfiDataSizeCapacityRw` written into `internal_args` for
                // this output above, which is still alive, and the guest no longer accesses it
                let source_size =
                    unsafe { internal_args_ptr.cast::<FfiDataSizeCapacityRw>().read() }.size;
                // SAFETY: Points to this output's pointer, size and capacity in `ExternalArgs`,
                // which are valid for writes during the call as per function contract and not
                // accessed by anything else, since the guest returned
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
