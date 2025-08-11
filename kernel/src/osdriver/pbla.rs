use alloc::sync::Arc;
use driver::{block::ahci::ahci::AHCI, println, qemu::QUartDevice};
use flat_device_tree::Fdt;

use crate::osdriver::ioremap_if_need;

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
