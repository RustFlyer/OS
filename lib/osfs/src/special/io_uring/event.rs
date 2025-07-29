use alloc::{sync::Arc, vec::Vec};
use core::mem;
use systype::error::SysError;

/// Submission Queue Entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoUringSqe {
    /// Operation code
    pub opcode: u8,
    /// Flags for the operation
    pub flags: u8,
    /// I/O priority
    pub ioprio: u16,
    /// File descriptor
    pub fd: i32,
    /// Offset or address2
    pub off: u64,
    /// Address of buffer or iovecs
    pub addr: u64,
    /// Buffer length or number of iovecs
    pub len: u32,
    /// Operation-specific flags
    pub op_flags: u32,
    /// User data to be returned in completion
    pub user_data: u64,
    /// Buffer ID for buffer selection or additional data
    pub buf_index: u16,
    /// Personality to use for this operation
    pub personality: u16,
    /// Splice file descriptor or additional data
    pub splice_fd_in: i32,
    /// Padding for future use
    pub pad2: [u64; 2],
}

/// Completion Queue Entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoUringCqe {
    /// User data from corresponding SQE
    pub user_data: u64,
    /// Result of the operation
    pub res: i32,
    /// Flags for the completion
    pub flags: u32,
    /// Big CQE support
    pub big_cqe: [u64; 0],
}

impl Default for IoUringSqe {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

impl Default for IoUringCqe {
    fn default() -> Self {
        Self {
            user_data: 0,
            res: 0,
            flags: 0,
            big_cqe: [],
        }
    }
}

/// Parameters passed to io_uring_setup
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoUringParams {
    /// Number of SQ entries
    pub sq_entries: u32,
    /// Number of CQ entries
    pub cq_entries: u32,
    /// Flags for the ring
    pub flags: u32,
    /// Offset of submission queue tail
    pub sq_thread_cpu: u32,
    /// SQ thread idle timeout
    pub sq_thread_idle: u32,
    /// Features supported
    pub features: u32,
    /// Workqueue file descriptor
    pub wq_fd: u32,
    /// Reserved fields
    pub resv: [u32; 3],
    /// Submission queue ring offsets
    pub sq_off: IoUringSqRingOffsets,
    /// Completion queue ring offsets
    pub cq_off: IoUringCqRingOffsets,
}

/// Submission queue ring offsets
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoUringSqRingOffsets {
    /// Head offset
    pub head: u32,
    /// Tail offset
    pub tail: u32,
    /// Ring mask offset
    pub ring_mask: u32,
    /// Ring entries offset
    pub ring_entries: u32,
    /// Flags offset
    pub flags: u32,
    /// Dropped counter offset
    pub dropped: u32,
    /// Array offset
    pub array: u32,
    /// Reserved field
    pub resv1: u32,
    /// User address
    pub user_addr: u64,
}

/// Completion queue ring offsets
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoUringCqRingOffsets {
    /// Head offset
    pub head: u32,
    /// Tail offset
    pub tail: u32,
    /// Ring mask offset
    pub ring_mask: u32,
    /// Ring entries offset
    pub ring_entries: u32,
    /// Overflow offset
    pub overflow: u32,
    /// CQEs offset
    pub cqes: u32,
    /// Flags offset
    pub flags: u32,
    /// Reserved field
    pub resv1: u32,
    /// User address
    pub user_addr: u64,
}

impl Default for IoUringParams {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl IoUringSqe {
    pub fn new() -> Self {
        unsafe { mem::zeroed() }
    }

    pub fn set_nop(&mut self, user_data: u64) {
        self.opcode = super::flags::IoUringOpcode::IORING_OP_NOP.bits();
        self.user_data = user_data;
    }

    pub fn set_read(&mut self, fd: i32, buf: u64, len: u32, offset: u64, user_data: u64) {
        self.opcode = super::flags::IoUringOpcode::IORING_OP_READ.bits();
        self.fd = fd;
        self.addr = buf;
        self.len = len;
        self.off = offset;
        self.user_data = user_data;
    }

    pub fn set_write(&mut self, fd: i32, buf: u64, len: u32, offset: u64, user_data: u64) {
        self.opcode = super::flags::IoUringOpcode::IORING_OP_WRITE.bits();
        self.fd = fd;
        self.addr = buf;
        self.len = len;
        self.off = offset;
        self.user_data = user_data;
    }
}

impl IoUringCqe {
    pub fn new(user_data: u64, res: i32, flags: u32) -> Self {
        Self {
            user_data,
            res,
            flags,
            big_cqe: [],
        }
    }
}

/// Ring buffer for submissions or completions
pub struct IoUringRing<T> {
    /// Ring entries
    pub entries: Vec<T>,
    /// Ring mask (size - 1, must be power of 2)
    pub mask: u32,
    /// Head index (consumer)
    pub head: u32,
    /// Tail index (producer)
    pub tail: u32,
    /// Ring flags
    pub flags: u32,
    /// Dropped entries count
    pub dropped: u32,
}

impl<T: Clone + Default> IoUringRing<T> {
    pub fn new(size: u32) -> Self {
        let size = size.next_power_of_two();
        Self {
            entries: alloc::vec![T::default(); size as usize],
            mask: size - 1,
            head: 0,
            tail: 0,
            flags: 0,
            dropped: 0,
        }
    }

    pub fn available_entries(&self) -> u32 {
        self.mask + 1 - (self.tail - self.head)
    }

    pub fn pending_entries(&self) -> u32 {
        self.tail - self.head
    }

    pub fn is_full(&self) -> bool {
        self.pending_entries() >= self.mask + 1
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub fn push(&mut self, entry: T) -> Result<(), T> {
        if self.is_full() {
            self.dropped += 1;
            return Err(entry);
        }

        let idx = (self.tail & self.mask) as usize;
        self.entries[idx] = entry;
        self.tail += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let idx = (self.head & self.mask) as usize;
        let entry = self.entries[idx].clone();
        self.head += 1;
        Some(entry)
    }

    pub fn peek(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }

        let idx = (self.head & self.mask) as usize;
        Some(&self.entries[idx])
    }
}

/// Registered buffer
#[derive(Debug, Clone)]
pub struct RegisteredBuffer {
    pub addr: u64,
    pub len: u32,
    pub buffer_id: u16,
}

/// Registered file descriptor
#[derive(Debug, Clone)]
pub struct RegisteredFile {
    pub fd: i32,
    pub file_index: u32,
}

/// IO request representing a pending operation
#[derive(Debug, Clone)]
pub struct IoRequest {
    pub sqe: IoUringSqe,
    pub state: IoRequestState,
    pub result: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IoRequestState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl IoRequest {
    pub fn new(sqe: IoUringSqe) -> Self {
        Self {
            sqe,
            state: IoRequestState::Pending,
            result: None,
        }
    }

    pub fn complete(&mut self, result: i32) {
        self.state = IoRequestState::Completed;
        self.result = Some(result);
    }

    pub fn cancel(&mut self) {
        self.state = IoRequestState::Cancelled;
        self.result = Some(-SysError::ECANCELED.code());
    }
}
