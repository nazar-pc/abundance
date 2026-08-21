extern crate alloc;

use crate::basic::BasicMemory;
use crate::*;
use ab_riscv_primitives::privilege::PrivilegeLevel;

/// `?` on a memory access inside a function returning [`ExecutionResult`], in const context,
/// which is what instruction bodies need
const fn load<Memory>(memory: &Memory, address: u64) -> ExecutionResult<Reg<u64>>
where
    Memory: [const] VirtualMemory,
{
    let value = memory.read::<u64>(address)?;
    ExecutionResult::Continue { rd: Reg::A0, value }
}

#[test]
fn question_mark_converts_memory_errors() {
    let memory = BasicMemory::<0x1000, 128>::default();

    assert!(matches!(
        load(&memory, 0x1000),
        ExecutionResult::Continue { rd: Reg::A0, .. }
    ));
    assert!(matches!(
        load(&memory, 0xdead_0000),
        ExecutionResult::Err(ExecutionError::OutOfBoundsRead { .. })
    ));
}

/// Round-trip one outcome through the opaque form and compare against the original, which is only
/// comparable through its `Debug` representation
fn assert_round_trip<I>(result: ThreadedExecutionResult<I>)
where
    I: Instruction,
{
    let expected = alloc::format!("{result:?}");
    // SAFETY: Tests only run on a platform they were built for
    let opaque = unsafe { OpaqueThreadedExecutionResult::new(result) };
    let actual = alloc::format!("{:?}", opaque.into_result());

    assert_eq!(expected, actual);
}

/// Every outcome must survive the trip through [`OpaqueThreadedExecutionResult`] unchanged, since
/// that is the only form in which a handler can report one
fn assert_all_outcomes_round_trip<I>(address: <I::Reg as Register>::Type)
where
    I: Instruction,
{
    let program_counter = address;

    assert_round_trip::<I>(ThreadedExecutionResult::stopped(program_counter));

    for error in [
        ExecutionError::UnalignedInstruction {
            address: PackedAddress::new(address),
        },
        ExecutionError::OutOfBoundsRead {
            address: PackedAddress::new(u64::MAX),
        },
        ExecutionError::OutOfBoundsWrite {
            address: PackedAddress::new(u64::MAX),
        },
        ExecutionError::EcallUnsupported {
            address: PackedAddress::new(address),
        },
        ExecutionError::IllegalInstruction {
            address: PackedAddress::new(address),
        },
        ExecutionError::CsrReadOnly {
            csr_index: u16::MAX,
        },
        ExecutionError::CsrIllegalRead { csr_index: 0x300 },
        ExecutionError::CsrIllegalWrite { csr_index: 0x301 },
        ExecutionError::CsrUnknown { csr_index: 0 },
        ExecutionError::CsrInsufficientPrivilege {
            csr_index: 0xc00,
            required: PrivilegeLevel::Machine,
            current: PrivilegeLevel::User,
        },
        ExecutionError::CsrInsufficientPrivilege {
            csr_index: 0xc01,
            required: PrivilegeLevel::Supervisor,
            current: PrivilegeLevel::Machine,
        },
        ExecutionError::Custom(u64::MAX.to_le_bytes()),
        ExecutionError::UnsupportedPlatform,
    ] {
        assert_round_trip::<I>(ThreadedExecutionResult::failed(program_counter, error));
    }
}

#[test]
fn outcomes_round_trip_rv64() {
    assert_all_outcomes_round_trip::<Rv64Instruction<Reg<u64>>>(u64::MAX);
    assert_all_outcomes_round_trip::<Rv64Instruction<Reg<u64>>>(0);
}

#[test]
fn outcomes_round_trip_rv32() {
    assert_all_outcomes_round_trip::<Rv32Instruction<Reg<u32>>>(u32::MAX);
    assert_all_outcomes_round_trip::<Rv32Instruction<Reg<u32>>>(0);
}

#[test]
fn opaque_outcome_fits_into_return_registers() {
    // Not a hard requirement of the type system, but the entire reason the opaque form exists: on
    // the platforms that return it in registers it must not be larger than what they return
    if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
        assert!(size_of::<OpaqueThreadedExecutionResult<Rv64Instruction<Reg<u64>>>>() <= 32);
    }
}
