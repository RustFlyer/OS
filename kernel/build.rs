use std::{env, fs, path::PathBuf};

use config::mm::{KERNEL_RAM_OFFSET, KERNEL_START_PHYS, RAM_SIZE};

fn main() {
    #![allow(non_snake_case)]

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    let link_script = fs::read_to_string(PathBuf::from(manifest_dir).join("link.ld")).unwrap();

    let ram_size = RAM_SIZE - KERNEL_RAM_OFFSET;

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Note: Remember to copy the constants from `lib/config/src/mm.rs` to here.
    // This is a workaround for the fact that the build script is compiled for the host
    // architecture, not the target architecture of the kernel.
    let VIRT_START: usize = match target_arch.as_str() {
        "riscv64" => {
            0xffff_ffc0_8000_0000
        }
        "loongarch64" => {
            0x9000_0000_8000_0000
        }
        _ => panic!("Unsupported target architecture"),
    };
    let KERNEL_START: usize = VIRT_START + KERNEL_RAM_OFFSET;

    let new = link_script
        .replace("%RAM_START%", &KERNEL_START_PHYS.to_string())
        .replace("%VIRT_START%", &KERNEL_START.to_string())
        .replace("%RAM_SIZE%", &ram_size.to_string());

    let dest = PathBuf::from(out_dir).join("link.ld");
    fs::write(&dest, new).unwrap();
    println!("cargo:rustc-link-arg=-T{}", dest.display());
}
