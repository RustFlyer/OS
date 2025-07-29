use alloc::collections::VecDeque;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use core::task::Waker;
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::file::PerfEventFile;
use super::{
    event::{PerfEventAttr, PerfEventCount, PerfSample},
    flags::{PerfEventAttrFlags, PerfType},
};

pub struct PerfEventInode {
    meta: InodeMeta,
    /// Event attributes
    attr: PerfEventAttr,
    /// Current counter value
    count: AtomicU64,
    /// Time when event was enabled
    time_enabled: AtomicU64,
    /// Time when event was actually running
    time_running: AtomicU64,
    /// Event ID
    id: u64,
    /// Whether event is currently enabled
    enabled: AtomicBool,
    /// Process ID being monitored (-1 for current)
    pid: i32,
    /// CPU being monitored (-1 for any)
    cpu: i32,
    /// Group leader file descriptor (-1 if this is leader)
    group_fd: i32,
    /// Sample queue for sampling events
    samples: SpinNoIrqLock<VecDeque<PerfSample>>,
    /// Wakers for blocked readers
    wakers: SpinNoIrqLock<Vec<Waker>>,
    /// Maximum samples in queue
    max_samples: usize,
    /// Sample period counter
    sample_counter: AtomicU64,
    /// Hardware counter allocation
    hw_counter_id: AtomicI32,
    /// Group leader (if this is a group member)
    group_leader: SpinNoIrqLock<Option<Arc<PerfEventFile>>>,
    /// Group members (if this is a group leader)
    group_members: SpinNoIrqLock<Vec<Weak<PerfEventFile>>>,
}

impl PerfEventInode {
    pub fn new(attr: PerfEventAttr, pid: i32, cpu: i32, group_fd: i32, id: u64) -> Arc<Self> {
        let flags = PerfEventAttrFlags::from_bits_truncate(attr.flags);

        Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            enabled: AtomicBool::new(!flags.contains(PerfEventAttrFlags::DISABLED)),
            attr,
            count: AtomicU64::new(0),
            time_enabled: AtomicU64::new(0),
            time_running: AtomicU64::new(0),
            id,
            pid,
            cpu,
            group_fd,
            samples: SpinNoIrqLock::new(VecDeque::new()),
            wakers: SpinNoIrqLock::new(Vec::new()),
            max_samples: 1024, // Reasonable default
            sample_counter: AtomicU64::new(0),
            hw_counter_id: AtomicI32::new(-1),
            group_leader: SpinNoIrqLock::new(None),
            group_members: SpinNoIrqLock::new(Vec::new()),
        })
    }

    /// Enable the event
    pub fn enable(&self) -> SysResult<()> {
        if self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        // In real implementation, this would:
        // 1. Program hardware counters
        // 2. Set up interrupt handlers for sampling
        // 3. Add to scheduler hooks for software events

        self.enabled.store(true, Ordering::Relaxed);
        self.time_enabled
            .store(self.get_current_time(), Ordering::Relaxed);

        log::debug!("[perf_event] enabled event id={}", self.id);
        Ok(())
    }

    /// Disable the event
    pub fn disable(&self) -> SysResult<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.enabled.store(false, Ordering::Relaxed);

        log::debug!("[perf_event] disabled event id={}", self.id);
        Ok(())
    }

    /// Reset the counter
    pub fn reset(&self) -> SysResult<()> {
        self.count.store(0, Ordering::Relaxed);
        self.time_enabled.store(0, Ordering::Relaxed);
        self.time_running.store(0, Ordering::Relaxed);
        self.sample_counter.store(0, Ordering::Relaxed);

        let mut samples = self.samples.lock();
        samples.clear();

        Ok(())
    }

    /// Read current counter value
    pub fn read_count(&self, buf: &mut [u8]) -> SysResult<usize> {
        let count = PerfEventCount {
            value: self.count.load(Ordering::Relaxed),
            time_enabled: self.time_enabled.load(Ordering::Relaxed),
            time_running: self.time_running.load(Ordering::Relaxed),
            id: self.id,
        };

        count
            .serialize_into(buf, self.attr.read_format)
            .map_err(|_| SysError::EINVAL)
    }

    /// Read samples from the ring buffer
    pub fn read_samples(&self, buf: &mut [u8]) -> SysResult<usize> {
        let mut samples = self.samples.lock();
        let mut total_bytes = 0;
        let mut buf_offset = 0;

        while let Some(sample) = samples.front() {
            match sample.serialize_into(&mut buf[buf_offset..]) {
                Ok(bytes) => {
                    samples.pop_front();
                    buf_offset += bytes;
                    total_bytes += bytes;
                }
                Err(_) => {
                    // Not enough space for this sample
                    break;
                }
            }
        }

        if total_bytes == 0 {
            // Check if non-blocking
            let flags = PerfEventAttrFlags::from_bits_truncate(self.attr.flags);
            // In real implementation, would check O_NONBLOCK on file descriptor
            return Err(SysError::EAGAIN);
        }

        Ok(total_bytes)
    }

    /// Increment the counter (called by kernel events)
    pub fn increment(&self, delta: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let old_count = self.count.fetch_add(delta, Ordering::Relaxed);
        let new_count = old_count + delta;

        // Update time_running
        self.time_running
            .store(self.get_current_time(), Ordering::Relaxed);

        // Check if we need to generate a sample
        if let Some(period) = self.attr.get_sample_period() {
            if period > 0 {
                let sample_count = self.sample_counter.fetch_add(delta, Ordering::Relaxed);
                if sample_count + delta >= period {
                    self.sample_counter.store(0, Ordering::Relaxed);
                    self.generate_sample(new_count);
                }
            }
        }

        // Check wakeup events
        if self.attr.wakeup_events_watermark > 0 {
            if new_count % self.attr.wakeup_events_watermark as u64 == 0 {
                self.wake_all_readers();
            }
        }
    }

    /// Generate a performance sample
    fn generate_sample(&self, count_value: u64) {
        let mut sample = PerfSample::new(self.attr.sample_type);

        // Populate sample fields
        if let Some(ref mut ip) = sample.ip {
            *ip = self.get_instruction_pointer();
        }

        if let Some(ref mut pid) = sample.pid {
            *pid = self.get_current_pid();
        }

        if let Some(ref mut tid) = sample.tid {
            *tid = self.get_current_tid();
        }

        if let Some(ref mut time) = sample.time {
            *time = self.get_current_time();
        }

        if let Some(ref mut period) = sample.period {
            *period = count_value;
        }

        // Add to sample queue
        let mut samples = self.samples.lock();
        if samples.len() >= self.max_samples {
            samples.pop_front(); // Drop oldest sample
        }
        samples.push_back(sample);
        drop(samples);

        self.wake_all_readers();
    }

    /// Wake all waiting readers
    fn wake_all_readers(&self) {
        let mut wakers = self.wakers.lock();
        for waker in wakers.drain(..) {
            waker.wake();
        }
    }

    /// Register a waker
    pub fn register_waker(&self, waker: Waker) {
        let mut wakers = self.wakers.lock();
        if self.has_data() {
            waker.wake();
        } else {
            if !wakers.iter().any(|w| w.will_wake(&waker)) {
                wakers.push(waker);
            }
        }
    }

    /// Check if there's data available
    pub fn has_data(&self) -> bool {
        !self.samples.lock().is_empty()
    }

    /// Get current attributes
    pub fn get_pattr(&self) -> &PerfEventAttr {
        &self.attr
    }

    /// Get event type
    pub fn get_event_type(&self) -> PerfType {
        match self.attr.r#type {
            0 => PerfType::Hardware,
            1 => PerfType::Software,
            2 => PerfType::Tracepoint,
            3 => PerfType::HwCache,
            4 => PerfType::Raw,
            5 => PerfType::Breakpoint,
            _ => PerfType::Software, // fallback
        }
    }

    /// Check if event is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Get event ID
    pub fn get_id(&self) -> u64 {
        self.id
    }

    /// Get monitored PID
    pub fn get_pid(&self) -> i32 {
        self.pid
    }

    /// Get monitored CPU
    pub fn get_cpu(&self) -> i32 {
        self.cpu
    }

    // Helper functions for sample generation
    #[allow(static_mut_refs)]
    fn get_current_time(&self) -> u64 {
        // In real implementation, would use high-resolution timer
        // For now, use a simple timestamp
        static mut TIME_COUNTER: AtomicU64 = AtomicU64::new(0);
        unsafe { TIME_COUNTER.fetch_add(1, Ordering::Relaxed) }
    }

    fn get_instruction_pointer(&self) -> u64 {
        // In real implementation, would get actual IP from interrupt context
        0xffffffff81234567 // placeholder
    }

    fn get_current_pid(&self) -> u32 {
        // In real implementation, would get current task PID
        if self.pid == -1 {
            1000 // placeholder for current PID
        } else {
            self.pid as u32
        }
    }

    fn get_current_tid(&self) -> u32 {
        // In real implementation, would get current thread ID
        1000 // placeholder
    }

    /// Handle different event types
    pub fn handle_hardware_event(&self, hw_event: super::flags::PerfHwId) {
        match hw_event {
            super::flags::PerfHwId::CpuCycles => {
                // Increment on CPU cycles
                self.increment(1);
            }
            super::flags::PerfHwId::Instructions => {
                // Increment on instruction retirement
                self.increment(1);
            }
            super::flags::PerfHwId::CacheReferences => {
                // Increment on cache references
                self.increment(1);
            }
            super::flags::PerfHwId::CacheMisses => {
                // Increment on cache misses
                self.increment(1);
            }
            _ => {
                // Handle other hardware events
                self.increment(1);
            }
        }
    }

    pub fn handle_software_event(&self, sw_event: super::flags::PerfSwIds) {
        match sw_event {
            super::flags::PerfSwIds::CpuClock => {
                // Increment based on CPU clock
                self.increment(1);
            }
            super::flags::PerfSwIds::TaskClock => {
                // Increment based on task clock
                self.increment(1);
            }
            super::flags::PerfSwIds::PageFaults => {
                // Increment on page faults
                self.increment(1);
            }
            super::flags::PerfSwIds::ContextSwitches => {
                // Increment on context switches
                self.increment(1);
            }
            _ => {
                // Handle other software events
                self.increment(1);
            }
        }
    }

    /// Setup hardware counter (simplified)
    pub fn setup_hardware_counter(&self) -> SysResult<()> {
        match self.get_event_type() {
            PerfType::Hardware => {
                // In real implementation:
                // 1. Find available PMU counter
                // 2. Program PMU registers
                // 3. Set up interrupt handler
                let counter_id = self.allocate_hw_counter()?;
                self.hw_counter_id.store(counter_id, Ordering::Relaxed);
                log::debug!("[perf_event] allocated HW counter {}", counter_id);
            }
            PerfType::Software => {
                // Software events don't need hardware counters
                log::debug!("[perf_event] setup software event");
            }
            _ => {
                log::debug!("[perf_event] setup other event type");
            }
        }
        Ok(())
    }

    /// Allocate hardware counter
    fn allocate_hw_counter(&self) -> SysResult<i32> {
        // Simplified allocation - in real implementation would manage PMU resources
        static COUNTER_ALLOCATOR: AtomicI32 = AtomicI32::new(0);
        let counter_id = COUNTER_ALLOCATOR.fetch_add(1, Ordering::Relaxed);
        if counter_id >= 8 {
            // Assume 8 hardware counters available
            return Err(SysError::ENOSPC);
        }
        Ok(counter_id)
    }

    /// Release hardware counter
    pub fn release_hardware_counter(&self) {
        let counter_id = self.hw_counter_id.load(Ordering::Relaxed);
        if counter_id >= 0 {
            // In real implementation: free PMU counter and cleanup
            log::debug!("[perf_event] released HW counter {}", counter_id);
            self.hw_counter_id.store(-1, Ordering::Relaxed);
        }
    }

    pub fn set_group_leader(&self, leader: Option<Arc<PerfEventFile>>) -> SysResult<()> {
        *self.group_leader.lock() = leader;
        Ok(())
    }

    pub fn get_group_leader(&self) -> Option<Arc<PerfEventFile>> {
        self.group_leader.lock().clone()
    }

    pub fn add_group_member(&self, member: Weak<PerfEventFile>) -> SysResult<()> {
        self.group_members.lock().push(member);
        Ok(())
    }

    pub fn sync_with_leader(&self, leader: &PerfEventInode) -> SysResult<()> {
        let leader_time = leader.time_enabled.load(Ordering::Relaxed);
        self.time_enabled.store(leader_time, Ordering::Relaxed);
        Ok(())
    }
}

impl Drop for PerfEventInode {
    fn drop(&mut self) {
        self.release_hardware_counter();
    }
}

impl Inode for PerfEventInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: config::inode::InodeMode::REG.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: 0,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }

    fn set_size(&self, _size: usize) -> SysResult<()> {
        Err(SysError::EINVAL)
    }
}
