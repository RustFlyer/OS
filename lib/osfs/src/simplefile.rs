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
    fn meta(&self) -> &vfs::file::FileMeta {
        todo!()
    }
}
