use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use core::cell::SyncUnsafeCell;

use lazy_static::lazy_static;

use mutex::SpinNoIrqLock;

use super::{DevError, DevResult};

pub use netbuf::{NetBuf, NetBufPtr, NetBufPtrOps};

pub mod netbuf;

/// Number of buffers in the network buffer pool.
const NET_BUF_POOL_SIZE: usize = 64;

/// The length of each network buffer in bytes.
///
/// This is the maximum size of a packet that can be sent or received by the device.
///
/// It is set to 1518 bytes, which is the maximum Ethernet frame size (1500 bytes) plus
/// the Ethernet header with 802.1Q VLAN tag (18 bytes), not including the preamble,
/// SFD (start frame delimiter), and FCS (frame check sequence).
pub const NET_BUF_LEN: usize = 1518;

lazy_static! {
    /// The global network buffer pool.
    pub static ref NET_BUF_POOL: NetBufPool = NetBufPool::new(256, 1526);
}

/// A pool of network buffers for a network device.
///
/// This pool manages a fixed number of fixed-length buffers. The user can allocate and
/// deallocate buffers from this pool via [`NetBufPool::alloc`] and
/// [`NetBufPool::dealloc`].
///
/// The implementation expects that the buffer pool has a static lifetime to simplify
/// the reference management.
pub struct NetBufPool {
    /// The number of buffers in the pool.
    size: usize,
    /// The length of each buffer in the pool.
    buf_len: usize,
    /// The backing memory for the pool.
    memory: SyncUnsafeCell<Vec<u8>>,
    /// A list of free buffers in the pool, represented by their offsets.
    free_list: SpinNoIrqLock<Vec<usize>>,
}

impl NetBufPool {
    /// Creates a new pool with `size` buffers, each of length `buf_len`.
    pub fn new(size: usize, buf_len: usize) -> Self {
        debug_assert!(size > 0);
        debug_assert!(
            buf_len > 1000 && buf_len < 65536,
            "Buffer length is expected to be between 1000 and 65536 bytes"
        );

        let memory = SyncUnsafeCell::new(vec![0; size * buf_len]);

        let mut free_list = Vec::with_capacity(size);
        for i in 0..size {
            free_list.push(i * buf_len);
        }

        Self {
            size,
            buf_len,
            memory,
            free_list: SpinNoIrqLock::new(free_list),
        }
    }

    /// Returns the size (number of buffers) of the pool.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns the length of each buffer.
    pub const fn buf_len(&self) -> usize {
        self.buf_len
    }

    /// Allocates a buffer from the pool.
    ///
    /// Returns `None` if no buffer is available.
    pub(super) fn alloc(&'static self) -> Option<NetBuf> {
        let pool_offset = self.free_list.lock().pop()?;
        let buffer = unsafe { &mut (*self.memory.get())[pool_offset..pool_offset + self.buf_len] };

        Some(NetBuf {
            buffer,
            header_len: 0,
            packet_len: 0,
            capacity: self.buf_len,
            pool_offset,
            pool: self,
        })
    }

    /// Allocates a buffer from the pool and wrap it in a [`Box`].
    ///
    /// Returns `None` if no buffer is available.
    pub(super) fn alloc_boxed(&'static self) -> Option<Box<NetBuf>> {
        Some(Box::new(self.alloc()?))
    }

    /// Deallocates a buffer at the given offset.
    ///
    /// `pool_offset` must be a multiple of `buf_len`.
    fn dealloc(&self, pool_offset: usize) {
        debug_assert_eq!(pool_offset % self.buf_len, 0);
        self.free_list.lock().push(pool_offset);
    }
}
