//! ISA strings

#[cfg(test)]
mod tests;

use crate::instructions::{ImplementedExtension, IsaExtension};
use const_format::StrWriter;

/// Maximum length of an ISA string in bytes
const MAX_ISA_STRING_LENGTH: usize = 1024;

/// Format an ISA string, see [`Instruction::ISA_STRING`] for details.
///
/// Panics if the ISA string is longer than [`MAX_ISA_STRING_LENGTH`], it is only used in constants
/// though, where this is a compilation error.
pub(super) const fn isa_string(
    xlen: u8,
    implemented_extensions: &[ImplementedExtension],
) -> StrWriter<[u8; MAX_ISA_STRING_LENGTH]> {
    let mut isa_string = StrWriter::new([0; MAX_ISA_STRING_LENGTH]);
    let mut writer = isa_string.as_mut();

    let mut previous = None;
    // Extensions are written in canonical order, which also skips duplicates
    while let Some(extension) = next_extension(implemented_extensions, previous) {
        let result = try {
            if previous.is_some() {
                writer.write_str("_")?;
            } else if matches!(extension.name.as_bytes(), b"i" | b"e") {
                writer.write_str("rv")?;
                writer.write_u8_display(xlen)?;
            }
            writer.write_str(extension.name)?;
            writer.write_u8_display(extension.major_version)?;
            writer.write_str("p")?;
            writer.write_u8_display(extension.minor_version)?;
        };
        assert!(result.is_ok(), "ISA string is too long");

        previous = Some(extension);
    }

    isa_string
}

/// Find the smallest ISA extension of implemented extensions in canonical order that is larger
/// than `previous`
const fn next_extension(
    implemented_extensions: &[ImplementedExtension],
    previous: Option<IsaExtension>,
) -> Option<IsaExtension> {
    let mut next = None;

    let mut remaining = implemented_extensions;
    while let [implemented_extension, rest @ ..] = remaining {
        remaining = rest;
        let mut extensions = implemented_extension.own_isa_extensions;
        while let [extension, rest @ ..] = extensions {
            extensions = rest;
            let extension = Some(*extension);
            if previous < extension && (next.is_none() || extension < next) {
                next = extension;
            }
        }
    }

    next
}
