use alloc::{
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};

pub struct UnixSocket {
    pub path: SpinNoIrqLock<Option<String>>,
    pub peer: SpinNoIrqLock<Option<Arc<UnixSocket>>>,
    pub buffer: SpinNoIrqLock<Vec<u8>>,
}

impl UnixSocket {
    pub fn new() -> Self {
        Self {
            path: SpinNoIrqLock::new(None),
            peer: SpinNoIrqLock::new(None),
            buffer: SpinNoIrqLock::new(Vec::new()),
        }
    }

    pub fn bind(self: Arc<Self>, path: &str) -> SysResult<()> {
        let mut p = self.path.lock();
        if p.is_some() {
            return Err(SysError::EINVAL);
        }
        *p = Some(path.to_string());

        UNIX_SOCKET_TABLE
            .lock()
            .insert(path.to_string(), self.clone());

        Ok(())
    }

    pub fn connect(&self, path: &str) -> SysResult<()> {
        let peer = UNIX_SOCKET_TABLE
            .lock()
            .get(path)
            .cloned()
            .ok_or(SysError::ECONNREFUSED)?;
        *self.peer.lock() = Some(peer);
        Ok(())
    }

    pub fn send(&self, buf: &[u8]) -> SysResult<usize> {
        if let Some(peer) = &*self.peer.lock() {
            peer.buffer.lock().extend_from_slice(buf);
            Ok(buf.len())
        } else {
            Err(SysError::ENOTCONN)
        }
    }

    pub fn recv(&self, buf: &mut [u8]) -> SysResult<usize> {
        let mut b = self.buffer.lock();
        let n = buf.len().min(b.len());
        buf[..n].copy_from_slice(&b[..n]);
        b.drain(..n);
        Ok(n)
    }
}

static UNIX_SOCKET_TABLE: SpinNoIrqLock<BTreeMap<String, Arc<UnixSocket>>> =
    SpinNoIrqLock::new(BTreeMap::new());

pub fn extract_path_from_sockaddr_un(path: &[u8; 108]) -> String {
    let len = path.iter().position(|&c| c == 0).unwrap_or(108);
    String::from_utf8_lossy(&path[..len]).to_string()
}
