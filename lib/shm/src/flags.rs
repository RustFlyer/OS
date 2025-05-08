use bitflags::bitflags;

bitflags! {
    #[derive(Debug)]
    pub struct ShmGetFlags: i32 {
        /// Create a new segment. If this flag is not used, then shmget() will
        /// find the segment associated with key and check to see if the user
        /// has permission to access the segment.
        const IPC_CREAT = 0o1000;
        /// This flag is used with IPC_CREAT to ensure that this call creates
        /// the segment.  If the segment already exists, the call fails.
        const IPC_EXCL = 0o2000;
    }
}

bitflags! {
    #[derive(Debug)]
    pub struct ShmAtFlags: i32 {
        /// Attach the segment for read-only access.If this flag is not specified,
        /// the segment is attached for read and write access, and the process
        /// must have read and write permission for the segment.
        const SHM_RDONLY = 0o10000;
        /// round attach address to SHMLBA boundary
        const SHM_RND = 0o20000;
        /// take-over region on attach (unimplemented)
        const SHM_REMAP = 0o40000;
        /// Allow the contents of the segment to be executed.
        const SHM_EXEC = 0o100000;
    }
}
