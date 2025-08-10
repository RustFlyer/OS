use alloc::{format, string::String, vec::Vec};

use config::inode::InodeMode;
use vfs::fanotify::types::{FanEventFileFlags, FanEventMask, FanInitFlags, FanMarkFlags};

pub struct ProcFdInfo {
    /// File offset,
    pub pos: u64,
    /// File access mode and file status.
    pub flags: InodeMode,
    /// Mount ID of the mount where the file resides.
    pub mnt_id: u32,
    /// Inode number.
    pub ino: u32,
    /// Extra information.
    pub extra_info: ExtraFdInfo,
}

/// Extra fdinfo informations for file descriptors of special types.
pub enum ExtraFdInfo {
    /// Normal file descriptor (no extra information).
    Normal,
    /// Fanotify file descriptor.
    Fanotify(FanotifyFdInfo),
}

/// Extra fdinfo informations for an fanotify file descriptor.
pub struct FanotifyFdInfo {
    /// `flags` passed to `fanotify_init`.
    pub flags: FanInitFlags,
    /// `event_f_flags` passed to `fanotify_init`.
    pub event_flags: FanEventFileFlags,
    /// Information of marks registered on the fanotify group associated with this file
    /// descriptor.
    pub marks: Vec<FanotifyMarkInfo>,
}

/// Extra fdinfo informations for each fanotify mark registered on an fanotify group.
pub struct FanotifyMarkInfo {
    /// Inode number of the target file.
    pub ino: u32,
    /// Device ID of the device where the target file resides.
    pub sdev: u64,
    /// Events mask for this mark.
    pub mask: FanEventMask,
    /// Ignore mask for this mark.
    pub ignored_mask: FanEventMask,
    /// Flags associated with the mark.
    pub mflags: FanMarkFlags,
}

impl ProcFdInfo {
    pub fn to_text(&self) -> String {
        let mut fdinfo = format!(
            "pos:\t{}\n\
             flags:\t{:o}\n\
             mnt_id:\t{}\n\
             ino:\t{}\n",
            self.pos, self.flags, self.mnt_id, self.ino
        );
        match &self.extra_info {
            ExtraFdInfo::Normal => {}
            ExtraFdInfo::Fanotify(fanotify_info) => {
                fdinfo.push_str(&format!(
                    "fanotify flags:{:x} event-flags:{:x}\n",
                    fanotify_info.flags, fanotify_info.event_flags,
                ));
                for mark in &fanotify_info.marks {
                    fdinfo.push_str(&format!(
                        "fanotify ino:{:x} sdev:{:x} mflags:{:x} mask:{:x} ignored_mask:{:x}\n",
                        mark.ino,
                        mark.sdev,
                        mark.mflags.bits(),
                        mark.mask.bits(),
                        mark.ignored_mask.bits(),
                    ));
                }
            }
        };
        fdinfo
    }
}
