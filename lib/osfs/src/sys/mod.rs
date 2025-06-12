use alloc::{format, sync::Arc};
use config::inode::InodeType;
use systype::error::SysResult;
use vfs::{dentry::Dentry, inode::Inode};

use crate::{
    simple::{dentry::SimpleDentry, inode::SimpleInode},
    sys::meminfo::{dentry::MemInfoDentry, inode::MemInfoInode},
};

pub mod fs;
pub mod meminfo;
pub mod superblock;

pub fn init_sysfs(root_dentry: Arc<dyn Dentry>) -> SysResult<()> {
    let devices_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    devices_inode.set_inotype(InodeType::Dir);
    let devices_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "devices",
        Some(devices_inode),
        Some(Arc::downgrade(&root_dentry)),
    );
    root_dentry.add_child(devices_dentry.clone());

    let system_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    system_inode.set_inotype(InodeType::Dir);
    let system_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "system",
        Some(system_inode),
        Some(Arc::downgrade(&devices_dentry)),
    );
    log::info!(
        "[init_sysfs] add system_dentry path = {}",
        system_dentry.path()
    );
    devices_dentry.add_child(system_dentry.clone());

    let node_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    node_inode.set_inotype(InodeType::Dir);
    let node_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "node",
        Some(node_inode),
        Some(Arc::downgrade(&system_dentry)),
    );
    log::info!("[init_sysfs] add node_dentry path = {}", node_dentry.path());
    system_dentry.add_child(node_dentry.clone());

    let online_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    online_inode.set_inotype(InodeType::File);
    let online_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "online",
        Some(online_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    log::info!(
        "[init_sysfs] add online_dentry path = {}",
        online_dentry.path()
    );
    node_dentry.add_child(online_dentry.clone());

    let node0_dentry: Arc<dyn Dentry> = create_node(node_dentry.clone(), 0);
    init_node(node0_dentry, 0);

    // let node1_dentry: Arc<dyn Dentry> = create_node(node_dentry.clone(), 1);
    // init_node(node1_dentry, 1);

    Ok(())
}

pub fn create_node(parent_dentry: Arc<dyn Dentry>, nodeid: usize) -> Arc<dyn Dentry> {
    let node0_inode = SimpleInode::new(parent_dentry.superblock().unwrap());
    node0_inode.set_inotype(InodeType::Dir);
    let node0_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        format!("node{}", nodeid).as_str(),
        Some(node0_inode),
        Some(Arc::downgrade(&parent_dentry)),
    );
    log::info!(
        "[init_sysfs] add node0_dentry path = {}",
        node0_dentry.path()
    );
    parent_dentry.add_child(node0_dentry.clone());
    node0_dentry
}

pub fn init_node(node_dentry: Arc<dyn Dentry>, nodeid: usize) {
    // meminfo
    let mem_inode = MemInfoInode::new(node_dentry.superblock().unwrap(), nodeid);
    mem_inode.set_inotype(InodeType::File);
    let mem_dentry: Arc<dyn Dentry> = MemInfoDentry::new(
        "meminfo",
        Some(mem_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    node_dentry.add_child(mem_dentry);

    // cpulist
    let cpulist_inode = SimpleInode::new(node_dentry.superblock().unwrap());
    cpulist_inode.set_inotype(InodeType::File);
    let cpulist_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "cpulist",
        Some(cpulist_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    node_dentry.add_child(cpulist_dentry);

    // cpumap
    let cpumap_inode = SimpleInode::new(node_dentry.superblock().unwrap());
    cpumap_inode.set_inotype(InodeType::File);
    let cpumap_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "cpumap",
        Some(cpumap_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    node_dentry.add_child(cpumap_dentry);

    // numastat
    let numastat_inode = SimpleInode::new(node_dentry.superblock().unwrap());
    numastat_inode.set_inotype(InodeType::File);
    let numastat_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "numastat",
        Some(numastat_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    node_dentry.add_child(numastat_dentry);

    // distance
    let distance_inode = SimpleInode::new(node_dentry.superblock().unwrap());
    distance_inode.set_inotype(InodeType::File);
    let distance_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "distance",
        Some(distance_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    node_dentry.add_child(distance_dentry);

    // hugepages (directory)
    let hugepages_inode = SimpleInode::new(node_dentry.superblock().unwrap());
    hugepages_inode.set_inotype(InodeType::Dir);
    let hugepages_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "hugepages",
        Some(hugepages_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    node_dentry.add_child(hugepages_dentry);

    // vmstat
    let vmstat_inode = SimpleInode::new(node_dentry.superblock().unwrap());
    vmstat_inode.set_inotype(InodeType::File);
    let vmstat_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "vmstat",
        Some(vmstat_inode),
        Some(Arc::downgrade(&node_dentry)),
    );
    node_dentry.add_child(vmstat_dentry);
}
