//! Calling methods of a payload one by one

#[cfg(test)]
mod tests;

use crate::payload::decoder::{DecodedInput, DecodedOutput, DecodedSlot};
use crate::payload::decoder_core::{
    MethodSink, OutputAllocator, PayloadDecoderCore, check_slot_output_size,
};
use crate::payload::{
    DeclaredOutput, FfiDataSizeCapacityRo, FfiDataSizeCapacityRw, TransactionMethodContext,
    TransactionOutputKind, TransactionPayloadDecoderError,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, MAX_OUTPUTS};
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
    // Outputs are allocated at offsets that are multiples of `Alignment::MAX`, so their memory is
    // aligned to `Alignment::MAX` when the output buffer is, which makes it aligned for every
    // output type
    assert!(align_of::<OutputChunk>() >= usize::from(Alignment::MAX.get()));
}

/// Element of the output buffer of [`TransactionPayloadCaller`]: 16 bytes aligned to
/// [`Alignment::MAX`]
#[derive(Debug, Copy, Clone)]
#[repr(C, align(16))]
pub struct OutputChunk([u8; 16]);

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
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn call_prepared(&self, method: PreparedMethod<'_>) -> Result<(), ContractError> {
        Env::call_prepared(self, method)
    }
}

/// Allocates the memory of declared outputs one after another in an output buffer
#[derive(Debug)]
struct OutputBufferAllocator {
    /// Size of the output buffer in bytes, at most `u32::MAX`, so that offsets fit into
    /// [`DeclaredOutput`]
    size: usize,
    /// Number of bytes before the end of the last output
    used: usize,
}

impl OutputAllocator for OutputBufferAllocator {
    /// Allocate memory of `capacity` bytes after all memory allocated before, at the next multiple
    /// of [`Alignment::MAX`].
    ///
    /// [`Alignment::MAX`] is the largest alignment of any type that methods accept, so the memory
    /// is aligned for every output type.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn alloc(&mut self, capacity: u32) -> Result<u32, TransactionPayloadDecoderError> {
        let offset = self
            .used
            .checked_next_multiple_of(usize::from(Alignment::MAX.get()))
            .ok_or(TransactionPayloadDecoderError::OutputBufferTooSmall)?;
        let used = offset
            .checked_add(capacity as usize)
            .ok_or(TransactionPayloadDecoderError::OutputBufferTooSmall)?;

        if used > self.size {
            return Err(TransactionPayloadDecoderError::OutputBufferTooSmall);
        }
        self.used = used;

        // Doesn't truncate, `offset` is at most `self.size`, which is at most `u32::MAX`
        Ok(offset as u32)
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
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
unsafe fn write_external_args<T>(external_args: &mut NonNull<c_void>, value: T) {
    // SAFETY: guaranteed by this function signature
    unsafe {
        external_args.cast::<T>().write(value);
        *external_args = external_args.byte_add(size_of::<T>());
    }
}

/// Writes `ExternalArgs` of a method into `external_args_buffer` as its arguments are decoded
#[derive(Debug)]
struct ExternalArgsWriter<'pending> {
    /// Start of `external_args_buffer`
    external_args: NonNull<c_void>,
    /// Where the next argument is written to
    external_args_cursor: NonNull<c_void>,
    /// Start of the output buffer, see [`TransactionPayloadCaller`]
    output_buffer: NonNull<u8>,
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

impl<'payload> MethodSink<'payload> for ExternalArgsWriter<'_> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn slot(&mut self, slot: DecodedSlot<'payload>) -> Result<(), TransactionPayloadDecoderError> {
        let address = match slot {
            DecodedSlot::Address(address) => NonNull::from_ref(address),
            DecodedSlot::Output(output) => {
                // Methods that wrote outputs were called already, so the size of an `#[output]` is
                // known. A return value, whose size the core checked already, is checked again, so
                // that this is a single comparison.
                check_slot_output_size(&output)?;

                // SAFETY: Declared outputs are allocated within the output buffer in
                // `TransactionPayloadCaller::new()` at multiples of `Alignment::MAX`, the output
                // buffer is aligned to `Alignment::MAX` by its element type, and an address, like
                // every `TrivialType`, has at most that alignment
                let address = unsafe { self.output_pointer(output.offset) }.cast::<Address>();
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
                // SAFETY: Declared outputs are allocated within the output buffer in
                // `TransactionPayloadCaller::new()`
                unsafe { self.output_pointer(output.offset) },
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
    fn output(&mut self, output: DecodedOutput, offset: u32) {
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

        // SAFETY: Declared outputs are allocated within the output buffer in
        // `TransactionPayloadCaller::new()`, which is where `offset` comes from
        let data_ptr = unsafe { self.output_pointer(offset) }.as_ptr();
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
    }
}

impl<'pending> ExternalArgsWriter<'pending> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn new(
        external_args_buffer: &mut [*mut c_void; EXTERNAL_ARGS_BUFFER_SIZE],
        output_buffer: NonNull<u8>,
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

    /// Pointer to the memory at `offset` in the output buffer.
    ///
    /// # Safety
    /// `offset` must not exceed the size of the output buffer, which is true for offsets of
    /// declared outputs.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    unsafe fn output_pointer(&self, offset: u32) -> NonNull<u8> {
        // SAFETY: Guaranteed by the function contract
        unsafe { self.output_buffer.byte_add(offset as usize) }
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

/// Errors for [`TransactionPayloadCaller::call_next_method()`]
#[derive(Debug, thiserror::Error)]
pub enum TransactionPayloadCallError {
    /// Decoder error
    #[error("Decoder error: {0}")]
    Decoder(TransactionPayloadDecoderError),
    /// Method call error
    #[error("Method call error: {0}")]
    Method(ContractError),
}

/// Caller of methods in a transaction payload created using `TransactionPayloadBuilder`.
///
/// The payload must be trusted (see [`SimpleWalletBase::execute()`]): what it declares about the
/// arguments of each method can't be checked without the method's metadata. The size of a return
/// value must be the size of the return type, and the capacity of a declared output must be enough
/// for the type of every `#[output]` that writes it.
/// [`TransactionPayloadDecoder`] applies the same rules without calling methods, so a payload can
/// be checked and shown to the user before it is signed.
///
/// All outputs that the payload declares are allocated in the output buffer when the caller is
/// created, so a payload whose outputs don't fit is rejected before any method is called.
///
/// Outputs are given to methods empty, with size `0`, whatever was written into their memory
/// before, so methods called from transactions must accept empty outputs, see
/// `TransactionPayloadBuilder::with_method_call()`.
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
    /// Start of the output buffer. Every pointer to an output is derived from it, so using one of
    /// them doesn't invalidate the others.
    output_buffer: NonNull<u8>,
    output_buffer_lifetime: PhantomData<&'a mut [MaybeUninit<OutputChunk>]>,
    map_context: fn(TransactionMethodContext) -> MethodContext,
}

impl<'a> TransactionPayloadCaller<'a> {
    /// Create a new instance, read the outputs that the payload declares, at most
    /// [`MAX_OUTPUTS`], into `declared_outputs` and allocate them in `output_buffer`.
    ///
    /// The size of `external_args_buffer` defines the max number of bytes allocated for
    /// `ExternalArgs`, which impacts the number of arguments that can be represented by
    /// `ExternalArgs`. The size is specified in pointers with `#[slot]` argument using one
    /// pointer, while `#[input]` and `#[output]` use one pointer and size + capacity each.
    ///
    /// The size of `output_buffer` defines how large the outputs that the payload declares can be
    /// together. Each of them starts at the next multiple of [`Alignment::MAX`] after the previous
    /// one.
    ///
    /// Returns an error if the payload is not aligned to [`Alignment::MAX`], or if the outputs that
    /// the payload declares can't be read or don't fit, so that no method of such a payload is
    /// called. Every payload starts with the outputs that it declares, so an empty payload is too
    /// small.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn new(
        payload: &'a [u128],
        external_args_buffer: &'a mut [*mut c_void; EXTERNAL_ARGS_BUFFER_SIZE],
        output_buffer: &'a mut [MaybeUninit<OutputChunk>],
        declared_outputs: &'a mut [MaybeUninit<DeclaredOutput>; MAX_OUTPUTS],
        map_context: fn(TransactionMethodContext) -> MethodContext,
    ) -> Result<Self, TransactionPayloadDecoderError> {
        let mut allocator = OutputBufferAllocator {
            size: size_of_val(output_buffer).min(u32::MAX as usize),
            used: 0,
        };

        Ok(Self {
            core: PayloadDecoderCore::new(payload, declared_outputs, &mut allocator)?,
            external_args_buffer,
            output_buffer: NonNull::from_mut(output_buffer).cast::<u8>(),
            output_buffer_lifetime: PhantomData,
            map_context,
        })
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
    // Not annotated with `no_panic`, since tests call it with stand-ins for `Env` that panic. Its
    // instance for `Env` is checked through `Self::call_next_method()`.
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
            self.output_buffer,
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
        //   into declared outputs, up to the size of what an earlier method wrote into them last:
        //   the size that a method wrote into an `#[output]`, which executors keep within the
        //   capacity with that many bytes initialized, as `ExecutorContext` requires, or the size
        //   that the payload declares for a return value, which is the size of the return type, has
        //   no padding (`TrivialType`) and doesn't exceed the capacity (the core checks it): every
        //   earlier method returned `Ok(())`, since the caller is consumed otherwise, so it wrote
        //   its outputs and return value as `CallPrepared` requires, sizes of `#[output]`s are only
        //   stored after that, and slots and inputs only refer to declared outputs that earlier
        //   methods wrote (see `MethodSink`)
        // * a slot only refers to a declared output whose size is that of an address, which the
        //   writer checks for every slot that refers to a declared output
        // * an `#[output]` is given size `0` whatever its memory held before, and a return value
        //   replaces the size of what its memory held before, so later methods only read bytes that
        //   the last write initialized
        // * declared outputs are allocated one after another with their capacities in
        //   `Self::new()`, so they don't overlap each other (one with capacity `0` has no memory
        //   that could overlap), while the payload and `external_args_buffer` are borrowed
        //   separately from the output buffer
        // * the outputs of a method are different declared outputs, and its slots and inputs don't
        //   refer to declared outputs that it writes (see `MethodSink`), so the memory that a
        //   method writes doesn't overlap with any other argument, while slots and inputs may share
        //   a declared output, which they only read
        // * the payload is trusted to declare the size of the return type as the size of the return
        //   value, and the core checked that it doesn't exceed the capacity of its declared output,
        //   so executors write the return value within that declared output, while otherwise they
        //   could write into other declared outputs, including ones that the same method reads
        // * declared outputs are allocated at multiples of `Alignment::MAX` in the output buffer,
        //   which is aligned to `Alignment::MAX` too, and no type that methods accept has a larger
        //   alignment, so the memory of every output is aligned for its type
        // * pointers into the output buffer are all derived from one pointer to the whole buffer,
        //   so creating one doesn't invalidate those created before
        // * the payload, `external_args_buffer` and the output buffer stay borrowed by the caller,
        //   so nothing else accesses them, and the payload and outputs of earlier methods are only
        //   read
        // * contents of outputs and return values are only given to later methods as slot
        //   addresses, which accept every bit pattern, and as inputs, which methods parse
        // * the payload is trusted to declare the arguments of the method with its fingerprint, and
        //   to declare as the return value only one that executors write (not the state returned by
        //   `#[init]`, which they store instead), which can't be checked without the method's
        //   metadata, the same way `NativeExecutor` trusts transaction payloads (see
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
