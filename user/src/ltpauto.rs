pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    #[allow(non_snake_case)]
    pub fn autorun() {
        let TESTCASES = Vec::from([
"stress",
"string01",
"support_numa",
"swapoff01",
"swapoff02",
"swapon01",
"swapon02",
"swapon03",
"swapping01",
"symlink01",
"symlink02",
"symlink03",
"symlink04",
"symlinkat01",
"sync01",
"sync_file_range01",
"sync_file_range02",
"syncfs01",
"syscall01",
"sysconf01",
"sysctl01",
"sysctl01.sh",
"sysctl02.sh",
"sysctl03",
"sysctl04",
"sysfs01",
"sysfs02",
"sysfs03",
"sysfs04",
"sysfs05",
"sysinfo01",
"sysinfo02",
"sysinfo03",
"syslog11",
"syslog12",
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
