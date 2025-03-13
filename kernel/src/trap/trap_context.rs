use core::arch::asm;
use riscv::register::sstatus::{FS, SPP};
use crate::riscv64::sstatus::Sstatus;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TrapContext {
    pub user_reg: [usize; 32],
    pub sstatus: Sstatus,
    pub sepc: usize,
}

impl TrapContext {
    /// Create a new TrapContext for a new Task.
    pub fn new(location: usize, sp:usize) -> Self {
        // disable Interrupt until trap handling
        let mut sstatus = sstatus::read();
        sstatus.set_sie(false);
        sstatus.set_spie(false);
        // switch to User priviledge after handling
        sstatus.set_spp(SPP::User);

        let mut context = Self {
            user_reg: [0; 32],
            sstatus,
            sepc: location,
        };

        context.set_user_sp(sp);
        context
    }

    /// Build TrapContext for loading an existed Task.
    pub fn init_user(
        &mut self,
        user_sp: usize,
        sepc: usize,
        argc: usize,
        argv: usize,
        envp: usize,
    ) {
        self.user_x[2] = user_sp;
        self.user_x[10] = argc;
        self.user_x[11] = argv;
        self.user_x[12] = envp;
        self.sepc = sepc;
        self.sstatus = sstatus::read();
        self.user_fx = UserFloatContext::new()
    }

    /// Syscall number
    pub fn syscall_no(&self) -> usize {
        // a7 == x17
        self.user_x[17]
    }

    pub fn syscall_args(&self) -> [usize; 6] {
        [
            self.user_x[10],
            self.user_x[11],
            self.user_x[12],
            self.user_x[13],
            self.user_x[14],
            self.user_x[15],
        ]
    }

    pub fn set_user_sp(&mut self, sp: usize) {
        // sp == x2
        self.user_x[2] = sp;
    }

    pub fn set_user_a0(&mut self, val: usize) {
        // a0 == x10
        self.user_x[10] = val;
    }

    /// Set user trap pc location
    pub fn set_entry_point(&mut self, entry: usize) {
        self.sepc = entry;
    }

    /// Move to next instructment before trap return
    pub fn set_user_pc_to_next(&mut self) {
        self.sepc += 4;
    }
}