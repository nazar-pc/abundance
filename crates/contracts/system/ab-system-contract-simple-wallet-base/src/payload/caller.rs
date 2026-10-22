//! Calling methods of a payload one by one

#[cfg(test)]
mod tests;

use crate::EXTERNAL_ARGS_BUFFER_SIZE;
use crate::payload::decoder::{DecodedInput, DecodedOutput, DecodedSlot};
use crate::payload::decoder_core::{MethodSink, PayloadDecoderCore, check_slot_output_size};
use crate::payload::{
    FfiDataSizeCapacityRo, FfiDataSizeCapacityRw, OutputBufferDetails, TransactionMethodContext,
    TransactionOutputKind, TransactionPayloadCallError, TransactionPayloadDecoderError,
};
use ab_contracts_common::env::{Env, MethodContext, PreparedMethod};
use ab_contracts_common::{ContractError, MAX_TOTAL_METHOD_ARGS};
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::{MaybeUninit, offset_of};
use core::ptr::NonNull;

const {
    // Offsets into `external_args_buffer` fit into `PendingOutputSize`
    assert!(size_of::<[*mut c_void; EXTERNAL_ARGS_BUFFER_SIZE]>() <= usize::from(u8::MAX));
    // Outputs are allocated at offsets that are aligned as the payload declares, up to
    // `Alignment::MAX`, so their memory is aligned as declared when the output buffer is aligned to
    // `Alignment::MAX`
    assert!(align_of::<u128>() >= usize::from(Alignment::MAX.get()));
}

/// Calls prepared methods for [`TransactionPayloadCaller`]: [`Env`], or something that stands in
/// for the host in tests.
///
/// # Safety
/// When a call returns `Ok(())`, the method's outputs and return value must have been written the
/// way `ExecutorContext` requires, as a successful [`Env::call_prepared()`] does (with the `guest`
/// feature, the host is trusted to write them the same way).
pub(crate) unsafe trait CallPrepared {
    /// Call `method`
    fn call_prepared(&self, method: PreparedMethod<'_>) -> Result<(), ContractError>;
}

// SAFETY: Executors write outputs and return values of methods that returned `Ok(())` the way
// `ExecutorContext` requires, and with the `guest` feature the host is trusted to do the same
unsafe impl CallPrepared for Env<'_> {
    #[inline(always)]
    fn call_prepared(&self, method: PreparedMethod<'_>) -> Result<(), ContractError> {
        Env::call_prepared(self, method)
    }
}

/// Memory for outputs of all methods, allocated one after another
#[derive(Debug)]
struct OutputBuffer<'a> {
    /// Pointer to the whole buffer. Every pointer to an output is derived from it, so using one of
    /// them doesn't invalidate the others.
    base: NonNull<u8>,
    /// Size of the buffer in bytes, at most `u32::MAX`, so that offsets fit into
    /// [`OutputBufferDetails`]
    size: usize,
    /// Number of bytes before the end of the last output
    used: usize,
    lifetime: PhantomData<&'a mut [MaybeUninit<u128>]>,
}

impl<'a> OutputBuffer<'a> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn new(buffer: &'a mut [MaybeUninit<u128>]) -> Self {
        debug_assert_eq!(align_of_val(buffer), usize::from(Alignment::MAX.get()));

        Self {
            size: size_of_val(buffer).min(u32::MAX as usize),
            base: NonNull::from_mut(buffer).cast::<u8>(),
            used: 0,
            lifetime: PhantomData,
        }
    }

    /// Allocate memory of `capacity` bytes with `alignment` after all memory allocated before.
    ///
    /// Returns its offset, or `None` if the buffer is not large enough.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn alloc(&mut self, capacity: u32, alignment: Alignment) -> Option<u32> {
        let alignment = usize::from(alignment.get());

        // Padding up to the next multiple of `alignment` is `-used` modulo `alignment`, because
        // `alignment` divides `2^usize::BITS`. `Alignment` tells the compiler that it is a power of
        // two, so `%` becomes a bit mask. The same padding written as
        // `(alignment - self.used % alignment) % alignment` takes more instructions.
        let padding_bytes = self.used.wrapping_neg() % alignment;

        let offset = self.used.checked_add(padding_bytes)?;
        let used = offset.checked_add(capacity as usize)?;

        if used > self.size {
            return None;
        }
        self.used = used;

        // Doesn't truncate, `offset` is at most `self.size`, which is at most `u32::MAX`
        Some(offset as u32)
    }

    /// Pointer to the memory at `offset`.
    ///
    /// # Safety
    /// `offset` must not exceed the size of the buffer, which is true for offsets returned by
    /// [`Self::alloc()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    unsafe fn pointer(&self, offset: u32) -> NonNull<u8> {
        // SAFETY: Guaranteed by the function contract
        unsafe { self.base.byte_add(offset as usize) }
    }
}

/// Size of an `#[output]` in `external_args_buffer`, which is read back after its method returned
/// `Ok(())`
#[derive(Debug, Copy, Clone)]
struct PendingOutputSize {
    output_index: u8,
    /// Offset of the size in bytes from the start of `external_args_buffer`
    external_args_offset: u8,
}

/// Sizes of the `#[output]`s of a method, with room for one per argument
type PendingOutputSizes = [MaybeUninit<PendingOutputSize>; MAX_TOTAL_METHOD_ARGS as usize];

/// Write to an external arguments pointer and move it forward.
///
/// # Safety
/// `external_args` must have enough capacity for the written value, and the current offset must
/// have the correct alignment for the type being written.
#[inline(always)]
unsafe fn write_external_args<T>(external_args: &mut NonNull<c_void>, value: T) {
    // SAFETY: guaranteed by this function signature
    unsafe {
        external_args.cast::<T>().write(value);
        *external_args = external_args.byte_add(size_of::<T>());
    }
}

/// Writes `ExternalArgs` of a method into `external_args_buffer` as its arguments are decoded
#[derive(Debug)]
struct ExternalArgsWriter<'output_buffer, 'a, 'pending> {
    /// Start of `external_args_buffer`
    external_args: NonNull<c_void>,
    /// Where the next argument is written to
    external_args_cursor: NonNull<c_void>,
    output_buffer: &'output_buffer mut OutputBuffer<'a>,
    /// Entries before `num_pending_output_sizes` are initialized.
    ///
    /// The array is borrowed, not owned. It is indexed at runtime, so as a field it would make the
    /// compiler keep the whole writer in memory instead of registers. The state of the caller,
    /// which the writer borrows, would then stay in memory too, and every argument would cost
    /// extra loads and stores.
    pending_output_sizes: &'pending mut PendingOutputSizes,
    /// Never exceeds the length of `pending_output_sizes`
    num_pending_output_sizes: usize,
}

impl<'payload> MethodSink<'payload> for ExternalArgsWriter<'_, '_, '_> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn slot(&mut self, slot: DecodedSlot<'payload>) -> Result<(), TransactionPayloadDecoderError> {
        let address = match slot {
            DecodedSlot::Address(address) => NonNull::from_ref(address),
            DecodedSlot::Output(output) => {
                // Methods of all outputs were called already, so their sizes are known
                check_slot_output_size(&output, output.size)?;

                // SAFETY: The core only gives back offsets that writers of the same caller
                // returned from `Self::output()`, which come from `OutputBuffer::alloc()`. They
                // are aligned as the payload declares for the output, which is at least the
                // alignment of an address for outputs used as slots (see `MethodSink::slot()`),
                // and the output buffer is aligned to `Alignment::MAX`.
                let address =
                    unsafe { self.output_buffer.pointer(output.offset) }.cast::<Address>();
                debug_assert!(address.is_aligned());

                address
            }
        };

        // SAFETY: The core gives at most `MAX_TOTAL_METHOD_ARGS` arguments per method to the sink,
        // which `external_args_buffer` is sized for, and the address is aligned for a pointer
        unsafe {
            write_external_args(
                &mut self.external_args_cursor,
                address.as_ptr().cast_const(),
            );
        }

        Ok(())
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn input(&mut self, input: DecodedInput<'payload>) {
        let (data_ptr, size) = match input {
            DecodedInput::Value {
                bytes,
                alignment: _,
            } => (
                NonNull::from_ref(bytes).as_non_null_ptr(),
                // Doesn't truncate, the size was read from the payload as `u32`
                bytes.len() as u32,
            ),
            DecodedInput::Output(output) => (
                // SAFETY: The core only gives back offsets that writers of the same caller
                // returned from `Self::output()`, which come from `OutputBuffer::alloc()`
                unsafe { self.output_buffer.pointer(output.offset) },
                output.size,
            ),
        };

        // SAFETY: The core gives at most `MAX_TOTAL_METHOD_ARGS` arguments per method to the sink,
        // which `external_args_buffer` is sized for, and the buffer is correctly aligned
        unsafe {
            write_external_args(
                &mut self.external_args_cursor,
                FfiDataSizeCapacityRo {
                    data_ptr,
                    size,
                    capacity: size,
                },
            );
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn output(&mut self, output: DecodedOutput) -> Result<u32, TransactionPayloadDecoderError> {
        let offset = self
            .output_buffer
            .alloc(output.capacity, output.alignment)
            .ok_or(TransactionPayloadDecoderError::OutputBufferTooSmall)?;

        // The size of a return value is known already. There is always room for the size, since
        // the core gives at most `MAX_TOTAL_METHOD_ARGS` arguments per method to the sink.
        if matches!(output.kind, TransactionOutputKind::Output)
            && let Some(pending_output_size) = self
                .pending_output_sizes
                .get_mut(self.num_pending_output_sizes)
        {
            // SAFETY: `external_args_cursor` is created from `external_args` and is within the
            // same allocation
            let external_args_offset = unsafe {
                self.external_args_cursor
                    .byte_add(offset_of!(FfiDataSizeCapacityRw, size))
                    .byte_offset_from_unsigned(self.external_args)
            };
            pending_output_size.write(PendingOutputSize {
                output_index: output.output_index,
                // Doesn't truncate, `external_args_buffer` has at most `u8::MAX` bytes
                external_args_offset: external_args_offset as u8,
            });
            self.num_pending_output_sizes += 1;
        }

        // SAFETY: Offset was just returned by `OutputBuffer::alloc()`
        let data_ptr = unsafe { self.output_buffer.pointer(offset) }.as_ptr();
        // SAFETY: The core gives at most `MAX_TOTAL_METHOD_ARGS` arguments per method to the sink,
        // which `external_args_buffer` is sized for, and the buffer is correctly aligned
        unsafe {
            write_external_args(
                &mut self.external_args_cursor,
                FfiDataSizeCapacityRw {
                    data_ptr,
                    size: output.size,
                    capacity: output.capacity,
                },
            );
        }

        Ok(offset)
    }
}

impl<'output_buffer, 'a, 'pending> ExternalArgsWriter<'output_buffer, 'a, 'pending> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn new(
        external_args_buffer: &mut [*mut c_void; EXTERNAL_ARGS_BUFFER_SIZE],
        output_buffer: &'output_buffer mut OutputBuffer<'a>,
        pending_output_sizes: &'pending mut PendingOutputSizes,
    ) -> Self {
        let external_args = NonNull::from_mut(external_args_buffer).cast::<c_void>();

        Self {
            external_args,
            external_args_cursor: external_args,
            output_buffer,
            pending_output_sizes,
            num_pending_output_sizes: 0,
        }
    }

    /// Sizes of `#[output]` arguments of the method
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn pending_output_sizes(&self) -> &[PendingOutputSize] {
        // SAFETY: Entries before `num_pending_output_sizes` are initialized, and it never exceeds
        // the length of `pending_output_sizes`
        unsafe {
            self.pending_output_sizes
                .get_unchecked(..self.num_pending_output_sizes)
                .assume_init_ref()
        }
    }
}

/// Caller of methods in a transaction payload created using `TransactionPayloadBuilder`.
///
/// The payload must be trusted (see [`SimpleWalletBase::execute()`]): what it declares about the
/// arguments of each method, such as the capacity of the return value, which must be the size of
/// the return type, can't be checked without the method's metadata.
/// [`TransactionPayloadDecoder`] applies the same rules without calling methods, so a payload can
/// be checked and shown to the user before it is signed.
///
/// Outputs are given to methods empty, with size `0`, so methods called from transactions must
/// accept empty outputs, see `TransactionPayloadBuilder::with_method_call()`.
///
/// Return values are given their size when a method is decoded, before it is called, since
/// executors only write the value (see [`TransactionOutput`]). Each method is called right after
/// it is decoded and calling stops at the first method that fails, so later methods can only refer
/// to return values of methods that succeeded and wrote them.
///
/// [`SimpleWalletBase::execute()`]: crate::SimpleWalletBase::execute
/// [`TransactionPayloadDecoder`]: crate::payload::decoder::TransactionPayloadDecoder
/// [`TransactionOutput`]: crate::payload::TransactionOutput
#[derive(Debug)]
pub struct TransactionPayloadCaller<'a> {
    core: PayloadDecoderCore<'a, 'a>,
    external_args_buffer: &'a mut [*mut c_void; EXTERNAL_ARGS_BUFFER_SIZE],
    output_buffer: OutputBuffer<'a>,
    map_context: fn(TransactionMethodContext) -> MethodContext,
}

impl<'a> TransactionPayloadCaller<'a> {
    /// Create a new instance.
    ///
    /// The size of `external_args_buffer` defines the max number of bytes allocated for
    /// `ExternalArgs`, which impacts the number of arguments that can be represented by
    /// `ExternalArgs`. The size is specified in pointers with `#[slot]` argument using one
    /// pointer, while `#[input]` and `#[output]` use one pointer and size + capacity each.
    ///
    /// The size of `output_buffer` defines how big the total size of `#[output]` and return values
    /// could be in all methods of the payload together.
    ///
    /// The size of `output_buffer_details` defines how many `#[output]` arguments and return values
    /// could exist in all methods of the payload together, up to 128, since tags refer to output
    /// indices up to 127.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn new(
        payload: &'a [u128],
        external_args_buffer: &'a mut [*mut c_void; EXTERNAL_ARGS_BUFFER_SIZE],
        output_buffer: &'a mut [MaybeUninit<u128>],
        output_buffer_details: &'a mut [MaybeUninit<OutputBufferDetails>],
        map_context: fn(TransactionMethodContext) -> MethodContext,
    ) -> Self {
        Self {
            core: PayloadDecoderCore::new(payload, output_buffer_details),
            external_args_buffer,
            output_buffer: OutputBuffer::new(output_buffer),
            map_context,
        }
    }

    /// Decode the next method (if any) in the payload and call it.
    ///
    /// Returns the caller for the remaining methods if the method succeeded, or `None` if there
    /// are no more methods in the payload. The caller is consumed when decoding or the method
    /// fails, since later methods might refer to return values that the failed method didn't
    /// write.
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn call_next_method(
        self,
        env: &Env<'_>,
    ) -> Result<Option<Self>, TransactionPayloadCallError> {
        self.call_next_method_with(env)
    }

    /// The same as [`Self::call_next_method()`], but with `env` that can stand in for [`Env`]
    #[inline(always)]
    pub(crate) fn call_next_method_with<Environment>(
        mut self,
        env: &Environment,
    ) -> Result<Option<Self>, TransactionPayloadCallError>
    where
        Environment: CallPrepared,
    {
        // Outside the writer, see `ExternalArgsWriter::pending_output_sizes`
        let mut pending_output_sizes: PendingOutputSizes =
            [MaybeUninit::uninit(); MAX_TOTAL_METHOD_ARGS as usize];
        let mut writer = ExternalArgsWriter::new(
            &mut *self.external_args_buffer,
            &mut self.output_buffer,
            &mut pending_output_sizes,
        );
        let maybe_method_header = self.core.decode_next_method(&mut writer);
        let Some(method_header) =
            maybe_method_header.map_err(TransactionPayloadCallError::Decoder)?
        else {
            return Ok(None);
        };

        // SAFETY: Bounds and alignment are checked by the core and the writer, and for as long as
        // the returned value borrows the caller:
        // * `external_args_buffer` starts with arguments written by the writer in the order and
        //   layout of `ExternalArgs` for the slots, inputs and outputs the payload declares: a
        //   pointer for each slot, then pointer, size and capacity for each input and output, where
        //   a return value is written like the last output, with size and capacity after its
        //   pointer that `ExternalArgs` doesn't have
        // * slot addresses are aligned, and slot addresses and inputs point into the payload or
        //   into outputs of earlier methods up to their sizes, which executors keep within
        //   capacities with that many bytes initialized, as `ExecutorContext` requires, while the
        //   size of a return value of an earlier method is its capacity, the size of the return
        //   type, which has no padding (`TrivialType`): every earlier method returned `Ok(())`,
        //   since the caller is consumed otherwise, so it wrote its outputs and return value as
        //   `CallPrepared` requires, sizes of outputs are only stored after that, and slots and
        //   inputs only refer to outputs of earlier methods (see `MethodSink`)
        // * outputs are allocated in the output buffer with their capacity after all memory
        //   allocated before, so they don't overlap each other or outputs of earlier methods, while
        //   the payload and `external_args_buffer` are borrowed separately from the output buffer
        // * pointers into the output buffer are all derived from one pointer to the whole buffer,
        //   so creating one doesn't invalidate those created before
        // * the payload, `external_args_buffer` and the output buffer stay borrowed by the caller,
        //   so nothing else accesses them, and the payload and outputs of earlier methods are only
        //   read
        // * contents of outputs and return values are only given to later methods as slot
        //   addresses, which accept every bit pattern, and as inputs, which methods parse
        // * the payload is trusted to declare the arguments of the method with its fingerprint and
        //   the size and alignment of the return type as the capacity and alignment of its return
        //   value, and to declare as the return value only one that executors write (not the state
        //   returned by `#[init]`, which they store instead), which can't be checked without the
        //   method's metadata, the same way `NativeExecutor` trusts transaction payloads (see
        //   `SimpleWalletBase::execute()` for what makes them trusted)
        let prepared_method = unsafe {
            PreparedMethod::new_untyped(
                *method_header.contract,
                *method_header.fingerprint,
                &mut *self.external_args_buffer,
                (self.map_context)(method_header.context),
            )
        };

        env.call_prepared(prepared_method)
            .map_err(TransactionPayloadCallError::Method)?;

        for pending_output_size in writer.pending_output_sizes() {
            // SAFETY: The offset points to the size of an `#[output]` within
            // `external_args_buffer`, which is aligned for `u32`. It is read through a new pointer,
            // since the one the writer has was created before the buffer was borrowed for the
            // call.
            let size = unsafe {
                NonNull::from_ref(&*self.external_args_buffer)
                    .cast::<u8>()
                    .byte_add(usize::from(pending_output_size.external_args_offset))
                    .cast::<u32>()
                    .read()
            };
            // SAFETY: The writer records only outputs that the core gave to it while decoding
            // this method, which succeeded, so the output exists. The method returned `Ok(())`, so
            // it wrote a size within the capacity of the output with that many bytes initialized,
            // as `CallPrepared` requires.
            unsafe {
                self.core
                    .set_output_size(pending_output_size.output_index, size);
            }
        }

        Ok(Some(self))
    }
}
