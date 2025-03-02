use crate::context::{MethodDetails, NativeExecutorContext};
use crate::slots::{SlotIndex, SlotKey, Slots};
use ab_contracts_common::env::{Env, EnvState};
use ab_contracts_common::metadata::decode::{
    ArgumentKind, ArgumentMetadataItem, MethodKind, MethodMetadataDecoder, MethodMetadataItem,
    MethodsContainerKind,
};
use ab_contracts_common::{Address, ContractError, ExitCode};
use ab_system_contract_address_allocator::AddressAllocator;
use aliasable::boxed::AliasableBox;
use parking_lot::Mutex;
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use std::slice;
use std::sync::Arc;
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

struct DelayedProcessingSlotReadOnly {
    size: u32,
}

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

/// Stores details about arguments that need to be processed after FFI call
enum DelayedProcessing {
    SlotReadOnly(DelayedProcessingSlotReadOnly),
    SlotReadWrite(DelayedProcessingSlotReadWrite),
}

struct DelayedProcessingCollection {
    inner: Vec<UnsafeCell<DelayedProcessing>>,
}

impl DelayedProcessingCollection {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    /// Insert new entry and get mutable reference to it, which doesn't inherit stack borrows tag
    fn insert_ro(
        &mut self,
        entry: DelayedProcessingSlotReadOnly,
    ) -> &DelayedProcessingSlotReadOnly {
        self.inner
            .push(UnsafeCell::new(DelayedProcessing::SlotReadOnly(entry)));
        // SAFETY: Created from a live value, which was just inserted
        let Some(DelayedProcessing::SlotReadOnly(entry)) =
            self.inner.last().map(|value| unsafe { &mut *value.get() })
        else {
            unreachable!("Just inserted `DelayedProcessing::SlotReadOnly` entry; qed");
        };

        entry
    }

    /// Insert new entry and get mutable reference to it, which doesn't inherit stack borrows tag
    fn insert_rw(
        &mut self,
        entry: DelayedProcessingSlotReadWrite,
    ) -> &mut DelayedProcessingSlotReadWrite {
        self.inner
            .push(UnsafeCell::new(DelayedProcessing::SlotReadWrite(entry)));
        // SAFETY: Created from a live value, which was just inserted
        let Some(DelayedProcessing::SlotReadWrite(entry)) =
            self.inner.last().map(|value| unsafe { &mut *value.get() })
        else {
            unreachable!("Just inserted `DelayedProcessing::SlotReadWrite` entry; qed");
        };

        entry
    }
}

#[must_use]
pub(super) struct FfiCallResult<'a> {
    delayed_processing: DelayedProcessingCollection,
    // It is important to keep this allocation around or else `delayed_processing` that may contain
    // pointers to its memory could try to read deallocated memory
    _internal_args: AliasableBox<UnsafeCell<[MaybeUninit<*mut c_void>]>>,
    // It is important to keep this allocation around or else `_internal_args` that may contain
    // pointers to its memory (indirectly by asking environment to keep allocation around) could try
    // to read deallocated memory
    _maybe_env: Option<AliasableBox<UnsafeCell<Env<'a>>>>,
    // Store slots instance, to make pointers to its contents remain valid for the duration of the
    // call
    slots: Arc<Mutex<Slots>>,
}

impl FfiCallResult<'_> {
    /// Persist slot changes
    pub(super) fn persist(self) -> Result<(), ContractError> {
        let slots = &mut *self.slots.lock();

        for entry in self.delayed_processing.inner {
            match entry.into_inner() {
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

                    // SAFETY: Correct pointer created earlier that is not used for anything
                    // else at the moment
                    let data_ptr = unsafe { data_ptr.as_ptr().read().cast_const() };
                    let slot_bytes = slots
                        .access_used_rw(slot_index)
                        .expect("Was used in `FfiCall` and must `Slots` was not dropped yet; qed");

                    // Guest created a different allocation for slot, copy bytes
                    if data_ptr != slot_bytes.as_mut_ptr() {
                        if data_ptr.is_null() {
                            error!("Contract returned `null` pointer for slot data");
                            return Err(ContractError::BadOutput);
                        }
                        // SAFETY: For native execution guest behavior is assumed to be trusted
                        // and provide a correct pointer and size
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
                    // SAFETY: For native execution guest behavior is assumed to be trusted and
                    // provide a correct size
                    unsafe {
                        slot_bytes.set_len(size);
                    }
                }
            }
        }

        Ok(())
    }
}

#[must_use]
pub(super) struct FfiCall<'a> {
    delayed_processing: DelayedProcessingCollection,
    internal_args: AliasableBox<UnsafeCell<[MaybeUninit<*mut c_void>]>>,
    maybe_env: Option<AliasableBox<UnsafeCell<Env<'a>>>>,
    slots: Arc<Mutex<Slots>>,
    new_address_ptr: Option<*mut Address>,
    ffi_fn: unsafe extern "C" fn(NonNull<NonNull<c_void>>) -> ExitCode,
}

impl<'a> FfiCall<'a> {
    #[allow(clippy::too_many_arguments, reason = "Internal API")]
    pub(super) fn new(
        parent_context: &NativeExecutorContext,
        force_view_only: bool,
        is_allocate_new_address_method: bool,
        slots: Arc<Mutex<Slots>>,
        contract: Address,
        method_details: MethodDetails,
        external_args: &'a mut NonNull<NonNull<c_void>>,
        env_state: EnvState,
    ) -> Result<Self, ContractError> {
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

        let total_arguments =
            usize::from(num_arguments) + method_kind.has_self().then_some(1).unwrap_or_default();
        // Allocate a buffer that will contain incrementally built `InternalArgs` that method
        // expects, according to its metadata.
        // `* 4` is due to slots having 2 pointers (detecting this accurately is more code, so this
        // just assumes the worst case), otherwise it would be 3 pointers: data + size + capacity.
        let internal_args = Box::<[*mut c_void]>::new_uninit_slice(total_arguments * 4);
        let internal_args = unsafe {
            std::mem::transmute::<
                Box<[MaybeUninit<*mut c_void>]>,
                Box<UnsafeCell<[MaybeUninit<*mut c_void>]>>,
            >(internal_args)
        };
        let internal_args = AliasableBox::from_unique(internal_args);

        // This pointer will be moving as the data structure is being constructed, while
        // `internal_args` will keep pointing to the beginning
        let mut internal_args_cursor = NonNull::<MaybeUninit<*mut c_void>>::new(
            internal_args.get().cast::<MaybeUninit<*mut c_void>>(),
        )
        .expect("Taken from non-null instance; qed")
        .cast::<*mut c_void>();
        // This pointer will be moving as the data structure is being read, while `external_args`
        // will keep pointing to the beginning
        let mut external_args_cursor = *external_args;
        // Delayed processing of sizes as capacities since knowing them requires processing all
        // arguments first.
        //
        // NOTE: It is important that this is never reallocated as it will invalidate all the
        // pointers to elements of this vector!
        let mut delayed_processing = DelayedProcessingCollection::with_capacity(total_arguments);

        // `true` when only `#[view]` method is allowed
        let view_only = match method_kind {
            MethodKind::Init
            | MethodKind::UpdateStateless
            | MethodKind::UpdateStatefulRo
            | MethodKind::UpdateStatefulRw => {
                if force_view_only || !parent_context.allow_env_mutation {
                    warn!(
                        force_view_only,
                        allow_env_mutation = %parent_context.allow_env_mutation,
                        "Only `#[view]` methods are allowed"
                    );
                    return Err(ContractError::Forbidden);
                }

                false
            }
            MethodKind::ViewStateless | MethodKind::ViewStateful => true,
        };

        // Order is important here, `maybe_env` must be before `slots_guard` to avoid deadlock
        let mut maybe_env = None::<AliasableBox<UnsafeCell<Env>>>;
        let mut slots_guard = slots.lock();

        // Handle `&self` and `&mut self`
        match method_kind {
            MethodKind::Init => {
                // No state handling is needed
            }
            MethodKind::UpdateStateless | MethodKind::ViewStateless => {
                // No state handling is needed
            }
            MethodKind::UpdateStatefulRo | MethodKind::ViewStateful => {
                let state_bytes = slots_guard
                    .use_ro(SlotKey {
                        owner: contract,
                        contract: Address::SYSTEM_STATE,
                    })
                    .ok_or(ContractError::Forbidden)?;

                if state_bytes.is_empty() {
                    warn!(
                        "Contract does not have state yet, can't call stateful method before init"
                    );
                    return Err(ContractError::Forbidden);
                }

                let result = delayed_processing.insert_ro(DelayedProcessingSlotReadOnly {
                    size: state_bytes.len(),
                });

                // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above
                // and aligned correctly
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
                let (slot_index, state_bytes) = slots_guard
                    .use_rw(slot_key, recommended_state_capacity)
                    .ok_or(ContractError::Forbidden)?;

                if state_bytes.is_empty() {
                    warn!(
                        "Contract does not have state yet, can't call stateful method before init"
                    );
                    return Err(ContractError::Forbidden);
                }

                let result = delayed_processing.insert_rw(DelayedProcessingSlotReadWrite {
                    // Is updated below
                    data_ptr: NonNull::dangling(),
                    size: state_bytes.len(),
                    capacity: state_bytes.capacity(),
                    slot_index,
                    must_be_not_empty: false,
                });

                // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size above
                // and aligned correctly
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
        while let Some(result) = arguments_metadata_decoder.decode_next() {
            let item = match result {
                Ok(result) => result,
                Err(error) => {
                    error!(%error, "Argument metadata decoding error");
                    return Err(ContractError::InternalError);
                }
            };

            let ArgumentMetadataItem { argument_kind, .. } = item;

            match argument_kind {
                ArgumentKind::EnvRo => {
                    let env = Box::new(UnsafeCell::new(Env::with_executor_context(
                        env_state,
                        Box::new(parent_context.new_nested(Arc::clone(&slots), false)),
                    )));
                    let env = AliasableBox::from_unique(env);
                    let env_ro = maybe_env.insert(env).get().cast_const();
                    // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                    // above and aligned correctly
                    unsafe {
                        write_ptr!(env_ro => internal_args_cursor as *const Env);
                    }

                    // Size for `#[env]` is implicit and doesn't need to be added to `InternalArgs`
                }
                ArgumentKind::EnvRw => {
                    if view_only {
                        return Err(ContractError::Forbidden);
                    }

                    let env = Box::new(UnsafeCell::new(Env::with_executor_context(
                        env_state,
                        Box::new(parent_context.new_nested(Arc::clone(&slots), true)),
                    )));
                    let env = AliasableBox::from_unique(env);
                    let env_rw = maybe_env.insert(env).get();

                    // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                    // above and aligned correctly
                    unsafe {
                        write_ptr!(env_rw => internal_args_cursor as *mut Env);
                    }

                    // Size for `#[env]` is implicit and doesn't need to be added to `InternalArgs`
                }
                ArgumentKind::TmpRo | ArgumentKind::SlotRo => {
                    let tmp = matches!(argument_kind, ArgumentKind::TmpRo);

                    let address = if tmp {
                        if view_only {
                            return Err(ContractError::Forbidden);
                        }

                        // Null contact is used implicitly for `#[tmp]` since it is not possible for
                        // this contract to write something there directly
                        &Address::NULL
                    } else {
                        // SAFETY: `external_args_cursor`'s must contain a valid pointer to address,
                        // moving right past that is safe
                        unsafe { &*read_ptr!(external_args_cursor as *const Address) }
                    };

                    let slot_key = SlotKey {
                        owner: *address,
                        contract,
                    };
                    let slot_bytes = slots_guard
                        .use_ro(slot_key)
                        .ok_or(ContractError::Forbidden)?;

                    let result = delayed_processing.insert_ro(DelayedProcessingSlotReadOnly {
                        size: slot_bytes.len(),
                    });

                    // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                    // above and aligned correctly
                    unsafe {
                        if !tmp {
                            write_ptr!(address => internal_args_cursor as *const Address);
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

                    let (address, capacity) = if tmp {
                        // Null contact is used implicitly for `#[tmp]` since it is not possible for
                        // this contract to write something there directly
                        (&Address::NULL, recommended_tmp_capacity)
                    } else {
                        // SAFETY: `external_args_cursor`'s must contain a valid pointer to address,
                        // moving right past that is safe
                        let address =
                            unsafe { &*read_ptr!(external_args_cursor as *const Address) };

                        (address, recommended_slot_capacity)
                    };

                    let slot_key = SlotKey {
                        owner: *address,
                        contract,
                    };
                    let (slot_index, slot_bytes) = slots_guard
                        .use_rw(slot_key, capacity)
                        .ok_or(ContractError::Forbidden)?;

                    let result = delayed_processing.insert_rw(DelayedProcessingSlotReadWrite {
                        // Is updated below
                        data_ptr: NonNull::dangling(),
                        size: slot_bytes.len(),
                        capacity: slot_bytes.capacity(),
                        slot_index,
                        must_be_not_empty: false,
                    });

                    // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                    // above and aligned correctly
                    unsafe {
                        if !tmp {
                            write_ptr!(address => internal_args_cursor as *const Address);
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
                    // SAFETY: `external_args_cursor`'s must contain a pointers to input + size
                    // + capacity.
                    // `internal_args_cursor`'s memory is allocated with sufficient size above and
                    // aligned correctly.
                    unsafe {
                        // Output
                        copy_ptr!(external_args_cursor => internal_args_cursor as *mut u8);
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
                ArgumentKind::Result => {
                    // `#[init]` method returns state of the contract and needs to be stored
                    // accordingly
                    if matches!(method_kind, MethodKind::Init) {
                        if view_only {
                            return Err(ContractError::Forbidden);
                        }

                        let slot_key = SlotKey {
                            owner: contract,
                            contract: Address::SYSTEM_STATE,
                        };
                        let (slot_index, state_bytes) = slots_guard
                            .use_rw(slot_key, recommended_state_capacity)
                            .ok_or(ContractError::Forbidden)?;

                        if !state_bytes.is_empty() {
                            debug!("Can't initialize already initialized contract");
                            return Err(ContractError::Forbidden);
                        }

                        let result = delayed_processing.insert_rw(DelayedProcessingSlotReadWrite {
                            // Is updated below
                            data_ptr: NonNull::dangling(),
                            size: 0,
                            capacity: state_bytes.capacity(),
                            slot_index,
                            must_be_not_empty: true,
                        });

                        // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                        // above and aligned correctly
                        unsafe {
                            result.data_ptr = write_ptr!(state_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                            write_ptr!(&mut result.size => internal_args_cursor as *mut u32);
                            write_ptr!(&result.capacity => internal_args_cursor as *const u32);
                        }
                    } else {
                        // SAFETY: `external_args_cursor`'s must contain a pointers to input + size
                        // + capacity.
                        // `internal_args_cursor`'s memory is allocated with sufficient size above
                        // and aligned correctly.
                        unsafe {
                            // Output
                            let ptr =
                                copy_ptr!(external_args_cursor => internal_args_cursor as *mut u8);
                            if is_allocate_new_address_method {
                                new_address_ptr.replace(ptr.cast::<Address>());
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

        drop(slots_guard);

        Ok(Self {
            delayed_processing,
            internal_args,
            maybe_env,
            slots,
            new_address_ptr,
            ffi_fn,
        })
    }

    pub(super) fn dispatch(mut self) -> Result<FfiCallResult<'a>, ContractError> {
        // Will only read initialized number of pointers, hence `NonNull<c_void>` even though
        // there is likely slack capacity with uninitialized data
        let internal_args =
            NonNull::<MaybeUninit<*mut c_void>>::new(self.internal_args.get_mut().as_mut_ptr())
                .expect("Taken from non-null instance; qed")
                .cast::<NonNull<c_void>>();

        // SAFETY: FFI function was generated at the same time as corresponding `Args` and must
        // match ABI of the fingerprint or else it wouldn't compile
        Result::<(), ContractError>::from(unsafe { (self.ffi_fn)(internal_args) })?;

        // Catch new address allocation and add it to new contracts in slots for code and other
        // things to become usable for it
        if let Some(new_address_ptr) = self.new_address_ptr {
            // Assert that the API has expected shape
            let _: fn(&mut AddressAllocator, &mut Env) -> Result<Address, ContractError> =
                AddressAllocator::allocate_address;
            // SAFETY: Method call to address allocator succeeded, so it must have returned an
            // address
            let new_address = unsafe { new_address_ptr.read() };
            if !self.slots.lock().add_new_contract(new_address) {
                warn!("Failed to add new contract returned by address allocator");
                return Err(ContractError::InternalError);
            }
        }

        Ok(FfiCallResult {
            delayed_processing: self.delayed_processing,
            _internal_args: self.internal_args,
            _maybe_env: self.maybe_env,
            slots: self.slots,
        })
    }
}
