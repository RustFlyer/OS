use alloc::{string::String, sync::Arc, vec::Vec};
use hashbrown::HashMap;
use mutex::SpinNoIrqLock;
use spin::lazy::Lazy;
use systype::error::{SysError, SyscallResult};

use crate::{
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyType {
    User,
    Ring,
}
impl KeyType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(KeyType::User),
            "keyring" => Some(KeyType::Ring),
            _ => None,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Key {
    pub id: u64,
    pub key_type: KeyType,
    pub description: String,
    pub payload: Vec<u8>,
}
#[derive(Debug, Default)]
pub struct KeyRing {
    keys: HashMap<u64, Key>,
    next_id: u64,
}
impl KeyRing {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            next_id: 1,
        }
    }
    pub fn add_key(
        &mut self,
        key_type: KeyType,
        description: impl Into<String>,
        payload: impl AsRef<[u8]>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let key = Key {
            id,
            key_type,
            description: description.into(),
            payload: payload.as_ref().to_vec(),
        };
        self.keys.insert(id, key);
        id
    }
    pub fn get_key(&self, id: u64) -> Option<&Key> {
        self.keys.get(&id)
    }
}

static KEY_RING: Lazy<Arc<SpinNoIrqLock<KeyRing>>> =
    Lazy::new(|| Arc::new(SpinNoIrqLock::new(KeyRing::new())));

static KEYRING_TABLE: Lazy<SpinNoIrqLock<HashMap<u64, Arc<SpinNoIrqLock<KeyRing>>>>> =
    Lazy::new(|| SpinNoIrqLock::new(HashMap::new()));

fn find_keyring_by_serial(serial: usize) -> Result<Arc<SpinNoIrqLock<KeyRing>>, SysError> {
    let table = KEYRING_TABLE.lock();
    let arc = table.get(&(serial as u64)).ok_or(SysError::EINVAL)?;
    Ok(arc.clone())
}

pub fn sys_add_key(
    type_ptr: usize,
    desc_ptr: usize,
    payload_ptr: usize,
    plen: usize,
    keyring_serial: usize,
) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();

    let key_type = UserReadPtr::<u8>::new(type_ptr, &addr_space).read_c_string(256)?;
    let description = UserReadPtr::<u8>::new(desc_ptr, &addr_space).read_c_string(256)?;

    let key_type = key_type.into_string().map_err(|_| SysError::EINVAL)?;
    let description = description.into_string().map_err(|_| SysError::EINVAL)?;

    let mut payload_ptr = UserReadPtr::<u8>::new(payload_ptr, &addr_space);
    let payload = unsafe { payload_ptr.try_into_slice(plen) }?;

    let ring = find_keyring_by_serial(keyring_serial)?;

    let key_type = KeyType::from_str(&key_type).ok_or(SysError::EINVAL)?;
    let key_id = ring.lock().add_key(key_type, &description, payload);

    Ok(key_id as usize)
}

pub fn sys_keyctl(
    operation: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> SyscallResult {
    pub const KEYCTL_SEARCH: usize = 3;
    pub const KEYCTL_READ: usize = 11;

    match operation {
        KEYCTL_SEARCH => {
            // arg2: keyring_serial
            // arg3: type_ptr (C string)
            // arg4: description_ptr (C string)
            // arg5: dest_keyring_serial (可选，简化可忽略)
            let keyring_serial = arg2;
            let type_ptr = arg3;
            let desc_ptr = arg4;

            let task = current_task();
            let addr_space = task.addr_space();

            let key_type = UserReadPtr::<u8>::new(type_ptr, &addr_space).read_c_string(256)?;
            let description = UserReadPtr::<u8>::new(desc_ptr, &addr_space).read_c_string(256)?;

            let key_type = key_type.into_string().map_err(|_| SysError::EINVAL)?;
            let description = description.into_string().map_err(|_| SysError::EINVAL)?;

            let ring = find_keyring_by_serial(keyring_serial)?;
            let key_type = KeyType::from_str(&key_type).ok_or(SysError::EINVAL)?;
            let ring = ring.lock();
            let key_id = ring
                .keys
                .values()
                .find(|k| k.key_type == key_type && k.description == description)
                .map(|k| k.id)
                .ok_or(SysError::ENOENT)?;
            Ok(key_id as usize)
        }
        KEYCTL_READ => {
            // arg2: key_id
            // arg3: buffer_ptr
            // arg4: buffer_len
            let key_id = arg2 as u64;
            let buffer_ptr = arg3;
            let buffer_len = arg4;

            // 在所有 keyring 里查找 key
            let table = KEYRING_TABLE.lock();
            let key = table
                .values()
                .find_map(|ring| ring.lock().get_key(key_id).cloned())
                .ok_or(SysError::ENOENT)?;

            let to_copy = core::cmp::min(buffer_len, key.payload.len());
            let task = current_task();
            let addr_space = task.addr_space();
            let mut user_buf = UserWritePtr::<u8>::new(buffer_ptr, &addr_space);
            unsafe {
                user_buf.write_array(&key.payload[..to_copy])?;
            }
            Ok(to_copy)
        }
        e => {
            log::error!("[sys_keyctl] not support {}", e);
            Err(SysError::ENOSYS)
        }
    }
}
