use alloc::{
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use downcast_rs::{DowncastSync, impl_downcast};

use config::{
    inode::{InodeMode, InodeState, InodeType},
    vfs::AccessFlags,
};
use mm::page_cache::PageCache;
use mutex::SpinNoIrqLock;
use systype::{
    error::{SysError, SysResult},
    time::TimeSpec,
};

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
    /// user define
    pub xattrs: BTreeMap<String, Vec<u8>>,
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
                xattrs: BTreeMap::new(),
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

impl InodeMetaInner {
    pub fn set_xattr(&mut self, name: &str, value: &[u8], flags: i32) -> SysResult<()> {
        let exists = self.xattrs.contains_key(name);
        match flags {
            1 if exists => return Err(SysError::EEXIST),
            2 if !exists => return Err(SysError::ENODATA),
            _ => {}
        }
        self.xattrs.insert(name.to_string(), value.to_vec());
        Ok(())
    }

    pub fn get_xattr(&self, name: &str) -> SysResult<&[u8]> {
        self.xattrs
            .get(name)
            .map(|v| v.as_slice())
            .ok_or(SysError::ENODATA)
    }

    pub fn remove_xattr(&mut self, name: &str) -> SysResult<()> {
        if self.xattrs.remove(name).is_some() {
            Ok(())
        } else {
            Err(SysError::ENODATA)
        }
    }
}

pub trait Inode: Send + Sync + DowncastSync {
    fn get_meta(&self) -> &InodeMeta;

    fn get_attr(&self) -> SysResult<Stat>;

    fn get_uid(&self) -> u32 {
        self.get_meta().inner.lock().uid
    }

    fn get_gid(&self) -> u32 {
        self.get_meta().inner.lock().gid
    }

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

    fn set_xattr(&self, name: &str, value: &[u8], flags: i32) -> SysResult<()> {
        self.get_meta().inner.lock().set_xattr(name, value, flags)
    }

    fn get_xattr(&self, name: &str) -> SysResult<Vec<u8>> {
        self.get_meta()
            .inner
            .lock()
            .get_xattr(name)
            .map(|v| v.to_vec())
    }

    fn remove_xattr(&self, name: &str) -> SysResult<()> {
        self.get_meta().inner.lock().remove_xattr(name)
    }

    fn check_permission(&self, euid: u32, egid: u32, groups: &[u32], access: AccessFlags) -> bool {
        let meta = self.get_meta().inner.lock();
        let mode = meta.mode.bits();

        if euid == 0 {
            if access.contains(AccessFlags::X_OK) {
                if mode & 0o111 == 0 {
                    return false;
                }
            }
            return true;
        }

        let is_owner = euid == meta.uid;
        let is_group = egid == meta.gid || groups.contains(&meta.gid);

        let (r, w, x) = if is_owner {
            (0o400, 0o200, 0o100)
        } else if is_group {
            (0o040, 0o020, 0o010)
        } else {
            (0o004, 0o002, 0o001)
        };

        if access.contains(AccessFlags::R_OK) && (mode & r == 0) {
            return false;
        }
        if access.contains(AccessFlags::W_OK) && (mode & w == 0) {
            return false;
        }
        if access.contains(AccessFlags::X_OK) && (mode & x == 0) {
            return false;
        }
        true
    }
}

impl_downcast!(sync Inode);
