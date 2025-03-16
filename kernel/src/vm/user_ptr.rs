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
    ffi::CStr,
    marker::PhantomData,
    ops::{ControlFlow, Deref, DerefMut},
    slice,
};

use alloc::vec::Vec;
use mm::address::VirtAddr;
use systype::SysResult;

use super::{
    addr_space::{self, AddrSpace},
    vm_area::MemPerm,
};

trait AccessType {}
trait ReadAccess: AccessType {}
trait WriteAccess: AccessType {}
struct ReadMarker;
struct WriteMarker;
struct ReadWriteMarker;

impl AccessType for ReadMarker {}
impl AccessType for WriteMarker {}
impl AccessType for ReadWriteMarker {}
impl ReadAccess for ReadMarker {}
impl WriteAccess for WriteMarker {}
impl ReadAccess for ReadWriteMarker {}
impl WriteAccess for ReadWriteMarker {}

/// A smart pointer that can be used to read memory in user address space.
pub type UserReadPtr<'a, T> = UserPtr<'a, T, ReadMarker>;

/// A smart pointer that can be used to write memory in user address space.
pub type UserWritePtr<'a, T> = UserPtr<'a, T, WriteMarker>;

/// A smart pointer that can be used to read and write memory in user address space.
pub type UserReadWritePtr<'a, T> = UserPtr<'a, T, ReadWriteMarker>;

/// Base type used to implement the smart pointers for read, write, and read-write access,
/// uniformly.
#[derive(Debug)]
struct UserPtr<'a, T, A>
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
    // /// Guard to ensure the `SUM` bit of `sstatus` register is set when accessing the memory.
    // sum_guard: SumGuard,
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
    pub fn new(addr: usize, addr_space: &'a mut AddrSpace) -> Self {
        Self {
            ptr: addr as *mut T,
            addr_space,
            _access: PhantomData,
            // sum_guard: SumGuard::new(),
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
impl<'a, T, A> UserPtr<'a, T, A>
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
impl<'a, T, A> UserPtr<'a, *const T, A>
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

impl<'a, A> UserPtr<'a, u8, A>
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
impl<'a, T, A> UserPtr<'a, T, A>
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
    pub unsafe fn try_into_mut_ref(&mut self) -> SysResult<&mut T> {
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
    pub unsafe fn try_into_mut_slice(&mut self, len: usize) -> SysResult<&mut [T]> {
        self.addr_space
            .check_user_access(self.ptr as usize, len * size_of::<T>(), MemPerm::W)?;
        Ok(unsafe { slice::from_raw_parts_mut(self.ptr, len) })
    }
}
