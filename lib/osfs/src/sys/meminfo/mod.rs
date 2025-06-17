use alloc::{format, string::String};
use mutex::SpinNoIrqLock;

pub mod dentry;
pub mod file;
pub mod inode;

pub static MEM_INFO: SpinNoIrqLock<MemInfo> = SpinNoIrqLock::new(MemInfo::new());

const TOTAL_MEM: usize = 16251136;
const FREE_MEM: usize = 327680;
const BUFFER: usize = 373336;
const CACHED: usize = 10391984;
const TOTAL_SWAP: usize = 4194300;

pub struct MemInfo {
    /// General memory
    pub total_mem: usize,
    pub free_mem: usize,
    pub avail_mem: usize,
    /// Buffer and cache
    pub buffers: usize,
    pub cached: usize,
    /// Swap space
    pub total_swap: usize,
    pub free_swap: usize,
    /// Share memory
    pub shmem: usize,
    pub slab: usize,
}

impl MemInfo {
    pub const fn new() -> Self {
        Self {
            total_mem: TOTAL_MEM,
            free_mem: FREE_MEM,
            avail_mem: TOTAL_MEM - FREE_MEM,
            buffers: BUFFER,
            cached: CACHED,
            total_swap: TOTAL_SWAP,
            free_swap: TOTAL_SWAP,
            shmem: 0,
            slab: 0,
        }
    }

    pub fn serialize_node_meminfo(&self, node: usize) -> String {
        let mem_used = self.total_mem - self.free_mem;
        let active = 2048;
        let inactive = 1024;
        let active_anon = 512;
        let inactive_anon = 256;
        let active_file = 1536;
        let inactive_file = 768;
        let unevictable = 0;
        let mlocked = 0;
        let dirty = 4;
        let writeback = 0;
        let filepages = 2304;
        let mapped = 128;
        let anonpages = 640;
        let shmem = 32;
        let kernelstack = 16;
        let pagetables = 24;
        let nfs_unstable = 0;
        let bounce = 0;
        let writebacktmp = 0;
        let slab = 256;
        let sreclaimable = 128;
        let sunreclaim = 128;
        let anonhugepages = 0;
        let hugepages_total = 0;
        let hugepages_free = 0;
        let hugepages_surp = 0;
        let hugepagesize = 2048;

        format!(
            "Node {node} MemTotal:       {total_mem} kB\n\
    Node {node} MemFree:        {free_mem} kB\n\
    Node {node} MemUsed:        {mem_used} kB\n\
    Node {node} Active:         {active} kB\n\
    Node {node} Inactive:       {inactive} kB\n\
    Node {node} Active(anon):   {active_anon} kB\n\
    Node {node} Inactive(anon): {inactive_anon} kB\n\
    Node {node} Active(file):   {active_file} kB\n\
    Node {node} Inactive(file): {inactive_file} kB\n\
    Node {node} Unevictable:    {unevictable} kB\n\
    Node {node} Mlocked:        {mlocked} kB\n\
    Node {node} Dirty:          {dirty} kB\n\
    Node {node} Writeback:      {writeback} kB\n\
    Node {node} FilePages:      {filepages} kB\n\
    Node {node} Mapped:         {mapped} kB\n\
    Node {node} AnonPages:      {anonpages} kB\n\
    Node {node} Shmem:          {shmem} kB\n\
    Node {node} KernelStack:    {kernelstack} kB\n\
    Node {node} PageTables:     {pagetables} kB\n\
    Node {node} NFS_Unstable:   {nfs_unstable} kB\n\
    Node {node} Bounce:         {bounce} kB\n\
    Node {node} WritebackTmp:   {writebacktmp} kB\n\
    Node {node} Slab:           {slab} kB\n\
    Node {node} SReclaimable:   {sreclaimable} kB\n\
    Node {node} SUnreclaim:     {sunreclaim} kB\n\
    Node {node} AnonHugePages:  {anonhugepages} kB\n\
    Node {node} HugePages_Total:{hugepages_total}\n\
    Node {node} HugePages_Free: {hugepages_free}\n\
    Node {node} HugePages_Surp: {hugepages_surp}\n\
    Node {node} Hugepagesize:   {hugepagesize} kB\n
    ",
            node = node,
            total_mem = self.total_mem,
            free_mem = self.free_mem,
            mem_used = mem_used,
            active = active,
            inactive = inactive,
            active_anon = active_anon,
            inactive_anon = inactive_anon,
            active_file = active_file,
            inactive_file = inactive_file,
            unevictable = unevictable,
            mlocked = mlocked,
            dirty = dirty,
            writeback = writeback,
            filepages = filepages,
            mapped = mapped,
            anonpages = anonpages,
            shmem = shmem,
            kernelstack = kernelstack,
            pagetables = pagetables,
            nfs_unstable = nfs_unstable,
            bounce = bounce,
            writebacktmp = writebacktmp,
            slab = slab,
            sreclaimable = sreclaimable,
            sunreclaim = sunreclaim,
            anonhugepages = anonhugepages,
            hugepages_total = hugepages_total,
            hugepages_free = hugepages_free,
            hugepages_surp = hugepages_surp,
            hugepagesize = hugepagesize,
        )
    }
}
