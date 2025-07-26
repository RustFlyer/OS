use alloc::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    string::{String, ToString},
    sync::{Arc, Weak},
};
use core::mem;

use crate_interface::call_interface;

use mutex::SpinNoIrqLock;

use crate::{
    dentry::Dentry, fanotify::types::FanMarkFlags, file::File, inode::Inode, superblock::SuperBlock,
};

use self::{
    constants::FANOTIFY_METADATA_VERSION,
    types::{
        FanEventFileFlags, FanEventMask, FanInitFlags, FanotifyEventData, FanotifyEventInfoFid,
        FanotifyEventInfoFidInner, FanotifyEventInfoHeader, FanotifyEventInfoType,
        FanotifyEventMetadata,
    },
};

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

    /// Subscribes to events on a filesystem object.
    ///
    /// This method creates an entry in the fanotify group for the specified filesystem
    /// object, with the specified flags, mark mask, and ignore mask. After this call, the
    /// group will receive events of interest on the object.
    ///
    /// `object_id` is the identifier for `object`, which is used as the key in the
    /// fanotify group's entry map.
    ///
    /// `object` must contains a valid weak reference to a filesystem object.
    ///
    /// The object must not already have an entry in the group.
    pub fn add_entry(
        self: &Arc<Self>,
        object: FsObject,
        flags: FanMarkFlags,
        mark: FanEventMask,
        ignore: FanEventMask,
    ) {
        let entry = Arc::new(FanotifyEntry {
            group: Arc::downgrade(self),
            object,
            flags: SpinNoIrqLock::new(flags),
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

    /// Gets an entry in the fanotify group by its object ID.
    pub fn get_entry(&self, object_id: FsObjectId) -> Option<Arc<FanotifyEntry>> {
        self.entries.lock().get(&object_id).cloned()
    }

    /// Unsubscribes from events on a filesystem object. After this call, the group
    /// will no longer receive events for the object.
    pub fn remove_entry(&self, object_id: FsObjectId) -> Option<Arc<FanotifyEntry>> {
        self.entries.lock().remove(&object_id)
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

    /// The flags that specify the behavior of the fanotify entry.
    ///
    /// This field has only 3 meaningful bits; other bits are ignored. They are:
    /// - `FAN_MARK_IGNORED` and `FAN_MARK_IGNORE`: These flags specify which ignore
    ///   style the entry uses. If `FAN_MARK_IGNORE` is set, successive `fanotify_mark`
    ///   calls that specify `FAN_MARK_IGNORED` will cause EEXIST error.
    /// - `FAN_MARK_IGNORED_SURV_MODIFY`: This flag specifies that whether the ignore
    ///   mask survives modifications to the filesystem object. If this flag is set
    ///   with `FAN_MARK_IGNORED`, successive `fanotify_mark` calls that do not specify
    ///   it will cause EEXIST error.
    flags: SpinNoIrqLock<FanMarkFlags>,

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
    /// Returns the flags of the entry.
    pub fn flags(&self) -> FanMarkFlags {
        *self.flags.lock()
    }

    /// Sets the flags of the entry.
    pub fn set_flags(&self, flags: FanMarkFlags) {
        *self.flags.lock() = flags;
    }

    /// Returns the mark mask of the entry.
    pub fn mark(&self) -> FanEventMask {
        *self.mark.lock()
    }

    /// Returns the ignore mask of the entry.
    pub fn ignore(&self) -> FanEventMask {
        *self.ignore.lock()
    }

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

    /// Publishes an event to the fanotify entry's event queue.
    ///
    /// This method is called when the current task performs a filesystem action on a
    /// filesystem object that may be monitored by this fanotify entry. A detailed
    /// description of when this method should be called is provided below.
    ///
    /// This method checks whether the action is of interest to the entry (i.e., whether
    /// it is in the mark mask and not in the ignore mask), and if so, it creates event
    /// data for the action and adds them to the entry's event queue. The event data
    /// starts with an fanotify event metadata, followed by zero or more information
    /// records.
    ///
    /// # Parameters
    ///
    /// `object` is a reference to the filesystem object's dentry on which the event
    /// occurred. By “occur on”, we mean:
    /// - For file events occurred on a file (FAN_OPEN, FAN_ACCESS, FAN_CLOSE_NOWRITE,
    ///   FAN_CLOSE_WRITE, etc.), this is the dentry of the accessed file.
    /// - For file events occurred on a directory (FAN_OPEN, FAN_ACCESS,
    ///   FAN_CLOSE_NOWRITE, etc.), this is the dentry of the accessed directory.
    /// - For directory events occurred on a directory (FAN_CREATE, FAN_DELETE, etc.),
    ///   this is the dentry of the directory whose dentries were modified.
    /// - Some file events cannot occur on a directory, such as FAN_CLOSE_WRITE. All
    ///   directory events cannot occur on a file. For these events, this method
    ///   must not be called.
    ///
    /// `event` must contain one bit set, which specifies the event kind that occurred.
    /// Additionally, it should contain FAN_ONDIR for file events which occurred on a
    /// directory and for directory events where the directory's modified dentry is a
    /// directory. Additionally, it should contain FAN_EVENT_ON_CHILD for file events
    /// that occurred on a child of the directory that this fanotify entry is marked on.
    ///
    /// `old_name` and `new_name` are set as follows:
    /// - For file events occurred on a file, both of them are the name of the accessed
    ///   file.
    /// - For file events occurred on a directory, both of them are `.`.
    /// - For directory events occurred on a directory, both of them are the name of the
    ///   directory's modified dentry that was modified. As a special case, for rename
    ///   events, `old_name` and `new_name` are the names of the directory's dentry
    ///   before and after being renamed, respectively.
    ///
    /// # Circumstances for calling this method
    ///
    /// If an event occurs on a filesystem object, then this method should be called
    /// exactly on the following fanotify entries if they exist:
    /// - The fanotify entry marked on the filesystem object itself.
    /// - The fanotify entry marked on the parent directory of the filesystem object, if
    ///   the event is a file event. FAN_EVENT_ON_CHILD is set in `event` in and only in
    ///   this case.
    /// - The fanotify entry marked on the mount that the filesystem object belongs to.
    /// - The fanotify entry marked on the filesystem that the filesystem object belongs
    ///   to.
    pub fn publish(
        &self,
        object: &Arc<dyn Dentry>,
        event: FanEventMask,
        old_name: &str,
        new_name: &str,
    ) {
        let mark = *self.mark.lock();
        let ignore = *self.ignore.lock();

        if event.contains(FanEventMask::ONDIR) && !ignore.contains(FanEventMask::ONDIR) {
            if !mark.contains(event) {
                return;
            }
        } else if !mark.difference(ignore).contains(event) {
            return;
        }

        log::info!(
            "[FanotifyEntry::publish] Publishing event: \
            mark={:?}, ignore={:?}, event={:?}, old_name={}, new_name={}",
            mark,
            ignore,
            event,
            old_name,
            new_name
        );

        let group = self.group.upgrade().unwrap();
        let group_flags = group.flags;

        let mut event_data = VecDeque::new();
        let event_dentry = Arc::clone(object);

        // Metadata.
        let metadata = Self::create_metadata(group_flags, event);
        event_data.push_back(FanotifyEventData::Metadata((metadata, event_dentry)));

        // Information records.
        if group_flags.contains(FanInitFlags::REPORT_FID) {
            let info = Self::create_fid_info(object, FanotifyEventInfoType::Fid, None);
            event_data.push_back(FanotifyEventData::Info(info));
        }
        if group_flags.contains(FanInitFlags::REPORT_DIR_FID) {
            let reported_object = if object.inode().unwrap().inotype().is_dir() {
                Some(Arc::clone(object))
            } else {
                object.parent()
            };
            if let Some(reported_object) = reported_object {
                if !group_flags.contains(FanInitFlags::REPORT_NAME) {
                    let info =
                        Self::create_fid_info(&reported_object, FanotifyEventInfoType::Dfid, None);
                    event_data.push_back(FanotifyEventData::Info(info));
                } else if event.contains(FanEventMask::RENAME) {
                    let info_old = Self::create_fid_info(
                        &reported_object,
                        FanotifyEventInfoType::OldDfidName,
                        Some(old_name.to_string()),
                    );
                    let info_new = Self::create_fid_info(
                        &reported_object,
                        FanotifyEventInfoType::NewDfidName,
                        Some(new_name.to_string()),
                    );
                    event_data.push_back(FanotifyEventData::Info(info_old));
                    event_data.push_back(FanotifyEventData::Info(info_new));
                } else {
                    let info = Self::create_fid_info(
                        &reported_object,
                        FanotifyEventInfoType::DfidName,
                        Some(old_name.to_string()),
                    );
                    event_data.push_back(FanotifyEventData::Info(info));
                }
            }
        }

        // Other information records should be added here...

        // Set the event length in the metadata.
        let event_len = event_data
            .iter()
            .map(|data| data.as_slice().len())
            .sum::<usize>() as u32;
        if let FanotifyEventData::Metadata((ref mut metadata, _)) = event_data[0] {
            metadata.event_len = event_len;
        } else {
            unreachable!()
        }

        // Add the event data to the entry's event queue.
        let mut event_queue = self.event_queue.lock();
        event_queue.extend(event_data);
    }

    /// Clears the ignore mask of the entry if the entry is not marked with
    /// `FAN_MARK_IGNORED_SURV_MODIFY`.
    ///
    /// This method should be called when the file that the entry is monitoring is
    /// modified (by writing, truncating, etc.).
    pub fn clear_ignore(&self) {
        if !self
            .flags
            .lock()
            .contains(FanMarkFlags::IGNORED_SURV_MODIFY)
        {
            log::info!(
                "[FanotifyEntry::clear_ignore] Clearing ignore mask: {:?}",
                *self.ignore.lock()
            );
            *self.ignore.lock() = FanEventMask::empty();
        }
    }

    /// Creates an fanotify event metadata for the specified group flags and event mask.
    ///
    /// The following two field are not set in this function:
    /// * `event_len` should be set before the metadata is inserted into the event queue.
    /// * `fd` should be set when the event is read by the user monitor process.
    fn create_metadata(group_flags: FanInitFlags, mask: FanEventMask) -> FanotifyEventMetadata {
        FanotifyEventMetadata {
            event_len: 0,
            vers: FANOTIFY_METADATA_VERSION,
            reserved: 0,
            metadata_len: mem::size_of::<FanotifyEventMetadata>() as u16,
            fd: 0,
            pid: if group_flags.contains(FanInitFlags::REPORT_TID) {
                call_interface!(systype::kinterface::KernelTaskOperations::current_pid())
            } else {
                call_interface!(systype::kinterface::KernelTaskOperations::current_tid())
            },
            mask,
        }
    }

    /// Creates an information record of structure type [`FanotifyEventInfoType`]
    /// containing a file handle for the specified filesystem object.
    ///
    /// `file_name` is only used for creating the [`FanotifyEventInfoFid`] of type
    /// [`FanotifyEventInfoType::DfidName`], [`FanotifyEventInfoType::OldDfidName`],
    /// or [`FanotifyEventInfoType::NewDfidName`].
    fn create_fid_info(
        object: &Arc<dyn Dentry>,
        info_type: FanotifyEventInfoType,
        file_name: Option<String>,
    ) -> FanotifyEventInfoFid {
        debug_assert!(
            file_name.is_none()
                || info_type == FanotifyEventInfoType::DfidName
                || info_type == FanotifyEventInfoType::OldDfidName
                || info_type == FanotifyEventInfoType::NewDfidName,
        );

        let mut handle_bytes = object.file_handle().to_raw_bytes();
        if let Some(name) = file_name {
            handle_bytes.extend_from_slice(name.as_bytes());
            handle_bytes.push(0); // Null terminator
        }

        let header = FanotifyEventInfoHeader {
            info_type,
            pad: 0,
            len: (mem::size_of::<FanotifyEventInfoFidInner>() + handle_bytes.len()) as u16,
        };
        let fsid = [0; 2];

        let mut info = FanotifyEventInfoFid::new(handle_bytes.len());
        info.set_hdr(header);
        info.set_fsid(fsid);
        info.handle_mut().copy_from_slice(handle_bytes.as_slice());
        info
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
