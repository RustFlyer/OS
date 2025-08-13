use alloc::vec::Vec;
use config::device::BLOCK_SIZE;

use crate::BLOCK_DEVICE;

#[cfg(target_arch = "loongarch64")]
pub mod ahci;

pub mod dw_mshc;
pub mod virtblk;

pub fn block_test() {
    let blk = BLOCK_DEVICE.get().unwrap();

    let mut buf = Vec::new();
    for i in 0..BLOCK_SIZE {
        buf.push(i as u8);
    }
    blk.write(10, &buf);

    let mut rbuf = [0u8; 512];
    blk.read(10, &mut rbuf);

    for i in 0..BLOCK_SIZE {
        let ti = i as u8;
        assert!(ti == rbuf[i])
    }

    log::info!("pass block_test")
}
