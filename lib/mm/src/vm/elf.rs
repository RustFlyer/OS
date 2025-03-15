/// Module for loading ELF files.
use elf::{self, ElfBytes, endian::LittleEndian, file::FileHeader};
use systype::{SysError, SysResult};

use crate::{
    address::VirtAddr,
    vm::{pte::PteFlags, vm_area::VmArea},
};

use super::addr_space::AddrSpace;

/// Loads an ELF executable into given address space.
///
/// Returns the entry point of the ELF executable,
///
/// # Errors
/// Returns an error if the loading fails. This can happen if the ELF file is invalid or if the
///
/// # Discussion
/// Current implementation of this function taks a slice of ELF data as input. This is because
/// we have not implemented the file system yet. In the future, this function should take a file
/// descriptor as input.
pub fn load_elf(addr_space: &mut AddrSpace, elf_data: &'static [u8]) -> SysResult<VirtAddr> {
    let elf = ElfBytes::<LittleEndian>::minimal_parse(elf_data).map_err(|_| SysError::ENOEXEC)?;

    // Check if the ELF file is valid.
    let FileHeader {
        class,
        e_entry,
        e_type,
        ..
    } = elf.ehdr;
    if class != elf::file::Class::ELF64 || e_type != elf::abi::ET_EXEC {
        return Err(SysError::ENOEXEC);
    }
    if e_entry == 0 {
        return Err(SysError::ENOEXEC);
    }

    // Adds memory-backed VMAs for each loadable segment.
    for segment in elf
        .segments()
        .ok_or(SysError::ENOEXEC)?
        .into_iter()
        .filter(|seg| seg.p_type == elf::abi::PT_LOAD)
    {
        let va_start = VirtAddr::new(segment.p_vaddr as usize);
        let va_end = VirtAddr::new((segment.p_vaddr + segment.p_memsz) as usize);

        let offset = segment.p_offset as usize;
        let memory_slice = &elf_data[offset..offset + segment.p_filesz as usize];

        let flags = segment.p_flags;
        let mut pte_flags = PteFlags::U | PteFlags::A | PteFlags::D;
        if flags & elf::abi::PF_X != 0 {
            pte_flags |= PteFlags::X;
        }
        if flags & elf::abi::PF_W != 0 {
            pte_flags |= PteFlags::W;
        }
        if flags & elf::abi::PF_R != 0 {
            pte_flags |= PteFlags::R;
        }

        let area = VmArea::new_memory_backed(va_start, va_end, pte_flags, memory_slice);
        addr_space.add_area(area)?;
    }

    Ok(VirtAddr::new(e_entry as usize))
}
