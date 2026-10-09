//! `WritableBodyTransaction` can't be implemented outside `ab_core_primitives`. Block body builders
//! do unchecked pointer arithmetic on the buffer, trusting `write_into()` to append exactly one
//! correctly encoded transaction to it.

use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_core_primitives::block::body::owned::WritableBodyTransaction;
use ab_core_primitives::transaction::owned::OwnedTransactionError;

struct CustomTransaction;

impl WritableBodyTransaction for CustomTransaction {
    fn write_into(&self, _buffer: &mut OwnedAlignedBuffer) -> Result<(), OwnedTransactionError> {
        Ok(())
    }
}

fn main() {}
