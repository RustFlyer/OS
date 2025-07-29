use alloc::{string::String, vec::Vec};
use core::mem;

/// Event types for userfaultfd
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UserfaultfdEventType {
    /// Page fault event
    Pagefault = 0x12,
    /// Fork event
    Fork = 0x13,
    /// Remap event  
    Remap = 0x14,
    /// Remove event
    Remove = 0x15,
    /// Unmap event
    Unmap = 0x16,
}

/// Userfaultfd message structure (matches Linux kernel)
#[repr(C)]
#[derive(Clone)]
pub struct UserfaultfdMsg {
    /// Event type
    pub event: u8,
    pub reserved1: u8,
    pub reserved2: u16,
    pub reserved3: u32,
    /// Event-specific data
    pub arg: UserfaultfdMsgArg,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union UserfaultfdMsgArg {
    pub pagefault: UserfaultfdMsgPagefault,
    pub fork: UserfaultfdMsgFork,
    pub remap: UserfaultfdMsgRemap,
    pub remove: UserfaultfdMsgRemove,
    pub reserved: UserfaultfdMsgReserved,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UserfaultfdMsgPagefault {
    /// Fault flags
    pub flags: u64,
    /// Fault address
    pub address: u64,
    /// Union for different fault types
    pub feat: UserfaultfdMsgPagefaultFeat,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union UserfaultfdMsgPagefaultFeat {
    /// Process ID that triggered the fault
    pub ptid: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UserfaultfdMsgFork {
    /// Child userfaultfd
    pub ufd: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UserfaultfdMsgRemap {
    /// Source address
    pub from: u64,
    /// Destination address  
    pub to: u64,
    /// Length
    pub len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UserfaultfdMsgRemove {
    /// Start address
    pub start: u64,
    /// End address
    pub end: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UserfaultfdMsgReserved {
    pub reserved1: u64,
    pub reserved2: u64,
    pub reserved3: u64,
}

impl UserfaultfdMsg {
    pub fn new_pagefault(address: u64, flags: u64, ptid: u32) -> Self {
        Self {
            event: UserfaultfdEventType::Pagefault as u8,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            arg: UserfaultfdMsgArg {
                pagefault: UserfaultfdMsgPagefault {
                    flags,
                    address,
                    feat: UserfaultfdMsgPagefaultFeat { ptid },
                },
            },
        }
    }

    pub fn new_fork(child_fd: u32) -> Self {
        Self {
            event: UserfaultfdEventType::Fork as u8,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            arg: UserfaultfdMsgArg {
                fork: UserfaultfdMsgFork { ufd: child_fd },
            },
        }
    }

    /// Serialize message to buffer
    pub fn serialize_into(&self, buf: &mut [u8]) -> Result<usize, ()> {
        let size = mem::size_of::<UserfaultfdMsg>();
        if buf.len() < size {
            return Err(());
        }

        // Safety: UserfaultfdMsg is repr(C) and we're copying to appropriately sized buffer
        unsafe {
            core::ptr::copy_nonoverlapping(self as *const _ as *const u8, buf.as_mut_ptr(), size);
        }

        Ok(size)
    }

    pub fn serialized_size() -> usize {
        mem::size_of::<UserfaultfdMsg>()
    }
}

/// Registered memory range
#[derive(Debug, Clone)]
pub struct UserfaultfdRange {
    /// Start address
    pub start: u64,
    /// End address
    pub end: u64,
    /// Registration mode
    pub mode: u64,
    /// Associated ioctls
    pub ioctls: u64,
}

impl UserfaultfdRange {
    pub fn new(start: u64, len: u64, mode: u64) -> Self {
        Self {
            start,
            end: start + len,
            mode,
            ioctls: super::flags::UserfaultfdIoctls::all().bits(),
        }
    }

    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }

    pub fn len(&self) -> u64 {
        self.end - self.start
    }
}
