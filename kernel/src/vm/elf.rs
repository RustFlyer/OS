//! Module for loading ELF files.

use alloc::{ffi::CString, string::String, sync::Arc, vec::Vec};

use aux::*;
use config::{
    mm::{USER_END, USER_INTERP_BASE, USER_STACK_LOWER, USER_STACK_UPPER},
    vfs::SeekFrom,
};
use elf::{self, ElfStream, ParseError as ElfParseError, endian::LittleEndian, file::FileHeader};
use mm::address::VirtAddr;
use osfuture::block_on;
use systype::{SysError, SysResult};
use vfs::{file::File, path::Path, sys_root_dentry};

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
    /// Returns the entry point of the ELF executable, and a partial auxiliary vector of
    /// the ELF executable.
    ///
    /// The returned auxiliary vector is not null-terminated, and the caller may add
    /// additional entries to it.
    ///
    /// The returned auxiliary vector contains the following entries:
    ///
    /// - AT_PHENT: size of program header entry
    /// - AT_PHNUM: number of program headers
    /// - AT_PHDR: address of program headers
    /// - AT_ENTRY: entry point of the user program
    /// - AT_BASE: base address of the dynamic linker (if loaded)
    ///
    /// and all entries that are initialized in [`construct_init_auxv`].
    ///
    /// # Errors
    /// Returns an error if the loading fails. This can happen if the file is not a valid
    /// ELF file.
    pub fn load_elf(&mut self, elf_file: Arc<dyn File>) -> SysResult<(VirtAddr, Vec<AuxHeader>)> {
        let elf_stream: ElfStream<LittleEndian, _> = ElfStream::open_stream(elf_file.as_ref())
            .map_err(|e| match e {
                ElfParseError::IOError(_) => SysError::EIO,
                _ => SysError::ENOEXEC,
            })?;

        let mut auxv = aux::construct_init_auxv();
        auxv.push(AuxHeader::new(
            AT_PHENT,
            elf_stream.ehdr.e_phentsize as usize,
        ));
        auxv.push(AuxHeader::new(AT_PHNUM, elf_stream.ehdr.e_phnum as usize));

        let first_segment_addr = elf_stream
            .segments()
            .iter()
            .filter(|phdr| phdr.p_type == elf::abi::PT_LOAD)
            .map(|phdr| phdr.p_vaddr as usize)
            .min()
            .ok_or(SysError::ENOEXEC)?;
        auxv.push(AuxHeader::new(
            AT_PHDR,
            first_segment_addr + elf_stream.ehdr.e_phoff as usize,
        ));

        // Load loadable segments (PT_LOAD).
        let mut entry = self.load_segments(Arc::clone(&elf_file), &elf_stream, 0)?;
        auxv.push(AuxHeader::new(AT_ENTRY, entry.to_usize()));

        // Load the dynamic linker if needed.
        let interp = {
            let mut interp_iter = elf_stream
                .segments()
                .iter()
                .filter(|phdr| phdr.p_type == elf::abi::PT_INTERP);
            if let Some(interp) = interp_iter.next() {
                if interp_iter.next().is_some() {
                    // Multiple PT_INTERP segments are not allowed.
                    return Err(SysError::EINVAL);
                }
                Some(interp)
            } else {
                None
            }
        };
        if let Some(interp) = interp {
            // Load the dynamic linker.
            let interp_name = {
                let offset = interp.p_offset as usize;
                let len = interp.p_filesz as usize;
                let mut buf = vec![0u8; len];
                elf_file.seek(SeekFrom::Start(offset as u64))?;
                block_on(async { elf_file.read(&mut buf).await })?;
                CString::from_vec_with_nul(buf)
                    .map_err(|_| SysError::ENOENT)?
                    .into_string()
                    .map_err(|_| SysError::ENOENT)?
            };
            let interp_file = {
                let dentry = Path::new(sys_root_dentry(), interp_name).walk()?;
                <dyn File>::open(dentry)?
            };
            let interp_stream: ElfStream<LittleEndian, _> =
                ElfStream::open_stream(interp_file.as_ref()).map_err(|e| match e {
                    ElfParseError::IOError(_) => SysError::EIO,
                    _ => SysError::ENOEXEC,
                })?;
            entry =
                self.load_segments(Arc::clone(&interp_file), &interp_stream, USER_INTERP_BASE)?;
            auxv.push(AuxHeader::new(AT_BASE, USER_INTERP_BASE));
        }

        Ok((entry, auxv))
    }

    /// Loads loadable segments (PT_LOAD) from an executable ELF file into the address
    /// space.
    ///
    /// `elf_file` is a [`File`] that points to the ELF file.
    /// `elf_stream` is an [`ElfStream`] that is associated with the ELF file.
    /// `base_offset` is an offset to be added to the virtual address of each segment.
    ///
    /// Returns the entry point of the program.
    ///
    /// # Errors
    /// Returns an error if the loading fails. This can happen if the file is not a valid
    /// ELF file.
    fn load_segments(
        &mut self,
        elf_file: Arc<dyn File>,
        elf_stream: &ElfStream<LittleEndian, &dyn File>,
        base_offset: usize,
    ) -> SysResult<VirtAddr> {
        // Do some checks on the ELF file header.
        let FileHeader {
            class,
            e_entry,
            e_type,
            ..
        } = elf_stream.ehdr;
        if class != elf::file::Class::ELF64 {
            return Err(SysError::ENOEXEC);
        }
        if !(e_type == elf::abi::ET_EXEC || e_type == elf::abi::ET_DYN) {
            return Err(SysError::ENOEXEC);
        }
        if e_entry == 0 {
            return Err(SysError::ENOEXEC);
        }

        // Map each loadable segment as a file-backed memory area.
        for segment in elf_stream
            .segments()
            .iter()
            .filter(|seg| seg.p_type == elf::abi::PT_LOAD)
        {
            let va_start = base_offset + segment.p_vaddr as usize;
            let va_end = base_offset + (segment.p_vaddr + segment.p_memsz) as usize;
            if !VirtAddr::check_validity(va_start) || !VirtAddr::check_validity(va_end) {
                return Err(SysError::ENOMEM);
            }
            let va_start = VirtAddr::new(va_start);
            let va_end = VirtAddr::new(va_end);

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

        let entry = base_offset + e_entry as usize;
        if !VirtAddr::check_validity(entry) {
            return Err(SysError::ENOMEM);
        }
        Ok(VirtAddr::new(entry))
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

    /// Initializes the user stack.
    ///
    /// This function initializes the user stack with the given arguments, environment
    /// variables, auxiliary vector, and other necessary data.
    ///
    /// The stack pointer is aligned to 16 bytes.
    ///
    /// `sp` is the stack pointer, which should points to the upper bound of the user
    /// stack.
    /// `args` is the vector of command line arguments.
    /// `envs` is the vector of environment variables.
    /// `auxv` is the vector of auxiliary headers.
    ///
    /// All of the above vectors should not include the null terminator. The function
    /// will add the null terminator to each of them.
    ///
    /// Returns the new stack pointer (which points to the stack top), the number of
    /// command line arguments, the pointer to the array of command line arguments, and
    /// the pointer to the array of environment variables.
    pub fn init_stack(
        &mut self,
        mut sp: usize,
        args: Vec<String>,
        envs: Vec<String>,
        auxv: Vec<AuxHeader>,
    ) -> SysResult<(usize, usize, usize, usize)> {
        let argc = args.len();

        // Align the stack pointer to 16 bytes.
        sp &= !0xf;

        // Helper closure to push a string to the stack.
        // Updates the stack pointer and returns the new stack pointer.
        let mut push_str = |string: String| -> SysResult<usize> {
            let string = CString::new(string).unwrap();
            let bytes = string.as_bytes_with_nul();
            sp -= bytes.len();
            unsafe {
                UserWritePtr::<u8>::new(sp, self).write_array(bytes)?;
            }
            Ok(sp)
        };

        // Push the environment variables and command line arguments to the stack,
        // and get pointers to them.
        let mut env_ptrs: Vec<usize> = Vec::with_capacity(envs.len() + 1);
        env_ptrs.push(0);
        for env in envs.into_iter().rev() {
            env_ptrs.push(push_str(env)?);
        }

        let mut arg_ptrs: Vec<usize> = Vec::with_capacity(args.len() + 1);
        arg_ptrs.push(0);
        for arg in args.into_iter().rev() {
            arg_ptrs.push(push_str(arg)?);
        }

        sp &= !0xf;

        // Helper closure to push an auxiliary header to the stack.
        // Updates the stack pointer and returns the new stack pointer.
        let mut push_aux = |aux: AuxHeader| -> SysResult<usize> {
            sp -= core::mem::size_of::<AuxHeader>();
            unsafe { UserWritePtr::<AuxHeader>::new(sp, self).write(aux)? }
            Ok(sp)
        };

        // Push the auxiliary vector to the stack.
        let null_aux = AuxHeader::new(aux::AT_NULL, 0);
        push_aux(null_aux)?;
        for aux in auxv.into_iter().rev() {
            push_aux(aux)?;
        }

        // Helper closure to push a `usize` to the stack.
        // Updates the stack pointer and returns the new stack pointer.
        let mut push_usize = |ptr: usize| -> SysResult<usize> {
            sp -= core::mem::size_of::<usize>();
            unsafe { UserWritePtr::<usize>::new(sp, self).write(ptr)? }
            Ok(sp)
        };

        // Push pointers to the environment variables and command line arguments to the stack.
        let mut env_ptr_ptr = 0;
        for ptr in env_ptrs {
            env_ptr_ptr = push_usize(ptr)?;
        }

        let mut arg_ptr_ptr = 0;
        for ptr in arg_ptrs {
            arg_ptr_ptr = push_usize(ptr)?;
        }

        // Push `argc` to the stack.
        push_usize(argc)?;

        Ok((sp, argc, arg_ptr_ptr, env_ptr_ptr))
    }

    /// Maps a heap into the address space.
    pub fn map_heap(&mut self) -> SysResult<()> {
        let length = 1 << 20; // 1 MiB
        let start = self
            .find_vacant_memory(
                VirtAddr::new(0),
                length,
                VirtAddr::new(0),
                VirtAddr::new(USER_END),
            )
            .ok_or(SysError::ENOMEM)?;
        let heap = VmArea::new_heap(start, VirtAddr::new(start.to_usize() + length));
        log::warn!("[map_heap] heap: [{heap:?}]");
        log::warn!("[map_heap] heap start: [{:#x}]", heap.start_va().to_usize());
        log::warn!("[map_heap] heap end: [{:#x}]", heap.end_va().to_usize());
        self.add_area(heap).unwrap();
        Ok(())
    }
}

pub mod aux {
    //! Module for auxiliary headers.
    //!
    //! This module defines the structure of auxiliary headers used in ELF files.
    //! The auxiliary headers are used to pass information from the kernel to the
    //! user program during the loading of an ELF executable. They are typically
    //! stored in the user stack and are used by the dynamic linker to initialize
    //! the program's environment.

    use alloc::vec::Vec;
    use config::mm::PAGE_SIZE;

    /// Auxiliary header for the ELF file.
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct AuxHeader {
        pub a_type: usize,
        pub a_val: usize,
    }

    impl AuxHeader {
        /// Creates a new auxiliary header.
        pub fn new(a_type: usize, a_val: usize) -> Self {
            Self { a_type, a_val }
        }
    }

    /// Constructs an initial auxiliary vector which is to be further expanded before
    /// passing to the user program.
    pub fn construct_init_auxv() -> Vec<AuxHeader> {
        let mut auxv = Vec::with_capacity(32);
        auxv.push(AuxHeader::new(AT_PAGESZ, PAGE_SIZE));
        auxv.push(AuxHeader::new(AT_FLAGS, 0));
        auxv.push(AuxHeader::new(AT_UID, 0));
        auxv.push(AuxHeader::new(AT_EUID, 0));
        auxv.push(AuxHeader::new(AT_GID, 0));
        auxv.push(AuxHeader::new(AT_EGID, 0));
        auxv.push(AuxHeader::new(AT_PLATFORM, 0));
        auxv.push(AuxHeader::new(AT_HWCAP, 0));
        auxv.push(AuxHeader::new(AT_CLKTCK, 100));
        auxv.push(AuxHeader::new(AT_SECURE, 0));
        auxv
    }

    /// Entry should be ignored
    pub const AT_IGNORE: usize = 1;
    /// File descriptor of program
    pub const AT_EXECFD: usize = 2;
    /// End of vector
    pub const AT_NULL: usize = 0;
    /// Program headers for program
    pub const AT_PHDR: usize = 3;
    /// Size of program header entry
    pub const AT_PHENT: usize = 4;
    /// Number of program headers
    pub const AT_PHNUM: usize = 5;
    /// System page size
    pub const AT_PAGESZ: usize = 6;
    /// Base address of interpreter
    pub const AT_BASE: usize = 7;
    /// Flags
    pub const AT_FLAGS: usize = 8;
    /// Entry point of program
    pub const AT_ENTRY: usize = 9;
    /// Program is not ELF
    pub const AT_NOTELF: usize = 10;
    /// Real uid
    pub const AT_UID: usize = 11;
    /// Effective uid
    pub const AT_EUID: usize = 12;
    /// Real gid
    pub const AT_GID: usize = 13;
    /// Effective gid
    pub const AT_EGID: usize = 14;
    /// String identifying CPU for optimizations
    pub const AT_PLATFORM: usize = 15;
    /// Arch dependent hints at CPU capabilities
    pub const AT_HWCAP: usize = 16;
    /// Frequency at which times() increments
    pub const AT_CLKTCK: usize = 17;
    /// Secure mode boolean
    pub const AT_SECURE: usize = 23;
    /// string identifying real platform, may differ from AT_PLATFORM.
    pub const AT_BASE_PLATFORM: usize = 24;
    /// Address of 16 random bytes
    pub const AT_RANDOM: usize = 25;
    /// Extension of AT_HWCAP
    pub const AT_HWCAP2: usize = 26;
    /// Filename of program
    pub const AT_EXECFN: usize = 31;
}
