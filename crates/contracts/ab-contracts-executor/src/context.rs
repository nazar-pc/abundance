mod aligned_buffer;
mod slots;

use crate::context::aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use crate::context::slots::{Slots, UsedSlots};
use ab_contracts_common::env::{Env, EnvState, ExecutorContext, MethodContext, PreparedMethod};
use ab_contracts_common::metadata::decode::{
    ArgumentKind, ArgumentMetadataItem, MethodKind, MethodMetadataDecoder, MethodMetadataItem,
    MethodsContainerKind,
};
use ab_contracts_common::method::MethodFingerprint;
use ab_contracts_common::{Address, ContractError, ExitCode, ShardIndex};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::slice;
use std::sync::{Arc, Weak};
use tracing::{debug, error, info_span};

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

/// Stores details about arguments that need to be processed after FFI call
enum DelayedProcessing {
    SlotReadOnly {
        size: u32,
    },
    SlotReadWrite {
        /// Pointer to `InternalArgs` where guest will store a pointer to potentially updated slot
        /// contents
        data_ptr: NonNull<*mut u8>,
        /// Pointer to slot's bytes buffer here bytes from `data_ptr` will need to be written
        /// after FFI function call
        slot_ptr: NonNull<OwnedAlignedBuffer>,
        /// Pointer to `InternalArgs` where guest will store potentially updated slot size,
        /// corresponds to `data_ptr`, filled during the second pass through the arguments
        /// (while reading `ExternalArgs`)
        size: u32,
        capacity: u32,
    },
}

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("Unsupported pointer width");

#[derive(Debug, Copy, Clone)]
pub(super) struct MethodDetails {
    pub(super) recommended_state_capacity: u32,
    pub(super) recommended_slot_capacity: u32,
    pub(super) recommended_tmp_capacity: u32,
    pub(super) method_metadata: &'static [u8],
    pub(super) ffi_fn: unsafe extern "C" fn(NonNull<NonNull<c_void>>) -> ExitCode,
}

#[derive(Debug)]
pub(super) struct NativeExecutorContext {
    shard_index: ShardIndex,
    /// Indexed by contract's code (crate name is treated as "code")
    methods_by_code: HashMap<&'static [u8], HashMap<MethodFingerprint, MethodDetails>>,
    // TODO: Think about optimizing locking
    slots: Slots,
    weak: Weak<Self>,
}

impl ExecutorContext for NativeExecutorContext {
    fn call_many(
        &self,
        previous_env_state: &EnvState,
        prepared_methods: &[PreparedMethod<'_>],
    ) -> Result<(), ContractError> {
        // TODO: Check slot misuse across recursive calls
        // TODO: Check read/write environment access permissions
        // `used_slots` must be before processing of the method because in the process of method
        // handling, some data structures will store pointers to `UsedSlot`'s internals.
        let mut used_slots = UsedSlots::new(&self.slots);

        // TODO: Parallelism
        for prepared_method in prepared_methods {
            let PreparedMethod {
                contract,
                fingerprint,
                external_args,
                method_context,
                ..
            } = prepared_method;

            let mut env = Env::with_executor_context(
                EnvState {
                    shard_index: self.shard_index,
                    own_address: *contract,
                    context: match method_context {
                        MethodContext::Keep => previous_env_state.context,
                        MethodContext::Reset => Address::NULL,
                        MethodContext::Replace => previous_env_state.own_address,
                    },
                    caller: previous_env_state.own_address,
                },
                self.weak
                    .upgrade()
                    .expect("Reference to itself, hence upgrade always succeeds; qed"),
            );

            let span = info_span!("NativeExecutorContext", %contract);
            let _span_guard = span.enter();

            let method_details = {
                let code = self
                    .slots
                    .get(contract, &Address::SYSTEM_CODE)
                    .ok_or_else(|| {
                        error!("Contract or its code not found");
                        ContractError::NotFound
                    })?;
                *self
                    .methods_by_code
                    .get(code.as_slice())
                    .ok_or_else(|| {
                        let code = String::from_utf8_lossy(&code);
                        error!(%code, "Contract's code not found in methods map");
                        ContractError::InternalError
                    })?
                    .get(fingerprint)
                    .ok_or_else(|| {
                        let code = String::from_utf8_lossy(&code);
                        error!(%code, %fingerprint, "Method's fingerprint not found");
                        ContractError::NotImplemented
                    })?
            };

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

            let total_arguments = usize::from(num_arguments)
                + method_kind.has_self().then_some(1).unwrap_or_default();
            // Allocate a buffer that will contain incrementally built `InternalArgs` that method
            // expects, according to its metadata.
            // `* 4` is due to slots having 2 pointers (detecting this accurately is more code,
            // so we just assume the worst case), otherwise it would be 3 pointers: data + size
            // + capacity.
            let mut internal_args = Box::<[*mut c_void]>::new_uninit_slice(total_arguments * 4);
            let internal_args = NonNull::from_mut(internal_args.as_mut()).cast::<*mut c_void>();

            // This pointer will be moving as the data structure is being constructed, while
            // `internal_args` will keep pointing to the beginning
            let mut internal_args_cursor = internal_args;
            // This pointer will be moving as the data structure is being read, while
            // `external_args` will keep pointing to the beginning
            let mut external_args_cursor = *external_args;
            // Delayed processing of sizes as capacities since knowing them requires processing all
            // arguments first
            let mut delayed_processing = Vec::with_capacity(total_arguments);

            // Handle `&self` and `&mut self`
            match method_kind {
                MethodKind::Init => {
                    // Handled after the rest of the arguments if needed
                }
                MethodKind::UpdateStateless | MethodKind::ViewStateless => {
                    // No state handling is needed
                }
                MethodKind::UpdateStatefulRo | MethodKind::ViewStateful => {
                    let state_bytes = used_slots.use_ro(contract, &Address::SYSTEM_STATE)?;

                    delayed_processing.push(DelayedProcessing::SlotReadOnly {
                        size: state_bytes.len(),
                    });
                    let Some(DelayedProcessing::SlotReadOnly { size }) = delayed_processing.last()
                    else {
                        unreachable!("Just inserted `SlotReadOnly` entry; qed");
                    };

                    // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                    // above and aligned correctly
                    unsafe {
                        write_ptr!(state_bytes.as_ptr() => internal_args_cursor as *const u8);
                        write_ptr!(size => internal_args_cursor as *const u32);
                    }
                }
                MethodKind::UpdateStatefulRw => {
                    let state_bytes = used_slots.use_rw(
                        contract,
                        &Address::SYSTEM_STATE,
                        recommended_state_capacity,
                    )?;

                    delayed_processing.push(DelayedProcessing::SlotReadWrite {
                        // Is updated below
                        data_ptr: NonNull::dangling(),
                        slot_ptr: NonNull::from_mut(state_bytes),
                        size: state_bytes.len(),
                        capacity: state_bytes.capacity(),
                    });
                    let Some(DelayedProcessing::SlotReadWrite {
                        data_ptr,
                        size,
                        capacity,
                        ..
                    }) = delayed_processing.last_mut()
                    else {
                        unreachable!("Just inserted `SlotReadWrite` entry; qed");
                    };

                    // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                    // above and aligned correctly
                    unsafe {
                        *data_ptr =
                            write_ptr!(state_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                        write_ptr!(size => internal_args_cursor as *mut u32);
                        write_ptr!(capacity => internal_args_cursor as *const u32);
                    }
                }
            }

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
                        // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                        // above and aligned correctly
                        unsafe {
                            write_ptr!(&env => internal_args_cursor as *const Env);
                        }

                        // Size for `#[env]` is implicit and doesn't need to be added to
                        // `InternalArgs`
                    }
                    ArgumentKind::EnvRw => {
                        // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                        // above and aligned correctly
                        unsafe {
                            write_ptr!(&mut env => internal_args_cursor as *mut Env);
                        }

                        // Size for `#[env]` is implicit and doesn't need to be added to
                        // `InternalArgs`
                    }
                    ArgumentKind::TmpRo => {
                        // Null contact is used implicitly for `#[tmp]` since it is not possible for
                        // this contract to write something there directly
                        let tmp_bytes = used_slots.use_ro(contract, &Address::NULL)?;

                        delayed_processing.push(DelayedProcessing::SlotReadOnly {
                            size: tmp_bytes.len(),
                        });
                        let Some(DelayedProcessing::SlotReadOnly { size }) =
                            delayed_processing.last()
                        else {
                            unreachable!("Just inserted `SlotReadOnly` entry; qed");
                        };

                        // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                        // above and aligned correctly
                        unsafe {
                            write_ptr!(tmp_bytes.as_ptr() => internal_args_cursor as *const u8);
                            write_ptr!(size => internal_args_cursor as *const u32);
                        }
                    }
                    ArgumentKind::TmpRw => {
                        // Null contact is used implicitly for `#[tmp]` since it is not possible for
                        // this contract to write something there directly
                        let tmp_bytes = used_slots.use_rw(
                            contract,
                            &Address::NULL,
                            recommended_tmp_capacity,
                        )?;

                        delayed_processing.push(DelayedProcessing::SlotReadWrite {
                            // Is updated below
                            data_ptr: NonNull::dangling(),
                            slot_ptr: NonNull::from_mut(tmp_bytes),
                            size: tmp_bytes.len(),
                            capacity: tmp_bytes.capacity(),
                        });
                        let Some(DelayedProcessing::SlotReadWrite {
                            data_ptr,
                            size,
                            capacity,
                            ..
                        }) = delayed_processing.last_mut()
                        else {
                            unreachable!("Just inserted `SlotReadWrite` entry; qed");
                        };

                        // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                        // above and aligned correctly
                        unsafe {
                            *data_ptr = write_ptr!(tmp_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                            write_ptr!(size => internal_args_cursor as *mut u32);
                            write_ptr!(capacity => internal_args_cursor as *const u32);
                        }
                    }
                    ArgumentKind::SlotRo => {
                        // SAFETY: `external_args_cursor`'s must contain a valid pointer to address,
                        // moving right past that is safe
                        let address =
                            unsafe { &*read_ptr!(external_args_cursor as *const Address) };

                        let slot_bytes = used_slots.use_ro(address, contract)?;

                        delayed_processing.push(DelayedProcessing::SlotReadOnly {
                            size: slot_bytes.len(),
                        });
                        let Some(DelayedProcessing::SlotReadOnly { size }) =
                            delayed_processing.last()
                        else {
                            unreachable!("Just inserted `SlotReadOnly` entry; qed");
                        };

                        // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                        // above and aligned correctly
                        unsafe {
                            write_ptr!(address => internal_args_cursor as *const Address);
                            write_ptr!(slot_bytes.as_ptr() => internal_args_cursor as *const u8);
                            write_ptr!(size => internal_args_cursor as *const u32);
                        }
                    }
                    ArgumentKind::SlotRw => {
                        // SAFETY: `external_args_cursor`'s must contain a valid pointer to address,
                        // moving right past that is safe
                        let address =
                            unsafe { &*read_ptr!(external_args_cursor as *const Address) };

                        let slot_bytes =
                            used_slots.use_rw(address, contract, recommended_slot_capacity)?;

                        delayed_processing.push(DelayedProcessing::SlotReadWrite {
                            // Is updated below
                            data_ptr: NonNull::dangling(),
                            slot_ptr: NonNull::from_mut(slot_bytes),
                            size: slot_bytes.len(),
                            capacity: slot_bytes.capacity(),
                        });
                        let Some(DelayedProcessing::SlotReadWrite {
                            data_ptr,
                            size,
                            capacity,
                            ..
                        }) = delayed_processing.last_mut()
                        else {
                            unreachable!("Just inserted `SlotReadWrite` entry; qed");
                        };

                        // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient size
                        // above and aligned correctly
                        unsafe {
                            write_ptr!(address => internal_args_cursor as *const Address);
                            *data_ptr = write_ptr!(slot_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                            write_ptr!(size => internal_args_cursor as *mut u32);
                            write_ptr!(capacity => internal_args_cursor as *const u32);
                        }
                    }
                    ArgumentKind::Input => {
                        // SAFETY: `external_args_cursor`'s must contain a pointers to input + size.
                        // `internal_args_cursor`'s memory is allocated with sufficient size above
                        // and aligned correctly.
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
                        // `internal_args_cursor`'s memory is allocated with sufficient size above
                        // and aligned correctly.
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
                            let state_bytes = used_slots.use_rw(
                                contract,
                                &Address::SYSTEM_STATE,
                                recommended_state_capacity,
                            )?;

                            if !state_bytes.is_empty() {
                                debug!("Can't initialize already initialized contract");
                                return Err(ContractError::Forbidden);
                            }

                            delayed_processing.push(DelayedProcessing::SlotReadWrite {
                                // Is updated below
                                data_ptr: NonNull::dangling(),
                                slot_ptr: NonNull::from_mut(state_bytes),
                                size: 0,
                                capacity: state_bytes.capacity(),
                            });
                            let Some(DelayedProcessing::SlotReadWrite {
                                data_ptr,
                                size,
                                capacity,
                                ..
                            }) = delayed_processing.last_mut()
                            else {
                                unreachable!("Just inserted `SlotReadWrite` entry; qed");
                            };

                            // SAFETY: `internal_args_cursor`'s memory is allocated with sufficient
                            // size above and aligned correctly
                            unsafe {
                                *data_ptr = write_ptr!(state_bytes.as_mut_ptr() => internal_args_cursor as *mut u8);
                                write_ptr!(size => internal_args_cursor as *mut u32);
                                write_ptr!(capacity => internal_args_cursor as *const u32);
                            }
                        } else {
                            // SAFETY: `external_args_cursor`'s must contain a pointers to input
                            // + size + capacity.
                            // `internal_args_cursor`'s memory is allocated with sufficient size
                            // above and aligned correctly.
                            unsafe {
                                // Output
                                copy_ptr!(external_args_cursor => internal_args_cursor as *mut u8);
                                // Size (might be a null pointer for trivial types)
                                let size_ptr = copy_ptr!(external_args_cursor => internal_args_cursor as *mut u32);
                                if !size_ptr.is_null() {
                                    // Override output size to be zero even if caller guest tried to
                                    // put something there
                                    size_ptr.write(0);
                                }
                                // Capacity
                                copy_ptr!(external_args_cursor => internal_args_cursor as *const u32);
                            }
                        }
                    }
                }
            }

            // SAFETY: FFI function was generated at the same time as corresponding `Args` and must
            // match ABI of the fingerprint or else it wouldn't compile
            Result::<(), ContractError>::from(unsafe {
                ffi_fn(internal_args.cast::<NonNull<c_void>>())
            })?;

            for entry in delayed_processing {
                match entry {
                    DelayedProcessing::SlotReadOnly { .. } => {
                        // No processing is necessary
                    }
                    DelayedProcessing::SlotReadWrite {
                        data_ptr,
                        mut slot_ptr,
                        size,
                        ..
                    } => {
                        // SAFETY: Correct pointer created earlier that is not used for anything
                        // else at the moment
                        let data_ptr = unsafe { data_ptr.as_ptr().read().cast_const() };
                        // SAFETY: Correct pointer created earlier that is not used for anything
                        // else at the moment (no other contract in the stack can access the same
                        // slot exclusively at the same time, which is guaranteed by `UsedSlots`
                        // API)
                        let slot_bytes = unsafe { slot_ptr.as_mut() };

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
        }

        used_slots.persist();

        Ok(())
    }
}

impl NativeExecutorContext {
    pub(super) fn new(
        shard_index: ShardIndex,
        methods_by_code: HashMap<&'static [u8], HashMap<MethodFingerprint, MethodDetails>>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|weak| NativeExecutorContext {
            shard_index,
            methods_by_code,
            slots: Slots::default(),
            weak: weak.clone(),
        })
    }

    pub(super) fn shard_index(&self) -> ShardIndex {
        self.shard_index
    }

    pub(super) fn force_insert(&self, owner: Address, contract: Address, value: &[u8]) {
        self.slots
            .put(owner, contract, SharedAlignedBuffer::from_bytes(value));
    }
}
