// This module is adapted from Phoenix OS, largely rewritten to improve
// readability and safety.

//! Module for validating and accessing addresses in user address space.
//!
//! This module provides a smart pointer type `UserPtr` that can be used to
//! access memory in user address space safely. It is used to ensure that
//! the kernel does not access memory the user process does not have access
//! to.
//!
//! Note that many functions for accessing user memory are unsafe, because
//! they require the caller to ensure that the memory holds a valid value.
//! If the value is not valid, later use of the value and dropping of the
//! value are undefined behavior.
//!
//! Note that types in this module largely bypass the Rust borrow checker
//! and memory safety guarantees. For example, a `ReadWritePtr` can be used
//! to get both a shared reference and a mutable reference, or even multiple
//! mutable references, to the same memory location. This is not safe in Rust;
//! it is the caller's responsibility to ensure that the memory is not aliased
//! in this way.

use core::{
    marker::PhantomData,
    ops::{ControlFlow, Deref, DerefMut},
    slice,
};

use alloc::vec::Vec;
use config::mm::PAGE_SIZE;
use mm::address::VirtAddr;
use systype::{SysError, SysResult};

use super::{
    addr_space::{self, AddrSpace},
    mem_perm::MemPerm,
};
use crate::{
    processor::current_hart,
    trap::trap_env::{set_kernel_stvec, set_kernel_stvec_user_rw},
};

/// Smart pointer that can be used to read memory in user address space.
pub type UserReadPtr<'a, T> = UserPtr<'a, T, ReadMarker>;
/// Smart pointer that can be used to write memory in user address space.
pub type UserWritePtr<'a, T> = UserPtr<'a, T, WriteMarker>;
/// Smart pointer that can be used to read and write memory in user address space.
pub type UserReadWritePtr<'a, T> = UserPtr<'a, T, ReadWriteMarker>;

trait AccessType {}
trait ReadAccess: AccessType {}
trait WriteAccess: AccessType {}

/// Marker for read access.
/// Do not use this type; it is public only to allow the use of `User*Ptr` types.
pub struct ReadMarker;
/// Marker for write access.
/// Do not use this type; it is public only to allow the use of `User*Ptr` types.
pub struct WriteMarker;
/// Marker for read-write access.
/// Do not use this type; it is public only to allow the use of `User*Ptr` types.
pub struct ReadWriteMarker;

impl AccessType for ReadMarker {}
impl AccessType for WriteMarker {}
impl AccessType for ReadWriteMarker {}
impl ReadAccess for ReadMarker {}
impl WriteAccess for WriteMarker {}
impl ReadAccess for ReadWriteMarker {}
impl WriteAccess for ReadWriteMarker {}

/// Base type used to implement the smart pointers for read, write, and read-write access,
/// uniformly.
///
/// Do not use this type; it is public only to allow the use of `User*Ptr` types.
#[derive(Debug)]
pub struct UserPtr<'a, T, A>
where
    A: AccessType,
{
    /// The raw pointer to the memory location.
    ///
    /// A mutable pointer is used here to allow general access to the memory location.
    /// Specific access control is enforced by the operations on the pointer defined by
    /// different blanket implementations for the access types, so this would be safe.
    ptr: *mut T,

    /// The address space the pointer is in.
    addr_space: &'a mut AddrSpace,

    /// Marker to indicate the access type of the pointer.
    _access: PhantomData<A>,
    /// Guard to ensure the `SUM` bit of `sstatus` register is set when accessing the memory.
    sum_guard: SumGuard,
}

/// Blanket implementation for general pointers.
impl<'a, T, A> UserPtr<'a, T, A>
where
    A: AccessType,
{
    /// Creates a new `UserPtr` from a virtual address.
    ///
    /// This function may construct a valid or invalid `UserPtr` depending on the
    /// address provided.
    ///
    /// A mutable reference to `AddrSpace` is needed because we may need to handle
    /// page faults when accessing the memory, which may mutate the page table.
    pub fn new(addr: usize, addr_space: &'a mut AddrSpace) -> Self {
        Self {
            ptr: addr as *mut T,
            addr_space,
            _access: PhantomData,
            sum_guard: SumGuard::new(),
        }
    }

    /// Checks if the pointer is null.
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Gets the address of the pointer.
    pub fn to_usize(&self) -> usize {
        self.ptr as usize
    }
}

/// Blanket implementation for read-access pointers.
impl<T, A> UserPtr<'_, T, A>
where
    A: ReadAccess,
{
    /// Reads a value from the memory location.
    ///
    /// This function will check if the memory location is accessible and
    /// read the value.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The value to be read must be valid.
    pub unsafe fn read(&mut self) -> SysResult<T> {
        self.addr_space
            .check_user_access(self.ptr as usize, size_of::<T>(), MemPerm::R)?;
        Ok(unsafe { self.ptr.read() })
    }

    /// Reads a value from the memory location without checking the validity
    /// of the memory location.
    ///
    /// This function is more unsafe than `read`, as it does not check the
    /// validity of the memory location. It is useful when the caller is sure
    /// that the memory location is valid.
    ///
    /// # Safety
    /// The value to be read must be valid, and the memory location must be
    /// accessible.
    pub unsafe fn read_uncheked(&mut self) -> T {
        self.ptr.read()
    }

    /// Reads an array of values from the memory location.
    ///
    /// This function will check if the memory location is accessible and
    /// read the values.
    ///
    /// `len` is the number of values to read.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The values to be read must be valid.
    pub unsafe fn read_vector(&mut self, len: usize) -> SysResult<Vec<T>> {
        self.addr_space
            .check_user_access(self.ptr as usize, len * size_of::<T>(), MemPerm::R)?;
        Ok(Vec::from_raw_parts(self.ptr, len, len))
    }

    /// Tries to convert the pointer to a reference of a value.
    ///
    /// This function will check if the memory location is accessible and try to
    /// convert the pointer to a reference of a value.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The value to be read must be valid.
    pub unsafe fn try_into_ref(&mut self) -> SysResult<&T> {
        self.addr_space
            .check_user_access(self.ptr as usize, size_of::<T>(), MemPerm::R)?;
        Ok(unsafe { &*self.ptr })
    }

    /// Tries to convert the pointer to a slice of values, with the given length.
    ///
    /// This function will check if the memory location is accessible and try to
    /// convert the pointer to a slice of values.
    ///
    /// `len` is the number of values in the slice.
    ///
    /// This function distinguish itself from `read_vector` by returning a slice
    /// in user space, instead of a vector, which may be more efficient in some cases.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The values in the slice must be valid.
    pub unsafe fn try_into_slice(&mut self, len: usize) -> SysResult<&[T]> {
        self.addr_space
            .check_user_access(self.ptr as usize, len * size_of::<T>(), MemPerm::R)?;
        Ok(unsafe { slice::from_raw_parts(self.ptr, len) })
    }
}

/// Blanket implementation for read-access pointers, whose target type is a raw pointer.
impl<T, A> UserPtr<'_, *const T, A>
where
    A: ReadAccess,
{
    /// Reads an array of pointers, null-terminated, from the memory location.
    ///
    /// This function will check if the memory location is accessible and read
    /// the pointers. It will read pointers until a null pointer is encountered
    /// or the maximum length `len` is reached.
    ///
    /// `len` is the maximum number of pointers to read.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// This function is safe, but it does not garantee that the pointers it reads
    /// are valid. The responsibility of checking the validity of the pointers lies
    /// with the caller.
    pub fn read_ptr_array(&mut self, len: usize) -> SysResult<Vec<*const T>> {
        let mut vec: Vec<*const T> = Vec::new();
        let mut push_and_check = |ptr: *const T| {
            if ptr.is_null() {
                return ControlFlow::Break(());
            }
            vec.push(ptr);
            ControlFlow::Continue(())
        };
        // SAFETY: every `*const T` is valid.
        unsafe {
            self.addr_space.check_user_access_with(
                self.ptr as usize,
                len * size_of::<*const T>(),
                MemPerm::R,
                &mut push_and_check,
            )?;
        }
        Ok(vec)
    }
}

impl<A> UserPtr<'_, u8, A>
where
    A: ReadAccess,
{
    /// Reads a C-style string (null-terminated byte array) from the memory
    /// location.
    ///
    /// This function will check if the memory location is accessible and read
    /// the string. It will read the string until a null byte is encountered
    /// or the maximum length `len` is reached.
    ///
    /// `len` is the maximum number of bytes in the resulting string.
    ///
    /// Returns the string as a vector of bytes, including the null terminator.
    /// If there is no null terminator in the first `len` bytes, the string will
    /// be truncated such that the last byte is the null terminator.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Note
    /// Pay attention that a C-style string is not necessarily a valid UTF-8
    /// [`str`] in Rust, nor can it always be represented as a [`Vec<char>`].
    pub fn read_c_string(&mut self, len: usize) -> SysResult<Vec<u8>> {
        let mut vec: Vec<u8> = Vec::new();
        let mut push_and_check = |byte: u8| {
            if byte == 0 {
                return ControlFlow::Break(());
            }
            vec.push(byte);
            ControlFlow::Continue(())
        };
        // SAFETY: every `u8` is valid.
        unsafe {
            self.addr_space.check_user_access_with(
                self.ptr as usize,
                len - 1,
                MemPerm::R,
                &mut push_and_check,
            )?;
        }
        vec.push(0);
        Ok(vec)
    }
}

/// Blanket implementation for write-access pointers.
impl<T, A> UserPtr<'_, T, A>
where
    A: WriteAccess,
{
    /// Writes a value to the memory location.
    ///
    /// This function will check if the memory location is accessible and
    /// write the value.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The value to be written must be valid.
    pub unsafe fn write(&mut self, value: T) -> SysResult<()> {
        self.addr_space
            .check_user_access(self.ptr as usize, size_of::<T>(), MemPerm::W)?;
        unsafe { self.ptr.write(value) };
        Ok(())
    }

    /// Writes a value to the memory location without checking the validity
    /// of the memory location.
    ///
    /// This function is more unsafe than `write`, as it does not check the
    /// validity of the memory location. It is useful when the caller is sure
    /// that the memory location is valid.
    ///
    /// # Safety
    /// The value to be written must be valid, and the memory location must be
    /// accessible.
    pub unsafe fn write_unchecked(&mut self, value: T) {
        self.ptr.write(value);
    }

    /// Writes an array of values to the memory location.
    ///
    /// This function will check if the memory location is accessible and
    /// write the values.
    ///
    /// `len` is the number of values to write.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The values to be written must be valid.
    pub unsafe fn write_vector(&mut self, values: &[T]) -> SysResult<()> {
        self.addr_space.check_user_access(
            self.ptr as usize,
            values.len() * size_of::<T>(),
            MemPerm::W,
        )?;
        unsafe {
            self.ptr
                .copy_from_nonoverlapping(values.as_ptr(), values.len())
        };
        Ok(())
    }

    /// Tries to convert the pointer to a mutable reference of a value.
    ///
    /// This function will check if the memory location is accessible and try to
    /// convert the pointer to a mutable reference of a value.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The value to be written must be valid.
    pub unsafe fn try_into_ref_mut(&mut self) -> SysResult<&mut T> {
        self.addr_space
            .check_user_access(self.ptr as usize, size_of::<T>(), MemPerm::W)?;
        Ok(unsafe { &mut *self.ptr })
    }

    /// Tries to convert the pointer to a mutable slice of values, with the given length.
    ///
    /// This function will check if the memory location is accessible and try to
    /// convert the pointer to a mutable slice of values.
    ///
    /// `len` is the number of values in the slice.
    ///
    /// This function distinguish itself from `write_vector` by returning a mutable slice
    /// in user space, instead of a vector, which may be more efficient in some cases.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// The values in the slice must be valid.
    pub unsafe fn try_into_slice_mut(&mut self, len: usize) -> SysResult<&mut [T]> {
        self.addr_space
            .check_user_access(self.ptr as usize, len * size_of::<T>(), MemPerm::W)?;
        Ok(unsafe { slice::from_raw_parts_mut(self.ptr, len) })
    }
}

impl AddrSpace {
    /// Checks if certain user memory access is allowed, given the starting address
    /// and length.
    ///
    /// `perm` must be `R`, `W`, or `RW`. `W` is equivalent to `RW`.
    ///
    /// `len` is the length in bytes of the memory region to be accessed.
    ///
    /// Returns `Ok(())` if the access is allowed, otherwise returns an `EFAULT` error.
    fn check_user_access(&mut self, mut addr: usize, len: usize, perm: MemPerm) -> SysResult<()> {
        if len == 0 {
            return Ok(());
        }
        if addr == 0 {
            return Err(SysError::EFAULT);
        }

        let end_addr = addr + len - 1;
        if !VirtAddr::check_validity(addr) || !VirtAddr::check_validity(end_addr) {
            return Err(SysError::EFAULT);
        }

        set_kernel_stvec_user_rw();

        let checker = if perm.contains(MemPerm::W) {
            try_write
        } else {
            try_read
        };

        while addr < end_addr {
            if unsafe { !checker(addr) } {
                // If the access failed, manually call the original page fault handler
                // to try mapping the page. If this also fails, then we know the access
                // is not allowed.
                if let Err(e) = self.handle_page_fault(VirtAddr::new(addr), perm) {
                    set_kernel_stvec();
                    return Err(e);
                }
            }
            addr += PAGE_SIZE;
        }

        set_kernel_stvec();
        Ok(())
    }

    /// Checks if certain user memory access is allowed, given the starting address,
    /// the length, and a closure which performs additional actions on primitive
    /// integer or pointer values along with the check and controls whether to stop
    /// the process early.
    ///
    /// `perm` must be `MemPerm::R`, `MemPerm::W`, or `MemPerm::R` | `MemPerm::W`.
    ///
    /// `len` is the max length in bytes of the memory region to be accessed. However,
    /// the closure may stop the process early, so the actual length may be less than
    /// `len`.
    ///
    /// `T` must be a primitive integer or pointer type, and `addr` must be aligned
    /// to the size of `T`. `len` must be a multiple of the size of `T`.
    ///
    /// The closure takes a shared reference to a `T` value on the memory region, and
    /// it should return a [`ControlFlow<()>`] value to indicate whether to stop the
    /// process early.
    ///
    /// Returns `Ok(())` if the access is allowed, otherwise returns an `EFAULT` error.
    ///
    /// # Safety
    /// The values that the closure operates on must be valid and properly aligned.
    unsafe fn check_user_access_with<F, T>(
        &mut self,
        mut addr: usize,
        len: usize,
        perm: MemPerm,
        f: &mut F,
    ) -> SysResult<()>
    where
        F: FnMut(T) -> ControlFlow<()>,
        T: Copy,
    {
        if len == 0 {
            return Ok(());
        }
        if addr == 0 {
            return Err(SysError::EFAULT);
        }

        debug_assert!(addr % size_of::<T>() == 0);
        debug_assert!(len % size_of::<T>() == 0);

        let end_addr = addr + len; // exclusive
        if !VirtAddr::check_validity(addr) || !VirtAddr::check_validity(end_addr - 1) {
            return Err(SysError::EFAULT);
        }

        set_kernel_stvec_user_rw();

        let checker = if perm.contains(MemPerm::W) {
            try_write
        } else {
            try_read
        };

        while addr < end_addr {
            if unsafe { !checker(addr) } {
                // If the access failed, manually call the original page fault handler
                // to try mapping the page. If this also fails, then we know the access
                // is not allowed.
                if let Err(e) = self.handle_page_fault(VirtAddr::new(addr), perm) {
                    set_kernel_stvec();
                    return Err(e);
                }
            }
            let end_in_page = usize::min(VirtAddr::new(addr + 1).round_up().to_usize(), end_addr);
            for item_addr in (addr..end_in_page).step_by(size_of::<T>()) {
                let item = unsafe { *(item_addr as *const T) };
                match f(item) {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(()) => {
                        set_kernel_stvec();
                        return Ok(());
                    }
                }
            }
            addr = end_in_page;
        }

        set_kernel_stvec();
        Ok(())
    }
}

/// Tries to read from a user memory region.
///
/// Returns `true` if the read is successful, otherwise `false`. A failed
/// read indicates that the memory region cannot be read from, or the address
/// is not mapped in the page table.
///
/// # Safety
/// This function must be called after calling `set_kernel_stvec_user_rw` to
/// enable kernel memory access to user space.
unsafe fn try_read(va: usize) -> bool {
    unsafe extern "C" {
        fn __try_read_user(va: usize) -> usize;
    }
    match __try_read_user(va) {
        0 => true,
        1 => false,
        _ => unreachable!(),
    }
}

/// Tries to write to a user memory region.
///
/// Returns `true` if the write is successful, otherwise `false`. A failed
/// write indicates that the memory region cannot be written to, or the address
/// is not mapped in the page table.
///
/// # Safety
/// This function must be called after calling `set_kernel_stvec_user_rw` to
/// enable kernel memory access to user space.
unsafe fn try_write(va: usize) -> bool {
    unsafe extern "C" {
        fn __try_write_user(va: usize) -> usize;
    }
    match __try_write_user(va) {
        0 => true,
        1 => false,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
struct SumGuard;

impl SumGuard {
    pub fn new() -> Self {
        current_hart().get_mut_pps().inc_sum_cnt();
        Self
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        current_hart().get_mut_pps().dec_sum_cnt();
    }
}
