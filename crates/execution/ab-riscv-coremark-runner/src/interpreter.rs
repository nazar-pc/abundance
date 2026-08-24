use crate::instruction::CoremarkInstruction;
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::marker::PhantomData;
use core::ops::ControlFlow;
use std::alloc::{Layout, alloc, dealloc, handle_alloc_error};
use std::hint::cold_path;
use std::ptr::NonNull;

/// Flat guest memory
#[derive(Debug, Copy, Clone)]
#[repr(align(16))]
pub(crate) struct GuestMemory<const BASE_ADDR: u64, const SIZE: usize> {
    data: [u8; SIZE],
}

impl<const BASE_ADDR: u64, const SIZE: usize> VirtualMemory for GuestMemory<BASE_ADDR, SIZE> {
    #[inline(always)]
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        // SAFETY: Only reading basic integers from initialized memory
        unsafe {
            Ok(self
                .data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
                .read_unaligned())
        }
    }

    #[inline(always)]
    unsafe fn read_unchecked<T>(&self, address: u64) -> T
    where
        T: BasicInt,
    {
        // SAFETY: Guaranteed by function contract
        unsafe {
            let offset = address.unchecked_sub(BASE_ADDR) as usize;
            self.data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset)
                .read_unaligned()
        }
    }

    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError> {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        self.data
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })
    }

    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8] {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset > self.data.len() as u64 {
            cold_path();
            return &[];
        }

        let remaining = self.data.get(offset as usize..).unwrap_or_default();
        remaining.get(..len as usize).unwrap_or(remaining)
    }

    #[inline(always)]
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        // SAFETY: Only writing basic integers to initialized memory
        unsafe {
            self.data
                .as_mut_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
                .write_unaligned(value);
        }

        Ok(())
    }

    fn write_slice(&mut self, address: u64, data: &[u8]) -> Result<(), VirtualMemoryError> {
        let offset = address.wrapping_sub(BASE_ADDR);

        if offset > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        let len = data.len();
        let Some(target_data) = self
            .data
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len))
        else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        };

        target_data.copy_from_slice(data);

        Ok(())
    }
}

impl<const BASE_ADDR: u64, const SIZE: usize> Default for GuestMemory<BASE_ADDR, SIZE> {
    fn default() -> Self {
        Self { data: [0; SIZE] }
    }
}

/// Everything [`EagerInstructionFetcher`] needs besides its position within the decoded
/// instruction stream.
///
/// This lives in a single heap allocation whose tail holds the decoded instructions themselves,
/// starting [`EagerInstructions::INSTRUCTIONS_OFFSET`] bytes from the beginning of it. That
/// is what keeps the fetcher itself down to two pointers, so it fits into two argument registers
/// when threaded through tail-called instruction handlers by value.
#[derive(Debug)]
#[repr(C)]
struct EagerInstructionFetcherState {
    /// Number of decoded instructions stored right after this header
    instructions_len: usize,
    /// Guest address that corresponds to the first decoded instruction
    base_addr: u64,
    /// Guest address at which execution stops gracefully
    return_trap_address: u64,
}

/// Instructions decoded upfront, which [`EagerInstructionFetcher`] walks.
///
/// Ownership of the allocation lives here rather than in the fetcher because the fetcher is moved
/// through tail-called instruction handlers by value. A destructor on it would make every handler
/// that can fail (every load, store, branch and jump) responsible for dropping it on the way out,
/// which costs a stack frame, callee-saved register spills and a reload in the hot path of each of
/// them, even though the failing path is never taken.
pub(crate) struct EagerInstructions {
    /// State header, together with the decoded instructions themselves, in a single heap
    /// allocation.
    ///
    /// This is a raw pointer rather than a `Box` on purpose: fetchers point into the same
    /// allocation, and going through a `Box` would assert unique access to that allocation on
    /// every use, invalidating pointers that must survive across all of them.
    state: NonNull<EagerInstructionFetcherState>,
}

impl fmt::Debug for EagerInstructions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EagerInstructions")
            .field("instructions_len", &self.instructions_len())
            .field("base_addr", &self.base_addr())
            .field("return_trap_address", &self.return_trap_address())
            .finish_non_exhaustive()
    }
}

impl Drop for EagerInstructions {
    fn drop(&mut self) {
        let layout = Self::allocation_layout(self.instructions_len());

        // SAFETY: Allocated with the global allocator using exactly this layout, and this is the
        // only owner of the allocation
        unsafe {
            dealloc(self.state.as_ptr().cast::<u8>(), layout);
        }
    }
}

impl EagerInstructions {
    /// Byte offset of the decoded instructions from the start of the allocation that
    /// [`Self::state`] points at
    const INSTRUCTIONS_OFFSET: usize = size_of::<EagerInstructionFetcherState>()
        .next_multiple_of(align_of::<CoremarkInstruction>());

    /// Layout of the allocation holding [`EagerInstructionFetcherState`] followed by
    /// `instructions_len` decoded instructions
    fn allocation_layout(instructions_len: usize) -> Layout {
        let (layout, instructions_offset) = Layout::new::<EagerInstructionFetcherState>()
            .extend(
                Layout::array::<CoremarkInstruction>(instructions_len)
                    .expect("Decoded instructions fit into memory, they were just allocated; qed"),
            )
            .expect("Decoded instructions fit into memory, they were just allocated; qed");

        debug_assert_eq!(instructions_offset, Self::INSTRUCTIONS_OFFSET);

        layout.pad_to_align()
    }

    fn instantiate(
        instructions: &[CoremarkInstruction],
        base_addr: u64,
        return_trap_address: u64,
    ) -> Self {
        let instructions_len = instructions.len();
        let layout = Self::allocation_layout(instructions_len);
        // SAFETY: The state itself is always there, so the layout has non-zero size
        let state = unsafe { alloc(layout) }.cast::<EagerInstructionFetcherState>();
        let Some(state) = NonNull::new(state) else {
            handle_alloc_error(layout);
        };

        // SAFETY: Freshly allocated for exactly this type, correctly aligned
        unsafe {
            state.write(EagerInstructionFetcherState {
                instructions_len,
                base_addr,
                return_trap_address,
            });
        }

        let instance = Self { state };

        // SAFETY: The allocation was made for exactly this many instructions and is distinct from
        // the ones being copied in
        unsafe {
            instance.instructions().copy_from_nonoverlapping(
                NonNull::from(instructions).cast::<CoremarkInstruction>(),
                instructions_len,
            );
        }

        instance
    }

    /// Pointer to the first decoded instruction
    #[inline(always)]
    fn instructions(&self) -> NonNull<CoremarkInstruction> {
        // SAFETY: Decoded instructions are stored at this offset of the same allocation as the
        // state
        unsafe { self.state.byte_add(Self::INSTRUCTIONS_OFFSET) }.cast::<CoremarkInstruction>()
    }

    /// Number of decoded instructions
    #[inline(always)]
    fn instructions_len(&self) -> usize {
        // SAFETY: State is initialized in the constructor and valid for as long as `self` is
        unsafe { (*self.state.as_ptr()).instructions_len }
    }

    /// Guest address that corresponds to the first decoded instruction
    #[inline(always)]
    fn base_addr(&self) -> u64 {
        // SAFETY: State is initialized in the constructor and valid for as long as `self` is
        unsafe { (*self.state.as_ptr()).base_addr }
    }

    /// Guest address at which execution stops gracefully
    #[inline(always)]
    fn return_trap_address(&self) -> u64 {
        // SAFETY: State is initialized in the constructor and valid for as long as `self` is
        unsafe { (*self.state.as_ptr()).return_trap_address }
    }

    /// Create a fetcher positioned at the instruction that guest address `pc` corresponds to
    ///
    /// # Safety
    /// `pc` must be a valid and aligned guest address of one of the decoded instructions.
    #[inline(always)]
    pub(super) unsafe fn fetcher(&self, pc: u64) -> EagerInstructionFetcher<'_> {
        let instruction_offset = (pc - self.base_addr()) as usize / size_of::<u16>();

        EagerInstructionFetcher {
            // SAFETY: Guaranteed by function contract, meaning `instruction_offset` is within
            // bounds of the decoded stream
            next_instruction: unsafe { self.instructions().add(instruction_offset) },
            state: self.state,
            instructions: PhantomData,
        }
    }

    /// Decode `instructions` and create a new instance holding the result.
    ///
    /// `base_addr` is the guest address of the first instruction, and `return_trap_address` is the
    /// address at which the interpreter will stop execution (gracefully).
    ///
    /// # Safety
    /// The instructions processed must end with a jump instruction, and `return_trap_address` must
    /// not fall inside them. Instruction fetching does not compare against the return trap, so an
    /// address inside the instructions would stop execution when jumped to but not when reached by
    /// falling through.
    #[inline(always)]
    pub(super) unsafe fn decode(
        instructions: &[u8],
        return_trap_address: u64,
        base_addr: u64,
    ) -> Self {
        let mut decoded_instructions: Vec<CoremarkInstruction> =
            Vec::with_capacity(instructions.len() / size_of::<u16>());

        let mut offset = 0;
        while let Some(instruction_bytes) = instructions.get(offset..offset + size_of::<u32>()) {
            let decoded_instruction = u32::from_le_bytes([
                instruction_bytes[0],
                instruction_bytes[1],
                instruction_bytes[2],
                instruction_bytes[3],
            ]);
            // Use `Unimp` as a fallback, though contract is expected to only contain legal
            // instructions
            let decoded_instruction = Instruction::try_decode(decoded_instruction).unwrap_or(
                CoremarkInstruction::Unimp {
                    rs1: Register::ZERO,
                    rs2: Register::ZERO,
                },
            );
            decoded_instructions.push(decoded_instruction);
            match decoded_instruction.size() {
                2 => {
                    offset += 2;
                }
                4 => {
                    // The second half of a 32-bit instruction is a valid offset and may or may not
                    // decode to a valid instruction on its own. Try to decode it but ignore
                    // decoding failures.

                    offset += 2;

                    // Could be both 16-bit and 32-bit instruction, need to handle end of the
                    // instruction stream
                    let instruction_word = if let Some(instruction_bytes) =
                        instructions.get(offset..offset + size_of::<u32>())
                    {
                        u32::from_le_bytes([
                            instruction_bytes[0],
                            instruction_bytes[1],
                            instruction_bytes[2],
                            instruction_bytes[3],
                        ])
                    } else {
                        u32::from_le_bytes([instruction_bytes[2], instruction_bytes[3], 0, 0])
                    };

                    decoded_instructions.push(Instruction::try_decode(instruction_word).unwrap_or(
                        CoremarkInstruction::Unimp {
                            rs1: Register::ZERO,
                            rs2: Register::ZERO,
                        },
                    ));
                    offset += 2;
                }
                instruction_size => {
                    unreachable!("Invalid instruction size {instruction_size}, expected 2 or 4");
                }
            }
        }

        let remainder_bytes = instructions.get(offset..).unwrap_or(&[]);

        if remainder_bytes.len() == size_of::<u16>() {
            let instruction_word =
                u32::from_le_bytes([remainder_bytes[0], remainder_bytes[1], 0, 0]);
            decoded_instructions.push(Instruction::try_decode(instruction_word).unwrap_or(
                CoremarkInstruction::Unimp {
                    rs1: Register::ZERO,
                    rs2: Register::ZERO,
                },
            ));
        }
        Self::instantiate(&decoded_instructions, base_addr, return_trap_address)
    }
}

/// Eager instruction fetcher walks instructions that [`EagerInstructions`] decoded upfront.
///
/// This is a plain `Copy` cursor without a destructor, see [`EagerInstructions`] for why.
#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) struct EagerInstructionFetcher<'a> {
    /// The instruction to be returned by the next [`InstructionFetcher::fetch_instruction()`]
    /// call.
    ///
    /// A pointer rather than an offset helps LLVM with SROA and aliasing analysis, so it can
    /// retain this in a native register instead of recomputing it from an offset on every
    /// fetch.
    next_instruction: NonNull<CoremarkInstruction>,
    /// Everything else the fetcher needs, borrowed from [`EagerInstructions`]
    state: NonNull<EagerInstructionFetcherState>,
    /// Fetcher borrows the decoded instructions it walks
    instructions: PhantomData<&'a EagerInstructions>,
}

const {
    // When fetcher is used with threaded dispatch, it must fit into two argument registers to be
    // passed used registers through tail calls
    assert!(size_of::<EagerInstructionFetcher<'_>>() == 16);
    // Drop glue on the fetcher would force a stack frame into every fallible handler, see
    // `EagerInstructions` for details
    assert!(!core::mem::needs_drop::<EagerInstructionFetcher<'_>>());
}

impl fmt::Debug for EagerInstructionFetcher<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EagerInstructionFetcher")
            .field("next_instruction", &self.next_instruction)
            .field("instructions_len", &self.instructions_len())
            .field("base_addr", &self.base_addr())
            .field("return_trap_address", &self.return_trap_address())
            .finish_non_exhaustive()
    }
}

impl<Memory> ProgramCounter<u64, Memory> for EagerInstructionFetcher<'_>
where
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn get_pc(&self) -> u64 {
        let decoded_instruction_byte_offset = self
            .next_instruction
            .as_ptr()
            .addr()
            .wrapping_sub(self.instructions().as_ptr().addr());

        self.base_addr()
            + decoded_instruction_byte_offset as u64 * size_of::<u16>() as u64
                / size_of::<CoremarkInstruction>() as u64
    }

    /// Moves within the decoded stream instead of resolving an address and converting it back,
    /// which is what going through [`Self::set_pc()`] would do.
    ///
    /// One comparison and one test are all this needs to hand every case it does not handle itself
    /// over to [`Self::set_pc()`]: a target past the end of the decoded stream, a backwards branch
    /// that ran off its start, and an unaligned target. The return trap sits outside the decoded
    /// stream, so a branch to it fails the bounds check here and is stopped by [`Self::set_pc()`]
    /// like any other jump to it.
    #[inline(always)]
    fn set_pc_relative(
        &mut self,
        memory: &Memory,
        instruction_size: u8,
        offset: i32,
    ) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        // Byte offset from the instruction being executed to the branch target. The program counter
        // is advanced during instruction fetching, so that instruction starts `instruction_size`
        // bytes back.
        let offset = (offset as isize).wrapping_sub(isize::from(instruction_size));
        // Every `size_of::<u16>()` of guest code owns one decoded instruction, so the target is
        // reached by moving within the decoded stream
        let byte_delta =
            offset * (size_of::<CoremarkInstruction>() / size_of::<u16>()).cast_signed();
        // This may land outside the decoded stream (including before its start), which is fine:
        // `wrapping_byte_offset()` only computes an address, it never dereferences the pointer, and
        // the bounds check below rejects such a target before it is ever used
        let new_next_instruction = self
            .next_instruction
            .as_ptr()
            .wrapping_byte_offset(byte_delta);
        let decoded_instruction_byte_offset = new_next_instruction
            .addr()
            .wrapping_sub(self.instructions().as_ptr().addr());

        // A target that does not land on a decoded instruction sits between two guest
        // instructions, which makes it an unaligned instruction rather than something to round to
        // the start of one. That rule lives in `set_pc()`, so rather than restating it here, where
        // it could drift, such a target simply fails to qualify for the fast path, as does one past
        // the end of the decoded stream, which a backwards branch that ran off its start wraps
        // around into.
        if decoded_instruction_byte_offset
            >= self.instructions_len() * size_of::<CoremarkInstruction>()
            || !decoded_instruction_byte_offset.is_multiple_of(size_of::<CoremarkInstruction>())
        {
            cold_path();
            let pc = <Self as ProgramCounter<_, Memory>>::get_pc(self);
            return self.set_pc(memory, pc.wrapping_add_signed(offset as i64));
        }

        // SAFETY: Just checked that `new_next_instruction` lands exactly on a decoded instruction
        // within the bounds of the (non-empty) decoded stream
        self.next_instruction = unsafe { NonNull::new_unchecked(new_next_instruction) };

        Ok(ControlFlow::Continue(()))
    }

    #[inline]
    fn set_pc(
        &mut self,
        _memory: &Memory,
        pc: u64,
    ) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        let address = pc;

        if address == self.return_trap_address() {
            cold_path();
            return Ok(ControlFlow::Break(()));
        }

        if !address.is_multiple_of(size_of::<u16>() as u64) {
            cold_path();
            return Err(ExecutionError::UnalignedInstruction {
                address: PackedAddress::new(address),
            });
        }

        let Some(offset) = address.checked_sub(self.base_addr()) else {
            cold_path();
            return Err(ExecutionError::OutOfBoundsRead {
                address: PackedAddress::new(address),
            });
        };
        let offset = offset as usize;
        let instruction_offset = offset / size_of::<u16>();

        if instruction_offset >= self.instructions_len() {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address }.into());
        }

        // SAFETY: `instruction_offset` was just checked to be within bounds of the decoded stream
        self.next_instruction = unsafe { self.instructions().add(instruction_offset) };

        Ok(ControlFlow::Continue(()))
    }
}

impl<Memory> InstructionFetcher<CoremarkInstruction, Memory> for EagerInstructionFetcher<'_>
where
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn peek_instruction(
        &mut self,
        _memory: &Memory,
    ) -> FetchInstructionResult<CoremarkInstruction> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method does the necessary bounds check, so the position always
        // points at a decoded instruction.
        let instruction = unsafe { self.next_instruction.read() };

        FetchInstructionResult::Instruction(instruction)
    }

    #[inline(always)]
    unsafe fn advance(&mut self, instruction_size: u8) {
        let byte_advance =
            usize::from(instruction_size) / size_of::<u16>() * size_of::<CoremarkInstruction>();
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
    fn fetch_instruction(
        &mut self,
        memory: &Memory,
    ) -> FetchInstructionResult<CoremarkInstruction> {
        let result =
            InstructionFetcher::<CoremarkInstruction, Memory>::peek_instruction(self, memory);

        if let FetchInstructionResult::Instruction(instruction) = result {
            // SAFETY: The instruction was just peeked successfully, and this is the only place that
            // moves past it
            unsafe {
                InstructionFetcher::<CoremarkInstruction, Memory>::advance(
                    self,
                    instruction.size(),
                );
            }
        }

        result
    }
}

impl EagerInstructionFetcher<'_> {
    /// Pointer to the first decoded instruction
    #[inline(always)]
    fn instructions(&self) -> NonNull<CoremarkInstruction> {
        // SAFETY: Decoded instructions are stored at this offset of the same allocation as the
        // state
        unsafe { self.state.byte_add(EagerInstructions::INSTRUCTIONS_OFFSET) }
            .cast::<CoremarkInstruction>()
    }

    /// Number of decoded instructions
    #[inline(always)]
    fn instructions_len(&self) -> usize {
        // SAFETY: State is initialized by `EagerInstructions` and borrowed for as long as `self`
        // is alive
        unsafe { (*self.state.as_ptr()).instructions_len }
    }

    /// Guest address that corresponds to the first decoded instruction
    #[inline(always)]
    fn base_addr(&self) -> u64 {
        // SAFETY: State is initialized by `EagerInstructions` and borrowed for as long as `self`
        // is alive
        unsafe { (*self.state.as_ptr()).base_addr }
    }

    /// Guest address at which execution stops gracefully
    #[inline(always)]
    fn return_trap_address(&self) -> u64 {
        // SAFETY: State is initialized by `EagerInstructions` and borrowed for as long as `self`
        // is alive
        unsafe { (*self.state.as_ptr()).return_trap_address }
    }
}
