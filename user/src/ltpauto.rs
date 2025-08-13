pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    #[allow(non_snake_case)]
    pub fn autorun() {
        let TESTCASES = Vec::from([
            "splice07",
            "fanotify01",
            "fanotify14",
            "epoll_ctl03",
            "access01",
            // "memfd_create01",
            "waitpid01",
            "getpid01",
            "chdir01",
            "pipe11",
            "fsync01",
            "rename01",
            "rename03",
            "fsetxattr01",
            "setxattr01",
            "clock_getres01",
            "mkdir09",
            "splice08",
            "confstr01",
        ]);

        for test in TESTCASES {
            let cmd = format!("ltp/testcases/bin/{}", test);
            let _ = run_cmd(&cmd);
        }
    }
}
