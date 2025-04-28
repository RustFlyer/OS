use core::{any::Any, ptr::NonNull};

use alloc::sync::Arc;

use super::netpool::NetBufPool;

pub trait NetBufPtrOps: Any {
    fn packet(&self) -> &[u8];
    fn packet_mut(&mut self) -> &mut [u8];
    fn packet_len(&self) -> usize;
}

pub struct NetBuf {
    header_len: usize,
    packet_len: usize,
    capacity: usize,
    buf_ptr: NonNull<u8>,
    pool_offset: usize,
    pool: Arc<NetBufPool>,
}
