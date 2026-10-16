use crate::variable_elements::VariableElements;

/// Container for storing variable number of bytes
pub type VariableBytes<const RECOMMENDED_ALLOCATION: u32 = 0> =
    VariableElements<u8, RECOMMENDED_ALLOCATION>;
