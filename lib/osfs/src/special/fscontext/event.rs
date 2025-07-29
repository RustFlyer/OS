use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};

/// Filesystem parameter types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsParameterType {
    /// Boolean flag
    Flag = 0,
    /// String value
    String = 1,
    /// Binary blob
    Blob = 2,
    /// Block device
    BlockDev = 3,
    /// File descriptor
    Fd = 4,
    /// Path
    Path = 5,
}

/// A filesystem parameter
#[derive(Debug, Clone)]
pub struct FsParameter {
    /// Parameter key
    pub key: String,
    /// Parameter type
    pub param_type: FsParameterType,
    /// Parameter value
    pub value: FsParameterValue,
}

#[derive(Debug, Clone)]
pub enum FsParameterValue {
    /// No value (for flags)
    None,
    /// String value
    String(String),
    /// Binary data
    Blob(Vec<u8>),
    /// File descriptor
    Fd(i32),
    /// Path name
    Path(String),
}

impl FsParameter {
    pub fn new_flag(key: String) -> Self {
        Self {
            key,
            param_type: FsParameterType::Flag,
            value: FsParameterValue::None,
        }
    }

    pub fn new_string(key: String, value: String) -> Self {
        Self {
            key,
            param_type: FsParameterType::String,
            value: FsParameterValue::String(value),
        }
    }

    pub fn new_blob(key: String, data: Vec<u8>) -> Self {
        Self {
            key,
            param_type: FsParameterType::Blob,
            value: FsParameterValue::Blob(data),
        }
    }

    pub fn new_fd(key: String, fd: i32) -> Self {
        Self {
            key,
            param_type: FsParameterType::Fd,
            value: FsParameterValue::Fd(fd),
        }
    }

    pub fn new_path(key: String, path: String) -> Self {
        Self {
            key,
            param_type: FsParameterType::Path,
            value: FsParameterValue::Path(path),
        }
    }
}

/// Filesystem context state
#[derive(Debug, Clone)]
pub struct FsContext {
    /// Filesystem type name
    pub fs_name: String,
    /// Current phase
    pub phase: u8,
    /// Purpose of this context
    pub purpose: u8,
    /// Superblock flags
    pub sb_flags: u32,
    /// Parameters collected so far
    pub parameters: BTreeMap<String, FsParameter>,
    /// Source specification (device/URL etc)
    pub source: Option<String>,
    /// Error messages
    pub error_log: Vec<String>,
    /// Whether creation succeeded
    pub created: bool,
    /// Associated superblock ID (if created)
    pub superblock_id: Option<u64>,
}

impl FsContext {
    pub fn new(fs_name: String, purpose: u8) -> Self {
        Self {
            fs_name,
            phase: super::flags::FsContextPhase::FS_CONTEXT_CREATE_PARAMS.bits(),
            purpose,
            sb_flags: 0,
            parameters: BTreeMap::new(),
            source: None,
            error_log: Vec::new(),
            created: false,
            superblock_id: None,
        }
    }

    /// Add a parameter to the context
    pub fn add_parameter(&mut self, param: FsParameter) -> Result<(), alloc::string::String> {
        if self.phase != super::flags::FsContextPhase::FS_CONTEXT_CREATE_PARAMS.bits()
            && self.phase != super::flags::FsContextPhase::FS_CONTEXT_RECONF_PARAMS.bits()
        {
            return Err("Context not accepting parameters".to_string());
        }

        // Special handling for source parameter
        if param.key == "source" {
            if let FsParameterValue::String(ref source) = param.value {
                self.source = Some(source.clone());
            }
        }

        self.parameters.insert(param.key.clone(), param);
        Ok(())
    }

    /// Create the filesystem (equivalent to FSCONFIG_CMD_CREATE)
    pub fn create_filesystem(&mut self) -> Result<u64, alloc::string::String> {
        if self.phase != super::flags::FsContextPhase::FS_CONTEXT_CREATE_PARAMS.bits() {
            return Err("Context not ready for creation".to_string());
        }

        self.phase = super::flags::FsContextPhase::FS_CONTEXT_CREATING.bits();

        // In a real implementation, this would:
        // 1. Validate all parameters
        // 2. Load the filesystem module if needed
        // 3. Create the superblock
        // 4. Initialize the filesystem

        // For now, simulate successful creation
        let sb_id = alloc_superblock_id();
        self.superblock_id = Some(sb_id);
        self.created = true;
        self.phase = super::flags::FsContextPhase::FS_CONTEXT_AWAITING_MOUNT.bits();

        Ok(sb_id)
    }

    /// Reconfigure the filesystem (equivalent to FSCONFIG_CMD_RECONFIGURE)
    pub fn reconfigure_filesystem(&mut self) -> Result<(), alloc::string::String> {
        if self.phase != super::flags::FsContextPhase::FS_CONTEXT_RECONF_PARAMS.bits() {
            return Err("Context not ready for reconfiguration".to_string());
        }

        self.phase = super::flags::FsContextPhase::FS_CONTEXT_RECONFIGURING.bits();

        // In a real implementation, this would apply the new parameters
        // to the existing superblock

        self.phase = super::flags::FsContextPhase::FS_CONTEXT_AWAITING_MOUNT.bits();
        Ok(())
    }

    /// Add an error message to the log
    pub fn log_error(&mut self, message: String) {
        self.error_log.push(message);
    }

    /// Get error messages as a single string
    pub fn get_error_log(&self) -> String {
        self.error_log.join("\n")
    }

    /// Check if the context has any errors
    pub fn has_errors(&self) -> bool {
        !self.error_log.is_empty()
    }

    /// Get the filesystem type name
    pub fn filesystem_type(&self) -> &str {
        &self.fs_name
    }

    /// Check if creation is complete
    pub fn is_created(&self) -> bool {
        self.created
    }
}

/// Allocate a unique superblock ID
fn alloc_superblock_id() -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};
    static NEXT_SB_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_SB_ID.fetch_add(1, Ordering::SeqCst)
}

/// Configuration command for fsconfig
#[derive(Debug, Clone)]
pub struct FsConfigCommand {
    pub cmd: u32,
    pub key: Option<String>,
    pub value: Option<FsParameterValue>,
    pub aux: i32,
}

impl FsConfigCommand {
    pub fn set_string(key: String, value: String) -> Self {
        Self {
            cmd: super::flags::FsConfigCmd::FSCONFIG_SET_STRING.bits(),
            key: Some(key),
            value: Some(FsParameterValue::String(value)),
            aux: 0,
        }
    }

    pub fn set_flag(key: String) -> Self {
        Self {
            cmd: super::flags::FsConfigCmd::FSCONFIG_SET_FLAG.bits(),
            key: Some(key),
            value: Some(FsParameterValue::None),
            aux: 0,
        }
    }

    pub fn create() -> Self {
        Self {
            cmd: super::flags::FsConfigCmd::FSCONFIG_CMD_CREATE.bits(),
            key: None,
            value: None,
            aux: 0,
        }
    }

    pub fn reconfigure() -> Self {
        Self {
            cmd: super::flags::FsConfigCmd::FSCONFIG_CMD_RECONFIGURE.bits(),
            key: None,
            value: None,
            aux: 0,
        }
    }
}
