use alloc::{
    boxed::Box,
    fmt::{self, Debug},
    slice,
    sync::Arc,
};
use core::any::Any;

use super::NetBufPool;

/// A network buffer for a network device.
///
/// It is a fixed-size buffer that can hold both a header and a packet part.
///
/// It has to be allocated from a static [`NetBufPool`]. When dropped, it is deallocated
/// automatically back to the pool.
pub struct NetBuf {
    /// A reference to the buffer memory.
    pub(super) buffer: &'static mut [u8],
    /// The length of the header part.
    pub(super) header_len: usize,
    /// The length of the packet part.
    pub(super) packet_len: usize,
    /// The capacity of the buffer in bytes.
    pub(super) capacity: usize,
    /// A reference to the buffer pool that this buffer belongs to.
    pub(super) pool: &'static NetBufPool,
    /// The offset into the buffer pool.
    pub(super) pool_offset: usize,
}

impl NetBuf {
    /// Returns a slice into the buffer from index `start` with length `len`.
    unsafe fn slice(&self, start: usize, len: usize) -> &[u8] {
        &self.buffer[start..start + len]
    }

    /// Returns a mutable slice into the buffer from index `start` with length `len`.
    unsafe fn slice_mut(&mut self, start: usize, len: usize) -> &mut [u8] {
        &mut self.buffer[start..start + len]
    }

    /// Returns the capacity of the buffer.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns a reference to the header part of the buffer.
    pub fn header(&self) -> &[u8] {
        unsafe { self.slice(0, self.header_len) }
    }

    /// Returns the length of the header part.
    pub const fn header_len(&self) -> usize {
        self.header_len
    }

    /// Returns a reference to the packet part of the buffer.
    pub fn packet(&self) -> &[u8] {
        unsafe { self.slice(self.header_len, self.packet_len) }
    }

    /// Returns a mutable reference to the packet part of the buffer.
    pub fn packet_mut(&mut self) -> &mut [u8] {
        unsafe { self.slice_mut(self.header_len, self.packet_len) }
    }

    /// Returns the length of the packet part.
    pub const fn packet_len(&self) -> usize {
        self.packet_len
    }

    /// Returns a reference to both the header and the packet parts as a contiguous
    /// slice.
    pub fn header_and_packet(&self) -> &[u8] {
        unsafe { self.slice(0, self.header_len + self.packet_len) }
    }

    /// Returns a reference to the buffer memory (all available bytes).
    pub fn buffer(&self) -> &[u8] {
        unsafe { self.slice(0, self.capacity) }
    }

    /// Returns a mutable reference to the buffer memory (all available bytes).
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe { self.slice_mut(0, self.capacity) }
    }

    /// Sets the length of the header part.
    pub fn set_header_len(&mut self, header_len: usize) {
        debug_assert!(header_len + self.packet_len <= self.capacity);
        self.header_len = header_len;
    }

    /// Sets the length of the packet part.
    pub fn set_packet_len(&mut self, packet_len: usize) {
        debug_assert!(self.header_len + packet_len <= self.capacity);
        self.packet_len = packet_len;
    }

    /// Converts a boxed buffer into a [`NetBufPtr`].
    ///
    /// This function is safe, but the caller must properly destroy the [`NetBuf`]
    /// that the returned [`NetBufPtr`] points to, and release the memory the [`NetBuf`]
    /// occupies. To achieve this, the caller should call [`NetBuf::from_buf_ptr`] to
    /// convert the [`NetBufPtr`] back into a boxed [`NetBuf`] and then drop it.
    pub fn into_buf_ptr(mut self: Box<Self>) -> Box<NetBufPtr> {
        let buffer = self.packet_mut().as_mut_ptr();
        let len = self.packet_len;
        let net_buf = Box::into_raw(self);
        Box::new(NetBufPtr::new(net_buf, buffer, len))
    }

    /// Converts a [`NetBufPtr`] back into a boxed [`NetBuf`].
    pub fn from_buf_ptr(ptr: NetBufPtr) -> Box<Self> {
        unsafe { Box::from_raw(ptr.raw_ptr) }
    }
}

impl Drop for NetBuf {
    /// Deallocates the buffer into the buffer pool.
    fn drop(&mut self) {
        self.pool.dealloc(self.pool_offset);
    }
}

impl Debug for NetBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetBuf")
            .field("header_len", &self.header_len)
            .field("packet_len", &self.packet_len)
            .field("capacity", &self.capacity)
            .field("buf_ptr", &self.buffer)
            .field("pool_offset", &self.pool_offset)
            .finish()
    }
}

/// A raw buffer struct for network device.
#[derive(Debug)]
pub struct NetBufPtr {
    /// The raw pointer of the original object.
    pub raw_ptr: *mut NetBuf,
    /// The pointer to the net buffer.
    buffer: *mut u8,
    /// The length of the packet part.
    packet_len: usize,
}

impl NetBufPtr {
    /// Creates a new [`NetBufPtr`].
    fn new(net_buf: *mut NetBuf, buffer: *mut u8, len: usize) -> Self {
        Self {
            raw_ptr: net_buf,
            buffer,
            packet_len: len,
        }
    }
}

pub trait NetBufPtrOps: Any + Debug {
    fn packet(&self) -> &[u8];
    fn packet_mut(&mut self) -> &mut [u8];
    fn packet_len(&self) -> usize;
}

impl NetBufPtrOps for NetBufPtr {
    fn packet(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.buffer, self.packet_len) }
    }

    fn packet_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.buffer, self.packet_len) }
    }

    fn packet_len(&self) -> usize {
        self.packet_len
    }
}
