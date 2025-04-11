//! Module for loading ELF files.

use alloc::{string::String, sync::Arc, vec::Vec};

use config::mm::{USER_STACK_LOWER, USER_STACK_UPPER};
use elf::{self, ElfStream, ParseError as ElfParseError, endian::LittleEndian, file::FileHeader};
use mm::address::VirtAddr;
use systype::{SysError, SysResult};
use vfs::file::File;

use crate::vm::user_ptr::UserWritePtr;

use super::{
    addr_space::AddrSpace,
    mem_perm::MemPerm,
    vm_area::{VmArea, VmaFlags},
};

impl AddrSpace {
    /// Loads an ELF executable into given address space.
    ///
    /// `elf_file` must be a regular file.
    ///
    /// Returns the entry point of the ELF executable.
    ///
    /// # Errors
    /// Returns an error if the loading fails. This can happen if the file is not a valid
    /// ELF file.
    ///
    /// # Discussion
    /// Current implementation of this function takes a slice of ELF data as input, which
    /// requires the whole ELF file to be loaded into memory before calling this function.
    /// This is because we have not implemented the file system yet. In the future, this
    /// function should take a file descriptor as input.
    pub fn load_elf(&mut self, elf_file: Arc<dyn File>) -> SysResult<VirtAddr> {
        let elf: ElfStream<LittleEndian, _> =
            ElfStream::open_stream(elf_file.as_ref()).map_err(|e| match e {
                ElfParseError::IOError(_) => SysError::EIO,
                _ => SysError::ENOEXEC,
            })?;

        // Do minimal checks on the ELF file header.
        let FileHeader {
            class,
            e_entry,
            e_type,
            ..
        } = elf.ehdr;
        if class != elf::file::Class::ELF64 {
            return Err(SysError::ENOEXEC);
        }
        // Note: Dynamic executables are not actually supported yet.
        if !(e_type == elf::abi::ET_EXEC || e_type == elf::abi::ET_DYN) {
            return Err(SysError::ENOEXEC);
        }
        if e_entry == 0 {
            return Err(SysError::ENOEXEC);
        }

        // Map VMAs for each loadable segment.
        for segment in elf
            .segments()
            .iter()
            .filter(|seg| seg.p_type == elf::abi::PT_LOAD)
        {
            let va_start = VirtAddr::new(segment.p_vaddr as usize);
            let va_end = VirtAddr::new((segment.p_vaddr + segment.p_memsz) as usize);

            let offset = segment.p_offset as usize;

            let flags = segment.p_flags;
            let mut prot = MemPerm::U;
            if flags & elf::abi::PF_X != 0 {
                prot |= MemPerm::X;
            }
            if flags & elf::abi::PF_W != 0 {
                prot |= MemPerm::W;
            }
            if flags & elf::abi::PF_R != 0 {
                prot |= MemPerm::R;
            }

            let area = VmArea::new_file_backed(
                va_start,
                va_end,
                VmaFlags::PRIVATE,
                prot,
                Arc::clone(&elf_file),
                offset,
                segment.p_filesz as usize,
            );
            self.add_area(area)?;
        }

        Ok(VirtAddr::new(e_entry as usize))
    }

    /// Maps a stack into the address space.
    ///
    /// Returns the address of the stack bottom, i.e., one byte exceeding the highest address of
    /// the stack.
    ///
    /// Current implementation hardcodes the stack size and position in [`config::mm`] module.
    pub fn map_stack(&mut self) -> SysResult<VirtAddr> {
        let stack = VmArea::new_stack(
            VirtAddr::new(USER_STACK_LOWER),
            VirtAddr::new(USER_STACK_UPPER),
        );
        let stack_bottom = stack.end_va();
        self.add_area(stack)?;
        Ok(stack_bottom)
    }

    /// Initializes user stack
    ///
    /// Pushes `envp-str`, `argv-str`, `platform-info`, `rand-bytes`, `envp-str-ptr`
    /// , `argv-str-ptr`, `argc` into the stack from top to bottom.
    ///
    /// Returns (`ptr->argc`, `argc`, `ptr->argv-str-ptr`, `ptr->envp-str-ptr`)
    ///
    /// # Attention
    /// the stack-top parameter should be aligned as the lowest bit is 0
    pub fn init_stack(
        &mut self,
        mut sp: usize,
        argc: usize,
        argv: Vec<String>,
        envp: Vec<String>,
    ) -> (usize, usize, usize, usize) {
        // log::info!("sp {:#x}", sp);
        debug_assert!(sp & 0xf == 0);

        let mut push_str = |sp: &mut usize, s: &str| -> usize {
            let len = s.len();
            *sp -= len + 1;
            unsafe {
                for (i, c) in s.bytes().enumerate() {
                    UserWritePtr::<u8>::new(*sp + i, self)
                        .write(c)
                        .expect("fail to write str in stack");
                }
                UserWritePtr::<u8>::new(*sp + len, self)
                    .write(0u8)
                    .expect("fail to write str-end in stack");
            }
            *sp
        };

        let env_ptrs: Vec<usize> = envp.iter().rev().map(|s| push_str(&mut sp, s)).collect();
        let arg_ptrs: Vec<usize> = argv.iter().rev().map(|s| push_str(&mut sp, s)).collect();

        let rand_size = 0;
        let platform = "RISC-V64";
        let rand_bytes = "Moon rises 9527";

        sp -= rand_size;
        push_str(&mut sp, platform);
        push_str(&mut sp, rand_bytes);

        sp = (sp - 1) & !0xf;

        let mut push_usize = |sp: &mut usize, ptr: usize| {
            *sp -= core::mem::size_of::<usize>();
            unsafe {
                UserWritePtr::<usize>::new(*sp, self)
                    .write(ptr)
                    .expect("fail to write usize in stack");
            }
        };

        push_usize(&mut sp, 0);
        env_ptrs.iter().for_each(|ptr| push_usize(&mut sp, *ptr));
        let env_ptr_ptr = sp;

        push_usize(&mut sp, 0);
        arg_ptrs.iter().for_each(|ptr| push_usize(&mut sp, *ptr));
        let arg_ptr_ptr = sp;

        push_usize(&mut sp, argc);

        (sp, argc, arg_ptr_ptr, env_ptr_ptr)
    }

    /// Maps a heap into the address space.
    pub fn map_heap(&mut self) -> SysResult<()> {
        let length = 1 << 20; // 1 MiB
        let start = self
            .find_vacant_memory(VirtAddr::new(0), length)
            .ok_or(SysError::ENOMEM)?;
        let heap = VmArea::new_heap(start, VirtAddr::new(start.to_usize() + length));
        self.add_area(heap).unwrap();
        Ok(())
    }
}
