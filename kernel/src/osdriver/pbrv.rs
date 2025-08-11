use alloc::{boxed::Box, sync::Arc, vec::Vec};
use driver::{
    block::dw_mshc::MMC,
    cpu::CPU,
    icu::plic::PLIC,
    println,
    qemu::QUartDevice,
    serial::{Serial, uart8250::Uart},
};

use flat_device_tree::{Fdt, node::FdtNode};

use config::{board::CLOCK_FREQ, mm::KERNEL_MAP_OFFSET};

use crate::osdriver::ioremap_if_need;

/// Guaranteed to have a PLIC
pub fn probe_plic(root: &Fdt) -> Option<PLIC> {
    log::debug!("probe_plic begin");
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
    log::debug!("probe_char_device_by_serial begin");
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
            let freq_raw = if stdout.property("clock-frequency").is_some() {
                stdout
                    .property("clock-frequency")
                    .expect("No clock-frequency property of stdout serial device")
                    .as_usize()
                    .expect("Parse clock-frequency to usize failed")
            } else {
                unsafe { CLOCK_FREQ }
            };

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

pub fn probe_char_device(fdt: &Fdt) -> Option<Arc<QUartDevice>> {
    log::debug!("probe_char_device begin");
    let chosen = fdt.chosen().ok();
    let mut stdout = chosen.and_then(|c| c.stdout().map(|n| n.node()));
    if stdout.is_none() {
        stdout = fdt.find_compatible(&["ns16550a", "snps,dw-apb-uart", "sifive,uart0"])
    }

    if let Some(node) = stdout {
        let reg = node.reg().next().unwrap();
        let _base = ioremap_if_need(reg.starting_address as usize, reg.size.unwrap());
        println!("[CHAR_DEVICE] INIT...");
        Some(Arc::new(QUartDevice::new(
            reg.starting_address as usize,
            reg.size.unwrap(),
            0,
        )))
    } else {
        None
    }
}
