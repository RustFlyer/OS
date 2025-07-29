use alloc::{format, string::String, vec::Vec};

/// Types of mount events
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MountEventType {
    /// Mount operation
    Mount = 1,
    /// Unmount operation
    Unmount = 2,
    /// Remount operation
    Remount = 3,
    /// Move mount operation
    Move = 4,
    /// Bind mount operation
    Bind = 5,
}

/// Mount tree node information
#[derive(Debug, Clone)]
pub struct MountTreeNode {
    /// Mount ID
    pub mount_id: u64,
    /// Parent mount ID
    pub parent_id: u64,
    /// Major device number
    pub major: u32,
    /// Minor device number
    pub minor: u32,
    /// Root of the mount within the filesystem
    pub root: String,
    /// Mount point relative to the process's root
    pub mount_point: String,
    /// Mount options
    pub mount_options: String,
    /// Optional fields
    pub optional_fields: Vec<String>,
    /// Filesystem type
    pub fs_type: String,
    /// Mount source
    pub mount_source: String,
    /// Super options
    pub super_options: String,
}

impl MountTreeNode {
    pub fn new(
        mount_id: u64,
        parent_id: u64,
        major: u32,
        minor: u32,
        root: String,
        mount_point: String,
        mount_options: String,
        fs_type: String,
        mount_source: String,
        super_options: String,
    ) -> Self {
        Self {
            mount_id,
            parent_id,
            major,
            minor,
            root,
            mount_point,
            mount_options,
            optional_fields: Vec::new(),
            fs_type,
            mount_source,
            super_options,
        }
    }

    /// Check if this is a bind mount
    pub fn is_bind_mount(&self) -> bool {
        self.mount_options.contains("bind")
    }

    /// Check if this mount is shared
    pub fn is_shared(&self) -> bool {
        self.optional_fields
            .iter()
            .any(|field| field.starts_with("shared:"))
    }

    /// Check if this mount is a slave
    pub fn is_slave(&self) -> bool {
        self.optional_fields
            .iter()
            .any(|field| field.starts_with("master:"))
    }

    /// Check if this mount is private
    pub fn is_private(&self) -> bool {
        !self.is_shared() && !self.is_slave()
    }

    /// Get the shared group ID if applicable
    pub fn get_shared_group(&self) -> Option<u32> {
        self.optional_fields
            .iter()
            .find(|field| field.starts_with("shared:"))
            .and_then(|field| field.strip_prefix("shared:"))
            .and_then(|id| id.parse().ok())
    }

    /// Get the master group ID if applicable
    pub fn get_master_group(&self) -> Option<u32> {
        self.optional_fields
            .iter()
            .find(|field| field.starts_with("master:"))
            .and_then(|field| field.strip_prefix("master:"))
            .and_then(|id| id.parse().ok())
    }
}

/// Mount attribute structure for mount_setattr
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MountAttr {
    /// Attribute mask
    pub attr_set: u64,
    /// Attributes to clear
    pub attr_clr: u64,
    /// Mount propagation type
    pub propagation: u64,
    /// User namespace file descriptor for ID mapping
    pub userns_fd: u64,
}

impl MountAttr {
    pub fn new() -> Self {
        Self {
            attr_set: 0,
            attr_clr: 0,
            propagation: 0,
            userns_fd: 0,
        }
    }

    /// Set read-only attribute
    pub fn set_readonly(&mut self, readonly: bool) {
        if readonly {
            self.attr_set |= super::flags::MountAttrFlags::MOUNT_ATTR_RDONLY.bits();
        } else {
            self.attr_clr |= super::flags::MountAttrFlags::MOUNT_ATTR_RDONLY.bits();
        }
    }

    /// Set nosuid attribute
    pub fn set_nosuid(&mut self, nosuid: bool) {
        if nosuid {
            self.attr_set |= super::flags::MountAttrFlags::MOUNT_ATTR_NOSUID.bits();
        } else {
            self.attr_clr |= super::flags::MountAttrFlags::MOUNT_ATTR_NOSUID.bits();
        }
    }

    /// Set nodev attribute
    pub fn set_nodev(&mut self, nodev: bool) {
        if nodev {
            self.attr_set |= super::flags::MountAttrFlags::MOUNT_ATTR_NODEV.bits();
        } else {
            self.attr_clr |= super::flags::MountAttrFlags::MOUNT_ATTR_NODEV.bits();
        }
    }

    /// Set noexec attribute
    pub fn set_noexec(&mut self, noexec: bool) {
        if noexec {
            self.attr_set |= super::flags::MountAttrFlags::MOUNT_ATTR_NOEXEC.bits();
        } else {
            self.attr_clr |= super::flags::MountAttrFlags::MOUNT_ATTR_NOEXEC.bits();
        }
    }
}

/// Detached mount tree representation
#[derive(Debug, Clone)]
pub struct DetachedMount {
    /// Root node of the detached tree
    pub root: MountTreeNode,
    /// Child mounts
    pub children: Vec<DetachedMount>,
    /// Mount attributes
    pub attributes: MountAttr,
    /// Whether this mount is recursive
    pub recursive: bool,
}

impl DetachedMount {
    pub fn new(root: MountTreeNode, recursive: bool) -> Self {
        Self {
            root,
            children: Vec::new(),
            attributes: MountAttr::new(),
            recursive,
        }
    }

    /// Add a child mount
    pub fn add_child(&mut self, child: DetachedMount) {
        self.children.push(child);
    }

    /// Count total number of mounts in this tree
    pub fn count_mounts(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(|child| child.count_mounts())
            .sum::<usize>()
    }

    /// Find a mount by path
    pub fn find_mount(&self, path: &str) -> Option<&DetachedMount> {
        if self.root.mount_point == path {
            return Some(self);
        }

        for child in &self.children {
            if let Some(found) = child.find_mount(path) {
                return Some(found);
            }
        }

        None
    }

    /// Serialize the mount tree to a string representation
    pub fn serialize(&self) -> String {
        let mut result = format!(
            "{} {} {}:{} {} {} {} {} {} {}",
            self.root.mount_id,
            self.root.parent_id,
            self.root.major,
            self.root.minor,
            self.root.root,
            self.root.mount_point,
            self.root.mount_options,
            self.root.fs_type,
            self.root.mount_source,
            self.root.super_options
        );

        for child in &self.children {
            result.push('\n');
            result.push_str(&child.serialize());
        }

        result
    }
}
