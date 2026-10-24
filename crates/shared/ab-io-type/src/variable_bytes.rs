use crate::variable_elements::VariableElements;

/// Container for storing variable number of bytes
pub type VariableBytes<'a, const RECOMMENDED_ALLOCATION: u32 = 0> =
    VariableElements<'a, u8, RECOMMENDED_ALLOCATION>;
