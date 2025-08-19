use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use arch::{interrupt::enable_external_interrupt, trap::disable_interrupt};
use config::device::MAX_HARTS;
use driver::{
    cpu::CPU,
    device::{OSDevId, OSDevice, OSDeviceMajor, OSDeviceMeta},
    icu::ICU,
};

use crate::osdriver::ioremap_if_need;

pub static mut OSDEVICE_MANAGER: Option<DeviceTreeManager> = None;

pub fn init_device_manager() {
    unsafe { OSDEVICE_MANAGER = Some(DeviceTreeManager::new()) }
}

#[allow(static_mut_refs)]
pub fn device_manager() -> &'static mut DeviceTreeManager {
    unsafe { OSDEVICE_MANAGER.as_mut().unwrap() }
}

#[allow(unused)]
pub struct DeviceTreeManager {
    pub icu: Option<Box<dyn ICU>>,

    pub cpus: Vec<CPU>,

    pub devices: BTreeMap<OSDevId, Arc<dyn OSDevice>>,

    pub net: Option<OSDeviceMeta>,

    pub irq_map: BTreeMap<usize, Arc<dyn OSDevice>>,
}

impl DeviceTreeManager {
    /// Creates a new DeviceManager instance with default values.
    /// Initializes the icu to None, reserves space for 8 CPUs, and creates
    /// empty BTreeMaps for devices and irq_map.
    pub fn new() -> Self {
        Self {
            icu: None,
            cpus: Vec::with_capacity(8),
            devices: BTreeMap::new(),
            net: None,
            irq_map: BTreeMap::new(),
        }
    }

    /// Initializes all devices that have been discovered and added to the
    /// device manager.
    pub fn initialize_devices(&mut self) {
        self.devices.values().for_each(|d| d.init());
    }

    pub fn map_devices(&self) {
        // Map probed devices
        for (id, dev) in self.devices() {
            log::debug!("mapping id {:?} device {}", id, dev.name());
            ioremap_if_need(dev.mmio_base(), dev.mmio_size());
        }
        if let Some(net_meta) = &self.net {
            ioremap_if_need(net_meta.mmio_base, net_meta.mmio_size);
        }
    }

    fn icu(&self) -> &Box<dyn ICU> {
        self.icu.as_ref().unwrap()
    }

    pub fn get(&self, dev_id: &OSDevId) -> Option<&Arc<dyn OSDevice>> {
        self.devices.get(dev_id)
    }

    pub fn devices(&self) -> &BTreeMap<OSDevId, Arc<dyn OSDevice>> {
        &self.devices
    }

    pub fn find_devices_by_major(&self, dmajor: OSDeviceMajor) -> Vec<Arc<dyn OSDevice>> {
        self.devices()
            .iter()
            .filter(|(dev_id, _)| dev_id.major == dmajor)
            .map(|(_, dev)| dev)
            .cloned()
            .collect()
    }

    pub fn enable_device_interrupts(&mut self) {
        if self.icu.is_none() {
            log::warn!("no icu");
            return;
        }

        let total = MAX_HARTS;
        for i in 0..total * 2 {
            for dev in self.devices.values() {
                if let Some(irq) = dev.irq_no() {
                    self.icu().enable_irq(irq, i);
                    let name = dev.name();
                    log::warn!("Enable external interrupt: {irq}, context:{i}, name: {name}");
                }
            }
        }
        enable_external_interrupt();
    }

    pub fn handle_irq(&mut self) {
        if self.icu.is_none() {
            log::warn!("no icu");
            return;
        }

        disable_interrupt();

        // First clain interrupt from icu
        if let Some(irq_number) = self.icu().claim_irq(self.irq_context()) {
            // log::warn!("new interrupt: {}", irq_number);
            if let Some(dev) = self.irq_map.get(&irq_number) {
                dev.handle_irq();
                // Complete interrupt when done
                self.icu().complete_irq(irq_number, self.irq_context());
                return;
            }
            log::error!("Unknown interrupt: {}", irq_number);
        } else {
            log::error!("No interrupt available");
        }
    }

    // Calculate the interrupt context from current hart id
    fn irq_context(&self) -> usize {
        1
    }

    /// mmio memory region map finished in this function
    pub fn map_devices_interrupt(&mut self) {
        // Add to interrupt map if have interrupts
        for dev in self.devices.values() {
            if let Some(irq) = dev.irq_no() {
                self.irq_map.insert(irq, dev.clone());
            }
        }
    }

    pub fn add_device(&mut self, id: OSDevId, device: Arc<dyn OSDevice>) {
        self.devices.insert(id, device);
    }

    pub fn set_icu(&mut self, icu: Box<dyn ICU>) {
        self.icu = Some(icu);
    }

    pub fn set_cpus(&mut self, cpus: Vec<CPU>) {
        self.cpus = cpus;
    }
}
