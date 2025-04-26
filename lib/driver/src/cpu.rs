#[derive(Debug)]
pub struct CPU {
    pub id: usize,
    pub usable: bool, // is the CPU usable? we need MMU
    pub clock_freq: usize,
    pub timebase_freq: usize,
}
