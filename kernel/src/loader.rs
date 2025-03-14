use crate::println;
use config::mm::*;
use core::arch::asm;
use lazy_static::lazy_static;

// 临时用来加载文件，之后完成mm内容后
// 替换为直接loader

lazy_static! {
    static ref APP_LOADER: AppLoader = AppLoader::new(100, [0; 100]);
}

pub struct AppLoader {
    num_app: usize,
    app_start: [usize; 100],
}

impl AppLoader {
    pub fn new(num_app: usize, app_start: [usize; 100]) -> Self {
        Self { num_app, app_start }
    }

    pub fn load_app(&self, app_id: usize) {
        if app_id >= self.num_app {
            panic!("All applications completed!");
        }
        println!("[kernel] Loading app_{}", app_id);
        // clear app area
        unsafe {
            core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);
            let app_src = core::slice::from_raw_parts(
                self.app_start[app_id] as *const u8,
                self.app_start[app_id + 1] - self.app_start[app_id],
            );
            let app_dst =
                core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
            app_dst.copy_from_slice(app_src);
            // memory fence about fetching the instruction memory
            asm!("fence.i");
        }
    }
}
