use core::arch::asm;

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

    pub stvec: usize, // address of __trap_from_user in trampoline

    pub stval: usize, // appended trap information

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

    pub fn save_last_user_a0(&mut self) {
        self.last_a0 = self.user_reg[10];
    }

    pub fn restore_last_user_a0(&mut self) {
        self.user_reg[10] = self.last_a0;
    }
}
