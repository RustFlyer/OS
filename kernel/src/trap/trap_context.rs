#[cfg(target_arch = "loongarch64")]
use loongArch64::register::{CpuMode, prmd};
#[cfg(target_arch = "riscv64")]
use riscv::register::sstatus::{self, SPP, Sstatus};

use arch::trap::disable_interrupt;

/// when sp points to user stack of a task/process,
/// sscratch(in RISCV) points to the start
/// of the TrapContext of this task/process in user address space,
/// until they switch when __trap_from_user, and the context begin to be saved
#[derive(Clone, Copy)]
#[repr(C)]
pub struct TrapContext {
    // integer registers and CSR to be saved when trap from user to kernel
    // Note: It's for both RISCV and LoongArch, but only use RISCV's register name for convenience
    pub user_reg: [usize; 32], // 0-31, general register

    #[cfg(target_arch = "riscv64")]
    pub sstatus: Sstatus, // 32, controls previlege level. seen as PRMD in LoongArch

    #[cfg(target_arch = "loongarch64")]
    pub sstatus: usize, // Prmd structure doesn't impl debug trait, use usize instead

    pub sepc: usize, // 33, the instruction that occurs trap (or the next instruction when trap returns)
    // aka. era(0x6) in LoongArch

    // callee-saved registers and constant addresses that guide trap into kernel space,
    // seted when kernel return to user
    pub k_sp: usize, // 34, kernel stack top of this process

    pub k_ra: usize, // 35, kernel return address, the __return_to_user instruction fn trap_return(task), and then the fn task_executor_unit(task)

    pub k_s: [usize; 12], // 36 - 47 callee-saved registers

    pub k_fp: usize, // 48, kernel stack frame of this process

    pub k_tp: usize, // 49, thread pointer, the kernel hart(which records CPU status) address, useless for now

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

        #[cfg(target_arch = "riscv64")]
        let mut status = sstatus::read();
        #[cfg(target_arch = "riscv64")]
        {
            status.set_sie(false);
            status.set_spie(false);
            // switch to User priviledge after handling
            status.set_spp(SPP::User);
        }

        #[cfg(target_arch = "loongarch64")]
        let mut status = prmd::read().raw(); // Prmd
        #[cfg(target_arch = "loongarch64")]
        {
            // prmd.set_pie(false);
            // TODO: set pplv to ring3, but it seems useless and dangerous and unimplemented
            // status.set_pplv(CpuMode::Ring3);
            status = status | 4;
        }

        let mut context = Self {
            user_reg: [0; 32],
            sstatus: status,
            sepc: entry,
            k_sp: 0,
            k_ra: 0,
            k_s: [0; 12],
            k_fp: 0,
            k_tp: 0,
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
        entry: usize,
        argc: usize,
        argv: usize,
        envp: usize,
    ) {
        #[cfg(target_arch = "riscv64")]
        {
            self.user_reg[2] = user_sp;
            self.user_reg[10] = argc;
            self.user_reg[11] = argv;
            self.user_reg[12] = envp;
            self.sepc = entry;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[3] = user_sp; // sp在loongArch中存放在3*8
            self.user_reg[4] = argc; // a0存放在4*8
            self.user_reg[5] = argv; // a1存放在5*8
            self.user_reg[6] = envp; // a2存放在6*8
            self.sepc = entry;
        }
    }

    /// this function can be called to get syscall number
    /// when trapped
    pub fn syscall_no(&self) -> usize {
        #[cfg(target_arch = "riscv64")]
        {
            // a7 == x17
            self.user_reg[17]
        }

        #[cfg(target_arch = "loongarch64")]
        {
            // 在loongArch中，syscall_no存放在a7 (register 11)
            self.user_reg[11]
        }
    }

    /// this function can be called to get syscall args
    /// when trapped
    pub fn syscall_args(&self) -> [usize; 6] {
        #[cfg(target_arch = "riscv64")]
        {
            [
                self.user_reg[10], // a0
                self.user_reg[11], // a1
                self.user_reg[12], // a2
                self.user_reg[13], // a3
                self.user_reg[14], // a4
                self.user_reg[15], // a5
            ]
        }

        #[cfg(target_arch = "loongarch64")]
        {
            [
                self.user_reg[4], // a0
                self.user_reg[5], // a1
                self.user_reg[6], // a2
                self.user_reg[7], // a3
                self.user_reg[8], // a4
                self.user_reg[9], // a5
            ]
        }
    }

    pub fn set_user_sp(&mut self, sp: usize) {
        #[cfg(target_arch = "riscv64")]
        {
            // sp == x2
            self.user_reg[2] = sp;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            // sp在loongArch中存放在3*8
            self.user_reg[3] = sp;
        }
    }

    pub fn set_user_ret_val(&mut self, val: usize) {
        #[cfg(target_arch = "riscv64")]
        {
            // a0 == x10
            self.user_reg[10] = val;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[4] = val;
        }
    }

    pub fn set_user_tp(&mut self, val: usize) {
        #[cfg(target_arch = "riscv64")]
        {
            // tp == x4
            self.user_reg[4] = val;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            // tp存放在2*8
            self.user_reg[2] = val;
        }
    }

    pub fn set_user_a0(&mut self, val: usize) {
        #[cfg(target_arch = "riscv64")]
        {
            self.user_reg[10] = val;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[4] = val;
        }
    }

    pub fn set_user_a1(&mut self, val: usize) {
        #[cfg(target_arch = "riscv64")]
        {
            self.user_reg[11] = val;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[5] = val;
        }
    }

    pub fn set_user_a2(&mut self, val: usize) {
        #[cfg(target_arch = "riscv64")]
        {
            self.user_reg[12] = val;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[6] = val;
        }
    }

    pub fn get_user_a0(&self) -> usize {
        #[cfg(target_arch = "riscv64")]
        {
            self.user_reg[10]
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[4]
        }
    }

    pub fn get_user_sp(&self) -> usize {
        #[cfg(target_arch = "riscv64")]
        {
            self.user_reg[2]
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[3]
        }
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

    pub fn save_last_user_ret_val(&mut self) {
        #[cfg(target_arch = "riscv64")]
        {
            self.last_a0 = self.user_reg[10];
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.last_a0 = self.user_reg[4];
        }
    }

    pub fn restore_last_user_ret_val(&mut self) {
        #[cfg(target_arch = "riscv64")]
        {
            self.user_reg[10] = self.last_a0;
        }

        #[cfg(target_arch = "loongarch64")]
        {
            self.user_reg[4] = self.last_a0;
        }
    }
}
