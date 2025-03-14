pub const SBI_HART_START: (usize, usize) = (0x48534d, 0);
pub const SBI_HART_STOP: (usize, usize) = (0x48534d, 1);
pub const SBI_HART_GET_STATUS: (usize, usize) = (0x48534d, 2);
pub const SBI_HART_SUSPEND: (usize, usize) = (0x48534d, 3);

pub const SBI_SET_TIMER: (usize, usize) = (0, 0);
pub const SBI_CONSOLE_PUTCHAR: (usize, usize) = (1, 0);
pub const SBI_CONSOLE_GETCHAR: (usize, usize) = (2, 0);
pub const SBI_CLEAR_IPI: (usize, usize) = (3, 0);
pub const SBI_SEND_IPI: (usize, usize) = (4, 0);
pub const SBI_REMOTE_FENCE_I: (usize, usize) = (5, 0);
pub const SBI_REMOTE_SFENCE_VMA: (usize, usize) = (6, 0);
pub const SBI_REMOTE_SFENCE_VMA_ASID: (usize, usize) = (7, 0);
pub const SBI_SHUTDOWN: (usize, usize) = (8, 0);
