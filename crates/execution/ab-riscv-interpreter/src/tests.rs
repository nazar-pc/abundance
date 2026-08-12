use crate::basic::BasicMemory;
use crate::*;

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
