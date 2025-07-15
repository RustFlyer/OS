use alloc::{collections::btree_map::BTreeMap, format, string::String, sync::Arc};

use config::{
    inode::{InodeMode, InodeType},
    vfs::OpenFlags,
};
use exe::{dentry::ExeDentry, inode::ExeInode};
use interrupts::{dentry::InterruptsDentry, inode::InterruptsInode};
use maps::{dentry::MapsDentry, inode::MapsInode};
use meminfo::{dentry::MemInfoDentry, inode::MemInfoInode};
use mounts::{dentry::MountsDentry, inode::MountsInode};
use stat::{dentry::StatDentry, inode::StatInode};
use systype::error::SysResult;
use vfs::{dentry::Dentry, inode::Inode, sys_root_dentry};

use crate::{
    proc::status::{dentry::StatusDentry, inode::StatusInode},
    simple::{dentry::SimpleDentry, inode::SimpleInode},
};

pub mod exe;
pub mod interrupts;
pub mod maps;
pub mod meminfo;
pub mod mounts;
pub mod stat;
pub mod status;

pub mod fs;
pub mod superblock;

#[crate_interface::def_interface]
pub trait KernelProcIf {
    fn exe() -> String;
    fn status() -> String;
    fn stat() -> String;
    fn stat_from_tid(tid: usize) -> String;
    fn maps() -> String;
    fn maps_from_tid(tid: usize) -> String;
    fn interrupts() -> BTreeMap<usize, usize>;
}

pub fn init_procfs(root_dentry: Arc<dyn Dentry>) -> SysResult<()> {
    // /proc/meminfo
    let mem_info_inode = MemInfoInode::new(root_dentry.superblock().unwrap());
    let mem_info_dentry = MemInfoDentry::new(
        "meminfo",
        Some(mem_info_inode),
        Some(Arc::downgrade(&root_dentry)),
    );
    root_dentry.add_child(mem_info_dentry);

    // /proc/mounts
    let mounts_inode = MountsInode::new(root_dentry.superblock().unwrap());
    let mounts_dentry = MountsDentry::new(Some(mounts_inode), Some(Arc::downgrade(&root_dentry)));
    root_dentry.add_child(mounts_dentry);

    // /proc/interrupts
    let interrupts_inode = InterruptsInode::new(root_dentry.superblock().unwrap());
    let interrupts_dentry = InterruptsDentry::new(
        "interrupts",
        Some(interrupts_inode),
        Some(Arc::downgrade(&root_dentry)),
    );
    root_dentry.add_child(interrupts_dentry);

    // /proc/sys
    let sys_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    sys_inode.set_inotype(InodeType::Dir);
    let sys_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("sys", Some(sys_inode), Some(Arc::downgrade(&root_dentry)));
    root_dentry.add_child(sys_dentry.clone());

    // /proc/sys/kernel
    let kernel_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    kernel_inode.set_inotype(InodeType::Dir);
    let kernel_dentry = SimpleDentry::new(
        "kernel",
        Some(kernel_inode),
        Some(Arc::downgrade(&sys_dentry)),
    );

    sys_dentry.add_child(kernel_dentry.clone());

    // /proc/sys/kernel/pid_max
    let pid_max_dentry = SimpleDentry::new(
        "pid_max",
        None,
        Some(Arc::downgrade(&kernel_dentry.clone().into_dyn())),
    );
    kernel_dentry.add_child(pid_max_dentry.clone());
    kernel_dentry
        .clone()
        .into_dyn()
        .create(pid_max_dentry.into_dyn_ref(), InodeMode::REG)?;
    let pid_max_file = pid_max_dentry.base_open()?;
    pid_max_file.set_flags(OpenFlags::O_WRONLY);
    osfuture::block_on(async { pid_max_file.write("32768\0".as_bytes()).await })?;

    // /proc/sys/kernel/tainted
    let tainted_dentry = SimpleDentry::new(
        "tainted",
        None,
        Some(Arc::downgrade(&kernel_dentry.clone().into_dyn())),
    );
    kernel_dentry.add_child(tainted_dentry.clone());
    kernel_dentry
        .clone()
        .into_dyn()
        .create(tainted_dentry.into_dyn_ref(), InodeMode::REG)?;
    let tainted_file = tainted_dentry.base_open()?;
    tainted_file.set_flags(OpenFlags::O_WRONLY);
    osfuture::block_on(async { tainted_file.write("0\0".as_bytes()).await })?;

    // /proc/cpuinfo
    let cpuinfo_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    cpuinfo_inode.set_inotype(InodeType::Dir);
    let cpuinfo_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "cpuinfo",
        Some(cpuinfo_inode),
        Some(Arc::downgrade(&root_dentry)),
    );
    root_dentry.add_child(cpuinfo_dentry.clone());

    // /proc/self
    let self_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    self_inode.set_inotype(InodeType::Dir);
    let self_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("self", Some(self_inode), Some(Arc::downgrade(&root_dentry)));
    root_dentry.add_child(self_dentry.clone());

    // /proc/self/exe
    let exe_inode = ExeInode::new(root_dentry.superblock().unwrap());
    let exe_dentry: Arc<dyn Dentry> =
        ExeDentry::new(Some(exe_inode), Some(Arc::downgrade(&self_dentry)));
    self_dentry.add_child(exe_dentry);

    // /proc/self/status
    let status_inode = StatusInode::new(root_dentry.superblock().unwrap());
    let status_dentry: Arc<dyn Dentry> =
        StatusDentry::new(Some(status_inode), Some(Arc::downgrade(&self_dentry)));
    self_dentry.add_child(status_dentry);

    // /proc/self/stat
    let stat_inode = StatInode::new(root_dentry.superblock().unwrap(), 0);
    let stat_dentry: Arc<dyn Dentry> =
        StatDentry::new(Some(stat_inode), Some(Arc::downgrade(&self_dentry)));
    self_dentry.add_child(stat_dentry);

    // /proc/self/mounts
    let mounts_inode = MountsInode::new(root_dentry.superblock().unwrap());
    let mounts_dentry: Arc<dyn Dentry> =
        MountsDentry::new(Some(mounts_inode), Some(Arc::downgrade(&self_dentry)));
    self_dentry.add_child(mounts_dentry);

    // /proc/self/maps
    let maps_inode = MapsInode::new(root_dentry.superblock().unwrap(), 0);
    let maps_dentry: Arc<dyn Dentry> =
        MapsDentry::new(Some(maps_inode), Some(Arc::downgrade(&self_dentry)));
    self_dentry.add_child(maps_dentry);

    Ok(())
}

pub fn create_thread_stat_file(tid: usize) {
    let root = sys_root_dentry();
    let root_dentry = root.lookup("proc").unwrap();

    let num = format!("{}", tid);
    if root_dentry.get_child(&num).is_some() {
        return;
    }

    // /proc/<tid>
    let num_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    num_inode.set_inotype(InodeType::Dir);
    let num_dentry: Arc<dyn Dentry> =
        SimpleDentry::new(&num, Some(num_inode), Some(Arc::downgrade(&root_dentry)));
    root_dentry.add_child(num_dentry.clone());

    // /proc/<tid>/stat
    let stat_inode = StatInode::new(root_dentry.superblock().unwrap(), tid);
    let stat_dentry: Arc<dyn Dentry> =
        StatDentry::new(Some(stat_inode), Some(Arc::downgrade(&num_dentry)));
    num_dentry.add_child(stat_dentry);

    // /proc/<tid>/maps
    let maps_inode = MapsInode::new(root_dentry.superblock().unwrap(), tid);
    let maps_dentry: Arc<dyn Dentry> =
        MapsDentry::new(Some(maps_inode), Some(Arc::downgrade(&num_dentry)));
    num_dentry.add_child(maps_dentry);
}
