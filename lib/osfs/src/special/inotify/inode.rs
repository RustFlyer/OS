use alloc::sync::Arc;
use alloc::{collections::VecDeque, vec::Vec};
use core::{
    sync::atomic::{AtomicI32, Ordering},
    task::Waker,
};
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::{
    event::{InotifyEvent, InotifyWatch},
    flags::InotifyFlags,
};

pub struct InotifyInode {
    meta: InodeMeta,
    flags: InotifyFlags,
    next_wd: AtomicI32,
    watches: SpinNoIrqLock<alloc::collections::BTreeMap<i32, InotifyWatch>>,
    events: SpinNoIrqLock<VecDeque<InotifyEvent>>,
    wakers: SpinNoIrqLock<Vec<Waker>>,
    max_events: usize,
}

impl InotifyInode {
    pub fn new(flags: InotifyFlags) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            flags,
            next_wd: AtomicI32::new(1),
            watches: SpinNoIrqLock::new(alloc::collections::BTreeMap::new()),
            events: SpinNoIrqLock::new(VecDeque::new()),
            wakers: SpinNoIrqLock::new(Vec::new()),
            max_events: 16384, // Default max events
        })
    }

    pub fn add_watch(
        &self,
        inode_id: u64,
        mask: u32,
        path: Option<alloc::string::String>,
    ) -> SysResult<i32> {
        let wd = self.next_wd.fetch_add(1, Ordering::SeqCst);
        let watch = InotifyWatch::new(wd, inode_id, mask, path);

        self.watches.lock().insert(wd, watch);
        Ok(wd)
    }

    pub fn remove_watch(&self, wd: i32) -> SysResult<()> {
        let mut watches = self.watches.lock();
        if watches.remove(&wd).is_some() {
            // Generate IN_IGNORED event
            self.push_event_internal(InotifyEvent::new(
                wd,
                super::flags::InotifyMask::IN_IGNORED.bits(),
                0,
                None,
            ));
            Ok(())
        } else {
            Err(SysError::EINVAL)
        }
    }

    pub fn push_event(&self, event: InotifyEvent) {
        self.push_event_internal(event);
    }

    fn push_event_internal(&self, event: InotifyEvent) {
        let mut events = self.events.lock();

        if events.len() >= self.max_events {
            // Remove oldest event and add overflow event
            events.pop_front();
            if !events
                .iter()
                .any(|e| e.mask & super::flags::InotifyMask::IN_Q_OVERFLOW.bits() != 0)
            {
                events.push_back(InotifyEvent::new(
                    -1,
                    super::flags::InotifyMask::IN_Q_OVERFLOW.bits(),
                    0,
                    None,
                ));
            }
        }

        events.push_back(event);
        self.wake_all_readers();
    }

    fn wake_all_readers(&self) {
        let mut wakers = self.wakers.lock();
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    pub fn register_waker(&self, waker: Waker) {
        let mut wakers = self.wakers.lock();
        if self.has_events_internal() {
            waker.wake();
        } else {
            if !wakers.iter().any(|w| w.will_wake(&waker)) {
                wakers.push(waker);
            }
        }
    }

    pub fn read_events(&self, buf: &mut [u8]) -> SysResult<usize> {
        let mut events = self.events.lock();
        let mut total_bytes = 0;
        let mut buf_offset = 0;

        while let Some(event) = events.front() {
            let event_size = event.serialized_size();
            if buf_offset + event_size > buf.len() {
                break;
            }

            let event = events.pop_front().unwrap();
            match event.serialize_into(&mut buf[buf_offset..]) {
                Ok(bytes) => {
                    buf_offset += bytes;
                    total_bytes += bytes;
                }
                Err(_) => break,
            }
        }

        if total_bytes == 0 && self.flags.contains(InotifyFlags::IN_NONBLOCK) {
            return Err(SysError::EAGAIN);
        }

        Ok(total_bytes)
    }

    pub fn has_events(&self) -> bool {
        self.has_events_internal()
    }

    fn has_events_internal(&self) -> bool {
        !self.events.lock().is_empty()
    }

    pub fn get_flags(&self) -> InotifyFlags {
        self.flags
    }

    pub fn notify_inode_event(
        &self,
        inode_id: u64,
        mask: u32,
        name: Option<alloc::string::String>,
    ) {
        let watches = self.watches.lock();
        for (wd, watch) in watches.iter() {
            if watch.inode_id == inode_id && (watch.mask & mask) != 0 {
                let event = InotifyEvent::new(*wd, mask, 0, name.clone());
                drop(watches); // Release lock before pushing event
                self.push_event_internal(event);
                return;
            }
        }
    }
}

impl Inode for InotifyInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: config::inode::InodeMode::REG.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: 0,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }

    fn set_size(&self, _size: usize) -> SysResult<()> {
        Err(SysError::EINVAL)
    }
}
