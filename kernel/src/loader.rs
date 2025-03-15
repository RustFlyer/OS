use alloc::vec::Vec;

/// 获取应用程序的数量
pub fn get_num_app() -> usize {
    unsafe extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

/// 获取应用程序的数据
pub fn get_app_data(app_id: usize) -> &'static [u8] {
    unsafe extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id],
        )
    }
}

/// 初始化应用程序
pub fn init() {
    let num_app = get_num_app();
    unsafe extern "C" {
        fn _app_names();
    }
    let mut start = _app_names as usize as *const u8;
    let mut v = Vec::with_capacity(num_app);
    unsafe {
        for _ in 0..num_app {
            let mut end = start;
            // 找到字符串的结束位置
            while end.read_volatile() != b'\0' {
                end = end.add(1);
            }
            // 将字符串转换为切片
            let slice = core::slice::from_raw_parts(start, end as usize - start as usize);
            // 将切片转换为字符串
            let str = core::str::from_utf8(slice).unwrap();
            // 将字符串推入向量
            v.push(str);
            // 移动到下一个字符串
            start = end.add(1);
        }
        // 将向量转换为Option
        APP_NAMES = Some(v);
    }
}

/// 应用程序名称
static mut APP_NAMES: Option<Vec<&'static str>> = None;

#[allow(unused, static_mut_refs)]
/// 从名称获取应用程序数据
pub fn get_app_data_by_name(name: &str) -> Option<&'static [u8]> {
    // warn!("app name {}", name);
    let app_names = unsafe { APP_NAMES.as_ref().unwrap() };
    let num_app = get_num_app();
    (0..num_app)
        .find(|&i| app_names[i] == name)
        .map(get_app_data)
}
