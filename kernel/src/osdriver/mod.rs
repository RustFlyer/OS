use config::mm::{DTB_ADDR, KERNEL_MAP_OFFSET};

#[cfg(target_arch = "riscv64")]
pub mod mmio;
#[cfg(target_arch = "loongarch64")]
pub mod pci;

#[cfg(target_arch = "riscv64")]
use mmio::*;
#[cfg(target_arch = "loongarch64")]
use pci::*;

use driver::BLOCK_DEVICE;
use flat_device_tree::Fdt;

pub fn probe_tree() {
    unsafe { DTB_ADDR = 0x00100000 };
    let device_tree = unsafe {
        Fdt::from_ptr((DTB_ADDR + KERNEL_MAP_OFFSET) as *const u8).expect("Parse DTB failed")
    };
    if let Ok(chosen) = device_tree.chosen() {
        if let Some(bootargs) = chosen.bootargs() {
            log::debug!("Bootargs: {:?}", bootargs);
        }
    }
    // log::debug!("Device: {:?}", device_tree.root());
    log::debug!("build dtb");

    probe_pci(&device_tree);

    // let mut pciroot = probe_pci_root(&device_tree);
    // let dev = probe_virtio_blk_pci(&mut pciroot);

    // if let Some(transport) = dev {
    //     log::debug!("init block");
    //     BLOCK_DEVICE.call_once(|| transport);
    // }

    // let blk = probe_virtio_blk(&device_tree);
    // BLOCK_DEVICE.call_once(|| blk.unwrap());

    // let chardev = probe_char_device(&device_tree);
    // CHAR_DEVICE.call_once(|| Arc::new(UartDevice::new_from(chardev.unwrap())));
}
