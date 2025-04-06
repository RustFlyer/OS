use vfs::file::File;

pub struct SFile {
    pub id: usize,
}

impl SFile {
    pub fn new() -> SFile {
        SFile { id: 0 }
    }
}

impl File for SFile {
    fn get_meta(&self) -> &vfs::file::FileMeta {
        todo!()
    }
    fn base_load_dir(&self) -> systype::SysResult<()> {
        todo!()
    }
    fn base_ls(&self, path: alloc::string::String) {
        todo!()
    }
    fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> systype::SyscallResult {
        todo!()
    }
    fn base_read_dir(&self) -> systype::SysResult<Option<vfs::direntry::DirEntry>> {
        todo!()
    }
    fn base_read_link(&self, buf: &mut [u8]) -> systype::SyscallResult {
        todo!()
    }
    fn base_write_at(&self, offset: usize, buf: &[u8]) -> systype::SyscallResult {
        todo!()
    }
}
