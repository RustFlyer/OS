use alloc::{string::String, vec::Vec};
use core::mem;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct InotifyEvent {
    /// Watch descriptor
    pub wd: i32,
    /// Mask describing event
    pub mask: u32,
    /// Unique cookie associating related events
    pub cookie: u32,
    /// Length of name field
    pub len: u32,
    /// Optional name of affected file
    pub name: Option<String>,
}

impl InotifyEvent {
    pub fn new(wd: i32, mask: u32, cookie: u32, name: Option<String>) -> Self {
        let len = if let Some(ref n) = name {
            // Round up to next multiple of 4 (alignment)
            (n.len() + 1 + 3) & !3
        } else {
            0
        };

        Self {
            wd,
            mask,
            cookie,
            len: len as u32,
            name,
        }
    }

    /// Serialize event to buffer in Linux inotify format
    pub fn serialize_into(&self, buf: &mut [u8]) -> Result<usize, ()> {
        let base_size = mem::size_of::<i32>() * 3 + mem::size_of::<u32>();
        let total_size = base_size + self.len as usize;

        if buf.len() < total_size {
            return Err(());
        }

        // Write header
        let mut offset = 0;
        buf[offset..offset + 4].copy_from_slice(&self.wd.to_ne_bytes());
        offset += 4;
        buf[offset..offset + 4].copy_from_slice(&self.mask.to_ne_bytes());
        offset += 4;
        buf[offset..offset + 4].copy_from_slice(&self.cookie.to_ne_bytes());
        offset += 4;
        buf[offset..offset + 4].copy_from_slice(&self.len.to_ne_bytes());
        offset += 4;

        // Write name if present
        if let Some(ref name) = self.name {
            let name_bytes = name.as_bytes();
            buf[offset..offset + name_bytes.len()].copy_from_slice(name_bytes);
            offset += name_bytes.len();
            // Null terminator
            buf[offset] = 0;
            offset += 1;
            // Pad to alignment
            while offset < base_size + self.len as usize {
                buf[offset] = 0;
                offset += 1;
            }
        }

        Ok(total_size)
    }

    pub fn serialized_size(&self) -> usize {
        mem::size_of::<i32>() * 3 + mem::size_of::<u32>() + self.len as usize
    }
}

#[derive(Debug, Clone)]
pub struct InotifyWatch {
    pub wd: i32,
    pub inode_id: u64,
    pub mask: u32,
    pub path: Option<String>,
}

impl InotifyWatch {
    pub fn new(wd: i32, inode_id: u64, mask: u32, path: Option<String>) -> Self {
        Self {
            wd,
            inode_id,
            mask,
            path,
        }
    }
}
