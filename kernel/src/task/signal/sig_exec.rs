use std::sync::Arc;
use std::mem::size_of;
use crate::task::Task;
use crate::task::sig_members::{
    SigInfo, 
    SigSet, 
    Sig,
    SigActionFlag,
    ActionType,
    SigContext,
    SigStack
};
use crate::vm::user_ptr::UserWritePtr;
use crate::syscall::SysResult;

pub fn sig_check(task: Arc<Task>, mut intr: bool) -> SysResult<()> {
    let old_mask = task.get_sig_mask();

    while let Some(si) = task.sig_manager_mut().dequeue_signal(&old_mask) {
        // if sig_exec turns to user handler, it will return true to break the loop and run user handler.
        if sig_exec(task.clone(), si) {
            break;
        }
    }
    Ok(())
}

fn sig_exec(task: Arc<Task>, si: SigInfo) {
    let action = task.sig_handlers_mut().get(si.sig);
    let cx = task.trap_context_mut();
    let old_mask = task.get_sig_mask();

    log::info!("[do signal] Handling signal: {:?} {:?}", si, action);
    if action.flags.contains(SigActionFlag::SA_RESTART) {
        cx.sepc -= 4;
        cx.restore_last_user_a0();
        log::info!("[do_signal] restart syscall");
    }
    match action.atype {
        ActionType::Ignore => false,
        ActionType::Kill => {
            kill(task, si.sig);
            false
        }
        ActionType::Stop => {
            stop(task, si.sig);
            false
        }
        ActionType::Cont => {
            cont(task, si.sig);
            false
        }
        ActionType::User { entry } => {
            // The signal being delivered is also added to the signal mask, unless
            // SA_NODEFER was specified when registering the handler.
            if !action.flags.contains(SigActionFlag::SA_NODEFER) {
                task.sig_mask().add_signal(si.sig)
            };
            // 信号定义中可能包含了在处理该信号时需要阻塞的其他信号集。
            // 这些信息定义在Action的mask字段
            *task.sig_mask() |= action.mask;
            // TODO: cx.user_fx.encounter_signal();
            // TODO: sig_stack isn't actually used for now (Even so in Phoenix, because sig_stack is always None)
            let sig_stack = task.sig_stack_mut().take();
            let sp = match sig_stack {
                Some(s) => {
                    log::error!("[sigstack] use user defined signal stack. Unimplemented");
                    s.get_stack_top()
                }
                None => {
                    // 如果进程未定义专门的信号栈，
                    // 用户自定义的信号处理函数将使用进程的普通栈空间，
                    // 即和其他普通函数相同的栈。这个栈通常就是进程的主栈，
                    // 也就是在进程启动时由操作系统自动分配的栈。
                    cx.user_x[2]
                }
            };
            // extend the sig_stack
            // 在栈上压入一个sig_cx，存储trap frame里的寄存器信息

            let mut new_sp = sp - size_of::<SigContext>();
            let sig_cx_ptr: UserWritePtr<SigContext> = new_sp.into();
            // TODO: should increase the size of the sig_stack? It seems umi doesn't
            let mut sig_cx = SigContext {
                flags: 0,
                link: 0,
                stack: sig_stack.unwrap_or_default(),
                mask: old_mask,
                sig: [0; 16],
                user_x: cx.user_x,
                fpstate: [0; 66],
            };
            sig_cx.user_x[0] = cx.sepc;
            log::trace!("[save_context_into_sigstack] sig_cx_ptr: {sig_cx_ptr:?}");
            unsafe { sig_cx_ptr.write(sig_cx)? };
            task.set_sig_cx_ptr(new_sp);
            // user defined void (*sa_handler)(int);
            cx.user_x[10] = si.sig.raw();
            // if sa_flags contains SA_SIGINFO, It means user defined function is
            // void (*sa_sigaction)(int, siginfo_t *, void *sig_cx); which two more
            // parameters
            // FIXME: `SigInfo` and `SigContext` may not be the exact struct in C, which will
            // cause a random bug that sometimes user will trap into kernel because of
            // accessing kernel addrress
            if action.flags.contains(SigActionFlag::SA_SIGINFO) {
                // log::error!("[SA_SIGINFO] set sig_cx {sig_cx:?}");
                // a2
                cx.user_x[12] = new_sp;
                #[derive(Default, Copy, Clone)]
                #[repr(C)]
                pub struct LinuxSigInfo {
                    pub si_signo: i32,
                    pub si_errno: i32,
                    pub si_code: i32,
                    pub _pad: [i32; 29],
                    _align: [u64; 0],
                }
                let mut siginfo_v = LinuxSigInfo::default();
                siginfo_v.si_signo = si.sig.raw() as _;
                siginfo_v.si_code = si.code;
                new_sp -= size_of::<LinuxSigInfo>();
                let siginfo_ptr: UserWritePtr<LinuxSigInfo> = new_sp.into();
                unsafe { siginfo_ptr.write(siginfo_v)? };
                cx.user_x[11] = new_sp;
            }
            cx.sepc = entry;
            // ra (when the sigaction set by user finished,it will return to
            // _sigreturn_trampoline, which calls sys_sigreturn)
            cx.user_x[1] = _sigreturn_trampoline as usize;
            // sp (it will be used later by sys_sigreturn to restore sig_cx)
            cx.user_x[2] = new_sp;
            cx.user_x[4] = sig_cx.user_x[4];
            cx.user_x[3] = sig_cx.user_x[3];
            // log::error!("{:#x}", new_sp);
            true
        }
    }
}

/// kill the process
fn kill(task: &Arc<Task>, sig: Sig) {
    // exit all the memers of a thread group
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_terminated();
        }
    });
    // 将信号放入低7位 (第8位是core dump标志,在gdb调试崩溃程序中用到)
    task.set_exit_code(sig.raw() as i32 & 0x7F);
}

fn stop(task: &Arc<Task>, sig: Sig) {
    log::warn!("[do_signal] task stopped!");
    task.with_mut_thread_group(|tg| {
        for t in tg.iter() {
            t.set_stopped();
            t.set_wake_up_signal(SigSet::SIGCONT);
        }
    });
    task.notify_parent(SigInfo::CLD_STOPPED, sig);
}

/// continue the process if it is currently stopped
fn cont(task: &Arc<Task>, sig: Sig) {
    log::warn!("[do_signal] task continue");
    task.with_mut_thread_group(|tg| {
        for t in tg.iter() {
            t.set_running();
            t.wake();
        }
    });
    task.notify_parent(SigInfo::CLD_CONTINUED, sig);
}