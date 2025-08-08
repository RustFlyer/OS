pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    #[allow(non_snake_case)]
    pub fn autorun() {
        let TESTCASES = Vec::from([
"futex_wake01",
"futex_wake02",
"futex_wake03",
"futex_wake04",
"futimesat01",
"fw_load",
        ]);

        for test in TESTCASES {
            let cmd = format!("ltp/testcases/bin/{}", test);
            run_cmd(&cmd);
        }
    }
}
