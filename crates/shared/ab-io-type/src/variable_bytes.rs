#[cfg(test)]
mod tests;

use crate::trivial_type::TrivialType;
use crate::variable_elements::VariableElements;

/// Container for storing variable number of bytes
pub type VariableBytes<const RECOMMENDED_ALLOCATION: u32 = 0> =
    VariableElements<u8, RECOMMENDED_ALLOCATION>;

impl<const RECOMMENDED_ALLOCATION: u32> VariableElements<u8, RECOMMENDED_ALLOCATION> {
    /// Reads and returns value of type `T` or `None` if there is not enough data.
    ///
    /// Checks alignment internally to support both aligned and unaligned reads.
    #[inline(always)]
    pub fn read_trivial_type<T>(&self) -> Option<T>
    where
        T: TrivialType,
    {
        if self.size() < T::SIZE {
            return None;
        }

        let ptr = self.get_initialized().as_ptr().cast::<T>();

        // SAFETY: Trivial types are safe to read as bytes, there are at least `T::SIZE` initialized
        // bytes as checked above
        let value = unsafe {
            if ptr.is_aligned() {
                ptr.read()
            } else {
                ptr.read_unaligned()
            }
        };

        Some(value)
    }
}
