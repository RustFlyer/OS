#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct CapUserHeader {
    pub version: u32,
    pub pid: i32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct CapUserData {
    pub effective: u32,
    pub permitted: u32,
    pub inheritable: u32,
}

pub const _LINUX_CAPABILITY_VERSION_1: u32 = 0x19980330;
pub const _LINUX_CAPABILITY_VERSION_2: u32 = 0x20071026;
pub const _LINUX_CAPABILITY_VERSION_3: u32 = 0x20080522;
pub const CAPABILITY_U32S_1: usize = 1;
pub const CAPABILITY_U32S_2: usize = 2;
pub const CAPABILITY_U32S_3: usize = 2; // Linux 3/4/5 use 2

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub effective: [u32; 2],
    pub permitted: [u32; 2],
    pub inheritable: [u32; 2],
}

impl Capabilities {
    pub fn new() -> Self {
        let all_caps = [u32::MAX, u32::MAX];

        Self {
            effective: all_caps,
            permitted: all_caps,
            inheritable: [0, 0],
        }
    }

    pub fn from_flags(flags: CapabilitiesFlags) -> Self {
        let arr = flags.to_u32_array();
        Self {
            effective: arr,
            permitted: arr,
            inheritable: arr,
        }
    }

    pub fn add_effective(&mut self, flags: CapabilitiesFlags) {
        let arr = flags.to_u32_array();
        self.effective[0] |= arr[0];
        self.effective[1] |= arr[1];
    }

    pub fn add_permitted(&mut self, flags: CapabilitiesFlags) {
        let arr = flags.to_u32_array();
        self.permitted[0] |= arr[0];
        self.permitted[1] |= arr[1];
    }

    pub fn add_inheritable(&mut self, flags: CapabilitiesFlags) {
        let arr = flags.to_u32_array();
        self.inheritable[0] |= arr[0];
        self.inheritable[1] |= arr[1];
    }

    pub fn remove_effective(&mut self, flags: CapabilitiesFlags) {
        let arr = flags.to_u32_array();
        self.effective[0] &= !arr[0];
        self.effective[1] &= !arr[1];
    }

    pub fn remove_permitted(&mut self, flags: CapabilitiesFlags) {
        let arr = flags.to_u32_array();
        self.permitted[0] &= !arr[0];
        self.permitted[1] &= !arr[1];
    }

    pub fn remove_inheritable(&mut self, flags: CapabilitiesFlags) {
        let arr = flags.to_u32_array();
        self.inheritable[0] &= !arr[0];
        self.inheritable[1] &= !arr[1];
    }

    pub fn has_effective(&self, flags: CapabilitiesFlags) -> bool {
        let arr = flags.to_u32_array();
        (self.effective[0] & arr[0]) == arr[0] && (self.effective[1] & arr[1]) == arr[1]
    }

    pub fn has_permitted(&self, flags: CapabilitiesFlags) -> bool {
        let arr = flags.to_u32_array();
        (self.permitted[0] & arr[0]) == arr[0] && (self.permitted[1] & arr[1]) == arr[1]
    }

    pub fn has_inheritable(&self, flags: CapabilitiesFlags) -> bool {
        let arr = flags.to_u32_array();
        (self.inheritable[0] & arr[0]) == arr[0] && (self.inheritable[1] & arr[1]) == arr[1]
    }

    pub fn get_effective_flags(&self) -> CapabilitiesFlags {
        CapabilitiesFlags::from_u32_array(self.effective)
    }

    pub fn get_permitted_flags(&self) -> CapabilitiesFlags {
        CapabilitiesFlags::from_u32_array(self.permitted)
    }

    pub fn get_inheritable_flags(&self) -> CapabilitiesFlags {
        CapabilitiesFlags::from_u32_array(self.inheritable)
    }

    pub fn set_all(&mut self, flags: CapabilitiesFlags) {
        let arr = flags.to_u32_array();
        self.effective = arr;
        self.permitted = arr;
        self.inheritable = arr;
    }

    pub fn set_effective(&mut self, flags: CapabilitiesFlags) {
        self.effective = flags.to_u32_array();
    }

    pub fn set_permitted(&mut self, flags: CapabilitiesFlags) {
        self.permitted = flags.to_u32_array();
    }

    pub fn set_inheritable(&mut self, flags: CapabilitiesFlags) {
        self.inheritable = flags.to_u32_array();
    }

    pub fn clear_all(&mut self) {
        self.effective = [0, 0];
        self.permitted = [0, 0];
        self.inheritable = [0, 0];
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CapabilitiesFlags: u64 {
        /// Change file ownership
        const CAP_CHOWN                 = 1 << 0;
        /// Bypass file read, write, and execute permission checks
        const CAP_DAC_OVERRIDE          = 1 << 1;
        /// Bypass file read permission checks and directory read and execute permission checks
        const CAP_DAC_READ_SEARCH       = 1 << 2;
        /// Bypass file mode restrictions that inhibit changing file ownership
        const CAP_FOWNER                = 1 << 3;
        /// Bypass permission checks for operations on files
        const CAP_FSETID                = 1 << 4;
        /// Bypass permission checks for sending signals
        const CAP_KILL                  = 1 << 5;
        /// Make arbitrary changes to file GIDs and supplementary GID list
        const CAP_SETGID                = 1 << 6;
        /// Make arbitrary changes to process UIDs
        const CAP_SETUID                = 1 << 7;
        /// Transfer any capability in your permitted set to any pid
        const CAP_SETPCAP               = 1 << 8;
        /// Allow use of FIFO and round-robin scheduling
        const CAP_LINUX_IMMUTABLE       = 1 << 9;
        /// Bind a socket to internet domain privileged ports (<1024)
        const CAP_NET_BIND_SERVICE      = 1 << 10;
        /// Allow broadcasting, listen to multicast
        const CAP_NET_BROADCAST         = 1 << 11;
        /// Allow interface configuration, administration of IP firewall, masquerading and accounting
        const CAP_NET_ADMIN             = 1 << 12;
        /// Allow use of RAW sockets and PACKET sockets
        const CAP_NET_RAW               = 1 << 13;
        /// Allow locking of shared memory segments
        const CAP_IPC_LOCK              = 1 << 14;
        /// Override IPC ownership checks
        const CAP_IPC_OWNER             = 1 << 15;
        /// Insert and remove kernel modules
        const CAP_SYS_MODULE            = 1 << 16;
        /// Allow ioperm/iopl access
        const CAP_SYS_RAWIO             = 1 << 17;
        /// Allow use of chroot()
        const CAP_SYS_CHROOT            = 1 << 18;
        /// Allow ptrace() of any process
        const CAP_SYS_PTRACE            = 1 << 19;
        /// Allow configuration of process accounting
        const CAP_SYS_PACCT             = 1 << 20;
        /// Allow configuration of the secure attention key, administration of the random device
        const CAP_SYS_ADMIN             = 1 << 21;
        /// Allow reboot() and kexec_load()
        const CAP_SYS_BOOT              = 1 << 22;
        /// Allow raising priority and setting priority on other processes
        const CAP_SYS_NICE              = 1 << 23;
        /// Override resource limits
        const CAP_SYS_RESOURCE          = 1 << 24;
        /// Allow manipulation of system clock
        const CAP_SYS_TIME              = 1 << 25;
        /// Allow configuration of tty devices
        const CAP_SYS_TTY_CONFIG        = 1 << 26;
        /// Allow the privileged aspects of mknod()
        const CAP_MKNOD                 = 1 << 27;
        /// Allow taking of leases on files
        const CAP_LEASE                 = 1 << 28;
        /// Allow writing the audit log via unicast netlink socket
        const CAP_AUDIT_WRITE           = 1 << 29;
        /// Allow configuration of audit via unicast netlink socket
        const CAP_AUDIT_CONTROL         = 1 << 30;
        /// Allow use of setfcap and removal of any capability from any process
        const CAP_SETFCAP               = 1 << 31;
        /// Override MAC access
        const CAP_MAC_OVERRIDE          = 1 << 32;
        /// Allow MAC configuration or state changes
        const CAP_MAC_ADMIN             = 1 << 33;
        /// Allow use of the syslog() system call
        const CAP_SYSLOG                = 1 << 34;
        /// Allow triggering something that will wake the system
        const CAP_WAKE_ALARM            = 1 << 35;
        /// Allow preventing system suspends
        const CAP_BLOCK_SUSPEND         = 1 << 36;
        /// Allow reading the audit log via multicast netlink socket
        const CAP_AUDIT_READ            = 1 << 37;
        /// Allow system performance and observability privileged operations
        const CAP_PERFMON               = 1 << 38;
        /// Allow BPF operations
        const CAP_BPF                   = 1 << 39;
        /// Allow checkpoint/restore related operations
        const CAP_CHECKPOINT_RESTORE    = 1 << 40;
    }
}

impl CapabilitiesFlags {
    /// Check if a specific capability is set
    pub fn has(&self, cap: CapabilitiesFlags) -> bool {
        self.contains(cap)
    }

    /// Add a capability
    pub fn add(&mut self, cap: CapabilitiesFlags) {
        self.insert(cap);
    }

    /// Convert to the two u32 array format used by the kernel
    pub fn to_u32_array(&self) -> [u32; 2] {
        let bits = self.bits();
        [bits as u32, (bits >> 32) as u32]
    }

    /// Create from two u32 values
    pub fn from_u32_array(arr: [u32; 2]) -> Self {
        let bits = (arr[0] as u64) | ((arr[1] as u64) << 32);
        Self::from_bits_truncate(bits)
    }
}
