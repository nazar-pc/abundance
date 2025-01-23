use core::ptr;

/// Concatenates metadata sources.
///
/// Returns both a scratch memory and number of bytes in it that correspond to metadata
pub const fn concat_metadata_sources(sources: &[&[u8]]) -> ([u8; 4096], usize) {
    let mut metadata_scratch = [0u8; 4096];
    // Just a way to convert above array into slice, `as_mut_slice` is not yet
    // stable in const environment
    let (_, mut remainder) = metadata_scratch.split_at_mut(0);

    // For loops are not yet usable in const environment
    let mut i = 0;
    while i < sources.len() {
        let source = sources[i];
        let target;
        (target, remainder) = remainder.split_at_mut(source.len());

        // TODO: Switch to `copy_from_slice` once stable:
        //  https://github.com/rust-lang/rust/issues/131415
        // The same as `target.copy_from_slice(&source);`, but it doesn't work in const environment
        // yet
        // SAFETY: Size is correct due to slicing above, pointers are created from valid independent
        // slices of equal length
        unsafe {
            ptr::copy_nonoverlapping(source.as_ptr(), target.as_mut_ptr(), source.len());
        }
        i += 1;
    }

    let remainder_len = remainder.len();
    let size = metadata_scratch.len() - remainder_len;
    (metadata_scratch, size)
}
