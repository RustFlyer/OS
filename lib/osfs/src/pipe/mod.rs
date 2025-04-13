use alloc::sync::Arc;
use inode::PipeInode;
use read::PipeReadFile;
use vfs::file::File;
use write::PipeWriteFile;

pub mod inode;
pub mod read;
pub mod readfile;
pub mod ringbuffer;
pub mod write;
pub mod writefile;

pub fn new_pipe(len: usize) -> (Arc<dyn File>, Arc<dyn File>) {
    let pipe_inode = PipeInode::new(len);
    let read_end = PipeReadFile::new(pipe_inode.clone());
    let write_end = PipeWriteFile::new(pipe_inode);
    (read_end, write_end)
}
