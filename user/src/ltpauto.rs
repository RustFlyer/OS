pub mod ltprun {
    extern crate alloc;
    use crate::run_cmd;
    use alloc::{format, vec::Vec};

    #[allow(non_snake_case)]
    pub fn autorun() {
        let TESTCASES = Vec::from([
"doio",
"du01.sh",
"dup01",
"dup02",
"dup03",
"dup04",
"dup05",
"dup06",
"dup07",
"dup201",
"dup202",
"dup203",
"dup204",
"dup205",
"dup206",
"dup207",
"dup3_01",
"dup3_02",
"dynamic_debug01.sh",

        ]);

        for test in TESTCASES {
            let cmd = format!("ltp/testcases/bin/{}", test);
            run_cmd(&cmd);
        }
    }
}
