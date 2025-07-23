pub mod manager;
pub mod probe;
use core::ptr::NonNull;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use driver::{
    CHAR_DEVICE, DeviceType,
    block::{dw_mshc::MMC, virtblk::VirtBlkDevice},
    cpu::CPU,
    device::OSDevice,
    plic::PLIC,
    println,
    serial::{Serial, uart8250::Uart},
};
use manager::{device_manager, init_device_manager};
use mm::address::PhysAddr;
use probe::*;

use flat_device_tree::{Fdt, node::FdtNode};

use config::{
    board::CLOCK_FREQ,
    mm::{DTB_ADDR, KERNEL_MAP_OFFSET},
};
use virtio_drivers::transport::{
    Transport,
    mmio::{MmioTransport, VirtIOHeader},
};

#[cfg(target_arch = "riscv64")]
use crate::vm::iomap::ioremap;

#[allow(unused)]
pub fn probe_device_tree() {
    println!("DTB_ADDR: {:#x}", unsafe { DTB_ADDR });
    let mut dtb_addr = unsafe { DTB_ADDR };
    println!("[CONSOLE] INIT SUCCESS");

    init_device_manager();
    println!("[DEVICE-MANAGER] INIT SUCCESS");

    #[cfg(target_arch = "riscv64")]
    ioremap(dtb_addr, 0xe865).expect("can not ioremap");

    #[cfg(target_arch = "loongarch64")]
    unsafe {
        dtb_addr = 0x00100000;
    }

    let device_tree = unsafe {
        println!("dt: {:#x}", dtb_addr + KERNEL_MAP_OFFSET);
        Fdt::from_ptr((dtb_addr + KERNEL_MAP_OFFSET) as *const u8).expect("Parse DTB failed")
    };

    if let Ok(chosen) = device_tree.chosen() {
        if let Some(bootargs) = chosen.bootargs() {
            log::debug!("Bootargs: {:?}", bootargs);
        }
    }

    unsafe {
        if let Ok(freq) = device_tree.cpus().next().unwrap().timebase_frequency() {
            CLOCK_FREQ = freq;
        }
    }
    log::warn!("clock freq set to {} Hz", unsafe { CLOCK_FREQ });

    println!("FIND DTB TREE {:?}", device_tree);
    probe_tree(&device_tree);

    let manager = device_manager();
    manager.map_devices();
    manager.initialize_devices();
    manager.map_devices_interrupt();
    manager.enable_device_interrupts();
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
        log::info!("plic base_address: {mmio_base:#x}, size: {mmio_size:#x}");
        ioremap_if_need(mmio_base, mmio_size);
        Some(PLIC::new(mmio_base, mmio_size))
    } else {
        log::error!("[PLIC probe] failed to find plic");
        None
    }
}

pub fn probe_char_device_by_serial(root: &Fdt) -> Option<Arc<Serial>> {
    let chosen = root.chosen();

    if chosen.is_err() {
        return None;
    }

    // Serial
    let mut stdout = chosen.unwrap().stdout().map(|node| node.node());
    if stdout.is_none() {
        println!("Non-standard stdout device, trying to workaround");
        let chosen = root.find_node("/chosen").expect("No chosen node");
        let stdout_path = chosen
            .properties()
            .find(|n| n.name == "stdout-path")
            .and_then(|n| {
                let bytes = unsafe {
                    core::slice::from_raw_parts_mut((n.value.as_ptr()) as *mut u8, n.value.len())
                };
                let mut len = 0;
                for byte in bytes.iter() {
                    if *byte == b':' {
                        return core::str::from_utf8(&n.value[..len]).ok();
                    }
                    len += 1;
                }
                core::str::from_utf8(&n.value[..n.value.len() - 1]).ok()
            })
            .unwrap();
        println!("Searching stdout: {}", stdout_path);
        stdout = root.find_node(stdout_path);
    }

    if stdout.is_none() {
        println!("Unable to parse /chosen, choosing first serial device");
        stdout = root.find_compatible(&[
            "ns16550a",
            "snps,dw-apb-uart", // C910, VF2
            "sifive,uart0",     // sifive_u QEMU (FU540)
        ])
    }

    let stdout = stdout.expect("Still unable to get stdout device");
    println!("Stdout: {}", stdout.name);

    let serial = probe_serial_console(&stdout);
    Some(Arc::new(serial))
}

/// This guarantees to return a Serial device
/// The device is not initialized yet
fn probe_serial_console(stdout: &FdtNode) -> Serial {
    let reg = stdout.reg().next().unwrap();
    let base_paddr = reg.starting_address as usize;
    let size = reg.size.unwrap();
    let base_vaddr = base_paddr + KERNEL_MAP_OFFSET;
    let irq_number = stdout.property("interrupts").unwrap().as_usize().unwrap();
    log::warn!("[probe_serial_console] IRQ number: {}", irq_number);
    let first_compatible = stdout
        .compatible()
        .unwrap()
        .first()
        .expect("no first_compatible");
    match first_compatible {
        "ns16550a" | "snps,dw-apb-uart" => {
            // VisionFive 2 (FU740)
            // virt QEMU

            // Parse clock frequency
            let freq_raw = stdout
                .property("clock-frequency")
                .expect("No clock-frequency property of stdout serial device")
                .as_usize()
                .expect("Parse clock-frequency to usize failed");
            let mut reg_io_width = 1;
            if let Some(reg_io_width_raw) = stdout.property("reg-io-width") {
                reg_io_width = reg_io_width_raw
                    .as_usize()
                    .expect("Parse reg-io-width to usize failed");
            }
            let mut reg_shift = 0;
            if let Some(reg_shift_raw) = stdout.property("reg-shift") {
                reg_shift = reg_shift_raw
                    .as_usize()
                    .expect("Parse reg-shift to usize failed");
            }
            log::error!(
                "uart: base_paddr:{base_paddr:#x}, size:{size:#x}, reg_io_width:{reg_io_width}, reg_shift:{reg_shift}, first_compatible:{first_compatible}"
            );

            ioremap_if_need(base_paddr, size);

            let uart = unsafe {
                Uart::new(
                    base_vaddr,
                    freq_raw,
                    115200,
                    reg_io_width,
                    reg_shift,
                    first_compatible == "snps,dw-apb-uart",
                )
            };
            Serial::new(base_paddr, size, irq_number, Box::new(uart))
        }
        _ => panic!("Unsupported serial console"),
    }
}

pub fn probe_cpu(root: &Fdt) -> Option<Vec<CPU>> {
    let dtb_cpus = root.cpus();
    for prop in root.find_node("/cpus").unwrap().properties() {
        log::info!("{:?}", prop);
    }
    let mut cpus = Vec::new();
    for dtb_cpu in dtb_cpus {
        let mut cpu = CPU {
            id: dtb_cpu.ids().unwrap().first().unwrap(),
            usable: true,
            clock_freq: dtb_cpu
                .properties()
                .find(|p| p.name == "clock-frequency")
                .map(|p| {
                    let mut a32: [u8; 4] = [0; 4];
                    let mut a64: [u8; 8] = [0; 8];
                    a32.copy_from_slice(p.value);
                    a64.copy_from_slice(p.value);
                    match p.value.len() {
                        4 => u32::from_be_bytes(a32) as usize,
                        8 => u64::from_be_bytes(a64) as usize,
                        _ => unreachable!(),
                    }
                })
                .unwrap_or(0),
            timebase_freq: dtb_cpu
                .timebase_frequency()
                .unwrap_or(unsafe { CLOCK_FREQ }),
        };

        // Mask CPU without MMU
        // Get RISC-V ISA string
        let isa = dtb_cpu.property("riscv,isa").expect("RISC-V ISA not found");
        if isa.as_str().unwrap().contains('u') {
            // Privleged mode is in ISA string
            if !isa.as_str().unwrap().contains('s') {
                cpu.usable = false;
            }
        }
        // Check mmu type
        let mmu_type = dtb_cpu.property("mmu-type");
        if mmu_type.is_none() {
            cpu.usable = false;
        }
        // Add to list
        cpus.push(cpu);
    }
    log::info!("cpus: {cpus:?}");
    Some(cpus)
}

pub fn probe_sdio_blk(root: &Fdt) -> Option<Arc<MMC>> {
    // Parse SD Card Host Controller
    if let Some(sdhci) = root.find_node("/soc/sdio1@16020000") {
        let base_address = sdhci.reg().next().unwrap().starting_address as usize;
        let size = sdhci.reg().next().unwrap().size.unwrap();
        let irq_number = 33; // Hard-coded from JH7110
        let sdcard = MMC::new(base_address, size, irq_number);
        log::info!("SD Card Host Controller found at 0x{:x}", base_address);
        return Some(Arc::new(sdcard));
    }
    log::warn!("SD Card Host Controller not found");
    None
}

pub async fn test_serial_output() {
    let buf = "Test Serial Output\n";
    CHAR_DEVICE.get().unwrap().write(buf.as_bytes()).await;
}
