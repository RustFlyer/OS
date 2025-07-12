use alloc::sync::Arc;
use core::arch::global_asm;
use timer::{IEvent, TimerState};

use systype::error::SysResult;

use super::sig_info::Sig;
use crate::task::{
    Task, TaskState,
    manager::TASK_MANAGER,
    sig_members::{ActionType, SS_DISABLE, SS_ONSTACK, SigActionFlag, SigContext},
    signal::sig_info::{LinuxSigInfo, SigDetails, SigInfo, SigSet},
};
use crate::vm::user_ptr::UserWritePtr;

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("riscv64_sigreturn_trampoline.asm"));
#[cfg(target_arch = "loongarch64")]
global_asm!(include_str!("loongarch64_sigreturn_trampoline.asm"));

pub async fn sig_check(task: Arc<Task>, interrupted: &mut bool) {
    let old_mask = task.get_sig_mask();

    while let Some(si) = task.sig_manager_mut().dequeue_signal(&old_mask) {
        // if sig_exec turns to user handler, it will return true to break the loop and run user handler.
        let ret = sig_exec(task.clone(), si, interrupted).await;

        match ret {
            Ok(b) if b => break,
            Ok(_) => continue,
            Err(e) => {
                log::error!("[sig_check] sig_exec: {:?}", e);
            }
        }
    }
}

async fn sig_exec(task: Arc<Task>, si: SigInfo, interrupted: &mut bool) -> SysResult<bool> {
    let action = task.sig_handlers_mut().lock().get(si.sig);
    let cx = task.trap_context_mut();
    #[cfg(target_arch = "loongarch64")]
    log::debug!("[sig context] TrapContext.sp: {:#x}", cx.user_reg[3]);
    let old_mask = task.get_sig_mask();

    // log::debug!(
    //     "[sig_exec] task [{}] Handling signal: {:?} {:?}",
    //     task.get_name(),
    //     si,
    //     action
    // );

    log::info!(
        "[sig_exec] task {} [{}] Handling signal: {:?} {:?}",
        task.tid(),
        task.get_name(),
        si.sig,
        action
    );

    if *interrupted && action.flags.contains(SigActionFlag::SA_RESTART) {
        cx.sepc -= 4;
        cx.restore_last_user_ret_val();
        *interrupted = false;
        log::info!("[sig_exec] restart syscall");
    }

    match action.atype {
        ActionType::Ignore => Ok(false),
        ActionType::Kill => {
            if task.tid() == 1 {
                log::warn!("[sig_exec] kill init task, ignored");
                return Ok(false);
            }
            kill(&task, si.sig);
            Ok(false)
        }
        ActionType::Stop => {
            if task.tid() == 1 {
                log::warn!("[sig_exec] kill init task, ignored");
                return Ok(false);
            }
            stop(&task, si.sig);
            Ok(false)
        }
        ActionType::Cont => {
            cont(&task, si.sig);
            Ok(false)
        }
        ActionType::User { entry } => {
            // log::debug!("[sig_exec] en");
            // return Ok(true);
            // The signal being delivered is also added to the signal mask, unless
            // SA_NODEFER was specified when registering the handler.
            if !action.flags.contains(SigActionFlag::SA_NODEFER) {
                task.sig_mask_mut().add_signal(si.sig)
            };
            // 信号定义中可能包含了在处理该信号时需要阻塞的其他信号集。
            // 这些信息定义在Action的mask字段
            *task.sig_mask_mut() |= action.mask;
            // TODO: cx.user_fx.encounter_signal();
            // TODO: sig_stack isn't actually used for now (Even so in Phoenix, because sig_stack is always None)
            // let sig_stack = task.sig_stack_mut().take();
            // let sp = match sig_stack {
            //     Some(s) => {
            //         log::error!("[sigstack] use user defined signal stack. Unimplemented");
            //         s.get_stack_top()
            //     }
            //     None => {
            //         // 如果进程未定义专门的信号栈，
            //         // 用户自定义的信号处理函数将使用进程的普通栈空间，
            //         // 即和其他普通函数相同的栈。这个栈通常就是进程的主栈，
            //         // 也就是在进程启动时由操作系统自动分配的栈。
            //         cx.get_user_sp()
            //     }
            // };

            let sig_stack = task.sig_stack_mut();
            let use_altstack = action.flags.contains(SigActionFlag::SA_ONSTACK)
                && (sig_stack.ss_flags & SS_DISABLE) == 0
                && (sig_stack.ss_flags & SS_ONSTACK) == 0;

            let sp = if use_altstack {
                sig_stack.ss_flags |= SS_ONSTACK;
                sig_stack.get_stack_top()
            } else {
                cx.get_user_sp()
            };

            // extend the sig_stack
            // 在栈上压入一个sig_cx，存储trap frame里的寄存器信息

            let mut new_sp = sp - size_of::<SigContext>();
            let addr_space = task.addr_space();
            let mut sig_cx_ptr = UserWritePtr::<SigContext>::new(new_sp, &addr_space);
            // TODO: should increase the size of the sig_stack? It seems umi doesn't
            let mut sig_cx = SigContext {
                flags: 0,
                link: 0,
                stack: *sig_stack,
                mask: old_mask,
                sig: [0; 16],
                user_reg: cx.user_reg,
                fpstate: [0; 66],
            };
            sig_cx.user_reg[0] = cx.sepc;
            // log::debug!("[sig context] sig_cx_ptr: {sig_cx_ptr:?}");

            unsafe { sig_cx_ptr.write(sig_cx)? };

            // restore the new stack pointer in Task for sigreturn to recover
            task.set_sig_cx_ptr(new_sp);
            // user defined void (*sa_handler)(int);
            cx.set_user_a0(si.sig.raw());

            // if sa_flags contains SA_SIGINFO, It means user defined function is
            // void (*sa_sigaction)(int, siginfo_t *, void *sig_cx); which two more
            // parameters
            // FIXME: `SigInfo` and `SigContext` may not be the exact struct in C, which will
            // cause a random bug that sometimes user will trap into kernel because of
            // accessing kernel addrress
            if action.flags.contains(SigActionFlag::SA_SIGINFO) {
                // log::error!("[SA_SIGINFO] set sig_cx {sig_cx:?}");
                // a2
                cx.set_user_a2(new_sp);

                let siginfo = if let SigDetails::Kill { pid: _, siginfo } = si.details {
                    match siginfo {
                        Some(info) => info,
                        None => {
                            log::warn!("[sig_exec] siginfo is None, use the default value");
                            LinuxSigInfo {
                                si_signo: si.sig.raw() as _,
                                si_code: si.code,
                                si_pid: task.tid() as _,
                                si_uid: task.uid() as _,
                                ..Default::default()
                            }
                        }
                    }
                } else {
                    log::error!("[sig_exec] SigDetail should be Kill, but got: {:?}", si.details);
                    panic!("sig_exec: unexpected siginfo details");
                };
                new_sp -= size_of::<LinuxSigInfo>();
                let mut siginfo_ptr = UserWritePtr::<LinuxSigInfo>::new(new_sp, &addr_space);
                unsafe { siginfo_ptr.write(siginfo)? };
                cx.set_user_a1(new_sp);
            }

            cx.sepc = entry;
            // ra (when the sigaction set by user finished,it will return to
            // _sigreturn_trampoline, which calls sys_sigreturn)
            cx.user_reg[1] = _sigreturn_trampoline as usize;
            // sp (it will be used later by sys_sigreturn to restore sig_cx)
            cx.set_user_sp(new_sp);

            #[cfg(target_arch = "riscv64")]
            {
                cx.user_reg[3] = sig_cx.user_reg[3];
                cx.user_reg[4] = sig_cx.user_reg[4];
            }
            #[cfg(target_arch = "loongarch64")]
            {
                assert!(cx.user_reg[2] == sig_cx.user_reg[2]);
                assert!(cx.user_reg[21] == sig_cx.user_reg[21]);
                assert!(cx.user_reg[28] == sig_cx.user_reg[28]);
            }

            // cx.user_reg
            //     .iter()
            //     .enumerate()
            //     .for_each(|(idx, u)| log::debug!("r[{idx:02}]: {:#x}", u));

            log::debug!(
                "[sig context] sig: {:#x}, entry: {:#x}",
                task.sig_manager_mut().bitmap.bits(),
                entry
            );

            Ok(true)
        }
    }
}

unsafe extern "C" {
    unsafe fn _sigreturn_trampoline();
}

/// kill the process
fn kill(task: &Arc<Task>, sig: Sig) {
    // exit all the memers of a thread group
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_state(TaskState::Zombie);
            t.wake();
            log::debug!(
                "[kill] wake all threads in group, now task: {:?}",
                t.get_name()
            );
        }
    });
    // 将信号放入低7位 (第8位是core dump标志,在gdb调试崩溃程序中用到)
    let mut exit_code = sig.raw() as i32 & 0x7F;
    if SigSet::DUMP_MASK.contain_signal(sig) {
        exit_code |= 0x80;
    }
    task.set_exit_code(exit_code);
}

fn stop(task: &Arc<Task>, sig: Sig) {
    log::warn!("[do_signal] task stopped!");
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_wake_up_signal(SigSet::SIGCONT);
            t.set_state(TaskState::Sleeping);
        }
    });
    task.notify_parent(SigInfo::CLD_STOPPED, sig);
}

/// continue the process if it is currently stopped
fn cont(task: &Arc<Task>, sig: Sig) {
    log::warn!("[do_signal] task continue");
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_state(TaskState::Running);
            t.wake();
        }
    });
    task.notify_parent(SigInfo::CLD_CONTINUED, sig);
}

#[derive(Debug)]
pub struct SigEvent {
    pub tid: usize,
    pub sig: i32,
}

impl IEvent for SigEvent {
    fn callback(self: Arc<Self>) -> TimerState {
        if let Some(task) = TASK_MANAGER.get_task(self.tid) {
            task.receive_siginfo(SigInfo {
                sig: Sig::from_i32(self.sig),
                code: SigInfo::USER,
                details: SigDetails::Kill {
                    pid: task.pid(),
                    siginfo: None,
                },
            });
        }
        TimerState::Cancelled
    }
}

pub fn clear_tasks() {
    let _ = TASK_MANAGER.for_each(|t| {
        if t.tid() > 5 {
            t.receive_siginfo(SigInfo {
                sig: Sig::SIGKILL,
                code: SigInfo::KERNEL,
                details: SigDetails::None,
            });
        }
        Ok(())
    });
}
