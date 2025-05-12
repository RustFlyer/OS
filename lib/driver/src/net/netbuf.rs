use core::{any::Any, fmt::Debug, ptr::NonNull};

use alloc::{boxed::Box, sync::Arc};

use super::netpool::NetBufPool;

pub trait NetBufPtrOps: Any + Debug {
    fn packet(&self) -> &[u8];
    fn packet_mut(&mut self) -> &mut [u8];
    fn packet_len(&self) -> usize;
}

pub struct NetBuf {
    pub(crate) header_len: usize,
    pub(crate) packet_len: usize,
    pub(crate) capacity: usize,
    pub(crate) buf_ptr: NonNull<u8>,
    pub(crate) pool_offset: usize,
    pub(crate) pool: Arc<NetBufPool>,
}

impl Debug for NetBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NetBuf")
            .field("header_len", &self.header_len)
            .field("packet_len", &self.packet_len)
            .field("capacity", &self.capacity)
            .field("buf_ptr", &self.buf_ptr)
            .field("pool_offset", &self.pool_offset)
            .finish()
    }
}

impl NetBuf {
    const unsafe fn get_slice(&self, start: usize, len: usize) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buf_ptr.as_ptr().add(start), len) }
    }

    const unsafe fn get_slice_mut(&mut self, start: usize, len: usize) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.buf_ptr.as_ptr().add(start), len) }
    }

    /// Returns the capacity of the buffer.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the length of the header part.
    pub const fn header_len(&self) -> usize {
        self.header_len
    }

    /// Returns the header part of the buffer.
    pub const fn header(&self) -> &[u8] {
        unsafe { self.get_slice(0, self.header_len) }
    }

    /// Returns the packet part of the buffer.
    pub const fn packet(&self) -> &[u8] {
        unsafe { self.get_slice(self.header_len, self.packet_len) }
    }

    /// Returns the mutable reference to the packet part.
    pub const fn packet_mut(&mut self) -> &mut [u8] {
        unsafe { self.get_slice_mut(self.header_len, self.packet_len) }
    }

    /// Returns both the header and the packet parts, as a contiguous slice.
    pub const fn packet_with_header(&self) -> &[u8] {
        unsafe { self.get_slice(0, self.header_len + self.packet_len) }
    }

    /// Returns the entire buffer.
    pub const fn raw_buf(&self) -> &[u8] {
        unsafe { self.get_slice(0, self.capacity) }
    }

    /// Returns the mutable reference to the entire buffer.
    pub const fn raw_buf_mut(&mut self) -> &mut [u8] {
        unsafe { self.get_slice_mut(0, self.capacity) }
    }

    /// Set the length of the header part.
    pub fn set_header_len(&mut self, header_len: usize) {
        debug_assert!(header_len + self.packet_len <= self.capacity);
        self.header_len = header_len;
    }

    /// Set the length of the packet part.
    pub fn set_packet_len(&mut self, packet_len: usize) {
        debug_assert!(self.header_len + packet_len <= self.capacity);
        self.packet_len = packet_len;
    }

    /// Converts the buffer into a [`NetBufPtr`].
    pub fn into_buf_ptr(mut self: Box<Self>) -> Box<NetBufPtr> {
        let buf_ptr = self.packet_mut().as_mut_ptr();
        let len = self.packet_len;
        Box::new(NetBufPtr::new(
            NonNull::new(Box::into_raw(self) as *mut u8).unwrap(),
            NonNull::new(buf_ptr).unwrap(),
            len,
        ))
    }

    /// Restore [`NetBuf`] struct from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it may cause some memory issues,
    /// so we must ensure that it is called after calling `into_buf_ptr`.
    pub unsafe fn from_buf_ptr(ptr: Box<NetBufPtr>) -> Box<Self> {
        unsafe { Box::from_raw(ptr.raw_ptr.as_ptr() as *mut Self) }
    }
}

impl Drop for NetBuf {
    /// Deallocates the buffer into the [`NetBufPool`].
    fn drop(&mut self) {
        self.pool.dealloc(self.pool_offset);
    }
}

/// A raw buffer struct for network device.
#[derive(Debug)]
pub struct NetBufPtr {
    // The raw pointer of the original object.
    pub raw_ptr: NonNull<u8>,
    // The pointer to the net buffer.
    buf_ptr: NonNull<u8>,
    len: usize,
}

impl NetBufPtr {
    /// Create a new [`NetBufPtr`].
    pub fn new(raw_ptr: NonNull<u8>, buf_ptr: NonNull<u8>, len: usize) -> Self {
        Self {
            raw_ptr,
            buf_ptr,
            len,
        }
    }

    /// Return raw pointer of the original object.
    pub fn raw_ptr<T>(&self) -> *mut T {
        self.raw_ptr.as_ptr() as *mut T
    }
}

impl NetBufPtrOps for NetBufPtr {
    /// Return [`NetBufPtr`] buffer len.
    fn packet_len(&self) -> usize {
        self.len
    }

    /// Return [`NetBufPtr`] buffer as &[u8].
    fn packet(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buf_ptr.as_ptr() as *const u8, self.len) }
    }

    /// Return [`NetBufPtr`] buffer as &mut [u8].
    fn packet_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.buf_ptr.as_ptr(), self.len) }
    }
}
