#[allow(non_camel_case_types)]
type c_uint = u32;
#[allow(non_camel_case_types)]
type c_int = i32;

pub const FAN_ACCESS: u64 = 0x0000_0001;
pub const FAN_MODIFY: u64 = 0x0000_0002;
pub const FAN_ATTRIB: u64 = 0x0000_0004;
pub const FAN_CLOSE_WRITE: u64 = 0x0000_0008;
pub const FAN_CLOSE_NOWRITE: u64 = 0x0000_0010;
pub const FAN_OPEN: u64 = 0x0000_0020;
pub const FAN_MOVED_FROM: u64 = 0x0000_0040;
pub const FAN_MOVED_TO: u64 = 0x0000_0080;
pub const FAN_CREATE: u64 = 0x0000_0100;
pub const FAN_DELETE: u64 = 0x0000_0200;
pub const FAN_DELETE_SELF: u64 = 0x0000_0400;
pub const FAN_MOVE_SELF: u64 = 0x0000_0800;
pub const FAN_OPEN_EXEC: u64 = 0x0000_1000;

pub const FAN_Q_OVERFLOW: u64 = 0x0000_4000;
pub const FAN_FS_ERROR: u64 = 0x0000_8000;

pub const FAN_OPEN_PERM: u64 = 0x0001_0000;
pub const FAN_ACCESS_PERM: u64 = 0x0002_0000;
pub const FAN_OPEN_EXEC_PERM: u64 = 0x0004_0000;

pub const FAN_EVENT_ON_CHILD: u64 = 0x0800_0000;

pub const FAN_RENAME: u64 = 0x1000_0000;

pub const FAN_ONDIR: u64 = 0x4000_0000;

pub const FAN_CLOSE: u64 = FAN_CLOSE_WRITE | FAN_CLOSE_NOWRITE;
pub const FAN_MOVE: u64 = FAN_MOVED_FROM | FAN_MOVED_TO;

pub const FAN_CLOEXEC: c_uint = 0x0000_0001;
pub const FAN_NONBLOCK: c_uint = 0x0000_0002;

pub const FAN_CLASS_NOTIF: c_uint = 0x0000_0000;
pub const FAN_CLASS_CONTENT: c_uint = 0x0000_0004;
pub const FAN_CLASS_PRE_CONTENT: c_uint = 0x0000_0008;

pub const FAN_UNLIMITED_QUEUE: c_uint = 0x0000_0010;
pub const FAN_UNLIMITED_MARKS: c_uint = 0x0000_0020;
pub const FAN_ENABLE_AUDIT: c_uint = 0x0000_0040;

pub const FAN_REPORT_PIDFD: c_uint = 0x0000_0080;
pub const FAN_REPORT_TID: c_uint = 0x0000_0100;
pub const FAN_REPORT_FID: c_uint = 0x0000_0200;
pub const FAN_REPORT_DIR_FID: c_uint = 0x0000_0400;
pub const FAN_REPORT_NAME: c_uint = 0x0000_0800;
pub const FAN_REPORT_TARGET_FID: c_uint = 0x0000_1000;

pub const FAN_REPORT_DFID_NAME: c_uint = FAN_REPORT_DIR_FID | FAN_REPORT_NAME;
pub const FAN_REPORT_DFID_NAME_TARGET: c_uint =
    FAN_REPORT_DFID_NAME | FAN_REPORT_FID | FAN_REPORT_TARGET_FID;

pub const FAN_MARK_ADD: c_uint = 0x0000_0001;
pub const FAN_MARK_REMOVE: c_uint = 0x0000_0002;
pub const FAN_MARK_DONT_FOLLOW: c_uint = 0x0000_0004;
pub const FAN_MARK_ONLYDIR: c_uint = 0x0000_0008;
pub const FAN_MARK_IGNORED_MASK: c_uint = 0x0000_0020;
pub const FAN_MARK_IGNORED_SURV_MODIFY: c_uint = 0x0000_0040;
pub const FAN_MARK_FLUSH: c_uint = 0x0000_0080;
pub const FAN_MARK_EVICTABLE: c_uint = 0x0000_0200;
pub const FAN_MARK_IGNORE: c_uint = 0x0000_0400;

pub const FAN_MARK_INODE: c_uint = 0x0000_0000;
pub const FAN_MARK_MOUNT: c_uint = 0x0000_0010;
pub const FAN_MARK_FILESYSTEM: c_uint = 0x0000_0100;

pub const FAN_MARK_IGNORE_SURV: c_uint = FAN_MARK_IGNORE | FAN_MARK_IGNORED_SURV_MODIFY;

pub const FANOTIFY_METADATA_VERSION: u8 = 3;

pub const FAN_EVENT_INFO_TYPE_FID: u8 = 1;
pub const FAN_EVENT_INFO_TYPE_DFID_NAME: u8 = 2;
pub const FAN_EVENT_INFO_TYPE_DFID: u8 = 3;
pub const FAN_EVENT_INFO_TYPE_PIDFD: u8 = 4;
pub const FAN_EVENT_INFO_TYPE_ERROR: u8 = 5;

pub const FAN_EVENT_INFO_TYPE_OLD_DFID_NAME: u8 = 10;
pub const FAN_EVENT_INFO_TYPE_NEW_DFID_NAME: u8 = 12;

pub const FAN_RESPONSE_INFO_NONE: u8 = 0;
pub const FAN_RESPONSE_INFO_AUDIT_RULE: u8 = 1;

pub const FAN_ALLOW: u32 = 0x01;
pub const FAN_DENY: u32 = 0x02;
pub const FAN_AUDIT: u32 = 0x10;
pub const FAN_INFO: u32 = 0x20;

pub const FAN_NOFD: c_int = -1;
pub const FAN_NOPIDFD: c_int = FAN_NOFD;
pub const FAN_EPIDFD: c_int = -2;
