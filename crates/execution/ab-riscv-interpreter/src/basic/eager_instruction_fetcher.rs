//! Instruction fetcher that walks a program decoded upfront

#[cfg(test)]
mod tests;

use crate::{
    Address, ExecutionError, FetchInstructionResult, InstructionFetcher, PackedAddress,
    ProgramCounter, VirtualMemory, VirtualMemoryError,
};
use ab_riscv_primitives::prelude::*;
use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use core::alloc::Layout;
use core::hint::cold_path;
use core::marker::PhantomData;
use core::ops::ControlFlow;
use core::ptr::NonNull;
use core::{fmt, mem};

/// Everything [`BasicEagerInstructionFetcher`] needs besides its position within the decoded
/// instruction stream.
///
/// This lives in a single heap allocation whose tail holds the decoded instructions themselves,
/// starting `BasicEagerInstructions::INSTRUCTIONS_OFFSET` bytes from the beginning of it. That is
/// what keeps the fetcher itself down to two pointers, so it fits into two argument registers when
/// threaded through tail-called instruction handlers by value.
#[derive(Debug)]
#[repr(C)]
struct BasicEagerInstructionFetcherState<I>
where
    I: Instruction,
{
    /// Number of decoded instructions stored right after this header
    instructions_len: usize,
    /// Size of the decoded instructions stored right after this header, in bytes.
    ///
    /// The same information as `instructions_len`, kept in the unit a relative branch measures
    /// its target in, so that the bounds check on every taken branch is a single comparison
    /// rather than a multiplication and a comparison. Deriving either from the other at run time
    /// would cost an instruction on a hot path, hence both are stored.
    instructions_size: usize,
    /// Guest address that corresponds to the first decoded instruction
    base_addr: Address<I>,
    /// Guest address at which execution stops gracefully
    return_trap_address: Address<I>,
}

/// Instructions decoded upfront, which [`BasicEagerInstructionFetcher`] walks.
///
/// Decoding a program once instead of on every fetch is what makes this faster than
/// [`BasicInstructionFetcher`](super::BasicInstructionFetcher), at the cost of holding the whole
/// decoded program in memory and of not seeing writes the program makes to the memory it was
/// decoded from.
///
/// The decoded stream has one slot per [`Instruction::ALIGNMENT`] bytes of guest code, which is
/// the granularity at which an instruction of this instruction set can start, and what makes an
/// address a position within the stream and back. With compressed instructions that is a halfword,
/// so the second half of a 32-bit instruction gets a slot of its own, holding whatever those bytes
/// decode to, which is only ever reached by jumping into the middle of an instruction. Without
/// them, no address in the middle of an instruction is aligned in the first place, so there is
/// nothing to hold a slot for and the stream is half the size.
///
/// Ownership of the allocation lives here rather than in the fetcher because the fetcher is moved
/// through tail-called instruction handlers by value. A destructor on it would make every handler
/// that can fail (every load, store, branch and jump) responsible for dropping it on the way out,
/// which costs a stack frame, callee-saved register spills and a reload in the hot path of each of
/// them, even though the failing path is never taken.
pub struct BasicEagerInstructions<I>
where
    I: Instruction,
{
    /// State header, together with the decoded instructions themselves, in a single heap
    /// allocation.
    ///
    /// This is a raw pointer rather than a `Box` on purpose: fetchers point into the same
    /// allocation, and going through a `Box` would assert unique access to that allocation on
    /// every use, invalidating pointers that must survive across all of them.
    state: NonNull<BasicEagerInstructionFetcherState<I>>,
}

impl<I> fmt::Debug for BasicEagerInstructions<I>
where
    I: Instruction,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BasicEagerInstructions")
            .field("instructions_len", &self.instructions_len())
            .field("base_addr", &self.base_addr())
            .field("return_trap_address", &self.return_trap_address())
            .finish_non_exhaustive()
    }
}

impl<I> Drop for BasicEagerInstructions<I>
where
    I: Instruction,
{
    fn drop(&mut self) {
        let layout = Self::allocation_layout(self.instructions_len());

        // SAFETY: Allocated with the global allocator using exactly this layout, and this is the
        // only owner of the allocation
        unsafe {
            dealloc(self.state.as_ptr().cast::<u8>(), layout);
        }
    }
}

impl<I> BasicEagerInstructions<I>
where
    I: Instruction,
{
    /// Byte offset of the decoded instructions from the start of the allocation that
    /// [`Self::state`] points at
    const INSTRUCTIONS_OFFSET: usize =
        size_of::<BasicEagerInstructionFetcherState<I>>().next_multiple_of(align_of::<I>());
    /// Bytes of guest code that one slot of the decoded stream corresponds to.
    ///
    /// This is what turns a guest address into a position within the decoded stream and back, so
    /// an instruction set whose slot is not a whole number of alignment steps has no such mapping
    /// and is refused right here, at compile time.
    const GUEST_BYTES_PER_SLOT: usize = {
        assert!(
            size_of::<I>().is_multiple_of(usize::from(I::ALIGNMENT)),
            "Decoded instruction size must be a multiple of instruction alignment"
        );

        usize::from(I::ALIGNMENT)
    };
    /// Bytes of the decoded stream that one byte of guest code covers
    const STREAM_BYTES_PER_GUEST_BYTE: usize = size_of::<I>() / Self::GUEST_BYTES_PER_SLOT;

    /// Layout of the allocation holding [`BasicEagerInstructionFetcherState`] followed by
    /// `instructions_len` decoded instructions
    fn allocation_layout(instructions_len: usize) -> Layout {
        let (layout, instructions_offset) = Layout::new::<BasicEagerInstructionFetcherState<I>>()
            .extend(Layout::array::<I>(instructions_len).expect(
                "Decoded stream that doesn't fit into the address space can't be allocated \
                anyway; qed",
            ))
            .expect(
                "Decoded stream that doesn't fit into the address space can't be allocated \
                anyway; qed",
            );

        debug_assert_eq!(instructions_offset, Self::INSTRUCTIONS_OFFSET);

        layout.pad_to_align()
    }

    /// Pointer to the first decoded instruction
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn instructions(&self) -> NonNull<I> {
        // SAFETY: Decoded instructions are stored at this offset of the same allocation as the
        // state
        unsafe { self.state.byte_add(Self::INSTRUCTIONS_OFFSET) }.cast::<I>()
    }

    /// Number of decoded instructions
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn instructions_len(&self) -> usize {
        // SAFETY: State is initialized in the constructor and valid for as long as `self` is
        unsafe { (*self.state.as_ptr()).instructions_len }
    }

    /// Guest address that corresponds to the first decoded instruction
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn base_addr(&self) -> Address<I> {
        // SAFETY: State is initialized in the constructor and valid for as long as `self` is
        unsafe { (*self.state.as_ptr()).base_addr }
    }

    /// Guest address at which execution stops gracefully
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn return_trap_address(&self) -> Address<I> {
        // SAFETY: State is initialized in the constructor and valid for as long as `self` is
        unsafe { (*self.state.as_ptr()).return_trap_address }
    }

    /// Create a fetcher positioned at the instruction that guest address `pc` corresponds to
    ///
    /// # Safety
    /// `pc` must be the address of one of the instructions [`Self::decode()`] was given, meaning
    /// it is within `base_addr..base_addr + instructions.len()` and is a multiple of
    /// [`Instruction::ALIGNMENT`], with `base_addr` and `instructions` being what that call
    /// received.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub unsafe fn fetcher(&self, pc: Address<I>) -> BasicEagerInstructionFetcher<'_, I> {
        const {
            // When fetcher is used with threaded dispatch, it must fit into two argument registers
            // to be passed through tail calls
            assert!(
                size_of::<BasicEagerInstructionFetcher<'_, I>>() == size_of::<NonNull<I>>() * 2,
                "`BasicEagerInstructionFetcher` must be two pointers large"
            );
            // Drop glue on the fetcher would force a stack frame into every fallible handler, see
            // `BasicEagerInstructions` for details
            assert!(
                !mem::needs_drop::<BasicEagerInstructionFetcher<'_, I>>(),
                "`BasicEagerInstructionFetcher` must not have drop glue"
            );
        }

        let instruction_offset =
            (pc.as_u64() - self.base_addr().as_u64()) as usize / Self::GUEST_BYTES_PER_SLOT;

        BasicEagerInstructionFetcher {
            // SAFETY: Guaranteed by function contract, meaning `instruction_offset` is within
            // bounds of the decoded stream
            next_instruction: unsafe { self.instructions().add(instruction_offset) },
            instructions: self.instructions(),
            _instructions: PhantomData,
        }
    }

    /// Decode `instructions` and create a new instance holding the result.
    ///
    /// `base_addr` is the guest address of the first instruction and `return_trap_address` is the
    /// address at which the interpreter will stop execution (gracefully).
    ///
    /// Every [`Instruction::ALIGNMENT`] bytes of guest code own a slot of the decoded stream,
    /// including, where instructions may be compressed, the second half of a 32-bit instruction,
    /// which is only ever reached by jumping into the middle of one. Such a slot may or may not
    /// decode into a valid instruction on its own, and `fallback` is what is stored when it
    /// doesn't, so it only has to fail when executed (`unimp` is the canonical choice).
    ///
    /// # Safety
    /// Execution of the resulting instruction stream skips the checks that
    /// [`BasicInstructionFetcher`](super::BasicInstructionFetcher) does, which is where the
    /// performance comes from. All of the following must hold:
    /// * The instructions must end with an unconditional jump, so that execution can't fall through
    ///   past the end of the decoded stream. Instruction fetching does not bounds-check the
    ///   position, only [`ProgramCounter::set_pc()`] and [`ProgramCounter::try_set_pc_relative()`]
    ///   do, which means the last instruction must be one that goes through them.
    /// * `return_trap_address` must not fall inside the instructions. Instruction fetching does not
    ///   compare against the return trap, so an address inside them would stop execution when
    ///   jumped to, but not when reached by falling through.
    /// * `base_addr` must be a multiple of [`Instruction::ALIGNMENT`], since it is the address of
    ///   the first decoded instruction, and every position within the decoded stream is resolved
    ///   relative to it.
    /// * `base_addr + instructions.len()` must not overflow the address space, which is what makes
    ///   the address of every decoded instruction representable.
    /// * The memory the program executes with must contain these very instructions at `base_addr`,
    ///   and the program must not modify them (there is no `Zifencei` support here). The decoded
    ///   stream is a snapshot taken here, and it is what execution walks, so writes into the code
    ///   region are not reflected in what is executed.
    pub unsafe fn decode(
        instructions: &[u8],
        fallback: I,
        return_trap_address: Address<I>,
        base_addr: Address<I>,
    ) -> Self {
        // Exactly as many slots as there are whole alignment steps of guest code, trailing bytes
        // that do not make up one have nothing to decode into
        let instructions_len = instructions.len() / Self::GUEST_BYTES_PER_SLOT;
        let layout = Self::allocation_layout(instructions_len);
        // SAFETY: The state itself is always there, so the layout has non-zero size
        let state = unsafe { alloc(layout) }.cast::<BasicEagerInstructionFetcherState<I>>();
        let Some(state) = NonNull::new(state) else {
            handle_alloc_error(layout);
        };

        // SAFETY: Freshly allocated for exactly this type, correctly aligned
        unsafe {
            state.write(BasicEagerInstructionFetcherState {
                instructions_len,
                // Does not overflow, the layout above was just computed from it
                instructions_size: instructions_len * size_of::<I>(),
                base_addr,
                return_trap_address,
            });
        }

        // The decoded instructions are uninitialized until the loop below writes every one of them,
        // and nothing reads them in between. Instructions are `Copy`, so even dropping the instance
        // in that state would just deallocate.
        let instance = Self { state };
        let decoded_instructions = instance.instructions();

        for slot_index in 0..instructions_len {
            let offset = slot_index * Self::GUEST_BYTES_PER_SLOT;
            let instruction = Self::decode_instruction(instructions, offset, fallback);

            // SAFETY: The allocation was made for exactly `instructions_len` instructions, and
            // this writes each of them once
            unsafe {
                decoded_instructions.add(slot_index).write(instruction);
            }
        }

        instance
    }

    /// Decode the instruction that the decoded stream's slot starting `offset` bytes into
    /// `instructions` holds.
    ///
    /// The caller iterates over exactly the slots that whole alignment steps of guest code make up,
    /// so there is always at least one such step left at `offset`.
    ///
    /// This is where all of the decoding lives, so that what remains of [`Self::decode()`] is the
    /// allocation, which is the only part of it that can't be proven panic-free.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn decode_instruction(instructions: &[u8], offset: usize, fallback: I) -> I {
        let instruction = match instructions.get(offset..) {
            Some([byte_0, byte_1, byte_2, byte_3, ..]) => {
                u32::from_le_bytes([*byte_0, *byte_1, *byte_2, *byte_3])
            }
            // Only reachable where instructions may be compressed: the last halfword of guest code
            // has nothing following it to read, so it is zero-extended into a word, which decodes
            // only if it is a compressed instruction
            Some([byte_0, byte_1, ..]) => u32::from_le_bytes([*byte_0, *byte_1, 0, 0]),
            // Not reachable through the above, and a slot with less than a halfword of guest code
            // has nothing that could decode anyway
            _ => {
                return fallback;
            }
        };

        I::try_decode(instruction).unwrap_or(fallback)
    }
}

/// Eager instruction fetcher walks instructions that [`BasicEagerInstructions`] decoded upfront.
///
/// This is a plain `Copy` cursor without a destructor, see [`BasicEagerInstructions`] for why.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct BasicEagerInstructionFetcher<'a, I>
where
    I: Instruction,
{
    /// The instruction to be returned by the next [`InstructionFetcher::fetch_instruction()`]
    /// call.
    ///
    /// A pointer rather than an offset helps LLVM with SROA and aliasing analysis, so it can
    /// retain this in a native register instead of recomputing it from an offset on every
    /// fetch.
    next_instruction: NonNull<I>,
    /// The first decoded instruction, borrowed from [`BasicEagerInstructions`].
    ///
    /// This points at the instructions rather than at the state header in front of them because
    /// the instructions are what every branch and jump measures its target against, while the
    /// header is read at a constant offset that the addressing mode absorbs. Pointing at the
    /// header instead costs the taken path of every branch an addition to find the instructions.
    instructions: NonNull<I>,
    /// Fetcher borrows the decoded instructions it walks
    _instructions: PhantomData<&'a BasicEagerInstructions<I>>,
}

impl<I> fmt::Debug for BasicEagerInstructionFetcher<'_, I>
where
    I: Instruction,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BasicEagerInstructionFetcher")
            .field("next_instruction", &self.next_instruction)
            .field("instructions_len", &self.instructions_len())
            .field("base_addr", &self.base_addr())
            .field("return_trap_address", &self.return_trap_address())
            .finish_non_exhaustive()
    }
}

impl<I, Memory> ProgramCounter<Address<I>, Memory> for BasicEagerInstructionFetcher<'_, I>
where
    I: Instruction,
    Memory: VirtualMemory,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn get_pc(&self) -> Address<I> {
        let decoded_instruction_byte_offset = self
            .next_instruction
            .as_ptr()
            .addr()
            .wrapping_sub(self.instructions.as_ptr().addr());

        Address::<I>::truncate_from_u64(
            self.base_addr().as_u64()
                + (decoded_instruction_byte_offset
                    / BasicEagerInstructions::<I>::STREAM_BYTES_PER_GUEST_BYTE)
                    as u64,
        )
    }

    /// Moves within the decoded stream instead of resolving an address and converting it back,
    /// which is what going through [`Self::set_pc()`] would do.
    ///
    /// One comparison and one test are all this needs to recognize every target it cannot resolve:
    /// one past the end of the decoded stream, a backwards branch that ran off its start, and an
    /// unaligned one. The return trap sits outside the decoded stream, so a branch to it fails the
    /// bounds check here too and is answered by [`Self::failed_branch()`] like any other target
    /// this refuses.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    unsafe fn try_set_pc_relative(&mut self, instruction_size: u8, offset: i32) -> bool {
        // Byte offset from the instruction being executed to the branch target. The program counter
        // is advanced during instruction fetching, so that instruction starts `instruction_size`
        // bytes back.
        let offset = (offset as isize).wrapping_sub(isize::from(instruction_size));
        // Every alignment step of guest code owns one decoded instruction, so the target is
        // reached by moving within the decoded stream
        let byte_delta =
            offset * BasicEagerInstructions::<I>::STREAM_BYTES_PER_GUEST_BYTE.cast_signed();
        // This may land outside the decoded stream (including before its start), which is fine:
        // `wrapping_byte_offset()` only computes an address, it never dereferences the pointer, and
        // the bounds check below rejects such a target before it is ever used
        let new_next_instruction = self
            .next_instruction
            .as_ptr()
            .wrapping_byte_offset(byte_delta);
        // Stored either way: on the way out it is the target `failed_branch()` reports on, and
        // until then nothing else is allowed to look at it
        // SAFETY: A wrapped pointer is never null, and nothing here dereferences it
        self.next_instruction = unsafe { NonNull::new_unchecked(new_next_instruction) };

        let decoded_instruction_byte_offset = new_next_instruction
            .addr()
            .wrapping_sub(self.instructions.as_ptr().addr());

        // A target that does not land on a decoded instruction sits between two guest
        // instructions, which makes it an unaligned instruction rather than something to round to
        // the start of one. That rule lives in `set_pc()`, so rather than restating it here, where
        // it could drift, such a target simply fails to qualify, as does one past the end of the
        // decoded stream, which a backwards branch that ran off its start wraps around into.
        decoded_instruction_byte_offset < self.instructions_size()
            && decoded_instruction_byte_offset.is_multiple_of(size_of::<I>())
    }

    /// Turns the refused target back into an address and hands it to [`Self::set_pc()`], which is
    /// where the rules about what is and is not an instruction address live.
    #[cold]
    #[inline(never)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    unsafe fn failed_branch(
        &mut self,
        memory: &Memory,
    ) -> Result<ControlFlow<()>, ExecutionError<Address<I>>> {
        // Signed, because a backwards branch that ran off the start of the decoded stream is
        // exactly one of the targets that gets here
        let decoded_instruction_byte_offset = self
            .next_instruction
            .as_ptr()
            .addr()
            .wrapping_sub(self.instructions.as_ptr().addr())
            .cast_signed();
        // Every alignment step of guest code owns one decoded instruction, and the position is
        // always a whole number of them from the start of the stream, so this is exact
        let address =
            Address::<I>::truncate_from_u64(self.base_addr().as_u64().wrapping_add_signed(
                (decoded_instruction_byte_offset
                    / BasicEagerInstructions::<I>::STREAM_BYTES_PER_GUEST_BYTE.cast_signed())
                    as i64,
            ));

        self.set_pc(memory, address)
    }

    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn set_pc(
        &mut self,
        _memory: &Memory,
        pc: Address<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<Address<I>>> {
        if pc == self.return_trap_address() {
            cold_path();
            return Ok(ControlFlow::Break(()));
        }

        let address = pc.as_u64();

        if !address.is_multiple_of(u64::from(I::ALIGNMENT)) {
            cold_path();
            return Err(ExecutionError::UnalignedInstruction {
                address: PackedAddress::new(pc),
            });
        }

        let Some(offset) = address.checked_sub(self.base_addr().as_u64()) else {
            cold_path();
            return Err(ExecutionError::OutOfBoundsRead {
                address: PackedAddress::new(address),
            });
        };
        let instruction_offset =
            offset as usize / BasicEagerInstructions::<I>::GUEST_BYTES_PER_SLOT;

        if instruction_offset >= self.instructions_len() {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address }.into());
        }

        // SAFETY: `instruction_offset` was just checked to be within bounds of the decoded stream
        self.next_instruction = unsafe { self.instructions.add(instruction_offset) };

        Ok(ControlFlow::Continue(()))
    }
}

impl<I, Memory> InstructionFetcher<I, Memory> for BasicEagerInstructionFetcher<'_, I>
where
    I: Instruction,
    Memory: VirtualMemory,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn peek_instruction(&mut self, _memory: &Memory) -> FetchInstructionResult<I> {
        // SAFETY: `BasicEagerInstructions::decode()` guarantees that the last instruction is a
        // jump, which means going through `Self::set_pc()` method does the necessary bounds check,
        // so the position always points at a decoded instruction.
        let instruction = unsafe { self.next_instruction.read() };

        FetchInstructionResult::Instruction(instruction)
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    unsafe fn advance(&mut self, instruction_size: u8) {
        let byte_advance = usize::from(instruction_size)
            * BasicEagerInstructions::<I>::STREAM_BYTES_PER_GUEST_BYTE;
        // Wrapping because nothing here dereferences the pointer: the contract of this method is
        // what makes the resulting position a decoded instruction, and the bounds check that
        // matters lives in `set_pc()`
        // SAFETY: A wrapped pointer is never null, and nothing here dereferences it
        self.next_instruction = unsafe {
            NonNull::new_unchecked(
                self.next_instruction
                    .as_ptr()
                    .wrapping_byte_add(byte_advance),
            )
        };
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn fetch_instruction(&mut self, memory: &Memory) -> FetchInstructionResult<I> {
        let result = InstructionFetcher::<I, Memory>::peek_instruction(self, memory);

        if let FetchInstructionResult::Instruction(instruction) = result {
            // SAFETY: The instruction was just peeked successfully, and this is the only place that
            // moves past it
            unsafe {
                InstructionFetcher::<I, Memory>::advance(self, instruction.size());
            }
        }

        result
    }
}

impl<I> BasicEagerInstructionFetcher<'_, I>
where
    I: Instruction,
{
    /// State header that [`BasicEagerInstructions`] keeps in front of the decoded instructions
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn state(&self) -> &BasicEagerInstructionFetcherState<I> {
        // SAFETY: Decoded instructions are stored at this offset from the state in the same
        // allocation, which `BasicEagerInstructions` initialized and this fetcher borrows for as
        // long as it is alive
        unsafe {
            self.instructions
                .byte_sub(BasicEagerInstructions::<I>::INSTRUCTIONS_OFFSET)
                .cast::<BasicEagerInstructionFetcherState<I>>()
                .as_ref()
        }
    }

    /// Number of decoded instructions
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn instructions_len(&self) -> usize {
        self.state().instructions_len
    }

    /// Size of the decoded instructions in bytes
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn instructions_size(&self) -> usize {
        self.state().instructions_size
    }

    /// Guest address that corresponds to the first decoded instruction
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn base_addr(&self) -> Address<I> {
        self.state().base_addr
    }

    /// Guest address at which execution stops gracefully
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn return_trap_address(&self) -> Address<I> {
        self.state().return_trap_address
    }
}
