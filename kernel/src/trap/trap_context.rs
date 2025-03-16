use arch::riscv64::{
    interrupt::disable_interrupt,
    sstatus::{self, Sstatus},
};
use core::arch::asm;
use riscv::register::sstatus::{FS, SPP};

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TrapContext {
    pub user_reg: [usize; 32], // 0-31

    pub sstatus: Sstatus, // 32
    pub sepc: usize,      // 33
    pub stvec: usize,
    pub stval: usize,

    pub k_sp: usize,      // 34
    pub k_ra: usize,      // 35
    pub k_s: [usize; 12], // 36 - 47
    pub k_fp: usize,      // 48
    pub k_tp: usize,      // 49

    pub user_fx: [f64; 32], // 50 - 81

    /// 这是RISC-V浮点控制和状态寄存器(Floating-point Control and Status Register)。
    /// 它用于控制浮点运算的行为和存储浮点运算的状态标志，比如舍入模式、异常标志等。
    pub fcsr: u32, // 32bit
    /// 当浮点状态为"脏"(Dirty)时，即浮点寄存器的内容被修改过，
    /// 这个标志位会被设置为1，表明需要保存浮点寄存器的内容。
    pub is_dirty: u8,
    /// 当任务切换或者从信号处理返回时，如果之前保存了浮点寄存器的状态，
    /// 这个标志位会被设置为1，表明需要恢复浮点寄存器的内容。
    pub is_need_restore: u8,
    /// 当处理信号时，如果浮点状态为脏，
    /// 这个标志位会被设置，以确保正确保存和恢复浮点状态。
    pub is_signal_dirty: u8,
}

impl TrapContext {
    /// 初始化用户态trap context
    pub fn new(entry: usize, sp: usize) -> Self {
        disable_interrupt();
        // disable Interrupt until trap handling
        let mut sstatus = sstatus::read();
        sstatus.set_sie(false);
        sstatus.set_spie(false);
        // switch to User priviledge after handling
        sstatus.set_spp(SPP::User);

        let mut context = Self {
            user_reg: [0; 32],
            sstatus,
            sepc: entry,
            stvec: 0,
            stval: 0,
            k_sp: 0,
            k_ra: 0,
            k_s: [0; 12],
            k_fp: 0,
            k_tp: 0,
            user_fx: [0.0; 32],
            fcsr: 0,
            is_dirty: 0,
            is_need_restore: 0,
            is_signal_dirty: 0,
        };

        context.set_user_sp(sp);
        context
    }

    /// 初始化用户态trap context
    pub fn init_user(
        &mut self,
        user_sp: usize,
        sepc: usize,
        argc: usize,
        argv: usize,
        envp: usize,
    ) {
        self.user_reg[2] = user_sp;
        self.user_reg[10] = argc;
        self.user_reg[11] = argv;
        self.user_reg[12] = envp;
        self.sepc = sepc;
        self.sstatus = sstatus::read();
    }

    pub fn yield_task(&mut self) {
        self.save_fx();
        self.is_need_restore = 1;
    }

    pub fn restore_fx(&mut self) {
        if self.is_need_restore == 0 {
            return;
        }
        self.is_need_restore = 0;
        let ptr = unsafe { self.user_fx.as_mut_ptr() };
        unsafe {
            macro_rules! load_regs {
                ($($i:literal),*) => {
                    $(
                        asm!(
                            "fld f{}, {offset}*8({ptr})",
                            const $i,
                            offset = const $i,
                            ptr = in(reg) ptr
                        );
                    )*
                };
            }
            load_regs!(
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31
            );

            asm!(
                "lw  {0}, 32*8({0})
                csrw fcsr, {0}",
                in(reg) ptr,
            );
        }
    }

    pub fn save_fx(&mut self) {
        if self.is_dirty == 0 {
            return;
        }
        self.is_dirty = 0;
        let ptr = unsafe { self.user_fx.as_mut_ptr() };
        unsafe {
            let mut _t: usize = 1;
            macro_rules! save_regs {
                ($($i:literal),*) => {
                    $(
                        asm!(
                            "fsd f{}, {offset}*8({ptr})",
                            const $i,
                            offset = const $i,
                            ptr = in(reg) ptr
                        );
                    )*
                };
            }
            save_regs!(
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31
            );

            asm!(
                "csrr {1}, fcsr
                sw  {1}, 32*8({0})
                ",
                in(reg) ptr,
                inout(reg) _t
            );
        };
    }

    pub fn syscall_no(&self) -> usize {
        // a7 == x17
        self.user_reg[17]
    }

    pub fn syscall_args(&self) -> [usize; 6] {
        [
            self.user_reg[10],
            self.user_reg[11],
            self.user_reg[12],
            self.user_reg[13],
            self.user_reg[14],
            self.user_reg[15],
        ]
    }

    pub fn set_user_sp(&mut self, sp: usize) {
        // sp == x2
        self.user_reg[2] = sp;
    }

    pub fn set_user_a0(&mut self, val: usize) {
        // a0 == x10
        self.user_reg[10] = val;
    }

    /// 设置用户态trap pc
    pub fn set_entry_point(&mut self, entry: usize) {
        self.sepc = entry;
    }

    /// pc 指向下一条指令
    pub fn sepc_forward(&mut self) {
        self.sepc += 4;
    }

    pub fn mark_dirty(&mut self, sstatus: Sstatus) {
        self.is_dirty |= (sstatus.fs() == FS::Dirty) as u8;
    }
}
