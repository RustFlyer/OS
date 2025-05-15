/*
 * @Author: greenhandzpx 893522573@qq.com
 * @Date: 2023-01-31 08:35:53
 * @LastEditors: greenhandzpx 893522573@qq.com
 * @LastEditTime: 2023-01-31 12:53:32
 * @FilePath: /os/src/sync/mutex/mod.rs
 * @Description:
 *
 * Copyright (c) 2023 by ${git_name_email}, All Rights Reserved.
 */

use sleep_mutex::SleepMutex;
use spin_mutex::SpinMutex;
use spin_then_sleep_mutex::SleepMutexCas;

#[cfg(target_arch = "loongarch64")]
use loongArch64::register::crmd;
#[cfg(target_arch = "riscv64")]
use riscv::register::sstatus;

pub mod optimistic_mutex;
pub mod share_mutex;
pub mod sleep_mutex;
pub mod spin_mutex;
pub mod spin_then_sleep_mutex;

pub use share_mutex::{ShareMutex, new_share_mutex};

pub type SpinLock<T> = SpinMutex<T, Spin>;
pub type SpinNoIrqLock<T> = SpinMutex<T, SpinNoIrq>;
pub type SleepLock<T> = SleepMutex<T, SpinNoIrq>;
pub type SleepCASLock<T> = SleepMutexCas<T, SpinNoIrq>;

/// Low-level support for mutex(spinlock, sleeplock, etc)
pub trait MutexSupport {
    /// Guard data
    type GuardData;
    /// Called before lock() & try_lock()
    fn before_lock() -> Self::GuardData;
    /// Called when MutexGuard dropping
    fn after_unlock(_: &mut Self::GuardData);
}

/// Spin MutexSupport
#[derive(Debug)]
pub struct Spin;

impl MutexSupport for Spin {
    type GuardData = ();
    #[inline(always)]
    fn before_lock() -> Self::GuardData {}
    #[inline(always)]
    fn after_unlock(_: &mut Self::GuardData) {}
}

/// Sie Guard
///
/// SieGuard 结构体的作用是管理 CPU 的中断使能状态（Supervisor Interrupt Enable, SIE）
/// ，通常用于在临界区代码中临时禁用中断，以确保代码的原子性执行。
/// SieGuard(bool): 包含一个布尔值，用于保存进入临界区之前的中断使能状态。
pub struct SieGuard(bool);

impl SieGuard {
    fn new() -> Self {
        let old_ie = {
            #[cfg(target_arch = "riscv64")]
            {
                let sie = sstatus::read().sie();
                unsafe {
                    sstatus::clear_sie();
                }
                sie
            }
            #[cfg(target_arch = "loongarch64")]
            {
                let ie = crmd::read().ie();
                crmd::set_ie(false);
                ie
            }
        };
        Self(old_ie)
    }
}

impl Drop for SieGuard {
    fn drop(&mut self) {
        if self.0 {
            #[cfg(target_arch = "riscv64")]
            unsafe {
                sstatus::set_sie();
            }
            #[cfg(target_arch = "loongarch64")]
            crmd::set_ie(true);
        }
    }
}

/// SpinNoIrq MutexSupport
#[derive(Debug)]
pub struct SpinNoIrq;

impl MutexSupport for SpinNoIrq {
    type GuardData = SieGuard;
    #[inline(always)]
    fn before_lock() -> Self::GuardData {
        SieGuard::new()
    }
    #[inline(always)]
    fn after_unlock(_: &mut Self::GuardData) {}
}
