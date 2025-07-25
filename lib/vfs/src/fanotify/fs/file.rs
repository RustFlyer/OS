use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use crate_interface::call_interface;

use config::vfs::FileInternalFlags;
use systype::error::{SysError, SysResult, SyscallResult};

use crate::file::{File, FileMeta};

use super::super::{FanotifyGroup, constants::FAN_NOFD, types::FanInitFlags};
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
    fn read_events(&self, mut buf: &mut [u8]) -> SysResult<usize> {
        let group = self.group();
        let group_flags = group.flags;
        let event_file_flags = group.event_file_flags;

        let mut total_read = 0;
        let mut is_first_event = true;
        let mut event_queue = group.event_queue.lock();

        while let Some(mut event) = event_queue.pop_front() {
            let event_object = event.object().cloned();
            let metadata = event.metadata_mut();

            let event_len = metadata.event_len as usize;
            if event_len > buf.len() {
                event_queue.push_front(event);
                if is_first_event {
                    // The buffer is too small to hold the event.
                    return Err(SysError::EINVAL);
                }
                break;
            }

            is_first_event = false;

            if let Some(event_object) = event_object {
                // Add the event file to the process's file descriptor table.
                metadata.fd = if group_flags
                    .intersects(FanInitFlags::REPORT_FID | FanInitFlags::REPORT_DIR_FID)
                {
                    FAN_NOFD
                } else {
                    let event_file = <dyn File>::open(event_object)?;
                    *event_file.meta().internal_flags.lock() |= FileInternalFlags::FMODE_NONOTIFY;

                    match call_interface!(
                        crate::fanotify::kinterface::KernelFdTableOperations::add_file(
                            event_file,
                            event_file_flags.into()
                        )
                    ) {
                        Ok(fd) => fd,
                        Err(e) => {
                            event_queue.push_front(event);
                            if is_first_event {
                                return Err(e);
                            } else {
                                break;
                            }
                        }
                    }
                };
            }

            log::info!(
                "Event metadata read: fd={}, pid={}, mask={:?}",
                metadata.fd,
                metadata.pid,
                metadata.mask
            );

            // Copy the metadata and information records into the buffer.
            for datum in event.iter() {
                let bytes = datum.as_slice();
                let len = bytes.len();
                buf[..len].copy_from_slice(bytes);
                buf = &mut buf[len..];
            }

            total_read += event_len;
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
        loop {
            let bytes_read = self.read_events(buf)?;
            if bytes_read > 0 {
                return Ok(bytes_read);
            } else if self.group().flags.contains(FanInitFlags::NONBLOCK) {
                return Err(SysError::EAGAIN);
            } else {
                let group = self.group();
                group.wait().await;
            }
        }
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
