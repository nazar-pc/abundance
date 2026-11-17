//! Files abstraction that allows reading concurrently using thread pool

use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_aligned_buffer::alignment::Align1;
use ab_direct_io_file::{IoBufError, OwnedPages};
use ab_farmer_components::ReadAtSync;
use ab_farmer_components::file_ext::{FileExt, OpenOptionsExt};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::{io, iter};

/// Wrapper data structure for multiple files to be used with [`rayon`] thread pool, where the same
/// file is opened multiple times, once for each thread for faster concurrent reads.
///
/// The benefit of multiple handles was measured (it is especially large on Windows), don't replace
/// them with a single shared file.
#[derive(Debug)]
pub struct RayonFiles<File> {
    files: Vec<File>,
}

impl<File> ReadAtSync for RayonFiles<File>
where
    File: ReadAtSync,
{
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        let thread_index = rayon::current_thread_index().unwrap_or_default();
        let Some(file) = self.files.get(thread_index) else {
            return Err(IoBufError {
                error: io::Error::other("No files entry for this rayon thread").into(),
                buffer,
            });
        };

        file.read_at(buffer, len, offset)
    }
}

impl RayonFiles<File> {
    /// Open file at specified path as many times as there is number of threads in current [`rayon`]
    /// thread pool.
    pub fn open<P>(path: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let files = iter::repeat_with(|| {
            let file = OpenOptions::new()
                .read(true)
                .advise_random_access()
                .open(path.as_ref())?;
            file.advise_random_access()?;

            Ok::<_, io::Error>(file)
        })
        .take(rayon::current_num_threads())
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { files })
    }
}

impl<File> RayonFiles<File>
where
    File: ReadAtSync,
{
    /// Open file at specified path as many times as there is number of threads in current [`rayon`]
    /// thread pool with a provided function
    pub fn open_with<P>(path: P, open: fn(&Path) -> io::Result<File>) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let files = iter::repeat_with(|| open(path.as_ref()))
            .take(rayon::current_num_threads())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { files })
    }
}
