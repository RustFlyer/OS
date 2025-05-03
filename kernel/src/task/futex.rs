use alloc::{sync::Arc, vec::Vec};
use bitflags::bitflags;
use core::{
    cmp::min,
    ops::DerefMut,
    sync::atomic::{AtomicU32, Ordering},
    task::Waker,
};
use hashbrown::HashMap;
use mutex::SpinNoIrqLock;
use spin::Lazy;

use mm::address::{PhysAddr, VirtAddr};
use systype::{SysError, SysResult, SyscallResult};

use crate::{
    task::{Task, tid::Tid},
    vm::{
        addr_space::AddrSpace,
        user_ptr::{SumGuard, UserReadPtr},
    },
};

pub struct FutexAddr {
    pub addr: VirtAddr,
    _guard: SumGuard,
}

impl FutexAddr {
    pub fn new_with_check(addr: usize, addrspace: &AddrSpace) -> SysResult<Self> {
        let futexaddr = Self {
            addr: VirtAddr::new(addr),
            _guard: SumGuard::new(),
        };
        futexaddr.check(addrspace)?;
        Ok(futexaddr)
    }

    pub fn addr(&self) -> usize {
        self.addr.to_usize()
    }

    pub fn check(&self, addrspace: &AddrSpace) -> SysResult<()> {
        unsafe {
            UserReadPtr::<VirtAddr>::new(self.addr.to_usize(), &addrspace).read()?;
        }
        Ok(())
    }

    pub fn read(&self) -> u32 {
        unsafe {
            let ptr = self.addr.to_usize() as *const AtomicU32;
            (*ptr).load(Ordering::Acquire)
        }
    }
}

pub static FUTEX_MANAGER: Lazy<SpinNoIrqLock<FutexManager>> =
    Lazy::new(|| SpinNoIrqLock::new(FutexManager::new()));

pub fn futex_manager() -> impl DerefMut<Target = FutexManager> {
    FUTEX_MANAGER.lock()
}

pub struct FutexManager(HashMap<FutexHashKey, Vec<FutexWaiter>>);

impl FutexManager {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add_waiter(&mut self, key: &FutexHashKey, waiter: FutexWaiter) -> SysResult<()> {
        log::info!("[futex::add_waiter] {:?} in {:?} ", waiter, key);
        if let Some(waiters) = self.0.get_mut(key) {
            waiters.push(waiter);
        } else {
            let mut waiters = Vec::new();
            waiters.push(waiter);
            self.0.insert(*key, waiters);
        }
        Ok(())
    }

    pub fn rm_waiter(&mut self, key: &FutexHashKey, tid: Tid) -> SysResult<()> {
        if let Some(waiters) = self.0.get_mut(key) {
            let index = waiters
                .iter()
                .enumerate()
                .find(|(_, waiter)| waiter.tid == tid)
                .map(|(i, _)| i)
                .ok_or(SysError::EINVAL)?;
            waiters.swap_remove(index);
        }
        Ok(())
    }

    pub fn wake(&mut self, key: &FutexHashKey, n: u32) -> SyscallResult {
        if let Some(waiters) = self.0.get_mut(key) {
            let n = min(n as usize, waiters.len());
            for _ in 0..n {
                let waiter = waiters.pop().unwrap();
                log::info!("[futex_wake] {:?} has been woken", waiter);
                waiter.wake();
            }
            Ok(n)
        } else {
            log::error!("can not find key {key:?}");
            Err(SysError::EINVAL)
        }
    }

    pub fn requeue_waiters(
        &mut self,
        old: FutexHashKey,
        new: FutexHashKey,
        n_req: usize,
    ) -> SyscallResult {
        let mut old_waiters = self.0.remove(&old).ok_or_else(|| {
            log::info!("[futex] no waiters in key {:?}", old);
            SysError::EINVAL
        })?;

        let n = min(n_req as usize, old_waiters.len());

        let iter = 0..n;
        if let Some(new_waiters) = self.0.get_mut(&new) {
            iter.for_each(|_| new_waiters.push(old_waiters.pop().unwrap()));
        } else {
            let mut new_waiters = Vec::with_capacity(n);
            iter.for_each(|_| new_waiters.push(old_waiters.pop().unwrap()));
            self.0.insert(new, new_waiters);
        }

        if !old_waiters.is_empty() {
            self.0.insert(old, old_waiters);
        }

        Ok(n)
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Copy, Clone)]
pub enum FutexHashKey {
    Shared { paddr: PhysAddr },
    Private { mm: VirtAddr, vaddr: VirtAddr },
}

impl FutexHashKey {
    pub fn new_share_key(vaddr: usize, addrspace: &AddrSpace) -> SysResult<FutexHashKey> {
        let vaddr = VirtAddr::new(vaddr);
        let ppn = addrspace
            .page_table
            .find_entry(vaddr.page_number())
            .ok_or(SysError::EFAULT)?
            .ppn();
        let paddr = PhysAddr::new(ppn.address().to_usize() + vaddr.page_offset());
        Ok(FutexHashKey::Shared { paddr })
    }

    pub fn new_private_key(vaddr: usize, addrspace: Arc<AddrSpace>) -> SysResult<FutexHashKey> {
        Ok(FutexHashKey::Private {
            mm: VirtAddr::new(Arc::as_ptr(&addrspace) as usize),
            vaddr: VirtAddr::new(vaddr),
        })
    }

    pub fn new_key(
        vaddr: usize,
        addrspace: Arc<AddrSpace>,
        is_private: bool,
    ) -> SysResult<FutexHashKey> {
        match is_private {
            true => FutexHashKey::new_private_key(vaddr, addrspace),
            false => FutexHashKey::new_share_key(vaddr, &addrspace),
        }
    }
}

#[derive(Debug)]
pub struct FutexWaiter {
    pub tid: Tid,
    pub waker: Waker,
}

impl FutexWaiter {
    pub fn new(task: &Task) -> Self {
        Self {
            tid: task.tid(),
            waker: task.get_waker(),
        }
    }

    pub fn wake(self) {
        self.waker.wake();
    }
}

bitflags! {
    #[repr(C)]
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
    pub struct FutexOp: i32 {
        /// Waits if the futex word at `uaddr` has the expected value.
        ///
        /// The calling thread will block until woken by a `FUTEX_WAKE`.
        const Wait = 0;
        /// Wakes up at most `val` threads blocked on `uaddr`.
        ///
        /// The set of woken waiters is not ordered or deterministic.
        const Wake = 1;
        const Fd = 2;
        // const FUTEX_FD: i32 = 2;
        /// Performs the same task as FUTEX_CMP_REQUEUE (see
        /// below), except that no check is made using the value in val3. (The
        /// argument val3 is ignored.)
        const Requeue = 3;
        /// First checks whether the location uaddr still contains the value
        /// `val3`. If not, the operation fails with the error EAGAIN.
        /// Otherwise, the operation wakes up a maximum of `val` waiters
        /// that are waiting on the futex at `uaddr`. If there are more
        /// than `val` waiters, then the remaining waiters are removed
        /// from the wait queue of the source futex at `uaddr` and added
        /// to the wait queue  of  the  target futex at `uaddr2`.  The
        /// `val2` argument specifies an upper limit on the
        /// number of waiters that are requeued to the futex at `uaddr2`.
        const CmpRequeue = 4;
        const WakeOp = 5;
        /// Priority-inheritance futex lock (PI mutexes).
        const LockPi = 6;
        const UnlockPi = 7;
        const TrylockPi = 8;
        /// Waits using a bit mask to match waiters.
        const WaitBitset = 9;
        const WakeBitset = 10;
        /// Requeues PI waiters.
        const WaitRequeuePi = 11;
        const MAINOPMASK = 15;
        /// Tells the kernel that the futex is process-private and not shared
        /// with another process.
        const Private = 128;
        const ClockRealtime = 256;
    }
}
