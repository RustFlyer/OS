use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};

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
