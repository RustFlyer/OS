pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    #[allow(non_snake_case)]
    pub fn autorun() {
        let TESTCASES = Vec::from([
"write05",
"write06",
"write_freezing.sh",
"writetest",
"writev01",
"writev02",
"writev03",
"writev05",
"writev06",
"writev07",
"zram01.sh",
"zram02.sh",
"zram03",
"zram_lib.sh",

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
