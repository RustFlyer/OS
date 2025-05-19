use core::{arch::asm, fmt::Debug};

use arch::riscv64::{
    interrupt::disable_interrupt,
    sstatus::{self, Sstatus},
};
use riscv::register::sstatus::{FS, SPP};

/// when sp points to user stack of a task/process,
/// sscratch(in RISCV) points to the start
/// of the TrapContext of this task/process in user address space,
/// until they switch when __trap_from_user, and the context begin to be saved
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TrapContext {
    // integer registers and CSR to be saved when trap from user to kernel
    pub user_reg: [usize; 32], // 0-31, general register

    pub sstatus: Sstatus, // 32, controls previlege level. its SIE part enables interrupt

    pub sepc: usize, // 33, the instruction that occurs trap (or the next instruction when trap returns)

    // callee-saved registers and constant addresses that guide trap into kernel space,
    // seted when kernel return to user
    pub k_sp: usize, // 34, kernel stack top of this process

    pub k_ra: usize, // 35, kernel return address, the __return_to_user instruction fn trap_return(task), and then the fn task_executor_unit(task)

    pub k_s: [usize; 12], // 36 - 47 callee-saved registers

    pub k_fp: usize, // 48, kernel stack frame of this process

    pub k_tp: usize, // 49, thread pointer, the kernel hart(which records CPU status) address, useless for now

    // float register to be saved, useless for now
    pub user_fx: [f64; 32], // 50 - 81

    // This is RISC-V Floating-point Control and Status Register
    // It can control the behaviour about floating-point number operation
    pub fcsr: u32,

    // This bit mark whether floating point number is dirty.
    // When it is marked as 1, it means that float reg has been modified.
    pub is_dirty: u8,

    // When task switch or application returns from sig-handle,
    // this bit will be marked as 1, meaning that float reg needs to be restored.
    pub is_need_restore: u8,

    // when handle signals with a dirty float mode,
    // this bit is marked as 1 to ensure that floating state is set correctly.
    pub is_signal_dirty: u8,

    pub last_a0: usize,
}

impl TrapContext {
    /// Initializes user trap context
    ///
    /// The application control stream starts from kernel space. Therefore,
    /// it's important to set correct trap context to guarantee that appication
    /// can return to user space from kernel space.
    ///
    /// Here `TrapContext` can catch application entry in the user memory space
    /// and store user stack top in sp, which ensure user application can have
    /// enough stack space to run.
    ///
    /// Due to a task spawned without any args passed in, `new()` function does
    /// not provide any argv_ptr or envp_ptr here.
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
            last_a0: 0,
        };

        context.set_user_sp(sp);
        context
    }

    /// initializes user trap context
    ///
    /// unlike `new()`, this function is called in the `execve()`. Therefore,
    /// argv, envp and argc are provided by caller thread. Also, this thread
    /// will have a new user stack and pass it into `init_user()` to initializes
    /// it.
    ///
    /// Then `init_user()` call `clear_fx()` to clean its float environment and
    /// initializes float regs for `execve()` function.
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

        // self.sstatus = sstatus::read();
        self.clear_fx();
    }

    /// clear float regs and set relevant privilege regs.
    pub fn clear_fx(&mut self) {
        self.user_fx.fill(0.0);
        self.fcsr = 0;
        self.is_dirty = 0;
        self.is_need_restore = 0;
        self.is_signal_dirty = 0;
    }

    /// this function can be called to get syscall number
    /// when trapped
    pub fn syscall_no(&self) -> usize {
        // a7 == x17
        self.user_reg[17]
    }

    /// this function can be called to get syscall args
    /// when trapped
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

    pub fn set_user_tp(&mut self, val: usize) {
        // tp == x4
        self.user_reg[4] = val;
    }

    /// set entry to user space.
    /// when the application temps to return from trap_return, it will
    /// step in `entry` address in user space.
    pub fn set_entry_point(&mut self, entry: usize) {
        self.sepc = entry;
    }

    /// pc points to the next instruction.
    pub fn sepc_forward(&mut self) {
        self.sepc += 4;
    }

    pub fn mark_dirty(&mut self, sstatus: Sstatus) {
        self.is_dirty |= (sstatus.fs() == FS::Dirty) as u8;
    }

    //XXX: save register when yield, but not checked yet.
    pub fn yield_task(&mut self) {
        self.save_fx();
        self.is_need_restore = 1;
    }

    pub fn restore_fx(&mut self) {
        if self.is_need_restore == 0 {
            return;
        }
        self.is_need_restore = 0;
        let ptr = self.user_fx.as_mut_ptr();
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
        let ptr = self.user_fx.as_mut_ptr();
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

    pub fn save_last_user_a0(&mut self) {
        self.last_a0 = self.user_reg[10];
    }

    pub fn restore_last_user_a0(&mut self) {
        self.user_reg[10] = self.last_a0;
    }

    pub fn display(&self) {
        log::info!("================TrapContext================");
        log::info!("sepc    : {:#x}", self.sepc);
        log::info!("sstatus : {:#x}", self.sstatus.bits());
        log::info!("sum     : {}", (self.sstatus.bits() & 1 << 8) > 0);
        log::info!("user_reg: {:#?}", self.user_reg);
        log::info!("================    End    ================");
    }
}
