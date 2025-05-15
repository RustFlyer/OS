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
//! # Safety
//!
//! Types and functions in this module largely bypass the Rust borrow checker
//! and memory safety guarantees. For example, a `ReadWritePtr` can be used
//! to get both a shared reference and a mutable reference, or even multiple
//! mutable references, to the same memory location. This is not safe in Rust;
//! it is the caller's responsibility to ensure that the memory is not aliased
//! in this way.
//!
//! Functions in this module have some common safety requirements:
//! - The caller must ensure that the value at the memory location is valid.
//!   However, because we can never rely on data from user space, this actually
//!   means that the only types that can be safely read from and write to user
//!   space are primitive types like integers and pointers. If you create a
//!   pointer to a struct, you are like to do something wrong.
//! - The caller must ensure that the memory location is valid and accessible,
//!   for `read_unchecked` and `write_unchecked` functions.

use core::{cmp, fmt::Debug, marker::PhantomData, ops::ControlFlow, slice};

use alloc::{ffi::CString, vec::Vec};
use config::mm::{PAGE_SIZE, USER_END};
use mm::address::VirtAddr;
use systype::{SysError, SysResult};

use super::{addr_space::AddrSpace, mapping_flags::MappingFlags};
use crate::{
    processor::current_hart,
    trap::trap_env::{set_kernel_trap_entry, set_user_rw_trap_entry},
};

/// Smart pointer that can be used to read memory in user address space.
pub type UserReadPtr<'a, T> = UserPtr<'a, T, ReadMarker>;
/// Smart pointer that can be used to write memory in user address space.
pub type UserWritePtr<'a, T> = UserPtr<'a, T, WriteMarker>;
/// Smart pointer that can be used to read and write memory in user address space.
pub type UserReadWritePtr<'a, T> = UserPtr<'a, T, ReadWriteMarker>;

/// Trait representing the access type of a pointer, i.e., read and/or write.
pub trait AccessType {}

/// Trait representing read access.
pub trait ReadAccess: AccessType {}

/// Trait representing write access.
pub trait WriteAccess: AccessType {}

/// Marker for read access.
/// Do not use this type; it is public only to allow the use of `User*Ptr` types.
#[derive(Debug)]
pub struct ReadMarker;

/// Marker for write access.
/// Do not use this type; it is public only to allow the use of `User*Ptr` types.
#[derive(Debug)]
pub struct WriteMarker;

/// Marker for read-write access.
/// Do not use this type; it is public only to allow the use of `User*Ptr` types.
#[derive(Debug)]
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
    addr_space: &'a AddrSpace,

    /// Guard to ensure the `SUM` bit of `sstatus` register is set when accessing
    /// the memory.
    sum_guard: SumGuard,

    /// Marker to indicate the access type of the pointer.
    access: PhantomData<A>,
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
    pub fn new(addr: usize, addr_space: &'a AddrSpace) -> Self {
        Self {
            ptr: addr as *mut T,
            addr_space,
            sum_guard: SumGuard::new(),
            access: PhantomData,
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
    /// See the module-level documentation for safety information.
    pub unsafe fn read(&mut self) -> SysResult<T> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            size_of::<T>(),
            MappingFlags::R,
        )?;
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
    /// See the module-level documentation for safety information.
    pub unsafe fn read_uncheked(&mut self) -> T {
        unsafe { self.ptr.read() }
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
    /// See the module-level documentation for safety information.
    pub unsafe fn read_array(&mut self, len: usize) -> SysResult<Vec<T>> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            len * size_of::<T>(),
            MappingFlags::R,
        )?;
        let mut vec: Vec<T> = Vec::with_capacity(len);
        unsafe {
            vec.as_mut_ptr().copy_from_nonoverlapping(self.ptr, len);
            vec.set_len(len);
        }
        Ok(vec)
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
    /// See the module-level documentation for safety information.
    pub unsafe fn try_into_ref(&mut self) -> SysResult<&T> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            size_of::<T>(),
            MappingFlags::R,
        )?;
        Ok(unsafe { &*self.ptr })
    }

    /// Tries to convert the pointer to a slice of values, with the given length.
    ///
    /// This function will check if the memory location is accessible and try to
    /// convert the pointer to a slice of values.
    ///
    /// `len` is the number of values in the slice.
    ///
    /// This function distinguish itself from `read_array` by returning a slice
    /// pointing to somewhre in the user space, instead of a vector, which may be
    /// more efficient in some cases.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// See the module-level documentation for safety information.
    pub unsafe fn try_into_slice(&mut self, len: usize) -> SysResult<&[T]> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            len * size_of::<T>(),
            MappingFlags::R,
        )?;
        Ok(unsafe { slice::from_raw_parts(self.ptr, len) })
    }
}

/// Blanket implementation for read-access pointers, whose target type is a
/// raw pointer.
impl<A> UserPtr<'_, usize, A>
where
    A: ReadAccess,
{
    /// Reads an zero-terminated array of `usize`s from the memory location.
    ///
    /// This function will check if the memory location is accessible and read
    /// the `usize`s. It will read `usize`s until a zero is encountered or the
    /// maximum length `len` is reached. This function is used to read a null-
    /// terminated pointer array, but it returns `usize`s rather than pointers
    /// in order to make the use of it convenience.
    ///
    /// `len` is the maximum number of pointers to read, including the null
    /// terminator. `len` must be greater than 0.
    ///
    /// Returns the array of `usize`s as a `Vec<usize>`, which does NOT include
    /// the null terminator. Therefore, at most `len - 1` `usize`s will be
    /// returned. If there is no null terminator in the first `len` pointers,
    /// `len - 1` `usize`s will be returned. Note that an empty `Vec` may be
    /// returned if the first pointer is null or if `len` is 1.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// This function is safe, but it does not garantee that each `usize`s it
    /// reads is also a valid pointer. The responsibility of checking the
    /// validity of the pointers lies with the caller.
    pub fn read_ptr_array(&mut self, len: usize) -> SysResult<Vec<usize>> {
        debug_assert!(len > 0);

        let mut vec: Vec<usize> = Vec::new();
        let mut push_and_check = |ptr: usize| {
            if ptr == 0 {
                return ControlFlow::Break(());
            }
            vec.push(ptr);
            ControlFlow::Continue(())
        };
        // SAFETY: every `usize` is valid.
        unsafe {
            access_with_checking(
                self.addr_space,
                self.ptr as usize,
                (len - 1) * size_of::<usize>(),
                MappingFlags::R,
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
    /// Reads a C-style string from the memory location.
    ///
    /// This function will check if the memory location is accessible and read
    /// the string. It will read the string until a null byte is encountered
    /// or the maximum length `len` is reached.
    ///
    /// `len` is the maximum number of bytes in the resulting string, including
    /// the null terminator. `len` must be greater than 0.
    ///
    /// Returns the string as a [`CString`], which includes the null terminator.
    /// If there is no null terminator in the first `len` bytes, the string will
    /// be truncated such that the last byte is the null terminator. Note that
    /// an empty string may be returned if the first byte is null or if `len` is 1.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Note
    /// Pay attention that a C-style string is not necessarily a valid UTF-8
    /// [`str`] in Rust, nor can it always be represented as a [`Vec<char>`].
    pub fn read_c_string(&mut self, len: usize) -> SysResult<CString> {
        debug_assert!(len > 0);

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
            access_with_checking(
                self.addr_space,
                self.ptr as usize,
                len - 1,
                MappingFlags::R,
                &mut push_and_check,
            )?;
        }
        // SAFETY: `vec` has no null byte in the middle.
        Ok(unsafe { CString::from_vec_unchecked(vec) })
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
    /// See the module-level documentation for safety information.
    pub unsafe fn write(&mut self, value: T) -> SysResult<()> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            size_of::<T>(),
            MappingFlags::W,
        )?;
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
    /// See the module-level documentation for safety information.
    pub unsafe fn write_unchecked(&mut self, value: T) {
        unsafe {
            self.ptr.write(value);
        }
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
    /// See the module-level documentation for safety information.
    pub unsafe fn write_array(&mut self, values: &[T]) -> SysResult<()> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            size_of_val(values),
            MappingFlags::W,
        )?;
        unsafe {
            self.ptr
                .copy_from_nonoverlapping(values.as_ptr(), values.len());
        }
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
    /// See the module-level documentation for safety information.
    pub unsafe fn try_into_mut_ref(&mut self) -> SysResult<&mut T> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            size_of::<T>(),
            MappingFlags::W,
        )?;
        Ok(unsafe { &mut *self.ptr })
    }

    /// Tries to convert the pointer to a mutable slice of values, with the given
    /// length.
    ///
    /// This function will check if the memory location is accessible and try to
    /// convert the pointer to a mutable slice of values.
    ///
    /// `len` is the number of values in the slice.
    ///
    /// This function distinguish itself from `write_array` by returning a slice
    /// pointing to somewhre in the user space, which provides more flexibility
    /// when writing to user space.
    ///
    /// # Error
    /// Returns an `EFAULT` error if the memory location is not accessible.
    ///
    /// # Safety
    /// See the module-level documentation for safety information.
    pub unsafe fn try_into_mut_slice(&mut self, len: usize) -> SysResult<&mut [T]> {
        check_user_access(
            self.addr_space,
            self.ptr as usize,
            len * size_of::<T>(),
            MappingFlags::W,
        )?;
        Ok(unsafe { slice::from_raw_parts_mut(self.ptr, len) })
    }
}

/// Checks if certain access to a given range of user memory is allowed.
///
/// `addr_space` is the address space of the current process.
///
/// `addr` is the starting address of the memory region to be accessed.
///
/// `len` is the length in bytes of the memory region to be accessed.
///
/// `perm` must be `R`, `W`, or `RW`. `W` is equivalent to `RW`.
///
/// Returns `Ok(())` if the access is allowed, otherwise returns an `EFAULT` error.
fn check_user_access(
    addr_space: &AddrSpace,
    mut addr: usize,
    len: usize,
    perm: MappingFlags,
) -> SysResult<()> {
    if len == 0 {
        return Ok(());
    }
    if addr == 0 {
        return Err(SysError::EFAULT);
    }

    let end_addr = addr + len;
    if !VirtAddr::check_validity(addr)
        || !VirtAddr::check_validity(end_addr - 1)
        || !VirtAddr::new(end_addr - 1).in_user_space()
    {
        return Err(SysError::EFAULT);
    }

    set_user_rw_trap_entry();

    let checker = if perm.contains(MappingFlags::W) {
        try_write
    } else {
        try_read
    };

    while addr < end_addr {
        if unsafe { !checker(addr) } {
            // If the access failed, manually call the original page fault handler
            // to try mapping the page. If this also fails, then we know the access
            // is not allowed.
            if let Err(e) = addr_space.handle_page_fault(VirtAddr::new(addr), perm) {
                set_kernel_trap_entry();
                return Err(e);
            }
        }
        addr += PAGE_SIZE;
    }

    set_kernel_trap_entry();
    Ok(())
}

/// Accesses a user memory region by element with a closure, checking the access
/// permissions during the process.
///
/// `addr_space` is the address space of the current process.
///
/// `addr` is the starting address of the memory region to be accessed.
///
/// `len` is the max length in bytes of the memory region to be accessed. However,
/// the closure may stop the process early, or the access may fail due to lack of
/// permissions, so the actual length of the memory region accessed may be less
/// than `len`.
///
/// `perm` must be `MemPerm::R`, `MemPerm::W`, or `MemPerm::R` | `MemPerm::W`.
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
unsafe fn access_with_checking<F, T>(
    addr_space: &AddrSpace,
    mut addr: usize,
    len: usize,
    perm: MappingFlags,
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

    debug_assert!(addr % align_of::<T>() == 0);
    debug_assert!(len % size_of::<T>() == 0);

    let end_addr = cmp::min(addr + len, USER_END);
    if addr >= end_addr {
        return Err(SysError::EFAULT);
    }

    set_user_rw_trap_entry();

    let checker = if perm.contains(MappingFlags::W) {
        try_write
    } else {
        try_read
    };

    while addr < end_addr {
        if unsafe { !checker(addr) } {
            // If the access failed, manually call the original page fault handler
            // to try mapping the page. If this also fails, then we know the access
            // is not allowed.
            if let Err(e) = addr_space.handle_page_fault(VirtAddr::new(addr), perm) {
                set_kernel_trap_entry();
                return Err(e);
            }
        }
        let end_in_page = cmp::min(VirtAddr::new(addr + 1).round_up().to_usize(), end_addr);
        for item_addr in (addr..end_in_page).step_by(size_of::<T>()) {
            let item = unsafe { *(item_addr as *const T) };
            match f(item) {
                ControlFlow::Continue(()) => {}
                ControlFlow::Break(()) => {
                    set_kernel_trap_entry();
                    return Ok(());
                }
            }
        }
        addr = end_in_page;
    }

    set_kernel_trap_entry();
    Ok(())
}

/// Tries to read from a user memory region.
///
/// Returns `true` if the read is successful, otherwise `false`. A failed
/// read indicates that the memory region cannot be read from, or the address
/// is not mapped in the page table.
///
/// # Safety
/// This function must be called after calling `set_user_rw_trap_entry` to
/// enable kernel memory access to user space.
unsafe fn try_read(va: usize) -> bool {
    unsafe extern "C" {
        fn __try_read_user(va: usize) -> usize;
    }
    match unsafe { __try_read_user(va) } {
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
/// This function must be called after calling `set_user_rw_trap_entry` to
/// enable kernel memory access to user space.
unsafe fn try_write(va: usize) -> bool {
    unsafe extern "C" {
        fn __try_write_user(va: usize) -> usize;
    }
    match unsafe { __try_write_user(va) } {
        0 => true,
        1 => false,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
struct SumGuard;

impl SumGuard {
    pub fn new() -> Self {
        #[cfg(target_arch = "riscv64")]
            current_hart().get_mut_pps().inc_sum_cnt();
        Self
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        #[cfg(target_arch = "riscv64")]
            current_hart().get_mut_pps().dec_sum_cnt();
    }
}

unsafe impl<'a, T, M> Send for UserPtr<'a, T, M>
where
    T: Send,
    M: Send + AccessType,
{
}
