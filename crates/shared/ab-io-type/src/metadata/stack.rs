/// Stack of compound types that are being decoded, the last frame is the innermost type.
///
/// Decoders use it instead of recursion, so the stack usage of decoders depends on `CAPACITY`
/// (the max nesting depth), but not on metadata.
#[derive(Debug)]
pub(super) struct Stack<Frame, const CAPACITY: usize> {
    frames: [Frame; CAPACITY],
    len: usize,
}

impl<Frame, const CAPACITY: usize> Stack<Frame, CAPACITY>
where
    Frame: Copy,
{
    /// Create an empty stack, `filler` is only used to initialize memory
    #[inline(always)]
    pub(super) const fn new(filler: Frame) -> Self {
        Self {
            frames: [filler; CAPACITY],
            len: 0,
        }
    }

    /// Push a frame, returns `None` if the stack is full
    #[inline(always)]
    pub(super) const fn push(&mut self, frame: Frame) -> Option<()> {
        *self.frames.get_mut(self.len)? = frame;
        self.len += 1;
        Some(())
    }

    /// Whether there are no frames
    #[inline(always)]
    pub(super) const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The innermost frame, `None` if the stack is empty
    #[inline(always)]
    pub(super) const fn last_mut(&mut self) -> Option<&mut Frame> {
        let index = self.len.checked_sub(1)?;
        self.frames.get_mut(index)
    }

    /// Remove the innermost frame, returns `None` if the stack is empty
    #[inline(always)]
    pub(super) const fn pop(&mut self) -> Option<()> {
        self.len = self.len.checked_sub(1)?;
        Some(())
    }
}
