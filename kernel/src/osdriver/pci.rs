/// Refer to virtio_drivers
use core::ptr::NonNull;
use core::{mem, ptr};

use alloc::sync::Arc;
use config::mm::KERNEL_MAP_OFFSET;
use driver::hal::VirtHalImpl;
use driver::net::loopback::LoopbackDev;
use driver::qemu::VirtBlkDevice;
use driver::{BLOCK_DEVICE, BlockDevice, println};
use flat_device_tree::Fdt;
use flat_device_tree::node::FdtNode;
use flat_device_tree::standard_nodes::Compatible;
use net::init_network;
use virtio_drivers::device::console::VirtIOConsole;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::transport::pci::bus::{BarInfo, MemoryBarType};
use virtio_drivers::transport::pci::bus::{
    Cam, Command, ConfigurationAccess, DeviceFunction, MmioCam, PciRoot,
};
use virtio_drivers::transport::pci::{VirtioPciError, virtio_device_type};
use virtio_drivers::transport::{self, Transport};
use virtio_drivers::transport::{DeviceType, pci::PciTransport};

/// search for pci root.
pub fn probe_pci_root<'a>(root: &'a Fdt<'a>) -> PciRoot<MmioCam<'a>> {
    let pcie_node = root
        .find_node("/pcie@20000000")
        .expect("PCIe node not found in device tree");

    let reg = pcie_node.reg().next().unwrap();

    log::debug!("{:?}", reg);
    let base = (reg.starting_address as usize + KERNEL_MAP_OFFSET) as *mut u8;
    let config_access = unsafe { MmioCam::new(base, Cam::Ecam) };
    let pci_root = unsafe { PciRoot::new(config_access) };

    pci_root
}

/// search for virtio blk in pci
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
                if bus == 0 && (device == 1 || device == 2) && function == 0 {
                    // DEBUG: output all bars in device
                    let bars = pci_root.bars(dev_fn).unwrap();
                    bars.iter().for_each(|bar| {
                        if bar.is_some() {
                            log::debug!("{:?}", bar.clone().unwrap());
                        }
                    });
                }

                // if device exists and it is valid, this function will return a transport
                match PciTransport::new::<VirtHalImpl, MmioCam>(pci_root, dev_fn) {
                    Ok(transport) => {
                        log::debug!("find {:?}", transport);
                        if transport.device_type() == DeviceType::Block {
                            let dev = Arc::new(VirtBlkDevice::new_from_pci(transport));
                            log::info!(
                                "[probe_virtio_blk_pci] created a new block device: {:?}",
                                dev.clone().block_size()
                            );
                            return Some(dev);
                        }
                    }
                    Err(e) => {
                        if let VirtioPciError::InvalidVendorId(_) = e {
                            // no device found in dev_fn
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

fn virtio_device(transport: impl Transport) {
    panic!("[virtio_device] Mmio not supported");
}

fn virtio_device_pci(transport: PciTransport) {
    match transport.device_type() {
        DeviceType::Block => virtio_blk_pci(transport),
        DeviceType::Console => virtio_console_pci(transport),
        DeviceType::Socket => log::warn!("[virtio_device_pci] Socket: not implemented"),
        t => log::warn!("Unrecognized virtio device: {:?}", t),
    }
}

fn virtio_blk_pci(transport: PciTransport) {
    BLOCK_DEVICE.call_once(|| Arc::new(VirtBlkDevice::new_from_pci(transport)));
    log::info!("virtio-blk test finished");
    println!("[BLOCK_DEVICE] INIT SUCCESS");
}

fn virtio_console_pci(transport: PciTransport) {
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

pub fn probe_pci<'a>(fdt: &'a Fdt<'a>) {
    for node in fdt.all_nodes() {
        // Dump information about the node for debugging.
        log::trace!(
            "{}: {:?}",
            node.name,
            node.compatible().map(Compatible::first),
        );
        for range in node.reg() {
            log::trace!(
                "  {:#018x?}, length {:?}",
                range.starting_address,
                range.size
            );
        }

        // Check whether it is a VirtIO MMIO device.
        if let (Some(compatible), Some(region)) = (node.compatible(), node.reg().next()) {
            if compatible.all().any(|s| s == "virtio,mmio")
                && region.size.unwrap_or(0) > size_of::<VirtIOHeader>()
            {
                log::debug!("Found VirtIO MMIO device at {:?}", region);

                let header = NonNull::new(region.starting_address as *mut VirtIOHeader).unwrap();
                match unsafe { MmioTransport::new(header, region.size.unwrap()) } {
                    Err(e) => log::warn!("Error creating VirtIO MMIO transport: {}", e),
                    Ok(transport) => {
                        log::info!(
                            "Detected virtio MMIO device with vendor id {:#X}, device type {:?}, version {:?}",
                            transport.vendor_id(),
                            transport.device_type(),
                            transport.version(),
                        );
                        virtio_device(transport);
                    }
                }
            }
        }
    }

    if let Some(pci_node) = fdt.find_compatible(&["pci-host-cam-generic"]) {
        log::info!("Found PCI node: {}", pci_node.name);
        enumerate_pci(pci_node, Cam::MmioCam);
    }
    if let Some(pcie_node) = fdt.find_compatible(&["pci-host-ecam-generic"]) {
        log::info!("Found PCIe node: {}", pcie_node.name);
        enumerate_pci(pcie_node, Cam::Ecam);
    }

    init_network(LoopbackDev::new(), true);
    println!("[NET_DEVICE] INIT SUCCESS");
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
                virtio_device_pci(transport);
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
