use alloc::collections::btree_map::BTreeMap;
use mutex::SpinNoIrqLock;
use smoltcp::wire::IpListenEndpoint;

type Port = u16;
type Fd = usize;

/// `PORT_MAP` is a global variable to manage the mapping relation
/// between `port` and (`Fd`, `IpListenEndpoint`).
///
/// # Use
/// - when binding port, it can look for the port and check whether it
///   is occupied by another relation. If not, it can store the mapping
///   for further usage.
/// - look for socket by port. For example, it can distribute the new
///   packet to the correct socket when you know that the new packet temps
///   to go to the 5001 port.
/// - delete socket by port. (e.g when a socket is closed)
/// - insert new socket. (e.g when a new socket is registered)
///
/// # Attention
/// - `PORT_MAP` does not support multi-mapping between a port and multi
///   sockets. If you want to implement this function, you should change
///   `BTreeMap<Port, (Fd, IpListenEndpoint)>` to `BTreeMap<Port, Vec(Fd, IpListenEndpoint)>`.
pub(crate) static PORT_MAP: PortMap = PortMap::new();
pub struct PortMap(SpinNoIrqLock<BTreeMap<Port, (Fd, IpListenEndpoint)>>);

impl PortMap {
    const fn new() -> Self {
        Self(SpinNoIrqLock::new(BTreeMap::new()))
    }

    pub fn get(&self, port: Port) -> Option<(Fd, IpListenEndpoint)> {
        self.0.lock().get(&port).cloned()
    }

    pub fn remove(&self, port: Port) {
        self.0.lock().remove(&port);
    }

    pub fn insert(&self, port: Port, fd: Fd, listen_endpoint: IpListenEndpoint) {
        self.0.lock().insert(port, (fd, listen_endpoint));
    }
}
