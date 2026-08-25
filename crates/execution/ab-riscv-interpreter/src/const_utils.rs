//! Utilities that work around missing `const` implementations in `core`

// TODO: Remove once native ranges are usable in const context
/// `start..end` range of integers.
///
/// Unlike [`core::ops::Range`], this one can be iterated over in const context. It exists purely
/// because `core` does not implement [`Iterator`] for its ranges as `const` yet.
///
/// Just like [`core::ops::Range`], this is deliberately not `Copy` to avoid surprises when
/// iterating over a copy of the range instead of the range itself.
#[derive(Debug, Clone)]
pub(crate) struct ConstRange<T> {
    start: T,
    end: T,
}

impl<T> ConstRange<T> {
    /// Range of integers from `start` (inclusive) to `end` (exclusive)
    #[inline(always)]
    pub(crate) const fn new(start: T, end: T) -> Self {
        Self { start, end }
    }
}

macro_rules! impl_const_range {
    ($($ty:ty),* $(,)?) => {
        $(
            const impl Iterator for ConstRange<$ty> {
                type Item = $ty;

                #[inline(always)]
                #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
                fn next(&mut self) -> Option<Self::Item> {
                    if self.start >= self.end {
                        return None;
                    }

                    let value = self.start;
                    self.start += 1;
                    Some(value)
                }

                #[inline(always)]
                #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
                fn size_hint(&self) -> (usize, Option<usize>) {
                    let remaining = self.end.saturating_sub(self.start) as usize;
                    (remaining, Some(remaining))
                }
            }

            impl ExactSizeIterator for ConstRange<$ty> {
                #[inline(always)]
                #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
                fn len(&self) -> usize {
                    self.end.saturating_sub(self.start) as usize
                }
            }
        )*
    };
}

impl_const_range!(u8, u32, usize);
