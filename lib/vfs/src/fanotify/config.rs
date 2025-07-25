use core::sync::atomic::AtomicU32;

/// Maximum number of events that can be queued in a fanotify event queue that is to be
/// created by calling `fanotify_init`.
pub static MAX_QUEUED_EVENTS: AtomicU32 = AtomicU32::new(16384);

/// Maximum number of fanotify groups that can be created per real user ID.
pub static MAX_USER_GROUPS: AtomicU32 = AtomicU32::new(128);

/// Maximum number of fanotify marks that can be created per real user ID.
pub static MAX_USER_MARKS: AtomicU32 = AtomicU32::new(8192);
