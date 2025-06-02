pub fn hart_start(hart_id: usize, start_addr: usize) -> usize {
    sbi_rt::hart_start(hart_id, start_addr, 0).unwrap()
}

#[inline]
pub fn hart_shutdown() -> ! {
    // sbi_rt::legacy::shutdown();
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    unreachable!()
}
