#[repr(C)]
pub struct FileHandle {
    handle_bytes: u32, // input: buffer len, output: written len
    handle_type: i32,  // output: handle type
    f_handle: [u8],    // dyn nums, length = handle_bytes
}
