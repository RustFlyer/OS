use alloc::vec::Vec;
use core::mem;

/// perf_event_attr structure (matches Linux kernel)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PerfEventAttr {
    /// Major type: hardware/software/tracepoint/etc.
    pub r#type: u32,
    /// Size of attribute structure
    pub size: u32,
    /// Type-specific configuration information
    pub config: u64,

    /// Sample period or frequency
    pub sample_period_freq: u64,

    /// What values to include in samples
    pub sample_type: u64,
    /// What values to read from counter
    pub read_format: u64,

    /// Various flags
    pub flags: u64,

    /// Events to wakeup on
    pub wakeup_events_watermark: u32,
    /// Type of breakpoint
    pub bp_type: u32,

    /// Breakpoint address or tracepoint id
    pub bp_addr_config1: u64,
    /// Breakpoint length or config2
    pub bp_len_config2: u64,
    /// Additional config
    pub config3: u64,

    /// Branch sample type
    pub branch_sample_type: u64,
    /// User register mask
    pub sample_regs_user: u64,
    /// User stack size
    pub sample_stack_user: u32,
    /// Clock ID for timestamps
    pub clockid: i32,
    /// Interrupt register mask
    pub sample_regs_intr: u64,
    /// AUX watermark
    pub aux_watermark: u32,
    /// AUX sample size
    pub sample_max_stack: u16,
    pub __reserved_2: u16,
    /// AUX sample size
    pub aux_sample_size: u32,
    pub __reserved_3: u32,
    /// Signal data
    pub sig_data: u64,
}

impl PerfEventAttr {
    pub fn new() -> Self {
        Self {
            r#type: 0,
            size: mem::size_of::<PerfEventAttr>() as u32,
            config: 0,
            sample_period_freq: 0,
            sample_type: 0,
            read_format: 0,
            flags: 0,
            wakeup_events_watermark: 0,
            bp_type: 0,
            bp_addr_config1: 0,
            bp_len_config2: 0,
            config3: 0,
            branch_sample_type: 0,
            sample_regs_user: 0,
            sample_stack_user: 0,
            clockid: 0,
            sample_regs_intr: 0,
            aux_watermark: 0,
            sample_max_stack: 0,
            __reserved_2: 0,
            aux_sample_size: 0,
            __reserved_3: 0,
            sig_data: 0,
        }
    }

    /// Validate the attribute structure
    pub fn validate(&self) -> bool {
        // Size validation
        if self.size < PERF_ATTR_SIZE_VER0 || self.size > mem::size_of::<PerfEventAttr>() as u32 {
            return false;
        }

        // Type validation
        if self.r#type > super::flags::PerfType::Breakpoint as u32 {
            return false;
        }

        true
    }

    /// Get sample period (if using period mode)
    pub fn get_sample_period(&self) -> Option<u64> {
        use super::flags::PerfEventAttrFlags;
        if PerfEventAttrFlags::from_bits_truncate(self.flags).contains(PerfEventAttrFlags::FREQ) {
            None
        } else {
            Some(self.sample_period_freq)
        }
    }

    /// Get sample frequency (if using freq mode)
    pub fn get_sample_freq(&self) -> Option<u64> {
        use super::flags::PerfEventAttrFlags;
        if PerfEventAttrFlags::from_bits_truncate(self.flags).contains(PerfEventAttrFlags::FREQ) {
            Some(self.sample_period_freq)
        } else {
            None
        }
    }
}

use super::flags::PERF_ATTR_SIZE_VER0;

/// Sample record header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PerfEventHeader {
    pub r#type: u32,
    pub misc: u16,
    pub size: u16,
}

/// Sample data structure
#[derive(Debug, Clone)]
pub struct PerfSample {
    pub header: PerfEventHeader,
    pub ip: Option<u64>,
    pub pid: Option<u32>,
    pub tid: Option<u32>,
    pub time: Option<u64>,
    pub addr: Option<u64>,
    pub id: Option<u64>,
    pub stream_id: Option<u64>,
    pub cpu: Option<u32>,
    pub period: Option<u64>,
    pub callchain: Option<Vec<u64>>,
    pub raw_data: Option<Vec<u8>>,
    // More fields can be added as needed
}

impl PerfSample {
    pub fn new(sample_type: u64) -> Self {
        Self {
            header: PerfEventHeader {
                r#type: 9, // PERF_RECORD_SAMPLE
                misc: 0,
                size: 0,
            },
            ip: if sample_type & super::flags::PerfSampleType::IP.bits() != 0 {
                Some(0)
            } else {
                None
            },
            pid: if sample_type & super::flags::PerfSampleType::TID.bits() != 0 {
                Some(0)
            } else {
                None
            },
            tid: if sample_type & super::flags::PerfSampleType::TID.bits() != 0 {
                Some(0)
            } else {
                None
            },
            time: if sample_type & super::flags::PerfSampleType::TIME.bits() != 0 {
                Some(0)
            } else {
                None
            },
            addr: if sample_type & super::flags::PerfSampleType::ADDR.bits() != 0 {
                Some(0)
            } else {
                None
            },
            id: if sample_type & super::flags::PerfSampleType::ID.bits() != 0 {
                Some(0)
            } else {
                None
            },
            stream_id: if sample_type & super::flags::PerfSampleType::STREAM_ID.bits() != 0 {
                Some(0)
            } else {
                None
            },
            cpu: if sample_type & super::flags::PerfSampleType::CPU.bits() != 0 {
                Some(0)
            } else {
                None
            },
            period: if sample_type & super::flags::PerfSampleType::PERIOD.bits() != 0 {
                Some(0)
            } else {
                None
            },
            callchain: if sample_type & super::flags::PerfSampleType::CALLCHAIN.bits() != 0 {
                Some(Vec::new())
            } else {
                None
            },
            raw_data: if sample_type & super::flags::PerfSampleType::RAW.bits() != 0 {
                Some(Vec::new())
            } else {
                None
            },
        }
    }

    /// Serialize sample to buffer
    pub fn serialize_into(&self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut offset = 0;

        // Write header
        if buf.len() < offset + mem::size_of::<PerfEventHeader>() {
            return Err(());
        }

        unsafe {
            core::ptr::copy_nonoverlapping(
                &self.header as *const _ as *const u8,
                buf[offset..].as_mut_ptr(),
                mem::size_of::<PerfEventHeader>(),
            );
        }
        offset += mem::size_of::<PerfEventHeader>();

        // Write sample fields based on sample_type
        // This is a simplified version - real implementation would handle all fields

        if let Some(ip) = self.ip {
            if buf.len() < offset + 8 {
                return Err(());
            }
            buf[offset..offset + 8].copy_from_slice(&ip.to_ne_bytes());
            offset += 8;
        }

        if let (Some(pid), Some(tid)) = (self.pid, self.tid) {
            if buf.len() < offset + 8 {
                return Err(());
            }
            buf[offset..offset + 4].copy_from_slice(&pid.to_ne_bytes());
            buf[offset + 4..offset + 8].copy_from_slice(&tid.to_ne_bytes());
            offset += 8;
        }

        // Update header size
        let total_size = offset as u16;
        buf[6..8].copy_from_slice(&total_size.to_ne_bytes());

        Ok(offset)
    }
}

/// Performance counter read result
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PerfEventCount {
    pub value: u64,
    pub time_enabled: u64,
    pub time_running: u64,
    pub id: u64,
}

impl PerfEventCount {
    pub fn new() -> Self {
        Self {
            value: 0,
            time_enabled: 0,
            time_running: 0,
            id: 0,
        }
    }

    /// Serialize to buffer based on read_format
    pub fn serialize_into(&self, buf: &mut [u8], read_format: u64) -> Result<usize, ()> {
        let mut offset = 0;

        // Always include value
        if buf.len() < 8 {
            return Err(());
        }
        buf[0..8].copy_from_slice(&self.value.to_ne_bytes());
        offset += 8;

        if read_format & super::flags::PerfReadFormat::TOTAL_TIME_ENABLED.bits() != 0 {
            if buf.len() < offset + 8 {
                return Err(());
            }
            buf[offset..offset + 8].copy_from_slice(&self.time_enabled.to_ne_bytes());
            offset += 8;
        }

        if read_format & super::flags::PerfReadFormat::TOTAL_TIME_RUNNING.bits() != 0 {
            if buf.len() < offset + 8 {
                return Err(());
            }
            buf[offset..offset + 8].copy_from_slice(&self.time_running.to_ne_bytes());
            offset += 8;
        }

        if read_format & super::flags::PerfReadFormat::ID.bits() != 0 {
            if buf.len() < offset + 8 {
                return Err(());
            }
            buf[offset..offset + 8].copy_from_slice(&self.id.to_ne_bytes());
            offset += 8;
        }

        Ok(offset)
    }
}
