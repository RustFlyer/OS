#[allow(non_snake_case)]
pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    pub fn autorun() {
        let TESTCASES = Vec::from([
            "pidfd_send_signal01",
            "pidfd_send_signal02",
            "pidfd_send_signal03",
            "rt_sigprocmask01",
            "rt_sigprocmask02",
            "rt_sigqueueinfo01",
            "rt_sigsuspend01",
            "sigaction01",
            "sigaction02",
            "sigaltstack01",
            "sigaltstack02",
            "sighold02",
            "signal01",
            "signal02",
            "signal03",
            "signal04",
            "signal05",
            "signal06",
            "signalfd01",
            "signalfd4_01",
            "signalfd4_02",
            "sigpending02",
            "sigprocmask01",
            "sigrelse01",
            "sigsuspend01",
            "sigtimedwait01",
            "sigwait01",
            "sigwaitinfo01",
        ]);

        println!("#### OS COMP TEST GROUP START ltp ####");
        for test in TESTCASES {
            println!("RUN LTP CASE {}", test);
            let cmd = format!("ltp/testcases/bin/{}", test);
            run_cmd(&cmd);
        }
        println!("#### OS COMP TEST GROUP END ltp ####");
    }

    pub fn runltp_rv() {
        let RVTESTCASES = Vec::from([
            "pidfd_send_signal01",
            "pidfd_send_signal02",
            "pidfd_send_signal03",
            "rt_sigprocmask01",
            "rt_sigprocmask02",
            "rt_sigqueueinfo01",
            "rt_sigsuspend01",
            "sigaction01",
            "sigaction02",
            "sigaltstack01",
            "sigaltstack02",
            "sighold02",
            "signal01",
            "signal02",
            "signal03",
            "signal04",
            "signal05",
            "signal06",
            "signalfd01",
            "signalfd4_01",
            "signalfd4_02",
            "sigpending02",
            "sigprocmask01",
            "sigrelse01",
            "sigsuspend01",
            "sigtimedwait01",
            "sigwait01",
            "sigwaitinfo01",
        ]);

        println!("#### OS COMP TEST GROUP START ltp ####");
        for test in RVTESTCASES {
            println!("RUN LTP CASE {}", test);
            let cmd = format!("ltp/testcases/bin/{}", test);
            run_cmd(&cmd);
        }
        println!("#### OS COMP TEST GROUP END ltp ####");
    }

    pub fn runltp_la() {
        let LATESTCASES = Vec::from([
            "pidfd_send_signal01",
            "pidfd_send_signal02",
            "pidfd_send_signal03",
            "rt_sigprocmask01",
            "rt_sigprocmask02",
            "rt_sigqueueinfo01",
            "rt_sigsuspend01",
            "sigaction01",
            "sigaction02",
            "sigaltstack01",
            "sigaltstack02",
            "sighold02",
            "signal01",
            "signal02",
            "signal03",
            "signal04",
            "signal05",
            "signal06",
            "signalfd01",
            "signalfd4_01",
            "signalfd4_02",
            "sigpending02",
            "sigprocmask01",
            "sigrelse01",
            "sigsuspend01",
            "sigtimedwait01",
            "sigwait01",
            "sigwaitinfo01",
        ]);

        println!("#### OS COMP TEST GROUP START ltp ####");
        for test in LATESTCASES {
            println!("RUN LTP CASE {}", test);
            let cmd = format!("ltp/testcases/bin/{}", test);
            run_cmd(&cmd);
        }
        println!("#### OS COMP TEST GROUP END ltp ####");
    }
}

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
