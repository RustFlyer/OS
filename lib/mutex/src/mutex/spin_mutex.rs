use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

use super::MutexSupport;

struct MutexGuard<'a, T: ?Sized, S: MutexSupport> {
    mutex: &'a SpinMutex<T, S>,
    support_guard: S::GuardData,
}

/// `SpinMutex` can include different `MutexSupport` type
#[derive(Debug)]
pub struct SpinMutex<T: ?Sized, S: MutexSupport> {
    // debug_cnt: UnsafeCell<usize>,
    lock: AtomicBool,
    _marker: PhantomData<S>,
    data: UnsafeCell<T>,
}

// 禁止Mutex跨越await导致死锁或无意义阻塞
impl<T: ?Sized, S: MutexSupport> !Sync for MutexGuard<'_, T, S> {}
impl<T: ?Sized, S: MutexSupport> !Send for MutexGuard<'_, T, S> {}

unsafe impl<T: ?Sized + Send, S: MutexSupport> Sync for SpinMutex<T, S> {}
unsafe impl<T: ?Sized + Send, S: MutexSupport> Send for SpinMutex<T, S> {}

impl<T, S: MutexSupport> SpinMutex<T, S> {
    /// Construct a SpinMutex
    pub const fn new(user_data: T) -> Self {
        SpinMutex {
            lock: AtomicBool::new(false),
            _marker: PhantomData,
            data: UnsafeCell::new(user_data),
            // debug_cnt: UnsafeCell::new(0),
        }
    }
    /// Wait until the lock looks unlocked before retrying
    #[inline(always)]
    fn wait_unlock(&self) {
        let mut try_count = 0usize;
        while self.lock.load(Ordering::Relaxed) {
            core::hint::spin_loop();
            try_count += 1;
            if try_count == 0x10000000 {
                log::error!("dead lock!!");
                panic!("Mutex: deadlock detected! try_count > {:#x}\n", try_count);
            }
        }
    }
    /// lock
    #[inline(always)]
    pub fn lock(&self) -> impl DerefMut<Target = T> + '_ {
        let support_guard = S::before_lock();
        // unsafe {
        //     (*self.debug_cnt.get()) += 1;
        //     println!("debug cnt addr {:#x}", self.debug_cnt.get() as usize);
        //     println!("debug cnt {}", (*self.debug_cnt.get()));
        // }
        loop {
            self.wait_unlock();
            if self
                .lock
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
        MutexGuard {
            mutex: self,
            support_guard,
        }
    }

    /// # Safety
    ///
    /// This is highly unsafe.
    /// You should ensure that context switch won't happen during
    /// the locked data's lifetime.
    #[inline(always)]
    pub unsafe fn sent_lock(&self) -> impl DerefMut<Target = T> + '_ {
        SendWrapper::new(self.lock())
    }
}

impl<T: ?Sized, S: MutexSupport> Deref for MutexGuard<'_, T, S> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized, S: MutexSupport> DerefMut for MutexGuard<'_, T, S> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized, S: MutexSupport> Drop for MutexGuard<'_, T, S> {
    /// The dropping of the MutexGuard will release the lock it was created from.
    #[inline(always)]
    fn drop(&mut self) {
        // debug_assert!(self.mutex.lock.load(Ordering::Relaxed));
        self.mutex.lock.store(false, Ordering::Release);
        S::after_unlock(&mut self.support_guard);
    }
}

/// A wrapper for a data structure that be sent between threads
pub struct SendWrapper<T>(pub T);

impl<T> SendWrapper<T> {
    pub fn new(data: T) -> Self {
        SendWrapper(data)
    }
}

unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

impl<T: Deref> Deref for SendWrapper<T> {
    type Target = T::Target;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T: DerefMut> DerefMut for SendWrapper<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}
