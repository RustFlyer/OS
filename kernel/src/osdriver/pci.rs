use core::mem;
use core::ptr::NonNull;

use alloc::sync::Arc;
use config::mm::KERNEL_MAP_OFFSET;
use driver::qemu::VirtBlkDevice;
use driver::qemu::hal::VirtHalImpl;
use driver::{BLOCK_DEVICE, BlockDevice};
use fdt::Fdt;
use virtio_drivers::transport::mmio::MmioTransport;
use virtio_drivers::transport::pci::VirtioPciError;
use virtio_drivers::transport::pci::bus::{
    Cam, ConfigurationAccess, DeviceFunction, MmioCam, PciRoot,
};
use virtio_drivers::transport::{self, Transport};
use virtio_drivers::transport::{DeviceType, pci::PciTransport};

pub fn probe_pci_root<'a>(root: &'a Fdt<'a>) -> PciRoot<MmioCam<'a>> {
    let pcie_node = root
        .find_node("/pcie@20000000")
        .expect("PCIe node not found in device tree");

    let reg = pcie_node.reg().unwrap().next().unwrap();

    log::debug!("{:?}", reg);

    // let ecam_base = reg.starting_address as usize;

    let base = (reg.starting_address as usize + KERNEL_MAP_OFFSET) as *mut u8;

    let config_access = unsafe { MmioCam::new(base, Cam::Ecam) };

    let pci_root = unsafe { PciRoot::new(config_access) };

    // log::debug!("{:?}", pci_root);

    // let mut ptr = (ecam_base + KERNEL_MAP_OFFSET) as *const u64;

    // unsafe {
    //     log::debug!("{:#x}", *ptr);
    //     ptr.add(4);
    //     log::debug!("{:#X}", *ptr);
    // }

    pci_root
}

pub fn probe_virtio_blk_pci(pci_root: &mut PciRoot<MmioCam>) -> Option<Arc<VirtBlkDevice>> {
    log::debug!("begin to find");
    for bus in 0..=0x7f {
        for device in 0..32 {
            for function in 0..8 {
                let dev_fn = DeviceFunction {
                    bus,
                    device,
                    function,
                };
                // log::debug!("look for {:?}", dev_fn);
                if bus == 0 && (device == 1 || device == 2) && function == 0 {
                    simdebug::stop();
                    let virt_addr = KERNEL_MAP_OFFSET
                        + ((bus as usize) << 20)
                        + ((device as usize) << 15)
                        + ((function as usize) << 12);
                    let bar0 =
                        unsafe { core::ptr::read_volatile((virt_addr + 0x10) as *const u32) };
                    log::info!("addr: {:#x} BAR0: {:#x}", virt_addr, bar0);

                    let bars = pci_root.bars(dev_fn).unwrap();
                    bars.iter().for_each(|bar| {
                        if bar.is_some() {
                            log::debug!("{:?}", bar.clone().unwrap());
                        }
                    });

                    if device == 2 {
                        let mut next_addr = 0x20001000; // 你可以选一块空闲的物理地址

                        for bar in 0..6 {
                            let orig = pci_root.bar_info(dev_fn, bar);

                            if let Err(_) = orig {
                                continue;
                            }

                            let bar_size = orig.unwrap().memory_address_size();

                            if let Some((addr, size)) = bar_size {
                                next_addr = ((next_addr / size) + 1) * size;
                                pci_root.set_bar_32(dev_fn, bar, next_addr as u32);
                                log::info!(
                                    "BAR{} assigned to {:#x} (size {:#x})",
                                    bar,
                                    next_addr,
                                    size
                                );
                                next_addr += size;
                            }
                        }
                    }
                }

                match PciTransport::new::<VirtHalImpl, MmioCam>(pci_root, dev_fn) {
                    Ok(transport) => {
                        log::debug!("find {:?}", transport);
                        if transport.device_type() == DeviceType::Block {
                            let dev = Arc::new(VirtBlkDevice::new_from(transport));
                            log::info!(
                                "[probe_virtio_blk_pci] created a new block device: {:?}",
                                dev.clone().block_size()
                            );
                            return Some(dev);
                        }
                    }
                    Err(e) => {
                        if let VirtioPciError::InvalidVendorId(_) = e {
                            continue;
                        }
                        log::error!("dev_fn: {:?}, {:?}", dev_fn, e);
                    }
                }
            }
        }
    }

    log::warn!("No virtio block device found on PCI");
    None
}
