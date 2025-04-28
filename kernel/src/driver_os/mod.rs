use core::{mem, ptr::NonNull};

use alloc::{string::ToString, sync::Arc};
use config::mm::{DTB_ADDR, KERNEL_MAP_OFFSET};
use driver::{
    BLOCK_DEVICE, BlockDevice, CHAR_DEVICE, DeviceType, MmioSerialPort,
    device::{DevId, DeviceMajor, DeviceMeta},
    net::{loopback::LoopbackDev, virtnet::create_virt_net_dev},
    println,
    qemu::{UartDevice, VirtBlkDevice},
};
use fdt::Fdt;
use mm::address::PhysAddr;
use net::init_network;
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

    let blk = probe_virtio_blk(&device_tree);
    BLOCK_DEVICE.call_once(|| blk.unwrap());

    let mut buf: [u8; 512] = [0; 512];
    BLOCK_DEVICE.get().unwrap().read(0, &mut buf);
    log::debug!("BLOCK_DEVICE INIT SUCCESS");

    let chardev = probe_char_device(&device_tree);
    CHAR_DEVICE.call_once(|| Arc::new(UartDevice::from_another(chardev.unwrap())));

    init_net(&device_tree);
    println!("CHAR_DEVICE INIT SUCCESS");

    log::debug!("probe_test finish");
}

pub fn probe_virtio_net(root: &Fdt) -> Option<DeviceMeta> {
    log::debug!("begin to probe_virtio_net");
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

    log::debug!("[probe_virtio_net] pass");
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

            if dev.is_some() {
                break;
            }
            KERNEL_PAGE_TABLE.iounmap(mmio_base_paddr.to_va_kernel().to_usize(), mmio_size);
        }
    }
    if dev.is_none() {
        log::warn!("No virtio block device found");
    }
    log::debug!("[probe_virtio_blk] pass");
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

pub fn probe_char_device(root: &Fdt) -> Option<MmioSerialPort> {
    log::debug!("[probe_char_device] start");
    let chosen = root.chosen();
    // Serial
    let mut stdout = chosen.stdout();
    if stdout.is_none() {
        log::debug!("Non-standard stdout device, trying to workaround");
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
        log::debug!("Searching stdout: {}", stdout_path);
        stdout = root.find_node(stdout_path);
    }
    if stdout.is_none() {
        log::debug!("Unable to parse /chosen, choosing first serial device");
        stdout = root.find_compatible(&[
            "ns16550a",
            "snps,dw-apb-uart", // C910, VF2
            "sifive,uart0",     // sifive_u QEMU (FU540)
        ])
    }
    let stdout = stdout.expect("Still unable to get stdout device");
    log::debug!("Stdout: {}", stdout.name);

    let serial = probe_serial_console(&stdout);
    log::debug!("[probe_char_device] pass");
    Some(serial)
}

/// This guarantees to return a Serial device
/// The device is not initialized yet
fn probe_serial_console(stdout: &fdt::node::FdtNode) -> MmioSerialPort {
    let reg = stdout.reg().unwrap().next().unwrap();
    let base_paddr = reg.starting_address as usize;
    let size = reg.size.unwrap();
    let base_vaddr = base_paddr + KERNEL_MAP_OFFSET;
    let irq_number = stdout.property("interrupts").unwrap().as_usize().unwrap();
    log::info!("IRQ number: {}", irq_number);
    let first_compatible = stdout.compatible().unwrap().first();
    match first_compatible {
        "ns16550a" | "snps,dw-apb-uart" => {
            // VisionFive 2 (FU740)
            // virt QEMU

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
            log::info!(
                "uart: base_paddr:{base_paddr:#x}, size:{size:#x}, reg_io_width:{reg_io_width}, reg_shift:{reg_shift}"
            );

            log::debug!("MmioSerialPort::new {:#x}", base_vaddr);
            unsafe { MmioSerialPort::new(base_vaddr) }
        }
        _ => panic!("Unsupported serial console"),
    }
}

pub fn init_net(root: &Fdt) {
    let netmeta = probe_virtio_net(root);
    if let Some(net_meta) = netmeta {
        let transport = probe_mmio_device(
            PhysAddr::new(net_meta.mmio_base).to_va_kernel().to_usize() as *mut u8,
            net_meta.mmio_size,
            Some(DeviceType::Network),
        )
        .unwrap();

        let dev = create_virt_net_dev(transport).expect("create virt net failed");
        init_network(dev, false);
    } else {
        log::info!("[init_net] can't find qemu virtio-net.");
        init_network(LoopbackDev::new(), true);
    }
}
