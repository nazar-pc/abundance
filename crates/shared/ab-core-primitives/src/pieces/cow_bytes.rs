use bytes::{Bytes, BytesMut};
use core::hash::{Hash, Hasher};
use core::{fmt, mem};

pub(super) enum CowBytes {
    Shared(Bytes),
    Owned(BytesMut),
}

impl fmt::Debug for CowBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.as_ref() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl PartialEq for CowBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}

impl Eq for CowBytes {}

impl Hash for CowBytes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl Clone for CowBytes {
    fn clone(&self) -> Self {
        match self {
            Self::Shared(bytes) => Self::Shared(bytes.clone()),
            // Always return shared clone
            Self::Owned(bytes) => Self::Shared(Bytes::copy_from_slice(bytes)),
        }
    }
}

impl AsRef<[u8]> for CowBytes {
    fn as_ref(&self) -> &[u8] {
        match self {
            CowBytes::Shared(bytes) => bytes.as_ref(),
            CowBytes::Owned(bytes) => bytes.as_ref(),
        }
    }
}

impl AsMut<[u8]> for CowBytes {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            CowBytes::Shared(bytes) => {
                *self = CowBytes::Owned(BytesMut::from(mem::take(bytes)));

                let CowBytes::Owned(bytes) = self else {
                    unreachable!("Just replaced; qed");
                };

                bytes.as_mut()
            }
            CowBytes::Owned(bytes) => bytes.as_mut(),
        }
    }
}
