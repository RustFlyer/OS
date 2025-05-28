use alloc::sync::Arc;

use config::vfs::StatFs;
use systype::error::SysResult;
use vfs::superblock::{SuperBlock, SuperBlockMeta};

use crate::{FatFs, as_sys_err, disk::DiskCursor};

pub struct FatSuperBlock {
    meta: SuperBlockMeta,
    pub(crate) fs: Arc<FatFs>,
}

impl FatSuperBlock {
    pub fn new(meta: SuperBlockMeta) -> Arc<Self> {
        let blk_dev = meta.device.as_ref().unwrap().clone();
        Arc::new(Self {
            meta,
            fs: Arc::new(
                FatFs::new(
                    DiskCursor {
                        sector: 0,
                        offset: 0,
                        blk_dev,
                    },
                    fatfs::FsOptions::new(),
                )
                .unwrap(),
            ),
        })
    }
}

impl SuperBlock for FatSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        let fs = &self.fs;
        let stat_fs = fs.stats().map_err(as_sys_err)?;
        let ft = fs.fat_type();
        let f_type = match ft {
            fatfs::FatType::Fat12 => 0x01,
            fatfs::FatType::Fat16 => 0x04,
            fatfs::FatType::Fat32 => 0x0c,
        };
        Ok(StatFs {
            f_type,
            f_bsize: stat_fs.cluster_size() as i64,
            f_blocks: stat_fs.total_clusters() as u64,
            f_bfree: stat_fs.free_clusters() as u64,
            f_bavail: stat_fs.free_clusters() as u64,
            f_files: 0,
            f_ffree: 0,
            f_fsid: [0, 0],
            f_namelen: 255,
            f_frsize: 0,
            f_flags: 0,
            f_spare: [0; 4],
        })
    }

    fn sync_fs(&self, _wait: isize) -> SysResult<()> {
        todo!()
    }
}
