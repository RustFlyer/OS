use alloc::sync::Arc;
use config::board::CLOCK_FREQ;
use core::{mem::size_of, ptr::NonNull};
use driver::{
    BLOCK_DEVICE, CHAR_DEVICE,
    device::OSDevice,
    net::{loopback::LoopbackDev, virtnet::create_virt_net_dev},
    println,
    qemu::QVirtBlkDevice,
};
use flat_device_tree::Fdt;
use net::init_network;
use virtio_drivers::transport::{DeviceType, Transport, mmio::MmioTransport, pci::bus::Cam};

use crate::osdriver::{ioremap_if_need, manager::device_manager, pci::enumerate_pci};

pub fn probe_tree(fdt: &Fdt) {
    log::debug!("probe_tree begin");

    // for riscv probe
    #[cfg(target_arch = "riscv64")]
    {
        // PLIC
        if let Some(plic) = crate::osdriver::pbrv::probe_plic(fdt) {
            device_manager().set_plic(plic);
            println!("[PLIC] INIT SUCCESS");

            // SERIAL
            if let Some(serial) = crate::osdriver::pbrv::probe_char_device_by_serial(fdt) {
                device_manager().add_device(serial.dev_id(), serial.clone());
                CHAR_DEVICE.call_once(|| serial);
                println!("[SERIAL] INIT SUCCESS");
            }

            // SDCARD
            if let Some(sdio) = crate::osdriver::pbrv::probe_sdio_blk(fdt) {
                device_manager().add_device(sdio.dev_id(), sdio.clone());
                BLOCK_DEVICE.call_once(|| sdio.clone());
                println!("[SDIOBLK] INIT SUCCESS");
            }
        } else {
            // CHAR(replaced by serial)
            crate::osdriver::pbrv::probe_char_device(fdt);
            println!("[CHAR] INIT SUCCESS");

            // BLOCK(probed in common part)
        }

        // CPUs
        if let Some(cpus) = crate::osdriver::pbrv::probe_cpu(&fdt) {
            device_manager().set_cpus(cpus);
        }

        // NetWork Simple
        init_network(LoopbackDev::new(), true);

        // CLOCK FREQ
        #[allow(static_mut_refs)]
        unsafe {
            if let Ok(freq) = fdt.cpus().next().unwrap().timebase_frequency() {
                CLOCK_FREQ = freq;
            }
            log::warn!("clock freq set to {} Hz", CLOCK_FREQ);
        }
    }

    // for loongarch probe
    #[cfg(target_arch = "loongarch64")]
    {
        // CHAR(used by both qemu & board)
        if let Some(char) = crate::osdriver::pbla::probe_char_device(fdt) {
            device_manager().add_device(char.dev_id(), char.clone());
            CHAR_DEVICE.call_once(|| char);
            println!("[CHAR] INIT SUCCESS");
        }

        // BLOCK(just board)
        if let Some(ahci) = crate::osdriver::pbla::probe_ahci_blk(fdt) {
            device_manager().add_device(ahci.dev_id(), ahci.clone());
            BLOCK_DEVICE.call_once(|| ahci.clone());
            println!("[AHCIBLK] INIT SUCCESS");
        }
    }

    // for common probe(special for qemu)
    for node in fdt.all_nodes() {
        if let (Some(compatible), Some(region)) = (node.compatible(), node.reg().next()) {
            if compatible.all().any(|s| s == "virtio,mmio")
                && region.size.unwrap_or(0)
                    > size_of::<virtio_drivers::transport::mmio::VirtIOHeader>()
            {
                log::debug!("Found MMIO virtio: {}", node.name);
                let vaddr = ioremap_if_need(region.starting_address as usize, region.size.unwrap());
                let header =
                    NonNull::new(vaddr as *mut virtio_drivers::transport::mmio::VirtIOHeader)
                        .unwrap();
                match unsafe { MmioTransport::new(header, region.size.unwrap()) } {
                    Ok(transport) => handle_mmio_device(transport),
                    Err(e) => log::warn!("Failed to create MmioTransport: {}", e),
                }
            }
        }
    }

    // PCI probe(mostly used in loongarch-qemu)
    // pci-host-cam-generic
    if let Some(pci_node) = fdt.find_compatible(&["pci-host-cam-generic"]) {
        enumerate_pci(pci_node, Cam::MmioCam);
    }

    // pci-host-ecam-generic
    if let Some(pcie_node) = fdt.find_compatible(&["pci-host-ecam-generic"]) {
        enumerate_pci(pcie_node, Cam::Ecam);
    }
}

fn handle_mmio_device(transport: MmioTransport<'static>) {
    match transport.device_type() {
        DeviceType::Block => {
            if BLOCK_DEVICE.get().is_none() {
                println!("Init virtio-blk");
                BLOCK_DEVICE.call_once(|| Arc::new(QVirtBlkDevice::new(transport)));
            } else {
                println!("virtio has been initialized!");
            }
        }
        DeviceType::Network => {
            println!("Init virtio-net");
            let dev = create_virt_net_dev(transport).expect("create virt net failed");
            init_network(dev, false);
        }
        DeviceType::Console => {
            println!("Init virtio-console (char)");
        }
        _ => log::warn!("Unknown MMIO device: {:?}", transport.device_type()),
    }
}
