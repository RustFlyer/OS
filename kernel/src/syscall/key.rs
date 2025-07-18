use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use hashbrown::HashMap;
use mutex::{SpinNoIrqLock, new_share_mutex};
use spin::{lazy::Lazy, once::Once};
use systype::error::{SysError, SyscallResult};

use crate::{
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyType {
    User,
    Ring,
    Logon,
    BigKey,
    Asymmetric,
    CifsIdmap,
    CifsSpnego,
    Pkcs7Test,
    Rxrpc,
    RxrpcS,
}
impl KeyType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(KeyType::User),
            "keyring" => Some(KeyType::Ring),
            "logon" => Some(KeyType::Logon),
            "big_key" => Some(KeyType::BigKey),
            "asymmetric" => Some(KeyType::Asymmetric),
            "cifs.idmap" => Some(KeyType::CifsIdmap),
            "cifs.spnego" => Some(KeyType::CifsSpnego),
            "pkcs7_test" => Some(KeyType::Pkcs7Test),
            "rxrpc" => Some(KeyType::Rxrpc),
            "rxrpc_s" => Some(KeyType::RxrpcS),
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

static _KEY_RING: Once<Arc<SpinNoIrqLock<KeyRing>>> = Once::new();

static KEYRING_TABLE: Once<SpinNoIrqLock<HashMap<u64, Arc<SpinNoIrqLock<KeyRing>>>>> = Once::new();

pub fn init_key() {
    _KEY_RING.call_once(|| Arc::new(SpinNoIrqLock::new(KeyRing::new())));
    KEYRING_TABLE.call_once(|| SpinNoIrqLock::new(HashMap::new()));
}

fn find_keyring_by_serial(serial: usize) -> Result<Arc<SpinNoIrqLock<KeyRing>>, SysError> {
    let mut table = KEYRING_TABLE.get().unwrap().lock();
    let ret = table.get(&(serial as u64));
    let arc = if ret.is_some() {
        ret.unwrap()
    } else {
        table.insert(serial as u64, new_share_mutex(KeyRing::new()));
        table.get(&(serial as u64)).unwrap()
    };
    Ok(arc.clone())
}

fn parse_uid_keyring(desc: &str) -> Option<(bool, u32)> {
    if let Some(rest) = desc.strip_prefix("_uid.") {
        rest.parse::<u32>().ok().map(|uid| (false, uid))
    } else if let Some(rest) = desc.strip_prefix("_uid_ses.") {
        rest.parse::<u32>().ok().map(|uid| (true, uid))
    } else {
        None
    }
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

    let key_type = KeyType::from_str(&key_type).ok_or(SysError::EINVAL)?;

    if key_type == KeyType::Ring {
        if let Some((_is_ses, uid)) = parse_uid_keyring(&description) {
            let current_uid = task.uid();
            let is_root = current_uid == 0;
            if !is_root && uid != current_uid as u32 {
                return Err(SysError::EPERM);
            }
        }
    }

    let max_len = match key_type {
        KeyType::Ring => 0,
        KeyType::User | KeyType::Logon => 32767,
        KeyType::BigKey => 1048575,
        _ => usize::MAX,
    };

    if plen > max_len {
        return Err(SysError::EINVAL);
    }

    log::debug!(
        "[sys_add_key] read payload {:#x} plen: {}",
        payload_ptr,
        plen
    );

    let mut payload_ptr_ptr = UserReadPtr::<u8>::new(payload_ptr, &addr_space);
    let payload: &[u8] = if plen == 0 {
        &[]
    } else {
        if payload_ptr_ptr.is_null() {
            // log::error!("[sys_add_key] plen = {plen} when payload_ptr is null");
            return Err(SysError::EFAULT);
        }
        unsafe { payload_ptr_ptr.try_into_slice(plen) }?
    };

    let ring = find_keyring_by_serial(keyring_serial)?;
    log::debug!("[sys_add_key] add in");

    let key_id = ring.lock().add_key(key_type, &description, payload);

    log::debug!("[sys_add_key] success");
    Ok(key_id as usize)
}

pub fn sys_keyctl(
    operation: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> SyscallResult {
    pub const KEYCTL_GET_KEYRING_ID: usize = 0;

    pub const KEY_SPEC_THREAD_KEYRING: isize = -1;
    pub const KEY_SPEC_PROCESS_KEYRING: isize = -2;
    pub const KEY_SPEC_SESSION_KEYRING: isize = -3;
    pub const KEY_SPEC_USER_KEYRING: isize = -4;
    pub const KEY_SPEC_USER_SESSION_KEYRING: isize = -5;

    pub const KEYCTL_JOIN_SESSION_KEYRING: usize = 1;
    pub const KEYCTL_SEARCH: usize = 3;
    pub const KEYCTL_READ: usize = 11;

    log::debug!("[sys_keyctl] operation: {}", operation);
    match operation {
        KEYCTL_GET_KEYRING_ID => {
            let idtype = arg2 as isize;
            let create = arg3 != 0;
            let task = current_task();
            let uid = task.uid();
            log::debug!("[sys_keyctl] idtype: {}, create: {}", idtype, create);

            let serial = match idtype {
                KEY_SPEC_PROCESS_KEYRING => task.pid() as u64,
                KEY_SPEC_USER_KEYRING => 0x10000000 + (uid as u64),
                KEY_SPEC_USER_SESSION_KEYRING => 0x20000000 + (uid as u64),
                _ => return Err(SysError::EINVAL),
            };

            // if none and create == 1, then create
            let mut table = KEYRING_TABLE.get().unwrap().lock();
            if !table.contains_key(&serial) {
                if create {
                    table.insert(serial, Arc::new(SpinNoIrqLock::new(KeyRing::new())));
                } else {
                    return Ok(0);
                }
            }
            Ok(serial as usize)
        }
        KEYCTL_JOIN_SESSION_KEYRING => {
            let task = current_task();
            let addr_space = task.addr_space();
            let name = if arg2 == 0 {
                "_ses".to_string()
            } else {
                let mut name_ptr = UserReadPtr::<u8>::new(arg2, &addr_space);
                name_ptr
                    .read_c_string(256)?
                    .into_string()
                    .map_err(|_| SysError::EINVAL)?
            };
            let serial = 0x20000000 + (task.uid() as u64);
            let mut table = KEYRING_TABLE.get().unwrap().lock();
            if !table.contains_key(&serial) {
                table.insert(serial, Arc::new(SpinNoIrqLock::new(KeyRing::new())));
            }
            Ok(serial as usize)
        }
        KEYCTL_SEARCH => {
            // arg2: keyring_serial
            // arg3: type_ptr (C string)
            // arg4: description_ptr (C string)
            // arg5: dest_keyring_serial (selected)
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

            let table = KEYRING_TABLE.get().unwrap().lock();
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
