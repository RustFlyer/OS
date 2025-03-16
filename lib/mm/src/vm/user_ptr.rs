//! Module for validating and accessing addresses in user address space.
//!
//! This module provides a smart pointer type `UserPtr` that can be used to access
//! memory in user address space safely. It is used to ensure that the kernel does
//! not access memory the user process does not have access to.

use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use systype::SysResult;

use crate::address::VirtAddr;

use super::addr_space::{self, AddrSpace};

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

/// Blanket implementation for all access types.
impl<'a, T, A> UserPtr<'a, T, A>
where
    A: AccessType,
{
    /// Create a new `UserPtr` from a virtual address.
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

    /// Check if the pointer is null.
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    /// Get the address of the pointer.
    pub fn to_usize(&self) -> usize {
        self.ptr as usize
    }
}

/// Blanket implementation for read-access pointers.
impl<'a, T, A> UserPtr<'a, T, A>
where
    A: ReadAccess,
{
    /// Read the value from the memory location.
    pub fn read(&self) -> SysResult<T> {
        todo!()
    }
}
