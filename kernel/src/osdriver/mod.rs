pub mod manager;
pub mod pci;
pub mod probe;

#[cfg(target_arch = "loongarch64")]
pub mod pbla;

#[cfg(target_arch = "riscv64")]
pub mod pbrv;

use config::mm::{DTB_ADDR, KERNEL_MAP_OFFSET};
use driver::{CHAR_DEVICE, println};
use flat_device_tree::Fdt;
use manager::{device_manager, init_device_manager};
use probe::*;

#[allow(unused)]
pub fn probe_device_tree() {
    println!("DTB_ADDR: {:#x}", unsafe { DTB_ADDR });
    let mut dtb_addr = unsafe { DTB_ADDR };

    init_device_manager();
    println!("[DEVICE-MANAGER] INIT SUCCESS");

    #[cfg(target_arch = "riscv64")]
    crate::vm::iomap::ioremap(dtb_addr, 0xe865).expect("can not ioremap");

    #[cfg(target_arch = "loongarch64")]
    {
        dtb_addr = 0x100000
    }

    #[cfg(feature = "qemu")]
    println!("RUNNING ON QEMU-9.21");
    #[cfg(feature = "board")]
    println!("RUNNING ON BOARD");

    let device_tree = unsafe {
        println!("dt: {:#x}", dtb_addr + KERNEL_MAP_OFFSET);
        let mut fdt;

        #[cfg(not(all(target_arch = "loongarch64", feature = "board")))]
        {
            fdt = Fdt::from_ptr((dtb_addr + KERNEL_MAP_OFFSET) as *const u8)
                .expect("Parse DTB failed");
        }

        #[cfg(all(target_arch = "loongarch64", feature = "board"))]
        {
            let fdt_ref: &[u8] = include_bytes!("../../../board/loongarch/ls2k1000_dp.dtb");
            let original_pointer = fdt_ref.as_ptr();
            println!("original_pointer: {:#x}", original_pointer as usize);
            fdt = unsafe { Fdt::from_ptr(original_pointer) }.unwrap()
        }

        fdt
    };

    probe_tree(&device_tree);

    let manager = device_manager();
    manager.map_devices();
    manager.initialize_devices();
    manager.map_devices_interrupt();
    manager.enable_device_interrupts();
}

pub fn ioremap_if_need(paddr: usize, size: usize) -> usize {
    log::debug!("map paddr: {paddr:#x}, size: {size:#x}");
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

pub async fn test_serial_output() {
    let buf = "Test Serial Output\n";
    CHAR_DEVICE.get().unwrap().write(buf.as_bytes()).await;
}
