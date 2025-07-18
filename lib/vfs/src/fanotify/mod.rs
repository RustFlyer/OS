use alloc::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    sync::{Arc, Weak},
};

use mutex::SpinNoIrqLock;

use crate::{
    fanotify::types::{FanEventMask, FanotifyEventData},
    file::File,
    inode::Inode,
    superblock::SuperBlock,
};

use self::types::{FanEventFileFlags, FanInitFlags};

pub mod fs;

pub mod constants;
pub mod kinterface;
pub mod types;

/// An fanotify group.
///
/// It intercepts filesystem events on specified filesystem objects (files, directories,
/// mounts, and filesystems), notifies the user space about these events, and optionally
/// asks the user space for permission to proceed with the operation that caused the
/// event.
///
/// It contains a list of [`FanotifyEntry`]s, each of which corresponds to a filesystem
/// object under monitoring. The group can be used to manage the entries, such as adding
/// new entries and removing existing ones.
///
/// It is designed to be owned by the user space process which has the file descriptor
/// created by the `fanotify_init` syscall. It can also be owned by multiple processes
/// that share the same file descriptor. When all those processes closes the file
/// descriptor, or when all of them exit, the group is dropped, and all entries in the
/// group are dropped as well.
pub struct FanotifyGroup {
    /// The fanotify entries in the group, which correspond to the filesystem objects
    /// being monitored.
    entries: SpinNoIrqLock<BTreeMap<FsObjectId, Arc<FanotifyEntry>>>,

    /// The flags that specify the behavior of the fanotify group.
    flags: FanInitFlags,

    /// The file status flags that will be set on the file that are created for fanotify
    /// events.
    event_file_flags: FanEventFileFlags,
}

impl FanotifyGroup {
    /// Creates a new fanotify group with the specified flags and event file flags.
    pub fn new(flags: FanInitFlags, event_file_flags: FanEventFileFlags) -> Self {
        Self {
            entries: SpinNoIrqLock::new(BTreeMap::new()),
            flags,
            event_file_flags,
        }
    }

    /// Creates an fanotify entry for the specified filesystem object in the group,
    /// and registers it on the object.
    ///
    /// `object_id` is the identifier for `object`, which is used as the key in the
    /// fanotify group's entry map.
    ///
    /// `object` must contains a valid weak reference to a filesystem object.
    ///
    /// The object must not already have an entry in the group.
    pub fn create_entry(
        self: &Arc<FanotifyGroup>,
        object: FsObject,
        mark: FanEventMask,
        ignore: FanEventMask,
    ) {
        let entry = Arc::new(FanotifyEntry {
            group: Arc::downgrade(self),
            object,
            mark: SpinNoIrqLock::new(mark),
            ignore: SpinNoIrqLock::new(ignore),
            event_queue: SpinNoIrqLock::new(VecDeque::new()),
            permission_queue: SpinNoIrqLock::new(VecDeque::new()),
        });

        let object_id = match &entry.object {
            FsObject::Inode(inode) => {
                let inode = inode.upgrade().unwrap();
                FsObjectId::Inode(inode.ino())
            }
            FsObject::Mount(mount) => {
                let mount = mount.upgrade().unwrap();
                FsObjectId::Mount(mount.dev_id())
            }
        };

        let entry_weak = Arc::downgrade(&entry);
        match &entry.object {
            FsObject::Inode(inode) => {
                let inode = inode.upgrade().unwrap();
                inode
                    .get_meta()
                    .inner
                    .lock()
                    .fanotify_entries
                    .push(entry_weak);
            }
            FsObject::Mount(mount) => {
                let mount = mount.upgrade().unwrap();
                mount.meta().fanotify_entries.lock().push(entry_weak);
            }
        };

        let option = self.entries.lock().insert(object_id, entry);
        debug_assert!(
            option.is_none(),
            "Fanotify entry for object {:?} already exists in the fanotify group",
            object_id
        );
    }

    /// Removes an entry from the fanotify group.
    pub fn remove_entry(&self, object_id: FsObjectId) -> Option<Arc<FanotifyEntry>> {
        self.entries.lock().remove(&object_id)
    }

    /// Gets the flags of the fanotify group.
    pub fn flags(&self) -> FanInitFlags {
        self.flags
    }

    /// Gets the event file flags of the fanotify group.
    pub fn event_file_flags(&self) -> FanEventFileFlags {
        self.event_file_flags
    }
}

/// An entry in a fanotify group.
///
/// It corresponds to a specific filesystem object that is being monitored by the
/// fanotify group it belongs to.
///
/// Each entry contains two bit masks: the mark mask and the ignore mask. The mark mask
/// specifies the event kinds that the group is interested in for the filesystem object;
/// the ignore mask specifies the event kinds that the group is not interested in.
pub struct FanotifyEntry {
    /// The fanotify group this entry belongs to.
    ///
    /// This is a weak reference, because the group may be destroyed while some entries
    /// exist. For example, the group may be destroyed when the user process closes the
    /// file descriptor obtained from the `fanotify_init` syscall, or when the process
    /// exits. Maintaining weak references allows the entry to be dropped when the group
    /// is dropped.
    group: Weak<FanotifyGroup>,

    /// The inode of the filesystem object being monitored.
    ///
    /// This is a weak reference, because the filesystem object may be removed or
    /// become inaccessible.
    ///
    /// TODO: Consider using an enum to represent either a reference to an inode or a
    /// reference to a mount.
    object: FsObject,

    /// The mark mask, which specifies the event kinds that the group is interested in.
    mark: SpinNoIrqLock<FanEventMask>,

    /// The ignore mask, which specifies the event kinds that the group is not interested
    /// in.
    ignore: SpinNoIrqLock<FanEventMask>,

    /// Data of pending events on this entry.
    event_queue: SpinNoIrqLock<VecDeque<FanotifyEventData>>,

    /// Permission events that are waiting for responses from userspace.
    permission_queue: SpinNoIrqLock<VecDeque<FanotifyPermissionEvent>>,
}

impl FanotifyEntry {
    /// Adds events in `mark` to the mark mask of the entry.
    pub fn add_mark(&self, mark: FanEventMask) {
        *self.mark.lock() |= mark;
    }

    /// Removes events in `mark` from the mark mask of the entry.
    pub fn remove_mark(&self, mark: FanEventMask) {
        *self.mark.lock() &= !mark;
    }

    /// Adds events in `ignore` to the ignore mask of the entry.
    pub fn add_ignore(&self, ignore: FanEventMask) {
        *self.ignore.lock() |= ignore;
    }

    /// Removes events in `ignore` from the ignore mask of the entry.
    pub fn remove_ignore(&self, ignore: FanEventMask) {
        *self.ignore.lock() &= !ignore;
    }

    /// Inserts an event or events into the event queue of the entry.
    ///
    /// This method takes an iterator of [`FanotifyEventData`] and insert them into the
    /// event queue of this entry in order. Every event must start with an fanotify
    /// event metadata, followed by zero or more information records.
    pub fn insert_event(&self, events: impl Iterator<Item = FanotifyEventData>) {
        self.event_queue.lock().extend(events);
    }
}

/// Data structure for pending permission events.
///
/// This represents a permission event that is waiting for a response from userspace.
#[derive(Debug, Clone)]
pub struct FanotifyPermissionEvent {
    /// File descriptor of the fanotify event file created for this permission event.
    pub event_fd: i32,

    /// Process ID that triggered the event.
    pub pid: i32,

    /// The event mask that triggered this permission check.
    pub mask: FanEventMask,

    /// Handle to the fanotify event file (weak reference to avoid cycles).
    pub event_file: Weak<dyn File>,
    // TODO: This will need to be defined based on how filesystem operations are handled
    // pub permission_callback: Box<dyn FnOnce(bool) + Send + Sync>,
}

/// A filesystem object that can be monitored by fanotify.
#[derive(Clone)]
pub enum FsObject {
    /// An inode as a filesystem object.
    Inode(Weak<dyn Inode>),
    /// A mount as a filesystem object.
    Mount(Weak<dyn SuperBlock>),
}

/// An identifier for a filesystem object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FsObjectId {
    /// Inode number.
    Inode(i32),
    /// Mount ID.
    Mount(u64),
}
