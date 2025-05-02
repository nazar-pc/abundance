use crate::owned::{OwnedTransactionBuilderError, OwnedTransactionLengths};
use crate::{TransactionHeader, TransactionSlot};
use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_io_type::trivial_type::TrivialType;
use core::slice;

#[derive(Debug, Clone)]
pub(super) struct BuilderBuffer {
    buffer: OwnedAlignedBuffer,
}

impl BuilderBuffer {
    pub(super) fn new(header: &TransactionHeader) -> Self {
        const _: () = {
            // Writing `OwnedTransactionLengths` after `TransactionHeader` must be aligned
            assert!(size_of::<TransactionHeader>() % align_of::<OwnedTransactionLengths>() == 0);
        };
        let mut buffer = OwnedAlignedBuffer::with_capacity(
            TransactionHeader::SIZE + OwnedTransactionLengths::SIZE,
        );
        // Always fits into `u32`
        let _: bool = buffer.append(header.as_bytes());
        // Always fits into `u32`
        let _: bool = buffer.append(OwnedTransactionLengths::default().as_bytes());
        Self { buffer }
    }

    fn get_lengths(&mut self) -> &mut OwnedTransactionLengths {
        unsafe {
            self.buffer
                .as_mut_ptr()
                .add(size_of::<TransactionHeader>())
                .cast::<OwnedTransactionLengths>()
                .as_mut_unchecked()
        }
    }

    pub(super) fn append_read_slots(
        &mut self,
        slots: &[TransactionSlot],
    ) -> Result<(), OwnedTransactionBuilderError> {
        {
            let lengths = self.get_lengths();
            let Ok(slots_len) = u16::try_from(slots.len()) else {
                return Err(OwnedTransactionBuilderError::TooManyReadSlots);
            };
            let Some(new_read_slots) = lengths.read_slots.checked_add(slots_len) else {
                return Err(OwnedTransactionBuilderError::TooManyReadSlots);
            };

            lengths.read_slots = new_read_slots;
        }

        const _: () = {
            // Writing `TransactionSlot` after `OwnedTransactionLengths` and `TransactionHeader`
            // must be aligned
            assert!(
                (size_of::<TransactionHeader>() + size_of::<OwnedTransactionLengths>())
                    % align_of::<TransactionSlot>()
                    == 0
            );
        };
        // SAFETY: `TransactionSlot` implements `TrivialType` and is safe to copy as bytes, it is
        // also correctly aligned due to check above
        if !self.buffer.append(unsafe {
            slice::from_raw_parts(slots.as_ptr().cast::<u8>(), size_of_val(slots))
        }) {
            return Err(OwnedTransactionBuilderError::TransactionTooLarge);
        }

        Ok(())
    }

    pub(super) fn append_write_slots(
        &mut self,
        slots: &[TransactionSlot],
    ) -> Result<(), OwnedTransactionBuilderError> {
        {
            let lengths = self.get_lengths();
            let Ok(slots_len) = u16::try_from(slots.len()) else {
                return Err(OwnedTransactionBuilderError::TooManyWriteSlots);
            };
            let Some(new_write_slots) = lengths.write_slots.checked_add(slots_len) else {
                return Err(OwnedTransactionBuilderError::TooManyWriteSlots);
            };

            lengths.write_slots = new_write_slots;
        }

        const _: () = {
            // Writing `TransactionSlot` after `OwnedTransactionLengths` and `TransactionHeader`
            // must be aligned
            assert!(
                (size_of::<TransactionHeader>() + size_of::<OwnedTransactionLengths>())
                    % align_of::<TransactionSlot>()
                    == 0
            );
        };
        // SAFETY: `TransactionSlot` implements `TrivialType` and is safe to copy as bytes, it is
        // also correctly aligned due to check above
        if !self.buffer.append(unsafe {
            slice::from_raw_parts(slots.as_ptr().cast::<u8>(), size_of_val(slots))
        }) {
            return Err(OwnedTransactionBuilderError::TransactionTooLarge);
        }

        Ok(())
    }

    pub(super) fn append_payload(
        &mut self,
        payload: &[u8],
    ) -> Result<(), OwnedTransactionBuilderError> {
        {
            let lengths = self.get_lengths();
            let Ok(payload_len) = u32::try_from(payload.len()) else {
                return Err(OwnedTransactionBuilderError::PayloadTooLarge);
            };
            let Some(new_payload_len) = lengths.payload.checked_add(payload_len) else {
                return Err(OwnedTransactionBuilderError::PayloadTooLarge);
            };

            lengths.payload = new_payload_len;
        }

        const _: () = {
            // Writing after `OwnedTransactionLengths`, `TransactionHeader` and (optionally)
            // `TransactionSlot` must be aligned to `u128`
            assert!(
                (size_of::<TransactionHeader>() + size_of::<OwnedTransactionLengths>())
                    % align_of::<u128>()
                    == 0
            );
            assert!(
                (size_of::<TransactionHeader>()
                    + size_of::<OwnedTransactionLengths>()
                    + size_of::<TransactionSlot>())
                    % align_of::<u128>()
                    == 0
            );
        };
        if !self.buffer.append(payload) {
            return Err(OwnedTransactionBuilderError::TransactionTooLarge);
        }

        Ok(())
    }

    pub(super) fn append_seal(&mut self, seal: &[u8]) -> Result<(), OwnedTransactionBuilderError> {
        {
            let lengths = self.get_lengths();
            if lengths.payload % u128::SIZE != 0 {
                return Err(OwnedTransactionBuilderError::PayloadIsNotMultipleOfU128);
            }
            let Ok(seal_len) = u32::try_from(seal.len()) else {
                return Err(OwnedTransactionBuilderError::SealTooLarge);
            };
            let Some(new_seal_len) = lengths.seal.checked_add(seal_len) else {
                return Err(OwnedTransactionBuilderError::SealTooLarge);
            };

            lengths.seal = new_seal_len;
        }

        const _: () = {
            // Writing after `OwnedTransactionLengths`, `TransactionHeader` and (optionally)
            // `TransactionSlot` must be aligned to `u128`
            assert!(
                (size_of::<TransactionHeader>() + size_of::<OwnedTransactionLengths>())
                    % align_of::<u128>()
                    == 0
            );
            assert!(
                (size_of::<TransactionHeader>()
                    + size_of::<OwnedTransactionLengths>()
                    + size_of::<TransactionSlot>())
                    % align_of::<u128>()
                    == 0
            );
        };
        if !self.buffer.append(seal) {
            return Err(OwnedTransactionBuilderError::TransactionTooLarge);
        }

        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<OwnedAlignedBuffer, OwnedTransactionBuilderError> {
        let lengths = self.get_lengths();
        if lengths.payload % u128::SIZE != 0 {
            return Err(OwnedTransactionBuilderError::PayloadIsNotMultipleOfU128);
        }
        Ok(self.buffer)
    }
}
