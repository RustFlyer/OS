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
use systype::error::{SysError, SysResult, SyscallResult};

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
            UserReadPtr::<VirtAddr>::new(self.addr.to_usize(), addrspace).read()?;
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

pub static FUTEX_MANAGER: Lazy<Vec<SpinNoIrqLock<FutexManager>>> = Lazy::new(|| {
    let mut v = Vec::new();
    for i in 0..2 {
        let fm = SpinNoIrqLock::new(FutexManager::new(i == 1));
        v.push(fm);
    }
    v
});

pub fn single_futex_manager() -> impl DerefMut<Target = FutexManager> {
    FUTEX_MANAGER.first().unwrap().lock()
}

pub fn futex_manager(is_multi_group: bool, val32: u32) -> impl DerefMut<Target = FutexManager> {
    if !is_multi_group {
        FUTEX_MANAGER.first().unwrap().lock()
    } else {
        let mut r = FUTEX_MANAGER.get(1).unwrap().lock();
        r.val32 = val32;
        r
    }
}

pub struct FutexManager {
    hash: HashMap<FutexHashKey, Vec<FutexWaiter>>,
    pub(crate) val32: u32,
    is_mask: bool,
}

impl FutexManager {
    pub fn new(is_mask: bool) -> Self {
        Self {
            hash: HashMap::new(),
            val32: 0,
            is_mask,
        }
    }

    pub fn add_waiter(&mut self, key: &FutexHashKey, mut waiter: FutexWaiter) -> SysResult<()> {
        log::info!("[futex::add_waiter] {:?} in {:?} ", waiter, key);
        if self.is_mask {
            waiter.mask = self.val32;
        }

        if let Some(waiters) = self.hash.get_mut(key) {
            waiters.push(waiter);
        } else {
            let waiters = vec![waiter];
            self.hash.insert(*key, waiters);
        }
        Ok(())
    }

    pub fn rm_waiter(&mut self, key: &FutexHashKey, tid: Tid) -> SysResult<()> {
        if let Some(waiters) = self.hash.get_mut(key) {
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
        // log::info!("[futex::wake] {:?} in {:?} ", n, key);
        if let Some(waiters) = self.hash.get_mut(key) {
            let n = min(n as usize, waiters.len());
            // log::debug!("[futex::wake] waiters: {:?}", waiters);
            for _ in 0..n {
                if self.is_mask {
                    let waiter = waiters.pop().unwrap();
                    log::info!("[futex_wake] {:?} has been woken masked", waiter);
                    if (waiter.mask & self.val32) != 0 {
                        waiter.wake();
                    } else {
                        waiters.push(waiter);
                    }
                } else {
                    let waiter = waiters.pop().unwrap();
                    log::info!("[futex_wake] {:?} has been woken", waiter);
                    waiter.wake();
                }

                if waiters.is_empty() {
                    break;
                }
            }
            Ok(n)
        } else {
            // log::error!("can not find key {key:?}");
            Err(SysError::EINVAL)
        }
    }

    pub fn requeue_waiters(
        &mut self,
        old: FutexHashKey,
        new: FutexHashKey,
        n_req: usize,
    ) -> SyscallResult {
        let mut old_waiters = self.hash.remove(&old).ok_or_else(|| {
            log::info!("[futex] no waiters in key {:?}", old);
            SysError::EINVAL
        })?;

        let n = min(n_req, old_waiters.len());

        let iter = 0..n;
        if let Some(new_waiters) = self.hash.get_mut(&new) {
            iter.for_each(|_| new_waiters.push(old_waiters.pop().unwrap()));
        } else {
            let mut new_waiters = Vec::with_capacity(n);
            iter.for_each(|_| new_waiters.push(old_waiters.pop().unwrap()));
            self.hash.insert(new, new_waiters);
        }

        if !old_waiters.is_empty() {
            self.hash.insert(old, old_waiters);
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

#[derive(Debug, Clone)]
pub struct FutexWaiter {
    pub tid: Tid,
    pub waker: Waker,
    pub mask: u32,
}

impl FutexWaiter {
    pub fn new(task: &Task) -> Self {
        Self {
            tid: task.tid(),
            waker: task.get_waker(),
            mask: 0,
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
        const DEBUG = 265;
    }
}

impl FutexOp {
    pub fn exstract_futex_flags(val: i32) -> FutexOp {
        let flag_values = [
            (FutexOp::DEBUG, FutexOp::DEBUG.bits()),
            (FutexOp::ClockRealtime, FutexOp::ClockRealtime.bits()),
            (FutexOp::Private, FutexOp::Private.bits()),
            (FutexOp::WaitRequeuePi, FutexOp::WaitRequeuePi.bits()),
            (FutexOp::WakeBitset, FutexOp::WakeBitset.bits()),
            (FutexOp::WaitBitset, FutexOp::WaitBitset.bits()),
            (FutexOp::TrylockPi, FutexOp::TrylockPi.bits()),
            (FutexOp::UnlockPi, FutexOp::UnlockPi.bits()),
            (FutexOp::LockPi, FutexOp::LockPi.bits()),
            (FutexOp::WakeOp, FutexOp::WakeOp.bits()),
            (FutexOp::CmpRequeue, FutexOp::CmpRequeue.bits()),
            (FutexOp::Requeue, FutexOp::Requeue.bits()),
            (FutexOp::Fd, FutexOp::Fd.bits()),
            (FutexOp::Wake, FutexOp::Wake.bits()),
        ];

        let mut rest = val;
        let mut res = FutexOp::empty();
        for (flag, bits) in flag_values {
            // log::trace!("{:?}, {:#x}", flag, bits);
            if bits != 0 && (rest & bits) == bits {
                res |= flag;
                rest -= bits;
            }
        }

        if rest == 0 && val == 0 {
            FutexOp::Wait
        } else {
            res
        }
    }

    pub fn exstract_main_futex_flags(val: i32) -> FutexOp {
        let flag_values = [
            (FutexOp::WaitRequeuePi, FutexOp::WaitRequeuePi.bits()),
            (FutexOp::WakeBitset, FutexOp::WakeBitset.bits()),
            (FutexOp::WaitBitset, FutexOp::WaitBitset.bits()),
            (FutexOp::TrylockPi, FutexOp::TrylockPi.bits()),
            (FutexOp::UnlockPi, FutexOp::UnlockPi.bits()),
            (FutexOp::LockPi, FutexOp::LockPi.bits()),
            (FutexOp::WakeOp, FutexOp::WakeOp.bits()),
            (FutexOp::CmpRequeue, FutexOp::CmpRequeue.bits()),
            (FutexOp::Requeue, FutexOp::Requeue.bits()),
            (FutexOp::Fd, FutexOp::Fd.bits()),
            (FutexOp::Wake, FutexOp::Wake.bits()),
        ];

        let mut rest = val;
        let mut res = FutexOp::empty();
        for (flag, bits) in flag_values {
            if bits != 0 && (rest & bits) == bits {
                res |= flag;
                rest -= bits;
            }
        }

        if rest == 0 && val == 0 {
            FutexOp::Wait
        } else {
            res
        }
    }
}
