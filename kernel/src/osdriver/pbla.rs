use crate::osdriver::ioremap_if_need;
use crate::osdriver::manager::device_manager;
use alloc::sync::Arc;
use alloc::vec::Vec;
use config::board::CLOCK_FREQ;
use driver::cpu::CPU;
use driver::icu::cascaded::CascadedICU;
use driver::icu::ehic::LoongArchEIOINTC;
use driver::icu::icu_lavirt::{LoongArchVirtICU, TriggerType};
use driver::icu::icu2k1000::LoongArch2K1000ICU;
use driver::icu::pch::LoongArchPCHPIC;
use driver::{block::ahci::ahci::AHCI, println, qemu::QUartDevice};
use flat_device_tree::Fdt;

pub fn probe_ahci_blk(root: &Fdt) -> Option<Arc<AHCI>> {
    // Parse SD Card Host Controller
    if let Some(ahcinod) = root.find_node("/2k1000-soc/ahci@400e0000") {
        let base_address = ahcinod.reg().next().unwrap().starting_address as usize;
        let size = ahcinod.reg().next().unwrap().size.unwrap();
        let irq_number = 33; // Hard-coded from JH7110
        let ahci = AHCI::new(base_address, size, irq_number);
        log::info!("AHCI Controller found at 0x{:x}", base_address);
        return Some(Arc::new(ahci));
    }
    log::warn!("AHCI Controller not found");
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
        let node = node;

        let irq_number = node.property("interrupts").unwrap().as_usize().unwrap();
        let irq = (irq_number >> 32) as u32;
        let flags = (irq_number & 0xFFFF_FFFF) as u32;

        let trig = match flags {
            1 => Some(TriggerType::RisingEdge),
            2 => Some(TriggerType::FallingEdge),
            4 => Some(TriggerType::HighLevel),
            8 => Some(TriggerType::LowLevel),
            _ => None,
        };

        let reg = node.reg().next().unwrap();
        let _base = ioremap_if_need(reg.starting_address as usize, reg.size.unwrap());
        println!("[CHAR_DEVICE] INIT..., irq_number: {}", irq);
        Some(Arc::new(QUartDevice::new(
            reg.starting_address as usize,
            reg.size.unwrap(),
            irq as usize,
        )))
    } else {
        None
    }
}

pub fn probe_cpu(root: &Fdt) -> Option<Vec<CPU>> {
    let dtb_cpus = root.cpus();
    for prop in root.find_node("/cpus").unwrap().properties() {
        println!("{:?}", prop);
    }
    let mut cpus = Vec::new();
    for dtb_cpu in dtb_cpus {
        let cpu = CPU {
            id: dtb_cpu.ids().unwrap().first().unwrap(),
            usable: true,
            clock_freq: unsafe { CLOCK_FREQ },
            timebase_freq: dtb_cpu
                .timebase_frequency()
                .unwrap_or(unsafe { CLOCK_FREQ }),
        };

        // Add to list
        cpus.push(cpu);
    }
    log::info!("cpus: {cpus:?}");
    Some(cpus)
}

pub fn probe_icu(root: &Fdt) -> Option<LoongArch2K1000ICU> {
    log::debug!("probe LoongArch2K1000ICU begin");
    if let Some(plic_node) = root.find_compatible(&["loongson,2k1000-icu"]) {
        let plic_reg = plic_node.reg().next().unwrap();
        let mmio_base = plic_reg.starting_address as usize;
        let mmio_size = plic_reg.size.unwrap();
        log::info!("LoongArch2K1000ICU base_address: {mmio_base:#x}, size: {mmio_size:#x}");
        ioremap_if_need(mmio_base, mmio_size);
        // let icu = LoongArch2K1000ICU::new(mmio_base, mmio_size, mmio_base, mmio_size);
        let icu = LoongArch2K1000ICU::new(
            0x1fe01400, 0x40, // main reg
            0x1fe01040, 0x10, // side reg
        );

        icu.set_trigger_type(
            driver::icu::icu2k1000::irq_numbers::UART0_IRQ,
            driver::icu::icu2k1000::TriggerType::RisingEdge,
        );

        Some(icu)
    } else {
        log::error!("[LoongArch2K1000ICU probe] failed to find LoongArch2K1000ICU");
        None
    }
}

pub fn probe_icu_virt(root: &Fdt) -> Option<LoongArchVirtICU> {
    log::debug!("ICU probe begin");

    if let Some(eiointc) = root.find_compatible(&["loongson,ls2k2000-eiointc"]) {
        // reg = <0x00 0x1400 0x00 0x800>
        let reg = match eiointc.reg().next() {
            Some(r) => r,
            None => {
                log::error!("eiointc: missing reg");
                return None;
            }
        };

        let mmio_base = reg.starting_address as usize;
        let mmio_size = reg.size.unwrap_or(0x800);
        log::error!(
            "EIOINTC(virt) base: {:#x}, size: {:#x}",
            mmio_base,
            mmio_size
        );
        ioremap_if_need(mmio_base, mmio_size);

        let icu = LoongArchVirtICU::new(mmio_base, mmio_size);

        log::info!("ICU probe: using EIOINTC (virt)");
        return Some(icu);
    }

    None
}

pub fn probe_cascaded_icu(root: &Fdt) -> Option<CascadedICU> {
    log::debug!("Probe cascaded ICU (EIOINTC + PCH-PIC)");

    // 1. 先探测 EIOINTC
    let eiointc_node = root.find_compatible(&["loongson,ls2k2000-eiointc"])?;
    let eiointc_reg = eiointc_node.reg().next()?;
    let eiointc_base = eiointc_reg.starting_address as usize;
    let eiointc_size = eiointc_reg.size.unwrap_or(0x800);

    log::info!("EIOINTC at {:#x}, size {:#x}", eiointc_base, eiointc_size);
    ioremap_if_need(eiointc_base, eiointc_size);

    let eiointc = LoongArchEIOINTC::new(eiointc_base, eiointc_size);

    // 2. 探测 PCH-PIC
    let pch_node = root.find_compatible(&["loongson,pch-pic-1.0"])?;
    let pch_reg = pch_node.reg().next()?;
    let pch_base = pch_reg.starting_address as usize;
    let pch_size = pch_reg.size.unwrap_or(0x400);

    // 读取 base-vec 属性
    let base_vec = pch_node
        .property("loongson,pic-base-vec")
        .and_then(|p| p.as_usize())
        .unwrap_or(0) as u32;

    log::info!(
        "PCH-PIC at {:#x}, size {:#x}, base_vec: {:#x}",
        pch_base,
        pch_size,
        base_vec
    );
    ioremap_if_need(pch_base, pch_size);

    let pch_pic = LoongArchPCHPIC::new(pch_base, pch_size, base_vec);

    let pch_irq_base = 0;
    let pch_irq_count = 64;

    Some(CascadedICU::new(
        eiointc,
        pch_pic,
        pch_irq_base,
        pch_irq_count,
    ))
}
