pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    #[allow(non_snake_case)]
    pub fn autorun() {
        let TESTCASES = Vec::from([
"fanotify01",
"splice07",
"fanotify14",
"epoll_ctl03",
"access01",
"rt_sigaction01",
"rt_sigaction02",
"rt_sigaction03",
"chdir01",
"waitpid01",
"fsetxattr01",
"setxattr01",
"rename01",
"rename03",
"pipe11",
"mkdir09",
"memfd_create01",
"fgetxattr01",
"clock_getres01",
"sysconf01",
"copy_file_range01",
"chmod01",
"confstr01",
"epoll-ltp",
"posix_fadvise03",
"posix_fadvise03_64",
"splice08",
"signal03",
"signal05",
"signal04",
        ]);

        for test in TESTCASES {
            let cmd = format!("ltp/testcases/bin/{}", test);
            println!("------------------------------- \n \
                    Running testcase: {} \n \
                    -------------------------------", cmd);
            run_cmd(&cmd);
        }
    }
}

// "wait01",
// "wait02",
// "wait401",
// "wait402",
// "wait403",
// "waitid01",
// "waitid02",
// "waitid03",
// "waitid04",
// "waitid05",
// "waitid06",
// "waitid09",
// "waitid10",
// "waitid11",
// "waitpid01",
// "waitpid03",
// "waitpid04",
// "waitpid06",
// "waitpid07",
// "waitpid08",
// "waitpid09",
// "waitpid10",
// "waitpid11",
// "waitpid12",
// "waitpid13",

// "pidfd_send_signal01",
// "pidfd_send_signal02",
// "pidfd_send_signal03",
// "rt_sigprocmask01",
// "rt_sigprocmask02",
// "rt_sigqueueinfo01",
// "rt_sigsuspend01",
// "sigaction01",
// "sigaction02",
// "sigaltstack01",
// "sigaltstack02",
// "sighold02",
// "signal01",
// "signal02",
// "signal03",
// "signal04",
// "signal05",
// "signal06",
// "signalfd01",
// "signalfd4_01",
// "signalfd4_02",
// "sigpending02",
// "sigprocmask01",
// "sigrelse01",
// "sigsuspend01",
// "sigtimedwait01",
// "sigwait01",
// "sigwaitinfo01",

// "pidfd_send_signal01", stuck and ESRCH
// "pidfd_send_signal02", enoent broken
// "pidfd_send_signal03", skiped for file not found
// "rt_sigprocmask01", *pass
// "rt_sigprocmask02", pass
// "rt_sigqueueinfo01", 138 unimplemented
// "rt_sigsuspend01", nothing
// "sigaction01", pass
// "sigaction02", pass
// "sigaltstack01", pass
// "sigaltstack02", pass
// "sighold02", “signal handler was executed”
// "signal01", timeout
// "signal02", pass
// "signal03", pass
// "signal04", pass
// "signal05", pass
// "signal06", not on x86
// "signalfd01", pass
// "signalfd4_01", "signalfd4(SFD_CLOEXEC) does not set close-on-exec flag"
// "signalfd4_02", "signalfd4(SFD_CLOEXEC) does not set close-on-exec flag"
// "sigpending02", *4 pass
// "sigprocmask01", *pass
// "sigrelse01", failed
// "sigsuspend01", nothing
// "sigtimedwait01", timeout
// "sigwait01", timeout
// "sigwaitinfo01", timeout
