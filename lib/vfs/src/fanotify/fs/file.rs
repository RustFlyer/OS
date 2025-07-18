use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use crate_interface::call_interface;

use systype::error::{SysError, SysResult, SyscallResult};

use crate::file::{File, FileMeta};

use super::super::{FanotifyGroup, types::FanotifyEventData};
use super::inode::FanotifyGroupInode;

/// File implementation for fanotify group file descriptor.
pub struct FanotifyGroupFile {
    pub(crate) meta: FileMeta,
}

impl FanotifyGroupFile {
    /// Gets the associated fanotify group from the inode.
    pub fn group(&self) -> Arc<FanotifyGroup> {
        let inode = self
            .inode()
            .downcast_arc::<FanotifyGroupInode>()
            .unwrap_or_else(|_| panic!("Expected FanotifyGroupInode"));
        Arc::clone(inode.group())
    }

    /// Reads pending events from all entries in the fanotify group.
    fn read_events(&self, buf: &mut [u8]) -> SysResult<usize> {
        let group = self.group();
        let entries = group.entries.lock();

        let mut total_read = 0;

        // Collect events from entries.
        for entry in entries.values() {
            let mut event_queue = entry.event_queue.lock();

            while let Some(mut event) = event_queue.pop_front() {
                let (mut metadata, event_file) =
                    if let FanotifyEventData::Metadata(metadata) = &mut event {
                        (metadata.0, Arc::clone(&metadata.1))
                    } else {
                        panic!("Expected FanotifyEventData::Metadata");
                    };

                let mut event_read = 0;
                let event_len = metadata.event_len as usize;
                if total_read + event_len > buf.len() {
                    event_queue.push_front(event);
                    break;
                }

                // Add the event file to the process's file descriptor table.
                let fd = match call_interface!(
                    crate::fanotify::kinterface::KernelFdTableOperations::add_file(
                        event_file,
                        group.event_file_flags.into()
                    )
                ) {
                    Ok(fd) => fd,
                    Err(e) => {
                        event_queue.push_front(event);
                        return Err(e);
                    }
                };

                // Set the file descriptor in the event metadata.
                metadata.fd = fd;

                // Copy the metadata into the buffer.
                let metadata_len = metadata.metadata_len as usize;
                buf[total_read..total_read + metadata_len].copy_from_slice(event.as_slice());
                event_read += metadata_len;

                // Copy the optional information records into the buffer.
                while event_read < event_len {
                    let record = event_queue.pop_front().unwrap();
                    let record_len = record.as_slice().len();
                    buf[total_read + event_read..total_read + event_read + record_len]
                        .copy_from_slice(record.as_slice());
                    event_read += record_len;
                }

                debug_assert_eq!(event_read, event_len);
                total_read += event_len;
            }
        }

        Ok(total_read)
    }

    // TODO: Implement write methods after fanotify event file system is ready
    //
    // The write implementation should:
    // 1. Parse FanotifyResponse from the input buffer
    // 2. Find the permission event by matching the event_fd
    // 3. Close the fanotify event file descriptor (remove from process fd table)
    // 4. Apply the permission decision (allow/deny) to the original operation
    // 5. Remove the permission event from the queue
    //
    // This requires integration with:
    // - Process file descriptor table management
    // - Filesystem operation callback mechanism
    // - Permission decision application
}

#[async_trait]
impl File for FanotifyGroupFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _offset: usize) -> SyscallResult {
        self.read_events(buf)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SyscallResult {
        // TODO: Implement fanotify permission response handling
        // This should:
        // 1. Parse FanotifyResponse from buf
        // 2. Find the corresponding fanotify event file by fd
        // 3. Close the fanotify event file descriptor
        // 4. Apply the permission decision (allow/deny)
        unimplemented!()
    }

    fn base_read_dir(&self) -> SysResult<Option<crate::direntry::DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_readlink(&self, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::EACCES)
    }
}
