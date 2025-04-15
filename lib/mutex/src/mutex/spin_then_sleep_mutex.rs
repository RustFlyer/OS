extern crate alloc;
use alloc::{collections::VecDeque, sync::Arc};
use core::{
    cell::{SyncUnsafeCell, UnsafeCell},
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};
use simdebug::stop;

use super::{MutexSupport, spin_mutex::SpinMutex};

const MAX_SPIN_COUNT: usize = 1000;

#[derive(Debug)]
struct MutexInner {
    locked: bool,
    queue: UnsafeCell<Option<VecDeque<Arc<GrantInfo>>>>,
}

/// SleepMutexCas can step over `await`
#[derive(Debug)]
pub struct SleepMutexCas<T: ?Sized, S: MutexSupport> {
    lock: SpinMutex<MutexInner, S>, // push at prev, release at next
    data: UnsafeCell<T>,            // actual data
}

unsafe impl<T: ?Sized + Send, S: MutexSupport> Send for SleepMutexCas<T, S> {}
unsafe impl<T: ?Sized + Send, S: MutexSupport> Sync for SleepMutexCas<T, S> {}

impl<T, S: MutexSupport> SleepMutexCas<T, S> {
    pub const fn new(user_data: T) -> Self {
        SleepMutexCas {
            lock: SpinMutex::new(MutexInner {
                locked: false,
                queue: UnsafeCell::new(None),
            }),
            data: UnsafeCell::new(user_data),
        }
    }
}

impl<T: ?Sized + Send, S: MutexSupport> SleepMutexCas<T, S> {
    /// Lock
    #[inline]
    pub async fn lock(&self) -> impl DerefMut<Target = T> + Send + Sync + '_ {
        let future = &mut SleepMutexCasFuture::new(self);
        unsafe { Pin::new_unchecked(future).init().await.await }
    }
}

struct GrantInfo {
    inner: SyncUnsafeCell<(AtomicBool, Option<Waker>)>,
}

struct SleepMutexCasFuture<'a, T: ?Sized, S: MutexSupport> {
    mutex: &'a SleepMutexCas<T, S>,
    grant: Arc<GrantInfo>,
}

impl<'a, T: ?Sized, S: MutexSupport> SleepMutexCasFuture<'a, T, S> {
    #[inline(always)]
    fn new(mutex: &'a SleepMutexCas<T, S>) -> Self {
        SleepMutexCasFuture {
            mutex,
            grant: Arc::new(GrantInfo {
                inner: SyncUnsafeCell::new((AtomicBool::new(false), None)),
            }),
        }
    }

    async fn init(self: Pin<&mut Self>) -> Pin<&mut SleepMutexCasFuture<'a, T, S>> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut spin_count = 0;

        loop {
            let inner = unsafe { &mut *this.mutex.lock.sent_lock() };

            if !inner.locked {
                // log::debug!("[sleepCasMutex] get mutex");
                inner.locked = true;
                unsafe { &mut *this.grant.inner.get() }
                    .0
                    .store(true, Ordering::Release);
                break;
            }

            if spin_count >= MAX_SPIN_COUNT {
                stop();
                // log::debug!("[sleepCasMutex] step into wait list");
                unsafe { &mut *this.grant.inner.get() }.1 = Some(take_waker().await);
                let queue = unsafe { &mut (*inner.queue.get()) };
                if queue.is_none() {
                    *queue = Some(VecDeque::new());
                }
                queue.as_mut().unwrap().push_back(this.grant.clone());
                break;
            }

            spin_count += 1;
            core::hint::spin_loop();
        }

        unsafe { Pin::new_unchecked(this) }
    }
}

impl<'a, T: ?Sized, S: MutexSupport> Future for SleepMutexCasFuture<'a, T, S> {
    type Output = SleepMutexCasGuard<'a, T, S>;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let granted = unsafe { &*self.grant.inner.get() }
            .0
            .load(Ordering::Acquire);
        match granted {
            false => Poll::Pending,
            true => {
                // log::trace!("[SleepMutexCasFuture::poll] granted");
                Poll::Ready(SleepMutexCasGuard { mutex: self.mutex })
            }
        }
    }
}

struct SleepMutexCasGuard<'a, T: ?Sized, S: MutexSupport> {
    mutex: &'a SleepMutexCas<T, S>,
}

unsafe impl<'a, T: ?Sized + Send, S: MutexSupport> Send for SleepMutexCasGuard<'a, T, S> {}
unsafe impl<'a, T: ?Sized + Send, S: MutexSupport> Sync for SleepMutexCasGuard<'a, T, S> {}

impl<'a, T: ?Sized, S: MutexSupport> Deref for SleepMutexCasGuard<'a, T, S> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized, S: MutexSupport> DerefMut for SleepMutexCasGuard<'a, T, S> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized, S: MutexSupport> Drop for SleepMutexCasGuard<'a, T, S> {
    #[inline]
    fn drop(&mut self) {
        // log::trace!("[SleepMutexCasGuard::drop] drop...");
        let mut inner = self.mutex.lock.lock();
        debug_assert!(inner.locked);
        let queue = unsafe { &mut (*inner.queue.get()) };
        if queue.is_none() {
            inner.locked = false;
            // log::error!("[SleepMutexCasGuard::drop] queue is none");
            return;
        }
        let waiter = match queue.as_mut().unwrap().pop_front() {
            None => {
                // The wait queue is empty
                inner.locked = false;
                // log::error!("[SleepMutexCasGuard::drop] queue is empty");
                return;
            }
            Some(waiter) => waiter,
        };
        drop(inner);
        // Waker should be fetched before we make the grant_inner.0 true
        // since it will be invalid after that.
        let grant_inner = unsafe { &mut *waiter.inner.get() };
        let waker = grant_inner.1.take().unwrap();
        grant_inner.0.store(true, Ordering::Release);
        waker.wake();
        // log::trace!("[SleepMutexCasGuard::drop] grant someone...");
    }
}

#[inline(always)]
pub async fn take_waker() -> Waker {
    TakeWakerFuture.await
}

struct TakeWakerFuture;

impl Future for TakeWakerFuture {
    type Output = Waker;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(cx.waker().clone())
    }
}
