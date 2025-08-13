pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    #[allow(non_snake_case)]
    pub fn autorun() {
        let TESTCASES = Vec::from([
"wait01",
"wait02",
"wait401",
"wait402",
"wait403",
"waitid01",
"waitid02",
"waitid03",
"waitid04",
"waitid05",
"waitid06",
"waitid07",
"waitid08",
"waitid09",
"waitid10",
"waitid11",
"waitpid01",
"waitpid03",
"waitpid04",
"waitpid06",
"waitpid07",
"waitpid08",
"waitpid09",
"waitpid10",
"waitpid11",
"waitpid12",
"waitpid13",
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
