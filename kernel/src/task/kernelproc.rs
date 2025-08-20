use alloc::{collections::BTreeMap, string::String, sync::Arc, vec::Vec};

use config::vfs::OpenFlags;
use mutex::ShareMutex;
use osfs::{
    dev::loopx::externf::KernelTableIf,
    fd_table::FdTable,
    proc::{
        KernelProcIf,
        fdinfo::info::{ExtraFdInfo, FanotifyFdInfo, FanotifyMarkInfo, ProcFdInfo},
    },
};
use systype::{
    error::{SysError, SysResult},
    kinterface::KernelTaskOperations,
};
use vfs::{
    fanotify::{
        FsObject, FsObjectId, fs::file::FanotifyGroupFile, kinterface::KernelFdTableOperations,
    },
    file::File,
};

use super::{TaskState, manager::TASK_MANAGER};
use crate::{processor::current_task, trap::trap_handler::TRAP_STATS};

struct KernelProcIfImpl;

#[crate_interface::impl_interface]
impl KernelProcIf for KernelProcIfImpl {
    fn exe() -> String {
        unsafe { current_task().elf().dentry().path() }
    }

    fn status() -> String {
        current_task().proc_status_read()
    }

    fn stat() -> String {
        current_task().proc_stat_read()
    }

    fn isdead() -> bool {
        current_task().get_state() == TaskState::Zombie
    }

    fn stat_from_tid(tid: usize) -> String {
        if let Some(task) = TASK_MANAGER.get_task(tid) {
            return task.proc_stat_read();
        }
        log::error!("no task {}", tid);
        return String::new();
    }

    fn maps() -> String {
        current_task().proc_maps_read()
    }

    fn maps_from_tid(tid: usize) -> String {
        if let Some(task) = TASK_MANAGER.get_task(tid) {
            return task.proc_maps_read();
        }
        log::error!("no task {}", tid);
        return String::new();
    }

    fn interrupts() -> BTreeMap<usize, usize> {
        TRAP_STATS.get_all()
    }

    fn fd(fd: usize) -> String {
        current_task().with_mut_fdtable(|table| table.get_file(fd).unwrap().dentry().path())
    }

    fn fdinfo_from_tid_and_fd(tid: usize, fd: usize) -> SysResult<ProcFdInfo> {
        let task = TASK_MANAGER.get_task(tid).ok_or(SysError::EINVAL)?;
        let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;

        let extra_info = if let Ok(group_file) = file.clone().downcast_arc::<FanotifyGroupFile>() {
            let group = group_file.group();
            let flags = group.flags();
            let event_flags = group.event_file_flags();

            let mut marks = Vec::new();
            for (&object_id, entry) in group.entries().lock().iter() {
                let ino = match object_id {
                    FsObjectId::Inode(ino) => ino,
                    // TODO: Are marks on mounts and filesystems not reported?
                    _ => continue,
                };
                let inode = if let FsObject::Inode(inode) = entry.object() {
                    inode.upgrade().unwrap()
                } else {
                    unreachable!()
                };
                let sdev = inode.dev_id_as_u64();
                let mask = entry.mark();
                let ignored_mask = entry.ignore();
                let mflags = entry.flags();

                marks.push(FanotifyMarkInfo {
                    ino,
                    sdev,
                    mask,
                    ignored_mask,
                    mflags,
                });
            }

            ExtraFdInfo::Fanotify(FanotifyFdInfo {
                flags,
                event_flags,
                marks,
            })
        } else {
            ExtraFdInfo::Normal
        };

        Ok(ProcFdInfo {
            flags: file.inode().get_meta().inner.lock().mode,
            pos: file.pos() as u64,
            mnt_id: 0,
            ino: file.inode().ino() as u32,
            extra_info,
        })
    }
}

struct KernelTableIfImpl;

#[crate_interface::impl_interface]
impl KernelTableIf for KernelTableIfImpl {
    fn table() -> ShareMutex<FdTable> {
        let task = current_task();
        task.fdtable_mut()
    }
}

struct KernelFdTableOperationsImpl;

#[crate_interface::impl_interface]
impl KernelFdTableOperations for KernelFdTableOperationsImpl {
    fn add_file(file: Arc<dyn File>, flags: OpenFlags) -> SysResult<i32> {
        let task = current_task();
        task.fdtable_mut()
            .lock()
            .alloc(file, flags)
            .map(|fd| fd as i32)
    }
}

struct KernelTaskOperationsImpl;

#[crate_interface::impl_interface]
impl KernelTaskOperations for KernelTaskOperationsImpl {
    fn current_pid() -> i32 {
        current_task().pid() as i32
    }

    fn current_tid() -> i32 {
        current_task().tid() as i32
    }
}
