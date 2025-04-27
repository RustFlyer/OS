use core::{mem, ptr::NonNull};

use alloc::{string::ToString, sync::Arc};
use config::mm::{DTB_ADDR, KERNEL_MAP_OFFSET};
use driver::{
    BlockDevice, DeviceType,
    device::{DevId, DeviceMajor, DeviceMeta},
    qemu::VirtBlkDevice,
};
use fdt::Fdt;
use mm::address::PhysAddr;
use virtio_drivers::transport::{self, Transport, mmio::MmioTransport};

use crate::vm::KERNEL_PAGE_TABLE;

pub fn probe_test() {
    let device_tree = unsafe {
        fdt::Fdt::from_ptr((DTB_ADDR + KERNEL_MAP_OFFSET) as *const u8).expect("Parse DTB failed")
    };

    if let Some(bootargs) = device_tree.chosen().bootargs() {
        log::debug!("Bootargs: {:?}", bootargs);
    }
    log::debug!("Device: {}", device_tree.root().model());

    probe_virtio_blk(&device_tree);

    probe_virtio_net(&device_tree);
}

pub fn probe_virtio_net(root: &Fdt) -> Option<DeviceMeta> {
    let device_tree = root;
    let mut net_meta = None;
    for node in device_tree.find_all_nodes("/soc/virtio_mmio") {
        log::debug!("[probe_virtio_net] probe node {}", node.name);
        for reg in node.reg()? {
            let mmio_base_paddr = PhysAddr::new(reg.starting_address as usize);
            let mmio_size = reg.size?;
            log::debug!("[probe_virtio_net] probe reg {:?}", reg);

            KERNEL_PAGE_TABLE
                .ioremap(mmio_base_paddr.to_usize(), mmio_size)
                .expect("can not ioremap");

            if probe_mmio_device(
                mmio_base_paddr.to_va_kernel().to_usize() as *mut u8,
                mmio_size,
                Some(DeviceType::Network),
            )
            .is_some()
            {
                log::debug!("[probe_virtio_net] find a net device");
                net_meta = {
                    Some(DeviceMeta {
                        mmio_base: mmio_base_paddr.to_usize(),
                        mmio_size,
                        name: "virtio-blk".to_string(),
                        dtype: DeviceType::Network,
                        dev_id: DevId {
                            major: DeviceMajor::Net,
                            minor: 0,
                        },
                        irq_no: None,
                    })
                }
            }
            if net_meta.is_some() {
                break;
            }
        }
    }

    if net_meta.is_none() {
        log::warn!("No virtio net device found");
    }
    net_meta
}

pub fn probe_virtio_blk(root: &Fdt) -> Option<Arc<VirtBlkDevice>> {
    let device_tree = root;
    let mut dev = None;
    for node in device_tree.find_all_nodes("/soc/virtio_mmio") {
        for reg in node.reg()? {
            let mmio_base_paddr = PhysAddr::new(reg.starting_address as usize);
            let mmio_size = reg.size?;
            let irq_no = node.property("interrupts").and_then(|i| i.as_usize());

            log::debug!("[probe_virtio_blk] irq_no :{:?}", irq_no);

            // First map memory, probe virtio device need to map it
            KERNEL_PAGE_TABLE
                .ioremap(mmio_base_paddr.to_usize(), mmio_size)
                .expect("can not ioremap");

            if let Some(transport) = probe_mmio_device(
                mmio_base_paddr.to_va_kernel().to_usize() as *mut u8,
                mmio_size,
                Some(DeviceType::Block),
            ) {
                dev = Some(Arc::new(VirtBlkDevice::new_from(transport)));
                log::info!(
                    "[probe_virtio_blk] created a new block device: {:?}",
                    dev.clone().unwrap().block_size()
                );
            }

            KERNEL_PAGE_TABLE.iounmap(mmio_base_paddr.to_va_kernel().to_usize(), mmio_size);

            if dev.is_some() {
                break;
            }
        }

        if dev.is_some() {
            break;
        }
    }
    if dev.is_none() {
        log::warn!("No virtio block device found");
    }
    log::debug!("");
    dev
}

pub fn probe_mmio_device(
    reg_base: *mut u8,
    _reg_size: usize,
    type_match: Option<DeviceType>,
) -> Option<MmioTransport> {
    use transport::mmio::VirtIOHeader;

    let header = NonNull::new(reg_base as *mut VirtIOHeader).unwrap();

    if let Ok(transport) = unsafe { MmioTransport::new(header) } {
        log::info!("[probe_mmio_device] transport: {:?}", transport);
        log::info!(
            "[probe_mmio_device] transport device: {:?}",
            transport.device_type()
        );
        if type_match.is_none() || transport.device_type() == type_match.unwrap() {
            log::info!(
                "Detected virtio MMIO device with vendor id: {:#x}, device type: {:?}, version: {:?}",
                transport.vendor_id(),
                transport.device_type(),
                transport.version(),
            );
            Some(transport)
        } else {
            mem::forget(transport);
            None
        }
    } else {
        None
    }
}
