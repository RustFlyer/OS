#[cfg(target_arch = "riscv64")]
pub mod mmio;
#[cfg(target_arch = "loongarch64")]
pub mod pci;

#[cfg(target_arch = "riscv64")]
use mmio::*;
#[cfg(target_arch = "loongarch64")]
use pci::*;

use flat_device_tree::Fdt;

use config::mm::{DTB_ADDR, KERNEL_MAP_OFFSET};

#[cfg(target_arch = "riscv64")]
use crate::vm::iomap::ioremap;

pub fn probe_tree() {
    log::debug!("begin to build dtb");

    #[cfg(target_arch = "loongarch64")]
    unsafe {
        DTB_ADDR = 0x00100000
    };

    let device_tree = unsafe {
        log::debug!("dt: {:#x}", DTB_ADDR + KERNEL_MAP_OFFSET);
        #[cfg(target_arch = "riscv64")]
        ioremap(DTB_ADDR, 24 * 1024).expect("can not ioremap");
        Fdt::from_ptr((DTB_ADDR + KERNEL_MAP_OFFSET) as *const u8).expect("Parse DTB failed")
    };

    if let Ok(chosen) = device_tree.chosen() {
        if let Some(bootargs) = chosen.bootargs() {
            log::debug!("Bootargs: {:?}", bootargs);
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        probe_mmio(&device_tree);
    }

    #[cfg(target_arch = "loongarch64")]
    {
        driver::init();
        probe_pci(&device_tree);
    }
}
