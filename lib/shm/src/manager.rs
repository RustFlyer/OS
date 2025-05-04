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

    pub fn attach(&self, shm_id: usize, lpid: usize) {
        let mut shm_manager = self.0.lock();
        let shm = shm_manager.get_mut(&shm_id).unwrap();
        shm.shmid_ds.attach(lpid);
    }

    pub fn detach(&self, shm_id: usize, lpid: usize) {
        let mut shm_manager = self.0.lock();
        let shm = shm_manager.get_mut(&shm_id).unwrap();
        if shm.shmid_ds.detach(lpid) {
            shm_manager.remove(&shm_id);
            unsafe {
                SHARED_MEMORY_KEY_ALLOCATOR.lock().dealloc(shm_id);
            }
        }
    }
}

pub static SHARED_MEMORY_MANAGER: Lazy<SharedMemoryManager> = Lazy::new(SharedMemoryManager::init);
pub static SHARED_MEMORY_KEY_ALLOCATOR: SpinNoIrqLock<VecIdAllocator> =
    SpinNoIrqLock::new(VecIdAllocator::new(2, usize::MAX));