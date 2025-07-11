use crate::{BLOCK_DEVICE, println};

pub fn block_device_test() {
    let block_device = BLOCK_DEVICE.get().unwrap();
    let mut write_buffer = [0u8; 512];
    let mut read_buffer = [0u8; 512];
    for i in 100..553 {
        for byte in write_buffer.iter_mut() {
            *byte = i as u8;
        }
        block_device.write(i as usize, &write_buffer);
        block_device.read(i as usize, &mut read_buffer);
        assert_eq!(write_buffer, read_buffer);
    }
    println!("block device test passed!");
}
