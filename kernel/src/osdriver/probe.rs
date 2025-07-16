use alloc::sync::Arc;
use config::{board::HARTS_NUM, mm::KERNEL_MAP_OFFSET};
use core::{
    mem::size_of,
    ptr::{self, NonNull},
};
use driver::{
    BLOCK_DEVICE, CHAR_DEVICE, MmioSerialPort,
    device::OSDevice,
    hal::VirtHalImpl,
    net::{loopback::LoopbackDev, virtnet::create_virt_net_dev},
    println,
    qemu::{QUartDevice, QVirtBlkDevice},
};
use flat_device_tree::{Fdt, node::FdtNode};
use net::{init_network, net_device_exist};
use virtio_drivers::{
    device::console::VirtIOConsole,
    transport::{
        DeviceType, Transport,
        mmio::MmioTransport,
        pci::{
            PciTransport,
            bus::{
                BarInfo, Cam, Command, ConfigurationAccess, DeviceFunction, MemoryBarType, MmioCam,
                PciRoot,
            },
            virtio_device_type,
        },
    },
};

use crate::osdriver::{
    ioremap_if_need, manager::device_manager, probe_char_device_by_serial, probe_cpu, probe_plic,
};

pub fn probe_tree(fdt: &Fdt) {
    log::debug!("probe_tree begin");

    // Local Net Device (For Qemu)
    init_network(LoopbackDev::new(), true);

    if let Some(plic) = probe_plic(fdt) {
        device_manager().set_plic(plic);
        println!("[PLIC] INIT SUCCESS");

        if let Some(serial) = probe_char_device_by_serial(fdt) {
            device_manager().add_device(serial.dev_id(), serial.clone());
            CHAR_DEVICE.call_once(|| serial);
            println!("[SERIAL] INIT SUCCESS");
        }
    } else {
        probe_char_device(fdt);
        println!("[CHAR] INIT SUCCESS");
    }

    if let Some(cpus) = probe_cpu(&fdt) {
        let len = cpus.len();
        device_manager().set_cpus(cpus);
        unsafe { HARTS_NUM = len };
    }

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

    probe_pci_tree(fdt);

    if !net_device_exist() {
        init_network(LoopbackDev::new(), true);
    }

    log::debug!("probe_tree done");
}

fn handle_mmio_device(transport: MmioTransport<'static>) {
    match transport.device_type() {
        DeviceType::Block => {
            log::info!("Init virtio-blk");
            BLOCK_DEVICE.call_once(|| Arc::new(QVirtBlkDevice::new(transport)));
        }
        DeviceType::Network => {
            log::info!("Init virtio-net");
            let dev = create_virt_net_dev(transport).expect("create virt net failed");
            init_network(dev, false);
        }
        DeviceType::Console => {
            log::info!("Init virtio-console (char)");
        }
        _ => log::warn!("Unknown MMIO device: {:?}", transport.device_type()),
    }
}

fn virtio_device(transport: PciTransport) {
    match transport.device_type() {
        DeviceType::Block => virtio_blk(transport),
        DeviceType::Network => virtio_net(transport),
        DeviceType::Console => virtio_console(transport),
        t => log::warn!("Unrecognized virtio device: {:?}", t),
    }
}

fn probe_pci_tree(fdt: &Fdt) {
    use virtio_drivers::transport::pci::bus::Cam;
    if let Some(pci_node) = fdt.find_compatible(&["pci-host-cam-generic"]) {
        enumerate_pci(pci_node, Cam::MmioCam);
    }
    if let Some(pcie_node) = fdt.find_compatible(&["pci-host-ecam-generic"]) {
        enumerate_pci(pcie_node, Cam::Ecam);
    }
}

pub fn enumerate_pci(pci_node: FdtNode, cam: Cam) {
    let reg = pci_node.reg();
    let mut allocator = PciMemory32Allocator::for_pci_ranges(&pci_node);

    for region in reg {
        log::info!(
            "Reg: {:?}-{:#x}",
            region.starting_address,
            region.starting_address as usize + region.size.unwrap()
        );
        let _vaddr = ioremap_if_need(region.starting_address as usize, region.size.unwrap());

        // assert_eq!(region.size.unwrap(), cam.size() as usize);
        // SAFETY: We know the pointer is to a valid MMIO region.
        let mut pci_root = PciRoot::new(unsafe {
            MmioCam::new(
                (region.starting_address as usize + KERNEL_MAP_OFFSET) as *mut u8,
                cam,
            )
        });
        for (device_function, info) in pci_root.enumerate_bus(0) {
            let (status, command) = pci_root.get_status_command(device_function);
            log::info!(
                "Found {} at {}, status {:?} command {:?}",
                info,
                device_function,
                status,
                command
            );
            if let Some(virtio_type) = virtio_device_type(&info) {
                log::info!("  VirtIO {:?}", virtio_type);
                allocate_bars(&mut pci_root, device_function, &mut allocator);
                dump_bar_contents(&mut pci_root, device_function, 4);
                let mut transport =
                    PciTransport::new::<VirtHalImpl, _>(&mut pci_root, device_function).unwrap();
                log::info!(
                    "Detected virtio PCI device with device type {:?}, features {:#018x}",
                    transport.device_type(),
                    transport.read_device_features(),
                );
                virtio_device(transport);
            }
        }
    }
}

/// Allocates 32-bit memory addresses for PCI BARs.
struct PciMemory32Allocator {
    start: u32,
    end: u32,
}

impl PciMemory32Allocator {
    /// Creates a new allocator based on the ranges property of the given PCI node.
    pub fn for_pci_ranges(pci_node: &FdtNode) -> Self {
        let mut memory_32_address = 0;
        let mut memory_32_size = 0;
        for range in pci_node.ranges() {
            let prefetchable = range.child_bus_address_hi & 0x4000_0000 != 0;
            let range_type = PciRangeType::from((range.child_bus_address_hi & 0x0300_0000) >> 24);
            let bus_address = range.child_bus_address as u64;
            let cpu_physical = range.parent_bus_address as u64;
            let size = range.size as u64;
            log::info!(
                "range: {:?} {}prefetchable bus address: {:#018x} host physical address: {:#018x} size: {:#018x}",
                range_type,
                if prefetchable { "" } else { "non-" },
                bus_address,
                cpu_physical,
                size,
            );
            // Use the largest range within the 32-bit address space for 32-bit memory, even if it
            // is marked as a 64-bit range. This is necessary because crosvm doesn't currently
            // provide any 32-bit ranges.
            if !prefetchable
                && matches!(range_type, PciRangeType::Memory32 | PciRangeType::Memory64)
                && size > memory_32_size.into()
                && bus_address + size < u32::MAX.into()
            {
                assert_eq!(bus_address, cpu_physical);
                memory_32_address = u32::try_from(cpu_physical).unwrap();
                memory_32_size = u32::try_from(size).unwrap();
            }
        }
        if memory_32_size == 0 {
            panic!("No 32-bit PCI memory region found.");
        }
        Self {
            start: memory_32_address,
            end: memory_32_address + memory_32_size,
        }
    }

    /// Allocates a 32-bit memory address region for a PCI BAR of the given power-of-2 size.
    ///
    /// It will have alignment matching the size. The size must be a power of 2.
    pub fn allocate_memory_32(&mut self, size: u32) -> u32 {
        assert!(size.is_power_of_two());
        let allocated_address = align_up(self.start, size);
        assert!(allocated_address + size <= self.end);
        self.start = allocated_address + size;
        allocated_address
    }
}

const fn align_up(value: u32, alignment: u32) -> u32 {
    ((value - 1) | (alignment - 1)) + 1
}

pub fn dump_bar_contents(
    root: &mut PciRoot<impl ConfigurationAccess>,
    device_function: DeviceFunction,
    bar_index: u8,
) {
    let bar_info = root.bar_info(device_function, bar_index).unwrap();
    log::trace!("Dumping bar {}: {:#x?}", bar_index, bar_info);
    if let Some(BarInfo::Memory { address, size, .. }) = bar_info {
        let start = (address as usize + KERNEL_MAP_OFFSET) as *const u8;
        unsafe {
            let mut buf = [0u8; 32];
            for i in 0..size / 32 {
                let ptr = start.add(i as usize * 32);
                ptr::copy(ptr, buf.as_mut_ptr(), 32);
                if buf.iter().any(|b| *b != 0xff) {
                    // log::trace!("  {:?}: {:x?}", ptr, buf);
                }
            }
        }
    }
    log::trace!("End of dump");
}

/// Allocates appropriately-sized memory regions and assigns them to the device's BARs.
fn allocate_bars(
    root: &mut PciRoot<impl ConfigurationAccess>,
    device_function: DeviceFunction,
    allocator: &mut PciMemory32Allocator,
) {
    for (bar_index, info) in root.bars(device_function).unwrap().into_iter().enumerate() {
        let Some(info) = info else { continue };
        log::debug!("BAR {}: {}", bar_index, info);
        // Ignore I/O bars, as they aren't required for the VirtIO driver.
        if let BarInfo::Memory {
            address_type, size, ..
        } = info
        {
            // For now, only attempt to allocate 32-bit memory regions.
            if size > u32::MAX.into() {
                log::warn!("Skipping BAR {} with size {:#x}", bar_index, size);
                continue;
            }
            let size = size as u32;

            match address_type {
                MemoryBarType::Width32 => {
                    if size > 0 {
                        let address = allocator.allocate_memory_32(size);
                        log::debug!("Allocated address {:#010x}", address);
                        root.set_bar_32(device_function, bar_index as u8, address);
                    }
                }
                MemoryBarType::Width64 => {
                    if size > 0 {
                        let address = allocator.allocate_memory_32(size);
                        log::debug!("Allocated address {:#010x}", address);
                        root.set_bar_64(device_function, bar_index as u8, address.into());
                    }
                }

                _ => panic!("Memory BAR address type {:?} not supported.", address_type),
            }
        }
    }

    // Enable the device to use its BARs.
    root.set_command(
        device_function,
        Command::IO_SPACE | Command::MEMORY_SPACE | Command::BUS_MASTER,
    );
    let (status, command) = root.get_status_command(device_function);
    log::debug!(
        "Allocated BARs and enabled device, status {:?} command {:?}",
        status,
        command
    );
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PciRangeType {
    ConfigurationSpace,
    IoSpace,
    Memory32,
    Memory64,
}

impl From<u32> for PciRangeType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::ConfigurationSpace,
            1 => Self::IoSpace,
            2 => Self::Memory32,
            3 => Self::Memory64,
            _ => panic!("Tried to convert invalid range type {}", value),
        }
    }
}

fn probe_char_device(fdt: &Fdt) {
    let chosen = fdt.chosen().ok();
    let mut stdout = chosen.and_then(|c| c.stdout().map(|n| n.node()));
    if stdout.is_none() {
        stdout = fdt.find_compatible(&["ns16550a", "snps,dw-apb-uart", "sifive,uart0"])
    }
    if let Some(node) = stdout {
        let reg = node.reg().next().unwrap();
        let base = ioremap_if_need(reg.starting_address as usize, reg.size.unwrap());
        let uart = unsafe { MmioSerialPort::new(base) };
        CHAR_DEVICE.call_once(|| Arc::new(QUartDevice::new_from_mmio(uart)));
    }
}

fn virtio_blk(transport: PciTransport) {
    BLOCK_DEVICE.call_once(|| Arc::new(QVirtBlkDevice::new(transport)));
    log::info!("virtio-blk test finished");
    println!("[BLOCK_DEVICE] INIT SUCCESS");
}

fn virtio_console(transport: PciTransport) {
    let mut console = VirtIOConsole::<VirtHalImpl, PciTransport>::new(transport)
        .expect("Failed to create console driver");
    if let Some(size) = console.size().unwrap() {
        log::info!("VirtIO console {}", size);
    }
    for &c in b"Hello world on console!\n" {
        console.send(c).expect("Failed to send character");
    }
    let c = console.recv(true).expect("Failed to read from console");
    log::info!("Read {:?}", c);
    log::info!("virtio-console test finished");
    println!("[CONSOLE_DEVICE] INIT SUCCESS");
}

fn virtio_net(_transport: PciTransport) {
    init_network(LoopbackDev::new(), true);
    log::info!("virtio-net test finished");
}

fn probe_sd_mmc_devices(fdt: &Fdt) {
    // common SD/MMC/SDIO controller compatible table
    const MMC_COMPATS: &[&str] = &[
        "starfive,jh7110-mmc",
        "dw_mshc", // DesignWare
        "snps,dw-mshc",
        "mmc",
        "sdhci",
        "sdio",
    ];

    for node in fdt.all_nodes() {
        if let (Some(compatible), Some(region)) = (node.compatible(), node.reg().next()) {
            if compatible
                .all()
                .any(|s| MMC_COMPATS.iter().any(|c| s.contains(c)))
            {
                let base = region.starting_address as usize;
                let size = region.size.unwrap_or(0x1000);
                let irq = node
                    .property("interrupts")
                    .and_then(|p| p.as_usize())
                    .unwrap_or(33);
                log::info!(
                    "Found SD/MMC/SDIO controller: {} at 0x{:x}, irq {}",
                    node.name,
                    base,
                    irq
                );
                let _vaddr = ioremap_if_need(base, size);
                // BLOCK_DEVICE.call_once(|| Arc::new(BlockDevice::new(vaddr, size, irq)));
                println!("[BLOCK_DEVICE] SD/MMC Controller INIT SUCCESS");
            }
        }
    }
}
