use alloc::sync::Arc;

use downcast_rs::{DowncastSync, impl_downcast};

use config::inode::{InodeMode, InodeState, InodeType};
use mm::page_cache::PageCache;
use mutex::SpinNoIrqLock;
use systype::{error::SysResult, time::TimeSpec};

use crate::{stat::Stat, superblock::SuperBlock};

/// Data that is common to all inodes.
pub struct InodeMeta {
    /// Inode number of the inode in its filesystem.
    pub ino: usize,
    /// Reference to the superblock of the filesystem this inode belongs to.
    pub superblock: Arc<dyn SuperBlock>,
    /// Page cache for the inode. If the inode is not a regular file or a block
    /// device, this field is not used.
    pub page_cache: PageCache,
    /// Interior mutable data of the inode.
    pub inner: SpinNoIrqLock<InodeMetaInner>,
}

pub struct InodeMetaInner {
    /// Mode of the inode.
    ///
    /// This includes the type of the inode (regular file, directory, etc.),
    /// and group/user permissions.
    pub mode: InodeMode,
    /// Size of a file in bytes.
    pub size: usize,
    /// Link count.
    pub nlink: usize,
    /// Last access time.
    pub atime: TimeSpec,
    /// Last modification time.
    pub mtime: TimeSpec,
    /// Last status change time.
    pub ctime: TimeSpec,
    /// State of the inode.
    pub state: InodeState,
    /// uid of the inode.
    pub uid: u32,
    /// gid of the inode.
    pub gid: u32,
}

impl InodeMeta {
    /// Creates a default inode metadata. The caller should fill each field after this call.
    pub fn new(ino: usize, superblock: Arc<dyn SuperBlock>) -> Self {
        Self {
            ino,
            superblock,
            page_cache: PageCache::default(),
            inner: SpinNoIrqLock::new(InodeMetaInner {
                mode: InodeMode::empty(),
                size: 0,
                nlink: 0,
                atime: TimeSpec::default(),
                mtime: TimeSpec::default(),
                ctime: TimeSpec::default(),
                state: InodeState::Uninit,
                uid: 0,
                gid: 0,
            }),
        }
    }
}

impl Drop for InodeMeta {
    fn drop(&mut self) {
        match self.inner.lock().state {
            InodeState::Uninit => {}
            InodeState::DirtyInode | InodeState::DirtyData | InodeState::DirtyAll => {
                log::trace!("Drop inode {} with dirty state", self.ino);
                // TODO: flush dirty data
            }
            InodeState::Synced => {}
        }
    }
}

pub trait Inode: Send + Sync + DowncastSync {
    fn get_meta(&self) -> &InodeMeta;

    fn get_attr(&self) -> SysResult<Stat>;

    fn ino(&self) -> usize {
        self.get_meta().ino
    }

    fn inotype(&self) -> InodeType {
        self.get_meta().inner.lock().mode.to_type()
    }

    fn size(&self) -> usize {
        self.get_meta().inner.lock().size
    }

    fn set_size(&self, size: usize) {
        self.get_meta().inner.lock().size = size;
    }

    fn state(&self) -> InodeState {
        self.get_meta().inner.lock().state
    }

    fn set_nlink(&self, nlink: usize) {
        self.get_meta().inner.lock().nlink = nlink;
    }

    fn set_time(&self, ts: TimeSpec) {
        self.get_meta().inner.lock().atime = ts;
        self.get_meta().inner.lock().ctime = ts;
        self.get_meta().inner.lock().mtime = ts;
    }

    fn set_state(&self, state: InodeState) {
        self.get_meta().inner.lock().state = state;
    }

    fn set_inotype(&self, inotype: InodeType) {
        self.get_meta().inner.lock().mode = InodeMode::from_type(inotype);
    }

    fn set_mode(&self, mode: InodeMode) {
        self.get_meta().inner.lock().mode = mode;
    }

    fn set_uid(&self, uid: u32) {
        self.get_meta().inner.lock().uid = uid;
    }

    fn set_gid(&self, gid: u32) {
        self.get_meta().inner.lock().gid = gid;
    }

    fn superblock(&self) -> Arc<dyn SuperBlock> {
        Arc::clone(&self.get_meta().superblock)
    }

    fn page_cache(&self) -> &PageCache {
        &self.get_meta().page_cache
    }
}

impl_downcast!(sync Inode);
