use arch::hart::hart_start;
use config::device::MAX_HARTS;
use config::mm::HART_START_ADDR;

/// 启动子HART
///
/// 启动其他HART，并打印启动状态
pub fn start_harts(hart_id: usize) {
    for i in 0..MAX_HARTS {
        if i == hart_id {
            continue;
        }
        hart_start(i, HART_START_ADDR);
    }
}
