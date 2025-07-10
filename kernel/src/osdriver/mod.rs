pub mod manager;
pub mod probe;
use driver::plic::PLIC;
use probe::*;

use flat_device_tree::Fdt;

use config::mm::{DTB_ADDR, KERNEL_MAP_OFFSET};

#[cfg(target_arch = "riscv64")]
use crate::vm::iomap::ioremap;

#[allow(unused)]
pub fn probe_device_tree() {
    let mut dtb_addr = unsafe { DTB_ADDR };
    #[cfg(target_arch = "riscv64")]
    ioremap(dtb_addr, 24 * 1024).expect("can not ioremap");
    #[cfg(target_arch = "loongarch64")]
    unsafe {
        dtb_addr = 0x00100000;
    }

    let device_tree = unsafe {
        log::debug!("dt: {:#x}", dtb_addr + KERNEL_MAP_OFFSET);
        Fdt::from_ptr((dtb_addr + KERNEL_MAP_OFFSET) as *const u8).expect("Parse DTB failed")
    };

    if let Ok(chosen) = device_tree.chosen() {
        if let Some(bootargs) = chosen.bootargs() {
            log::debug!("Bootargs: {:?}", bootargs);
        }
    }

    probe_tree(&device_tree);
}

pub fn ioremap_if_need(paddr: usize, size: usize) -> usize {
    #[cfg(target_arch = "riscv64")]
    {
        crate::vm::iomap::ioremap(paddr, size).expect("can not ioremap");
        paddr + KERNEL_MAP_OFFSET
    }
    #[cfg(target_arch = "loongarch64")]
    {
        paddr + KERNEL_MAP_OFFSET
    }
}

/// Guaranteed to have a PLIC
pub fn probe_plic(root: &Fdt) -> Option<PLIC> {
    if let Some(plic_node) = root.find_compatible(&["riscv,plic0", "sifive,plic-1.0.0"]) {
        let plic_reg = plic_node.reg().next().unwrap();
        let mmio_base = plic_reg.starting_address as usize;
        let mmio_size = plic_reg.size.unwrap();
        log::info!("plic base_address:{mmio_base:#x}, size:{mmio_size:#x}");
        ioremap_if_need(mmio_base, mmio_size);
        Some(PLIC::new(mmio_base, mmio_size))
    } else {
        log::error!("[PLIC probe] faild to find plic");
        None
    }
}
