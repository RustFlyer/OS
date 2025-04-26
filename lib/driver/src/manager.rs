use core::{mem, ptr::NonNull};

use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use virtio_drivers::transport::{self, DeviceType, Transport, mmio::MmioTransport};

use crate::{
    cpu::CPU,
    device::{DevId, Device, DeviceMeta},
    plic::PLIC,
};

/// The DeviceManager struct is responsible for managing the devices within the
/// system. It handles the initialization, probing, and interrupt management for
/// various devices.
pub struct DeviceManager {
    /// Optional PLIC (Platform-Level Interrupt Controller) to manage external
    /// interrupts.
    pub plic: Option<PLIC>,

    /// Vector containing CPU instances. The capacity is set to accommodate up
    /// to 8 CPUs.
    pub cpus: Vec<CPU>,

    /// A BTreeMap that maps device IDs (DevId) to device instances (Arc<dyn
    /// Device>). This map stores all the devices except for network devices
    /// which are managed separately by the `InterfaceWrapper` in the `net`
    /// module.
    pub devices: BTreeMap<DevId, Arc<dyn Device>>,

    pub net: Option<DeviceMeta>,

    /// A BTreeMap that maps interrupt numbers (irq_no) to device instances
    /// (Arc<dyn Device>). This map is used to quickly locate the device
    /// responsible for handling a specific interrupt.
    pub irq_map: BTreeMap<usize, Arc<dyn Device>>,
}

pub(crate) fn probe_mmio_device(
    reg_base: *mut u8,
    _reg_size: usize,
    type_match: Option<DeviceType>,
) -> Option<MmioTransport> {
    use transport::mmio::VirtIOHeader;

    let header = NonNull::new(reg_base as *mut VirtIOHeader).unwrap();
    if let Ok(transport) = unsafe { MmioTransport::new(header) } {
        if type_match.is_none() || transport.device_type() == type_match.unwrap() {
            log::info!(
                "Detected virtio MMIO device with vendor id: {:#X}, device type: {:?}, version: {:?}",
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
