use crate::task::tid::{Tid, TidHandle, tid_alloc};

extern crate alloc;
use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use mutex::SpinLock;

use core::cell::SyncUnsafeCell;
use core::task::Waker;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    Running,
    Sleeping,
    Waiting,
    Zombie,
}

pub struct Task {
    tid: TidHandle,
    process: Option<Weak<Task>>,
    is_process: bool,

    inner: SpinLock<TaskInner>,
}

pub struct TaskInner {
    state: TaskState,
    parent: Option<Weak<Task>>,
    children: BTreeMap<Tid, Weak<Task>>,
    waker: SyncUnsafeCell<Option<Waker>>,

    exit_code: u32,
}

impl Task {
    pub fn tid(&self) -> Tid {
        self.tid.0
    }

    pub fn process(self: &Arc<Self>) -> Arc<Task> {
        if self.is_process() {
            self.process.as_ref().cloned().unwrap().upgrade().unwrap()
        } else {
            self.clone()
        }
    }

    pub fn is_process(&self) -> bool {
        self.is_process
    }

    pub fn is_in_state(&self, state: TaskState) -> bool {
        self.inner.lock().state == state
    }

    pub fn set_state(&self, state: TaskState) {
        self.inner.lock().state = state;
    }

    pub fn get_state(&self) -> TaskState {
        self.inner.lock().state
    }

    pub fn set_parent(&self, parent: Arc<Task>) {
        self.inner.lock().parent = Some(Arc::downgrade(&parent));
    }

    pub fn add_child(&self, child: Arc<Task>) {
        self.inner
            .lock()
            .children
            .insert(child.tid(), Arc::downgrade(&child));
    }

    pub fn remove_child(&self, child: Arc<Task>) {
        self.inner.lock().children.remove(&child.tid());
    }

    pub fn new() -> Self {
        let inner = TaskInner {
            state: TaskState::Running,
            parent: None,
            children: BTreeMap::new(),
            waker: SyncUnsafeCell::new(None),
            exit_code: 0,
        };
        Task {
            tid: tid_alloc(),
            process: None,
            is_process: false,
            inner: SpinLock::new(inner),
        }
    }

    pub fn set_exit_code(&self, exit_code: u32) {
        self.inner.lock().exit_code = exit_code;
    }

    pub fn set_waker(&self, waker: Waker) {
        unsafe {
            *self.inner.lock().waker.get() = Some(waker);
        }
    }

    pub fn get_waker(&self) -> Option<&Waker> {
        unsafe { self.inner.lock().waker.get().as_ref().unwrap().as_ref() }
    }

    pub fn get_exit_code(&self) -> u32 {
        self.inner.lock().exit_code
    }

    pub fn exit(&self) {
        self.inner.lock().state = TaskState::Zombie;
        self.inner.lock().exit_code = 0;
        todo!()
    }
}
