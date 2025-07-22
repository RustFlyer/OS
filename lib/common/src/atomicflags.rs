/// # Example
///
/// ```
/// use bitflags::bitflags;
/// use core::sync::atomic::Ordering;
/// use your_crate::atomic_bitflags;  
///
/// bitflags! {
///     struct MyFlags: u32 {
///         const A = 0b0001;
///         const B = 0b0010;
///     }
/// }
/// atomic_bitflags!(MyFlags, AtomicU32);
///
/// fn main() {
///    let atomic_flags = AtomicMyFlags::new(MyFlags::A | MyFlags::B);
///
///    atomic_flags.fetch_and(MyFlags::A, Ordering::SeqCst);
///    atomic_flags.fetch_or(MyFlags::B, Ordering::Relaxed);
///    let current = atomic_flags.load(Ordering::Acquire);
///    println!("{:?}", current);
/// }
///```
#[macro_export]
macro_rules! atomic_bitflags {
    ($name:ident, $atomic_ty:ident) => {
        paste::paste! {
            /// An atomic wrapper for the `$name` bitflags type, based on `$atomic_ty`.
            #[derive(Debug)]
            pub struct [<Atomic $name>](core::sync::atomic::$atomic_ty);

            impl [<Atomic $name>] {
                /// Creates a new atomic bitflags instance.
                #[inline]
                pub fn new(value: $name) -> Self {
                    Self(core::sync::atomic::$atomic_ty::new(value.bits()))
                }

                /// Loads the current flags value atomically.
                #[inline]
                pub fn load(&self, order: core::sync::atomic::Ordering) -> $name {
                    $name::from_bits_truncate(self.0.load(order))
                }

                /// Stores a new flags value atomically.
                #[inline]
                pub fn store(&self, value: $name, order: core::sync::atomic::Ordering) {
                    self.0.store(value.bits(), order)
                }

                /// Atomically performs a bitwise OR with the given flags.
                #[inline]
                pub fn fetch_or(&self, value: $name, order: core::sync::atomic::Ordering) -> $name {
                    $name::from_bits_truncate(self.0.fetch_or(value.bits(), order))
                }

                /// Atomically performs a bitwise AND with the given flags.
                #[inline]
                pub fn fetch_and(&self, value: $name, order: core::sync::atomic::Ordering) -> $name {
                    $name::from_bits_truncate(self.0.fetch_and(value.bits(), order))
                }

                /// Atomically performs a bitwise XOR with the given flags.
                #[inline]
                pub fn fetch_xor(&self, value: $name, order: core::sync::atomic::Ordering) -> $name {
                    $name::from_bits_truncate(self.0.fetch_xor(value.bits(), order))
                }

                /// Atomically updates the flags value using a closure.
                #[inline]
                pub fn fetch_update<F>(&self, order: core::sync::atomic::Ordering, mut f: F) -> Result<$name, $name>
                where
                    F: FnMut($name) -> Option<$name>,
                {
                    self.0
                        .fetch_update(order, order, |bits| {
                            f($name::from_bits_truncate(bits)).map(|flags| flags.bits())
                        })
                        .map($name::from_bits_truncate)
                        .map_err($name::from_bits_truncate)
                }
            }

            // Optionally implement Send and Sync (safe here as the wrapped atomic type is Send + Sync)
            unsafe impl Send for [<Atomic $name>] {}
            unsafe impl Sync for [<Atomic $name>] {}
        }
    };
}

pub mod test_atomicflags {
    bitflags::bitflags! {
    pub struct Apple: u32 {
            const Sweet = 0b0001;
            const Sour = 0b0010;
        }
    }

    atomic_bitflags!(Apple, AtomicU32);
    pub fn test_atomicflags() {
        use core::sync::atomic::Ordering::Relaxed;
        let apple = AtomicApple::new(Apple::Sweet);
        let taste = apple.load(Relaxed);
        log::debug!("apple is {:?}", taste.bits());

        apple.store(Apple::Sour, Relaxed);
        log::debug!("now apple tastes {:?}", apple.load(Relaxed).bits());
    }
}
