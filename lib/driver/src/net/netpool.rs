use alloc::{sync::Arc, vec::Vec};
use mutex::SpinNoIrqLock;

use super::{DevError, DevResult};

pub struct NetBufPool {
    capacity: usize,
    buf_len: usize,
    pool: Vec<u8>,
    free_list: SpinNoIrqLock<Vec<usize>>,
}

impl NetBufPool {
    /// Creates a new pool with the given `capacity`, and all buffer lengths are
    /// set to `buf_len`.
    pub fn new(capacity: usize, buf_len: usize) -> DevResult<Arc<Self>> {
        if capacity == 0 {
            return Err(DevError::InvalidParam);
        }
        if buf_len < 1000 || buf_len > 60000 {
            return Err(DevError::InvalidParam);
        }

        let pool = alloc::vec![0; capacity * buf_len];
        let mut free_list = Vec::with_capacity(capacity);
        for i in 0..capacity {
            free_list.push(i * buf_len);
        }
        Ok(Arc::new(Self {
            capacity,
            buf_len,
            pool,
            free_list: SpinNoIrqLock::new(free_list),
        }))
    }
}
