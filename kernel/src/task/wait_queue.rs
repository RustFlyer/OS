use alloc::{collections::BTreeMap, sync::{Arc, Weak}, vec::Vec};
use mutex::SpinNoIrqLock;

use super::{Task, tid::{PGid, Pid}};
use crate::syscall::process::WaitFor;

/// 等待项，表示一个等待的任务
#[derive(Clone)]
pub struct WaitEntry {
    /// 等待的任务
    pub task: Arc<Task>,
    /// 等待条件
    pub target: WaitFor,
}

/// 等待队列，管理所有等待某个条件的任务
pub struct WaitQueue {
    /// 等待队列中的所有等待项
    waiters: Vec<WaitEntry>,
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self {
            waiters: Vec::new(),
        }
    }

    /// 添加一个等待项到队列
    pub fn add_waiter(&mut self, entry: WaitEntry) {
        self.waiters.push(entry);
    }

    /// 移除一个等待项
    pub fn remove_waiter(&mut self, task: &Arc<Task>) -> bool {
        if let Some(pos) = self.waiters.iter().position(|entry| Arc::ptr_eq(&entry.task, task)) {
            self.waiters.remove(pos);
            true
        } else {
            false
        }
    }

    /// 唤醒符合条件的等待任务
    pub fn wake_matching(&mut self, exited_child_pid: Pid, exited_child_pgid: PGid, parent_task: &Arc<Task>) -> Vec<Arc<Task>> {
        let mut woken_tasks = Vec::new();
        let mut indices_to_remove = Vec::new();

        for (i, entry) in self.waiters.iter().enumerate() {
            let should_wake = match &entry.target {
                WaitFor::AnyChild => {
                    // 检查exited_child是否是该任务的子进程
                    entry.task.children_mut().lock().contains_key(&exited_child_pid)
                },
                WaitFor::Pid(pid) => {
                    // 检查是否等待的是这个特定子进程
                    *pid == exited_child_pid && entry.task.children_mut().lock().contains_key(&exited_child_pid)
                },
                WaitFor::PGid(pgid) => {
                    // 检查exited_child是否属于指定的进程组，且是等待任务所在进程组的某个进程的子进程
                    is_child_of_process_group(*pgid, exited_child_pid)
                },
                WaitFor::AnyChildInGroup => {
                    // 检查exited_child是否属于同一个进程组，且是该进程组内某个进程的子进程
                    is_child_of_process_group(entry.task.get_pgid(), exited_child_pid)
                },
            };

            if should_wake {
                woken_tasks.push(entry.task.clone());
                indices_to_remove.push(i);
            }
        }

        // 从后往前删除，避免索引错乱
        for &i in indices_to_remove.iter().rev() {
            self.waiters.remove(i);
        }

        woken_tasks
    }

    /// 获取等待队列的长度
    pub fn len(&self) -> usize {
        self.waiters.len()
    }

    /// 检查等待队列是否为空
    pub fn is_empty(&self) -> bool {
        self.waiters.is_empty()
    }
}

fn is_child_of_process_group(pgid: PGid, exited_child_pid: Pid) -> bool {
    use crate::task::process_manager::PROCESS_GROUP_MANAGER;
    
    if let Some(group) = PROCESS_GROUP_MANAGER.get_group(pgid) {
        for process_weak in group {
            if let Some(process) = process_weak.upgrade() {
                if process.is_process() && process.children_mut().lock().contains_key(&exited_child_pid) {
                    log::info!("[WaitQueueManager] exited_child {} is a child of process group {}", exited_child_pid, pgid);
                    return true;
                }
            }
        }
    }
    false
}

/// 全局等待队列管理器
pub struct WaitQueueManager {
    /// 全局等待队列
    global_queue: SpinNoIrqLock<WaitQueue>,
}

impl WaitQueueManager {
    pub const fn new() -> Self {
        Self {
            global_queue: SpinNoIrqLock::new(WaitQueue::new()),
        }
    }

    /// 添加等待任务
    pub fn add_waiter(&self, task: Arc<Task>, target: WaitFor) {
        let entry = WaitEntry { task, target };
        log::debug!("[WaitQueueManager] Added waiter for condition: {:?}", entry.target);
        self.global_queue.lock().add_waiter(entry);
    }

    /// 移除等待任务
    pub fn remove_waiter(&self, task: &Arc<Task>) -> bool {
        let removed = self.global_queue.lock().remove_waiter(task);
        if removed {
            log::debug!("[WaitQueueManager] Removed waiter for task {}", task.tid());
        }
        removed
    }

    /// 当子进程退出时，唤醒所有符合条件的等待任务
    pub fn notify_child_exit(&self, exited_child: &Arc<Task>, parent_task: &Arc<Task>) {
        let exited_child_pid = exited_child.tid();
        let exited_child_pgid = exited_child.get_pgid();
        
        let woken_tasks = self.global_queue.lock().wake_matching(
            exited_child_pid, 
            exited_child_pgid, 
            parent_task
        );

        log::info!("[WaitQueueManager] Child {} exited, waking {} waiting tasks", 
                   exited_child_pid, woken_tasks.len());
        
        // 唤醒所有匹配的等待任务
        for task in woken_tasks {
            task.wake();
            log::debug!("[WaitQueueManager] Woke up task {} waiting for child exit", task.tid());
        }
    }

    /// 获取等待队列的统计信息
    pub fn get_stats(&self) -> usize {
        self.global_queue.lock().len()
    }
}

/// 全局等待队列管理器实例
pub static WAIT_QUEUE_MANAGER: WaitQueueManager = WaitQueueManager::new();
