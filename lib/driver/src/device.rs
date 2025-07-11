use alloc::{string::String, sync::Arc};
use downcast_rs::DowncastSync;
use virtio_drivers::transport::DeviceType;

use crate::{BlockDevice, CharDevice, net::NetDevice};

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(usize)]
pub enum OSDeviceMajor {
    Serial = 4,
    Block = 8,
    Net = 9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OSDevId {
    /// Major Device Number
    pub major: OSDeviceMajor,
    /// Minor Device Number. It Identifies different device instances of the
    /// same type
    pub minor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OSDeviceMeta {
    /// Device id.
    pub dev_id: OSDevId,
    /// Name of the device.
    pub name: String,
    /// Mmio start address.
    pub mmio_base: usize,
    /// Mmio size.
    pub mmio_size: usize,
    /// Interrupt number.
    pub irq_no: Option<usize>,
    /// Device type.
    pub dtype: OSDeviceKind,
    /// PCI bus/device/function (if applicable)
    pub pci_bdf: Option<(u8, u8, u8)>, // (bus, device, function)
    /// PCI BAR index (if applicable)
    pub pci_bar: Option<u8>,
    /// PCI vendor/device id (if applicable)
    pub pci_ids: Option<(u16, u16)>, // (vendor_id, device_id)
}

pub trait OSDevice: Sync + Send + DowncastSync {
    fn meta(&self) -> &OSDeviceMeta;

    fn init(&self);

    fn handle_irq(&self);

    fn dev_id(&self) -> OSDevId {
        self.meta().dev_id
    }

    fn name(&self) -> &str {
        &self.meta().name
    }

    fn mmio_base(&self) -> usize {
        self.meta().mmio_base
    }

    fn mmio_size(&self) -> usize {
        self.meta().mmio_size
    }

    fn irq_no(&self) -> Option<usize> {
        self.meta().irq_no
    }

    fn dtype(&self) -> OSDeviceKind {
        self.meta().dtype
    }

    fn pci_bdf(&self) -> Option<(u8, u8, u8)> {
        self.meta().pci_bdf
    }

    fn pci_bar(&self) -> Option<u8> {
        self.meta().pci_bar
    }

    fn pci_ids(&self) -> Option<(u16, u16)> {
        self.meta().pci_ids
    }

    fn as_blk(self: Arc<Self>) -> Option<Arc<dyn BlockDevice>> {
        None
    }

    fn as_char(self: Arc<Self>) -> Option<Arc<dyn CharDevice>> {
        None
    }

    fn as_net(self: Arc<Self>) -> Option<Arc<dyn NetDevice>> {
        None
    }
}

use virtio_drivers::transport::DeviceType as VirtioDeviceType;

/// 统一的设备类型枚举，支持板载、VirtIO、PCI等多种设备
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OSDeviceKind {
    /// 板载/SoC内置串口（如 16550A、dw-apb-uart、sifive-uart 等）
    Uart,
    /// 板载/SoC内置 SD/MMC/eMMC 控制器
    SDMMC,
    /// 板载/SoC内置 SPI 控制器
    SPI,
    /// 板载/SoC内置 I2C 控制器
    I2C,
    /// 板载/SoC内置 Flash 控制器
    Flash,
    /// 板载/SoC内置定时器
    Timer,
    /// 板载/SoC内置 GPIO 控制器
    GPIO,
    /// 板载/SoC内置 RTC 控制器
    RTC,
    /// 板载/SoC内置 Watchdog
    Watchdog,
    /// VirtIO 设备（MMIO 或 PCI），带具体类型
    Virtio(VirtioDeviceType),
    /// PCI/PCIe 设备（非 VirtIO），如 NVMe、传统网卡等
    PCI {
        vendor_id: u16,
        device_id: u16,
        class_code: u8,
        subclass: u8,
        prog_if: u8,
    },
    Other(&'static str),
}
