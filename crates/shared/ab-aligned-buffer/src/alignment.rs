//! Alignment of buffers

use crate::sealed;
use core::fmt;

/// Alignment in bytes, a power of two from 1 to 4096.
///
/// Used as a type parameter of buffers for the alignment of their contents, and as a value for the
/// alignment of allocations.
pub trait Alignment:
    sealed::Sealed + fmt::Debug + Default + Copy + Send + Sync + Unpin + 'static
{
    /// Alignment in bytes
    const ALIGN: usize;
}

/// Alignment that is at least as strict as `A`, such that anything aligned to `Self` is also
/// aligned to `A`
#[diagnostic::on_unimplemented(message = "Alignment `{Self}` doesn't guarantee alignment `{A}`")]
pub trait AtLeast<A>: Alignment
where
    A: Alignment,
{
}

macro_rules! define_alignments {
    ($($name:ident = $align:literal),+ $(,)?) => {
        $(
            #[doc = concat!("Alignment of ", stringify!($align), " bytes")]
            #[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
            pub struct $name;

            impl sealed::Sealed for $name {}

            impl Alignment for $name {
                const ALIGN: usize = $align;
            }
        )+
    };
}

define_alignments!(
    Align1 = 1,
    Align2 = 2,
    Align4 = 4,
    Align8 = 8,
    Align16 = 16,
    Align32 = 32,
    Align64 = 64,
    Align128 = 128,
    Align256 = 256,
    Align512 = 512,
    Align1024 = 1024,
    Align2048 = 2048,
    Align4096 = 4096,
);

/// Every alignment is at least 1 byte, which generic code can rely on with just an `Alignment`
/// bound
impl<A> AtLeast<Align1> for A where A: Alignment {}

/// Implements `AtLeast` for every alignment and every alignment that is not stricter than it,
/// alignments must be listed from the strictest one, `Align1` is covered by the implementation
/// above
macro_rules! impl_at_least {
    () => {};
    ($strictest:ident $(, $rest:ident)*) => {
        impl AtLeast<$strictest> for $strictest {}
        $(
            impl AtLeast<$rest> for $strictest {}
        )*
        impl_at_least!($($rest),*);
    };
}

impl_at_least!(
    Align4096, Align2048, Align1024, Align512, Align256, Align128, Align64, Align32, Align16,
    Align8, Align4, Align2
);
