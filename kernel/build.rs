use std::{env, fs, path::PathBuf};

use config::mm::{KERNEL_RAM_OFFSET, KERNEL_START, KERNEL_START_PHYS, RAM_SIZE};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    let link_script = fs::read_to_string(PathBuf::from(manifest_dir).join("link.ld")).unwrap();

    let ram_size = RAM_SIZE - KERNEL_RAM_OFFSET;

    let new = link_script
        .replace("%RAM_START%", &KERNEL_START_PHYS.to_string())
        .replace("%VIRT_START%", &KERNEL_START.to_string())
        .replace("%RAM_SIZE%", &ram_size.to_string());

    let dest = PathBuf::from(out_dir).join("link.ld");
    fs::write(&dest, new).unwrap();
    println!("cargo:rustc-link-arg=-T{}", dest.display());
}
