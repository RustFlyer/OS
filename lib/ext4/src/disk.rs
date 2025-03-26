extern crate alloc;

use alloc::sync::Arc;
use config::board::BLOCK_SIZE;
use driver::BlockDevice;
use lwext4_rust::{
    KernelDevOp,
    bindings::{SEEK_CUR, SEEK_END, SEEK_SET},
};
use systype::SysResult;

pub struct Disk {
    block_id: usize,
    offset: usize,
    dev: Arc<dyn BlockDevice>,
}

impl Disk {
    pub fn new(dev: Arc<dyn BlockDevice>) -> Self {
        assert!(dev.size(), BLOCK_SIZE);
        Self {
            block_id: 0,
            offset: 0,
            dev,
        }
    }

    pub fn size(&self) -> usize {
        self.dev.size()
    }

    pub fn pos(&self) -> usize {
        self.block_id * BLOCK_SIZE + self.offset
    }

    pub fn set_pos(&mut self, buf: u64) {
        self.block_id = buf / BLOCK_SIZE as usize;
        self.offset = buf % BLOCK_SIZE as usize;
    }

    /// Reads one block (whole or partial)
    pub fn read_one(&mut self, buf: &mut [u8]) -> SysResult<usize> {
        let read_size = if self.offset == 0 && buf.len() >= BLOCK_SIZE {
            self.dev.read(self.block_id, &mut buf[..BLOCK_SIZE]);
            self.block_id += 1;
            BLOCK_SIZE
        } else if buf.len() >= BLOCK_SIZE - self.offset {
            let length = BLOCK_SIZE - self.offset;
            self.dev.read(self.block_id, &mut buf[..length]);
            self.block_id += 1;
            self.offset = 0;
            length
        } else {
            let length = buf.len();
            self.dev.read(self.block_id, &mut buf[..length]);
            self.offset += length;
            length
        };
        Ok(read_size)
    }

    /// Writes one block (whole or partial)
    pub fn write_one(&mut self, buf: &[u8]) -> SysResult<usize> {
        let write_size = if self.offset == 0 && buf.len() >= BLOCK_SIZE {
            self.dev.write(self.block_id, &mut buf[..BLOCK_SIZE]);
            self.block_id += 1;
            BLOCK_SIZE
        } else if buf.len() >= BLOCK_SIZE - self.offset {
            let length = BLOCK_SIZE - self.offset;
            self.dev.write(self.block_id, &mut buf[..length]);
            self.block_id += 1;
            self.offset = 0;
            length
        } else {
            let length = buf.len();
            self.dev.write(self.block_id, &mut buf[..length]);
            self.offset += length;
            length
        };
        Ok(write_size)
    }
}

impl KernelDevOp for Disk {
    type DevType = Disk;
    fn flush(dev: &mut Self::DevType) -> Result<usize, i32>
    where
        Self: Sized,
    {
        Ok(0)
    }

    /// Reads blocks until buf is full
    fn read(dev: &mut Self::DevType, mut buf: &mut [u8]) -> Result<usize, i32> {
        let mut read_size = 0;
        while !buf.is_empty() {
            match dev.read_one(buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf = &mut buf[n..];
                    read_size += n;
                }
                Err(_) => return Err(-1),
            };
        }

        Ok(read_size)
    }

    /// Writes blocks until buf is empty
    fn write(dev: &mut Self::DevType, mut buf: &[u8]) -> Result<usize, i32> {
        let mut write_size = 0;
        while !buf.is_empty() {
            match dev.write_one(buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf = &mut buf;
                    write_size += n;
                }
                Err(_) => return -1,
            }
        }
        Ok(write_size)
    }

    #[allow(non_snake_case)]
    fn seek(dev: &mut Self::DevType, off: i64, whence: i32) -> Result<i64, i32> {
        let new_pos = match whence {
            SEEK_SET => Some(off),
            SEEK_CUR => dev.pos().checked_add_signed(off).map(|v| v as u64),
            SEEK_END => dev.size().checked_add_signed(off).map(|v| v as i64),
            _ => {
                log::error!("invalid whence {}", whence);
                Some(off)
            }
        }
        .ok_or(-1);

        if new_pos > dev.size() {
            log::warn!("pos > dev.size!!");
        }

        dev.set_pos(new_pos);
        Ok(new_pos)
    }
}
