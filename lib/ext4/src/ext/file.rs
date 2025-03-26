use lwext4_rust::bindings::{ext4_fclose, ext4_file};

pub struct ExtFile(ext4_file);

impl Drop for ExtFile {
    fn drop(&mut self) {
        unsafe {
            ext4_fclose(&self.0);
        }
    }
}
