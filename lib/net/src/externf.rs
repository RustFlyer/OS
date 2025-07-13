use alloc::sync::Arc;

#[crate_interface::def_interface]
pub trait NetSocketIf {
    fn check_socket_reuseaddr(fd: usize) -> Option<bool>;
}
