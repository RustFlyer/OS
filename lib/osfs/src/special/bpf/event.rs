use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::{cell::SyncUnsafeCell, mem};
use mutex::{SpinNoIrqLock, new_share_mutex};

/// BPF instruction
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BpfInsn {
    /// Opcode
    pub code: u8,
    /// Destination register (4 bits) and source register (4 bits)
    pub dst_src: u8,
    /// Offset
    pub off: i16,
    /// Immediate value
    pub imm: i32,
}

impl BpfInsn {
    pub fn new(code: u8, dst_reg: u8, src_reg: u8, off: i16, imm: i32) -> Self {
        Self {
            code,
            dst_src: (dst_reg & 0xf) | ((src_reg & 0xf) << 4),
            off,
            imm,
        }
    }

    pub fn dst_reg(&self) -> u8 {
        self.dst_src & 0xf
    }

    pub fn src_reg(&self) -> u8 {
        (self.dst_src >> 4) & 0xf
    }
}

/// BPF program object
#[derive(Debug, Clone)]
pub struct BpfProgram {
    /// Program ID
    pub id: u32,
    /// Program type
    pub prog_type: u32,
    /// Program name
    pub name: String,
    /// Program instructions
    pub insns: Vec<BpfInsn>,
    /// Program license
    pub license: String,
    /// Kernel version
    pub kern_version: u32,
    /// Program flags
    pub prog_flags: u32,
    /// Log buffer
    pub log_buf: Option<Vec<u8>>,
    /// Log level
    pub log_level: u32,
    /// Expected attach type
    pub expected_attach_type: u32,
    /// Attach BTF ID
    pub attach_btf_id: u32,
    /// Attach prog FD
    pub attach_prog_fd: i32,
    /// FD array
    pub fd_array: Option<Vec<i32>>,
    /// Line info
    pub line_info: Option<Vec<u8>>,
    /// Function info
    pub func_info: Option<Vec<u8>>,
}

impl BpfProgram {
    pub fn new(prog_type: u32, name: String, insns: Vec<BpfInsn>, license: String) -> Self {
        static mut PROG_ID_COUNTER: u32 = 1;
        let id = unsafe {
            let id = PROG_ID_COUNTER;
            PROG_ID_COUNTER = PROG_ID_COUNTER.wrapping_add(1);
            id
        };

        Self {
            id,
            prog_type,
            name,
            insns,
            license,
            kern_version: 0,
            prog_flags: 0,
            log_buf: None,
            log_level: 0,
            expected_attach_type: 0,
            attach_btf_id: 0,
            attach_prog_fd: -1,
            fd_array: None,
            line_info: None,
            func_info: None,
        }
    }

    pub fn insn_count(&self) -> u32 {
        self.insns.len() as u32
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.insns.is_empty() {
            return Err("Program cannot be empty");
        }

        if self.insns.len() > 1000000 {
            return Err("Program too large");
        }

        if self.license.is_empty() {
            return Err("License required");
        }

        // Basic instruction validation
        for insn in &self.insns {
            if insn.dst_reg() >= 11 || insn.src_reg() >= 11 {
                return Err("Invalid register number");
            }
        }

        Ok(())
    }
}

/// BPF map object
pub struct BpfMap {
    /// Map ID
    pub id: u32,
    /// Map type
    pub map_type: u32,
    /// Map name
    pub name: String,
    /// Key size
    pub key_size: u32,
    /// Value size
    pub value_size: u32,
    /// Maximum entries
    pub max_entries: u32,
    /// Map flags
    pub map_flags: u32,
    /// Inner map FD
    pub inner_map_fd: i32,
    /// Numa node
    pub numa_node: u32,
    /// BTF key type ID
    pub btf_key_type_id: u32,
    /// BTF value type ID
    pub btf_value_type_id: u32,
    /// BTF FD
    pub btf_fd: i32,
    /// BTF vmlinux value type ID
    pub btf_vmlinux_value_type_id: u32,
    /// Map data storage
    pub data: Box<dyn MapStorage>,
}

unsafe impl Sync for BpfMap {}
unsafe impl Send for BpfMap {}

impl BpfMap {
    pub fn new(
        map_type: u32,
        name: String,
        key_size: u32,
        value_size: u32,
        max_entries: u32,
        map_flags: u32,
    ) -> Self {
        static mut MAP_ID_COUNTER: u32 = 1;
        let id = unsafe {
            let id = MAP_ID_COUNTER;
            MAP_ID_COUNTER = MAP_ID_COUNTER.wrapping_add(1);
            id
        };

        let data: Box<dyn MapStorage> = match map_type {
            1 => Box::new(HashMapStorage::new(max_entries as usize)), // BPF_MAP_TYPE_HASH
            2 => Box::new(ArrayMapStorage::new(
                max_entries as usize,
                value_size as usize,
            )), // BPF_MAP_TYPE_ARRAY
            _ => Box::new(GenericMapStorage::new()),
        };

        Self {
            id,
            map_type,
            name,
            key_size,
            value_size,
            max_entries,
            map_flags,
            inner_map_fd: -1,
            numa_node: 0,
            btf_key_type_id: 0,
            btf_value_type_id: 0,
            btf_fd: -1,
            btf_vmlinux_value_type_id: 0,
            data,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.key_size == 0 || self.key_size > 512 {
            return Err("Invalid key size");
        }

        if self.value_size == 0 || self.value_size > 1048576 {
            return Err("Invalid value size");
        }

        if self.max_entries == 0 {
            return Err("Max entries must be > 0");
        }

        Ok(())
    }
}

/// Map storage trait
pub trait MapStorage {
    fn lookup(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn update(&mut self, key: &[u8], value: &[u8], flags: u64) -> Result<(), &'static str>;
    fn delete(&mut self, key: &[u8]) -> Result<(), &'static str>;
    fn get_next_key(&self, key: Option<&[u8]>) -> Option<Vec<u8>>;
}

/// Hash map storage implementation
pub struct HashMapStorage {
    data: alloc::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    max_entries: usize,
}

impl HashMapStorage {
    pub fn new(max_entries: usize) -> Self {
        Self {
            data: alloc::collections::BTreeMap::new(),
            max_entries,
        }
    }
}

impl MapStorage for HashMapStorage {
    fn lookup(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    fn update(&mut self, key: &[u8], value: &[u8], flags: u64) -> Result<(), &'static str> {
        if self.data.len() >= self.max_entries && !self.data.contains_key(key) {
            return Err("Map full");
        }

        // Check update flags (BPF_NOEXIST = 1, BPF_EXIST = 2)
        match flags & 0x3 {
            1 => {
                // BPF_NOEXIST
                if self.data.contains_key(key) {
                    return Err("Key exists");
                }
            }
            2 => {
                // BPF_EXIST
                if !self.data.contains_key(key) {
                    return Err("Key does not exist");
                }
            }
            _ => {} // BPF_ANY or other
        }

        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), &'static str> {
        if self.data.remove(key).is_some() {
            Ok(())
        } else {
            Err("Key not found")
        }
    }

    fn get_next_key(&self, key: Option<&[u8]>) -> Option<Vec<u8>> {
        match key {
            None => self.data.keys().next().cloned(),
            Some(k) => {
                let mut found = false;
                for map_key in self.data.keys() {
                    if found {
                        return Some(map_key.clone());
                    }
                    if map_key == k {
                        found = true;
                    }
                }
                None
            }
        }
    }
}

/// Array map storage implementation
pub struct ArrayMapStorage {
    data: Vec<Option<Vec<u8>>>,
    value_size: usize,
}

impl ArrayMapStorage {
    pub fn new(max_entries: usize, value_size: usize) -> Self {
        Self {
            data: alloc::vec![None; max_entries],
            value_size,
        }
    }
}

impl MapStorage for ArrayMapStorage {
    fn lookup(&self, key: &[u8]) -> Option<Vec<u8>> {
        if key.len() != 4 {
            return None;
        }
        let index = u32::from_ne_bytes([key[0], key[1], key[2], key[3]]) as usize;
        self.data.get(index).and_then(|v| v.clone())
    }

    fn update(&mut self, key: &[u8], value: &[u8], _flags: u64) -> Result<(), &'static str> {
        if key.len() != 4 {
            return Err("Invalid key size for array");
        }
        if value.len() != self.value_size {
            return Err("Invalid value size");
        }
        let index = u32::from_ne_bytes([key[0], key[1], key[2], key[3]]) as usize;
        if index >= self.data.len() {
            return Err("Index out of bounds");
        }
        self.data[index] = Some(value.to_vec());
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), &'static str> {
        if key.len() != 4 {
            return Err("Invalid key size for array");
        }
        let index = u32::from_ne_bytes([key[0], key[1], key[2], key[3]]) as usize;
        if index >= self.data.len() {
            return Err("Index out of bounds");
        }
        self.data[index] = None;
        Ok(())
    }

    fn get_next_key(&self, key: Option<&[u8]>) -> Option<Vec<u8>> {
        let start_index = match key {
            None => 0,
            Some(k) => {
                if k.len() != 4 {
                    return None;
                }
                let index = u32::from_ne_bytes([k[0], k[1], k[2], k[3]]) as usize;
                index + 1
            }
        };

        for i in start_index..self.data.len() {
            if self.data[i].is_some() {
                return Some((i as u32).to_ne_bytes().to_vec());
            }
        }
        None
    }
}

/// Generic map storage (fallback)
pub struct GenericMapStorage;

impl GenericMapStorage {
    pub fn new() -> Self {
        Self
    }
}

impl MapStorage for GenericMapStorage {
    fn lookup(&self, _key: &[u8]) -> Option<Vec<u8>> {
        None
    }

    fn update(&mut self, _key: &[u8], _value: &[u8], _flags: u64) -> Result<(), &'static str> {
        Err("Unsupported map type")
    }

    fn delete(&mut self, _key: &[u8]) -> Result<(), &'static str> {
        Err("Unsupported map type")
    }

    fn get_next_key(&self, _key: Option<&[u8]>) -> Option<Vec<u8>> {
        None
    }
}
