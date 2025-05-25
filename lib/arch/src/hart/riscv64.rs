pub fn hart_start(hart_id: usize, start_addr: usize) -> usize {
    sbi_rt::hart_start(hart_id, start_addr, 0).unwrap()
}

pub fn hart_shutdown() -> ! {
    sbi_rt::hart_stop().unwrap();
    panic!("SBI shutdown failed");
}
