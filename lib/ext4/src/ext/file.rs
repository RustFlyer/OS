use alloc::ffi::CString;
use core::mem::MaybeUninit;
use log::debug;
use simdebug::stop;

use lwext4_rust::bindings::{
    SEEK_CUR, SEEK_END, SEEK_SET, ext4_fclose, ext4_file, ext4_fopen2, ext4_fread, ext4_fseek,
    ext4_fsize, ext4_ftell, ext4_ftruncate, ext4_fwrite,
};

/// Wrapper for C-interface `ext4_file` struct which represents a file.
pub struct ExtFile(ext4_file);

/// Enumeration for file seek types.
/// This is used in [`ExtFile::seek`] to specify how to interpret the offset.
#[repr(u32)]
pub enum FileSeekType {
    SeekSet = SEEK_SET,
    SeekCur = SEEK_CUR,
    SeekEnd = SEEK_END,
}

impl Drop for ExtFile {
    fn drop(&mut self) {
        unsafe {
            ext4_fclose(&mut self.0);
        }
    }
}

impl ExtFile {
    /// Opens a file at the given path with the specified flags and returns a handle to it.
    pub fn open(path: &str, flags: i32) -> Result<Self, i32> {
        let c_path = CString::new(path).unwrap();
        let mut file: MaybeUninit<ext4_file> = MaybeUninit::uninit();
        let err = unsafe { ext4_fopen2(file.as_mut_ptr(), c_path.as_ptr(), flags) };
        log::info!("extfile open: {err}");
        match err {
            0 => unsafe { Ok(Self(file.assume_init())) },
            e => {
                log::warn!("ext4_fopen failed: {}, error = {}", path, err);
                Err(e)
            }
        }
    }

    /// Reads data from the file into the provided buffer. This function will try to
    /// read `buf.len()` bytes into `buf`, but it may read fewer bytes if it reaches EOF.
    /// This function will advance the file offset.
    ///
    /// Returns the number of bytes read. If it returns 0, it means it reached EOF.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, i32> {
        let mut count = 0;
        // stop();
        let err = unsafe { ext4_fread(&mut self.0, buf.as_mut_ptr() as _, buf.len(), &mut count) };
        // debug!("{:?}", buf);

        match err {
            0 => Ok(count),
            e => {
                log::warn!("ext4_fread failed: error = {}", err);
                Err(e)
            }
        }
    }

    /// Writes data from the provided buffer to the file. This function will try to
    /// write `buf.len()` bytes from `buf` to the file, but it may write fewer bytes
    /// if there is not enough space. This function will advance the file offset.
    ///
    /// Returns the number of bytes written.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, i32> {
        let mut count = 0;
        let err = unsafe { ext4_fwrite(&mut self.0, buf.as_ptr() as _, buf.len(), &mut count) };

        match err {
            0 => Ok(count),
            e => {
                log::warn!("ext4_fwrite failed: error = {}", err);
                Err(e)
            }
        }
    }

    /// Seeks to a specific position in the file.
    ///
    /// `offset` is the number of bytes to seek. `seek_type` specifies how to interpret the
    /// offset:
    /// - `SeekSet`: Seek from the beginning of the file.
    /// - `SeekCur`: Seek from the current position in the file.
    /// - `SeekEnd`: Seek from the end of the file.
    pub fn seek(&mut self, offset: i64, seek_type: FileSeekType) -> Result<(), i32> {
        let err = unsafe { ext4_fseek(&mut self.0, offset, seek_type as u32) };
        match err {
            0 => Ok(()),
            _ => {
                log::warn!("ext4_fseek failed: error = {}", err);
                Err(err)
            }
        }
    }

    /// Returns the current position in the file.
    pub fn tell(&mut self) -> u64 {
        unsafe { ext4_ftell(&mut self.0) }
    }

    /// Returns the size of the file in bytes.
    pub fn size(&mut self) -> u64 {
        unsafe { ext4_fsize(&mut self.0) }
    }

    /// Truncates the file to the specified size.
    ///
    /// This function will change the size of the file to `size` bytes. If the file size
    /// is larger than `size`, the extra data will be discarded. If the file size is
    /// smaller than `size`, the file will be padded with zeros.
    pub fn truncate(&mut self, size: u64) -> Result<(), i32> {
        let err = unsafe { ext4_ftruncate(&mut self.0, size) };
        match err {
            0 => Ok(()),
            e => {
                log::warn!("ext4_ftruncate failed: error = {}", err);
                Err(e)
            }
        }
    }
}
