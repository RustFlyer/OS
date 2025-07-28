use alloc::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    slice,
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{cmp, mem, ptr, sync::atomic, task::Waker};

use crate_interface::call_interface;

use mutex::SpinNoIrqLock;
use osfuture::{suspend_now, take_waker};

use crate::{
    dentry::Dentry,
    fanotify::{
        config::MAX_QUEUED_EVENTS,
        types::{FanMarkFlags, FanotifyEventInfoError, FanotifyEventInfoPid},
    },
    file::File,
    inode::Inode,
    superblock::SuperBlock,
};

use self::{
    constants::FANOTIFY_METADATA_VERSION,
    types::{
        FanEventFileFlags, FanEventMask, FanInitFlags, FanotifyEventInfoFid,
        FanotifyEventInfoFidInner, FanotifyEventInfoHeader, FanotifyEventInfoType,
        FanotifyEventMetadata,
    },
};

pub mod config;
pub mod constants;
pub mod fs;
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

    /// Data of pending events on this group.
    event_queue: SpinNoIrqLock<VecDeque<FanotifyEventData>>,

    /// Maximum number of events that can be queued in the event queue.
    max_queued_events: u32,
}

impl FanotifyGroup {
    /// Creates a new fanotify group with the specified flags and event file flags.
    pub fn new(flags: FanInitFlags, event_file_flags: FanEventFileFlags) -> Self {
        Self {
            entries: SpinNoIrqLock::new(BTreeMap::new()),
            flags,
            event_file_flags,
            event_queue: SpinNoIrqLock::new(VecDeque::new()),
            max_queued_events: if flags.contains(FanInitFlags::UNLIMITED_QUEUE) {
                u32::MAX
            } else {
                MAX_QUEUED_EVENTS.load(atomic::Ordering::Relaxed)
            },
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

    /// Removes all entries in the group that are a file or a directory.
    pub fn flush_normal_entries(&self) {
        let mut entries = self.entries.lock();
        entries.retain(|_, entry| !matches!(entry.object, FsObject::Inode(_)));
    }

    /// Removes all entries in the group that are a mount.
    pub fn flush_mount_entries(&self) {
        let mut entries = self.entries.lock();
        entries.retain(|_, entry| !matches!(entry.object, FsObject::Mount(_)));
    }

    /// Removes all entries in the group that are a filesystem.
    pub fn flush_filesystem_entries(&self) {
        let mut entries = self.entries.lock();
        entries.retain(|_, entry| !matches!(entry.object, FsObject::Mount(_)));
    }

    /// Publishes an event to the fanotify group's event queue.
    ///
    /// This method is called when the current task performs a filesystem action on a
    /// filesystem object that the fanotify group is monitoring.
    ///
    /// This method creates event data for the action and adds it to the group's event
    /// queue. The event data starts with an fanotify event metadata, followed by zero or
    /// more information records.
    ///
    /// # Parameters
    ///
    /// `object` is a reference to the filesystem object's dentry on which the event
    /// occurred. By “occur on” or “event on” a filesystem object, we mean:
    /// - For file events occurred on a file (FAN_OPEN, FAN_ACCESS, FAN_CLOSE_NOWRITE,
    ///   FAN_CLOSE_WRITE, etc.), this is the dentry of the accessed file.
    /// - For file events occurred on a directory (FAN_OPEN, FAN_ACCESS,
    ///   FAN_CLOSE_NOWRITE, etc.), this is the dentry of the accessed directory.
    /// - For directory events occurred on a directory (FAN_CREATE, FAN_DELETE, etc.),
    ///   this is the dentry of the directory whose dentries were modified.
    ///
    /// Note that some file events cannot occur on a directory, such as FAN_CLOSE_WRITE.
    /// All directory events cannot occur on a file.
    ///
    /// `entries` contains fanotify entries in this group that are associated with the
    /// filesystem object. Specifically, it contains the entries marked on the object
    /// itself, on its parent directory, on the mount the object belongs to, and on the
    /// filesystem the object belongs to. If any of these entries does not exist, the
    /// corresponding entry in the structure is `None`. If all of them do not exist, this
    /// method needs not be called.
    ///
    /// `event` must contain one bit set, which specifies the event kind that occurred.
    /// Additionally, it should contain FAN_ONDIR for file events which occurred on a
    /// directory, and for directory events where the directory's modified dentry is a
    /// directory.
    ///
    /// `old_name` and `new_name` should be set as follows:
    /// - For file events occurred on a file, both of them are the name of the accessed
    ///   file.
    /// - For file events occurred on a directory, both of them are `.`.
    /// - For directory events occurred on a directory, both of them are the name of the
    ///   directory's modified dentry. As a special case, for rename events, `old_name`
    ///   and `new_name` are the names of the directory's dentry before and after being
    ///   renamed, respectively.
    pub(crate) fn publish(
        self: Arc<Self>,
        object: &Arc<dyn Dentry>,
        subobject: Option<&Arc<dyn Dentry>>,
        entries: FanotifyEntrySet,
        event: FanEventMask,
        old_name: &str,
        new_name: &str,
    ) {
        // Check whether the event is marked and not ignored.
        let object_entry = entries.object;
        let parent_entry = entries.parent;
        let mount_entry = entries.mount;
        let fs_entry = entries.fs;

        let object_mark = object_entry
            .clone()
            .map_or(FanEventMask::empty(), |e| e.mark());
        let parent_mark = parent_entry
            .clone()
            .map_or(FanEventMask::empty(), |e| e.mark());
        let mount_mark = mount_entry
            .clone()
            .map_or(FanEventMask::empty(), |e| e.mark());
        let fs_mark = fs_entry.clone().map_or(FanEventMask::empty(), |e| e.mark());

        if !(object_mark.contains(event)
            || ((FanEventMask::FILE_EVENT_MASK | FanEventMask::ONDIR).contains(event)
                && parent_mark.contains(event | FanEventMask::EVENT_ON_CHILD))
            || mount_mark.contains(event)
            || fs_mark.contains(event))
        {
            log::debug!(
                "[FanotifyGroup::publish] Event not marked: event={:?}, object={:?}, \
                object_mark={:?}, parent_mark={:?}, mount_mark={:?}, fs_mark={:?}",
                event,
                object.path(),
                object_mark,
                parent_mark,
                mount_mark,
                fs_mark
            );
            return;
        }

        let object_ignore = object_entry.map_or(FanEventMask::empty(), |e| e.ignore());
        let parent_ignore = parent_entry.map_or(FanEventMask::empty(), |e| e.ignore());
        let mount_ignore = mount_entry.map_or(FanEventMask::empty(), |e| e.ignore());
        let fs_ignore = fs_entry.map_or(FanEventMask::empty(), |e| e.ignore());

        if object_ignore.contains(event)
            || (FanEventMask::FILE_EVENT_MASK | FanEventMask::ONDIR).contains(event)
                && parent_ignore.contains(event | FanEventMask::EVENT_ON_CHILD)
            || mount_ignore.contains(event)
            || fs_ignore.contains(event)
        {
            log::debug!(
                "[FanotifyGroup::publish] Event ignored: event={:?}, object={:?}, \
                object_ignore={:?}, parent_ignore={:?}, mount_ignore={:?}, fs_ignore={:?}",
                event,
                object.path(),
                object_ignore,
                parent_ignore,
                mount_ignore,
                fs_ignore
            );
            return;
        }

        log::info!(
            "[FanotifyGroup::publish] Publishing event: \
            event={:?}, old_name={}, new_name={}",
            event,
            old_name,
            new_name
        );

        let queue_len = self.event_queue.lock().len();
        if queue_len >= self.max_queued_events as usize {
            log::warn!(
                "[FanotifyGroup::publish] Event queue is full, dropping event: \
                event={:?}, old_name={}, new_name={}",
                event,
                old_name,
                new_name
            );

            if queue_len == self.max_queued_events as usize {
                let mut metadata = self.create_metadata(FanEventMask::Q_OVERFLOW);
                metadata.event_len = mem::size_of::<FanotifyEventMetadata>() as u32;

                let mut event_data = FanotifyEventData::new(None);
                event_data.add_datum(FanotifyEventDatum::Metadata(metadata));

                self.event_queue.lock().push_back(event_data);
            }
            return;
        }

        let mut event_data = FanotifyEventData::new(Some(Arc::clone(object)));
        let flags = self.flags;

        // Create the metadata.
        let metadata = self.create_metadata(event);
        event_data.add_datum(FanotifyEventDatum::Metadata(metadata));

        // Create information records.
        if flags.contains(FanInitFlags::REPORT_FID) {
            let info = Self::create_fid_info(object, FanotifyEventInfoType::Fid, None);
            event_data.add_datum(FanotifyEventDatum::Info(info));
        }

        if flags.contains(FanInitFlags::REPORT_DIR_FID) {
            let reported_object = if object.inode().unwrap().inotype().is_dir() {
                Some(Arc::clone(object))
            } else {
                object.parent()
            };
            if let Some(reported_object) = reported_object {
                if !flags.contains(FanInitFlags::REPORT_NAME) {
                    let info =
                        Self::create_fid_info(&reported_object, FanotifyEventInfoType::Dfid, None);
                    event_data.add_datum(FanotifyEventDatum::Info(info));
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
                    event_data.add_datum(FanotifyEventDatum::Info(info_old));
                    event_data.add_datum(FanotifyEventDatum::Info(info_new));
                } else {
                    let info = Self::create_fid_info(
                        &reported_object,
                        FanotifyEventInfoType::DfidName,
                        Some(old_name.to_string()),
                    );
                    event_data.add_datum(FanotifyEventDatum::Info(info));
                }
            }
        }

        if flags.contains(FanInitFlags::REPORT_TARGET_FID)
            && (FanEventMask::DIR_EVENT_MASK | FanEventMask::ONDIR).contains(event)
        {
            let subobject = subobject.unwrap();
            let info = Self::create_fid_info(
                subobject,
                FanotifyEventInfoType::Fid,
                None,
            );
            event_data.add_datum(FanotifyEventDatum::Info(info));
        }

        // Other information records should be added here...

        // Set the event length in the metadata.
        let event_len = event_data
            .iter()
            .map(|data| data.as_slice().len())
            .sum::<usize>() as u32;
        event_data.metadata_mut().event_len = event_len;

        // Add the event data to the entry's event queue.
        {
            let mut event_queue = self.event_queue.lock();
            let mut event_merge_to: Option<&mut FanotifyEventData> = None;

            // Check if the current event can be merged into one of the last 10 events in
            // the event queue.
            for existing_event in event_queue.iter_mut().rev().take(10) {
                let existing_object = existing_event.object().unwrap();
                let existing_metadata = existing_event.metadata();

                if Arc::ptr_eq(existing_object, object)
                    && existing_metadata.pid == metadata.pid
                    && !existing_metadata.mask.contains(metadata.mask)
                    && ((existing_metadata.mask ^ metadata.mask) & FanEventMask::ONDIR).is_empty()
                    && existing_event.data()[1..] == event_data.data()[1..]
                {
                    event_merge_to = Some(existing_event);
                    break;
                }
            }

            if let Some(event_merge_to) = event_merge_to {
                event_merge_to.metadata_mut().mask |= metadata.mask;
            } else {
                event_queue.push_back(event_data);
            }
        }

        // Wake up a process waiting for events in this group.
        self.wake();
    }

    pub(crate) async fn wait(self: Arc<Self>) {
        let group_key = FanotifyGroupKey(self);
        let waker = take_waker().await;
        {
            let mut wakers = FANOTIFY_WAKERS.lock();
            wakers.entry(group_key).or_default().push_back(waker);
        }
        suspend_now().await;
    }

    fn wake(self: Arc<Self>) {
        let group_key = FanotifyGroupKey(self);
        let mut wakers = FANOTIFY_WAKERS.lock();
        if let Some(waker_list) = wakers.get_mut(&group_key) {
            if let Some(waker) = waker_list.pop_front() {
                waker.wake_by_ref();
            }
        }
    }

    /// Creates an fanotify event metadata for this group flags with the specified event
    /// mask.
    ///
    /// The following two fields of event metadata are not set in this function:
    /// * `event_len` should be set before the metadata is inserted into the event queue.
    /// * `fd` should be set when the event is read by the user monitor process.
    fn create_metadata(&self, mask: FanEventMask) -> FanotifyEventMetadata {
        FanotifyEventMetadata {
            event_len: 0,
            vers: FANOTIFY_METADATA_VERSION,
            reserved: 0,
            metadata_len: mem::size_of::<FanotifyEventMetadata>() as u16,
            fd: 0,
            pid: if self.flags.contains(FanInitFlags::REPORT_TID) {
                call_interface!(systype::kinterface::KernelTaskOperations::current_pid())
            } else {
                call_interface!(systype::kinterface::KernelTaskOperations::current_tid())
            },
            mask,
        }
    }

    /// Creates an information record of structure type [`FanotifyEventInfoType`]
    /// containing a file handle and optionally a file name for the specified filesystem
    /// object.
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

        // let mut handle_bytes = object.file_handle().to_raw_bytes();
        let file_handle = object.file_handle();
        log::info!(
            "[FanotifyGroup::create_fid_info] Creating fid info: object={:?}, file_handle={:?}",
            object.path(),
            file_handle,
        );
        let mut handle_bytes = file_handle.to_raw_bytes();
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

    /// Permission events that are waiting for responses from userspace.
    permission_queue: SpinNoIrqLock<VecDeque<FanotifyPermissionEvent>>,
}

impl FanotifyEntry {
    /// Returns a reference to the group this entry belongs to.
    pub fn group(&self) -> Arc<FanotifyGroup> {
        self.group.upgrade().unwrap()
    }

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

    /// Adds events and flags in `mark` to the mark mask of the entry.
    pub fn add_mark(&self, mark: FanEventMask) {
        *self.mark.lock() |= mark;
    }

    /// Removes events and flags in `mark` from the mark mask of the entry.
    pub fn remove_mark(&self, mark: FanEventMask) {
        *self.mark.lock() &= !mark;
    }

    /// Adds events and flags in `ignore` to the ignore mask of the entry.
    pub fn add_ignore(&self, ignore: FanEventMask) {
        *self.ignore.lock() |= ignore;
    }

    /// Removes events and flags in `ignore` from the ignore mask of the entry.
    pub fn remove_ignore(&self, ignore: FanEventMask) {
        *self.ignore.lock() &= !ignore;
    }

    /// Clears the ignore mask of the entry if the entry is not marked with
    /// `FAN_MARK_IGNORED_SURV_MODIFY`.
    ///
    /// This method should be called when the file that the entry is monitoring is
    /// modified (by writing, truncating, etc.).
    pub fn clear_ignore(&self) {
        if !self.flags().contains(FanMarkFlags::IGNORED_SURV_MODIFY) {
            log::info!(
                "[FanotifyEntry::clear_ignore] Clearing ignore mask: {:?}",
                *self.ignore.lock()
            );
            *self.ignore.lock() = FanEventMask::empty();
        }
    }
}

/// A set of fanotify entries that is associated with a filesystem object.
///
/// This structure is used as a parameter to [`FanotifyGroup::publish`]. It contains
/// entries for the object itself, its parent directory, the mount it belongs to, and the
/// filesystem it belongs to. When an event occurs on a filesystem object, these entries
/// are used to determine whether the event is marked and not ignored.
#[derive(Default, Clone)]
pub(crate) struct FanotifyEntrySet {
    pub object: Option<Arc<FanotifyEntry>>,
    pub parent: Option<Arc<FanotifyEntry>>,
    pub mount: Option<Arc<FanotifyEntry>>,
    pub fs: Option<Arc<FanotifyEntry>>,
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

/// Data of an fanotify event, which consists of a metadata structure and optionally
/// several information records.
#[derive(Clone)]
pub struct FanotifyEventData {
    /// The dentry of the filesystem object that triggered the event. For Q_OVERFLOW
    /// events, this is `None`.
    dentry: Option<Arc<dyn Dentry>>,
    /// The event data, which contains a metadata and optional information records.
    data: Vec<FanotifyEventDatum>,
}

impl FanotifyEventData {
    /// Creates an empty fanotify event data structure with the specified dentry.
    fn new(dentry: Option<Arc<dyn Dentry>>) -> Self {
        Self {
            dentry,
            data: Vec::new(),
        }
    }

    /// Adds an event datum to the event data.
    fn add_datum(&mut self, datum: FanotifyEventDatum) {
        self.data.push(datum);
    }

    /// Returns a reference to the dentry of the filesystem object that triggered the
    /// event.
    fn object(&self) -> Option<&Arc<dyn Dentry>> {
        self.dentry.as_ref()
    }

    fn data(&self) -> &[FanotifyEventDatum] {
        &self.data
    }

    /// Returns an immutable reference to the metadata structure of the event data.
    ///
    /// # Panic
    /// If no datum is added to the event data, or if the first datum is not a metadata
    /// structure, this method will panic.
    fn metadata(&self) -> &FanotifyEventMetadata {
        match &self.data[0] {
            FanotifyEventDatum::Metadata(metadata) => metadata,
            _ => panic!("First datum must be metadata"),
        }
    }

    /// Returns a mutable reference to the metadata structure of the event data.
    ///
    /// # Panic
    /// If no datum is added to the event data, or if the first datum is not a metadata
    /// structure, this method will panic.
    fn metadata_mut(&mut self) -> &mut FanotifyEventMetadata {
        match &mut self.data[0] {
            FanotifyEventDatum::Metadata(metadata) => metadata,
            _ => panic!("First datum must be metadata"),
        }
    }

    /// Returns an iterator over the event data.
    fn iter(&self) -> slice::Iter<FanotifyEventDatum> {
        self.data.iter()
    }
}

/// Enum representing an fanotify metadata structure or an information record structure.
#[derive(Clone)]
pub(crate) enum FanotifyEventDatum {
    /// Fanotify event metadata. The first element is an incomplete metadata. The second
    /// element is a reference of the [`Dentry`] of the filesystem object which triggered
    /// the event, which is to be opened and added to the user process's file descriptor
    /// table.
    Metadata(FanotifyEventMetadata),
    Info(FanotifyEventInfoFid),
    Pid(FanotifyEventInfoPid),
    Error(FanotifyEventInfoError),
}

impl FanotifyEventDatum {
    /// Returns a byte slice representation of the data, which is to be read by a user
    /// process.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            FanotifyEventDatum::Metadata(metadata) => unsafe {
                slice::from_raw_parts(
                    metadata as *const FanotifyEventMetadata as *const u8,
                    mem::size_of::<FanotifyEventMetadata>(),
                )
            },
            FanotifyEventDatum::Info(info) => info.as_bytes(),
            FanotifyEventDatum::Pid(pid) => unsafe {
                slice::from_raw_parts(
                    pid as *const FanotifyEventInfoPid as *const u8,
                    mem::size_of::<FanotifyEventInfoPid>(),
                )
            },
            FanotifyEventDatum::Error(error) => unsafe {
                slice::from_raw_parts(
                    error as *const FanotifyEventInfoError as *const u8,
                    mem::size_of::<FanotifyEventInfoError>(),
                )
            },
        }
    }
}

impl PartialEq for FanotifyEventDatum {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FanotifyEventDatum::Metadata(_), FanotifyEventDatum::Metadata(_)) => {}
            (FanotifyEventDatum::Info(_), FanotifyEventDatum::Info(_)) => {}
            (FanotifyEventDatum::Pid(_), FanotifyEventDatum::Pid(_)) => {}
            (FanotifyEventDatum::Error(_), FanotifyEventDatum::Error(_)) => {}
            _ => return false,
        }

        // Compare the byte slices of the data.
        let self_slice = self.as_slice();
        let other_slice = other.as_slice();

        self_slice == other_slice
    }
}

impl Eq for FanotifyEventDatum {}

/// A wrapper of `Arc<FanotifyGroup>` that uses pointer comparison for ordering.
pub(crate) struct FanotifyGroupKey(pub Arc<FanotifyGroup>);

impl Ord for FanotifyGroupKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        (self.0.as_ref() as *const FanotifyGroup).cmp(&(other.0.as_ref() as *const FanotifyGroup))
    }
}

impl PartialOrd for FanotifyGroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for FanotifyGroupKey {}

impl PartialEq for FanotifyGroupKey {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.0.as_ref(), other.0.as_ref())
    }
}

static FANOTIFY_WAKERS: SpinNoIrqLock<BTreeMap<FanotifyGroupKey, VecDeque<Waker>>> =
    SpinNoIrqLock::new(BTreeMap::new());
