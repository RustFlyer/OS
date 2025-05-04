use hashbrown::HashMap;
use id_allocator::{IdAllocator, VecIdAllocator};
use spin::Lazy;
use mutex::SpinNoIrqLock;

use crate::SharedMemory;

pub struct SharedMemoryManager(pub SpinNoIrqLock<HashMap<usize, SharedMemory>>);

impl SharedMemoryManager {
    pub fn init() -> Self {
        Self(SpinNoIrqLock::new(HashMap::new()))
    }

    pub fn attach(&self, id: usize, lpid: usize) {
        let mut manager = self.0.lock();
        let shm = manager.get_mut(&id).unwrap();
        shm.stat.attach(lpid);
    }

    pub fn detach(&self, id: usize, lpid: usize) {
        let mut manager = self.0.lock();
        let shm = manager.get_mut(&id).unwrap();
        if shm.stat.detach(lpid) {
            manager.remove(&id);
            unsafe {
                SHARED_MEMORY_KEY_ALLOCATOR.lock().dealloc(id);
            }
        }
    }
}

pub static SHARED_MEMORY_MANAGER: Lazy<SharedMemoryManager> = Lazy::new(SharedMemoryManager::init);
pub static SHARED_MEMORY_KEY_ALLOCATOR: SpinNoIrqLock<VecIdAllocator> =
    SpinNoIrqLock::new(VecIdAllocator::new(2, usize::MAX));