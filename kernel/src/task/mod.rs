pub mod futex;
pub mod future;
pub mod kernelproc;
pub mod manager;
pub mod process_manager;
pub mod sig_members;
pub mod signal;
pub mod task;
pub mod taskf;
pub mod threadgroup;
pub mod tid;
pub mod time;
pub mod timeid;

use arch::time::get_time_duration;
use future::spawn_kernel_task;
use net::poll_interfaces;
use osfuture::yield_now;
pub use task::{Task, TaskState};

use osfs::sys_root_dentry;
use timer::{TIMER_MANAGER, sleep_ms};
use vfs::file::File;

pub fn init() {
    let init_proc = {
        let root = sys_root_dentry();
        let dentry = root.lookup("init_proc").unwrap();
        <dyn File>::open(dentry).unwrap()
    };

    Task::spawn_from_elf(init_proc, "init_proc");
    // timer_init();
    net_poll_init();
    // elf_test();
}

/// `timer_init` spawns a global timer update kernel thread.
/// It is spawned to prevent all threads are blocked and then no
/// timer is updated. The kernel thread can update timer all the time
/// and wake up sleeping future if its timer has expired whether
/// all user futures are sleeping or not.
pub fn timer_init() {
    spawn_kernel_task(async {
        let mut ticks: usize = 0;
        loop {
            ticks += 1;
            if ticks % 1000 == 0 {
                let current = get_time_duration();
                TIMER_MANAGER.check(current);
                ticks = 0;
            }
            yield_now().await;
        }
    });
}

pub fn net_poll_init() {
    spawn_kernel_task(async {
        loop {
            sleep_ms(10).await;
            poll_interfaces();
            // log::debug!("net poll again");
        }
    });
}

#[deprecated = "Legacy elf load test."]
pub fn static_elf_test() {
    // let hello_world = get_app_data_by_name("hello_world").unwrap();
    // let time_test = get_app_data_by_name("time_test").unwrap();
    // let add = get_app_data_by_name("add").unwrap();
    // let add1 = get_app_data_by_name("add1").unwrap();
    // let add2 = get_app_data_by_name("add2").unwrap();
    // let file_test = get_app_data_by_name("file_test").unwrap();
    // let elf_data2 = get_app_data_by_name("time_test").unwrap();

    // Task::spawn_from_elf(hello_world, "hello_world");
    // Task::spawn_from_elf(time_test, "time_test");
    // Task::spawn_from_elf(add, "add");
    // Task::spawn_from_elf(add1, "add1");
    // Task::spawn_from_elf(add2, "add2");
    // Task::spawn_from_elf(file_test, "file_test");
    // Task::spawn_from_elf(elf_data2);
}

#[allow(unused)]
pub fn elf_test() {
    let open_file = |path: &str| {
        let root = sys_root_dentry();
        let dentry = root.lookup(path).unwrap();
        <dyn File>::open(dentry)
    };

    let hello_world = open_file("hello_world").unwrap();
    let time_test = open_file("time_test").unwrap();
    let add = open_file("add").unwrap();
    let add1 = open_file("add1").unwrap();
    let add2 = open_file("add2").unwrap();
    let file_test = open_file("file_test").unwrap();

    // Task::spawn_from_elf(hello_world, "hello_world");
    // Task::spawn_from_elf(time_test, "time_test");
    // Task::spawn_from_elf(add, "add");
    // Task::spawn_from_elf(add1, "add1");
    // Task::spawn_from_elf(add2, "add2");
    // Task::spawn_from_elf(file_test, "file_test");
}
