use std::{env, fs, path::PathBuf};

use config::mm::{KERNEL_RAM_OFFSET, KERNEL_START_PHYS, RAM_SIZE};

fn main() {
    #![allow(non_snake_case)]

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_triple = env::var("TARGET").unwrap();
    let compile_profile = env::var("PROFILE").unwrap();

    let ram_size = RAM_SIZE - KERNEL_RAM_OFFSET;
    let target_arch_acronym = match target_arch.as_str() {
        "riscv64" => "rv",
        "loongarch64" => "la",
        _ => panic!("Unsupported target architecture"),
    };

    // Note: Remember to copy the constants from `lib/config/src/mm.rs` to here.
    // This is a workaround for the fact that the build script is compiled for the host
    // architecture, not the target architecture of the kernel.
    let VIRT_START: usize = match target_arch.as_str() {
        "riscv64" => 0xffff_ffc0_8000_0000,
        "loongarch64" => 0x9000_0000_8000_0000,
        _ => panic!("Unsupported target architecture"),
    };
    let KERNEL_START: usize = VIRT_START + KERNEL_RAM_OFFSET;

    // Generate the linker script.
    let link_script = fs::read_to_string(PathBuf::from(manifest_dir.clone()).join("linker.ld"))
        .unwrap()
        .replace("%RAM_START%", &KERNEL_START_PHYS.to_string())
        .replace("%VIRT_START%", &KERNEL_START.to_string())
        .replace("%RAM_SIZE%", &ram_size.to_string());
    let linker_script_dest = PathBuf::from(out_dir).join("linker.ld");
    fs::write(&linker_script_dest, link_script).unwrap();
    println!("cargo:rustc-link-arg=-T{}", linker_script_dest.display());

    let linkapp = fs::read_to_string(PathBuf::from(manifest_dir.clone()).join("src/linkapp.asm.tmpl"))
        .unwrap()
        .replace("<cargo-target>", &target_triple)
        .replace("<cargo-profile>", &compile_profile)
        .replace("<cargo-target-acronym>", target_arch_acronym);
    let linkapp_dest = PathBuf::from(manifest_dir).join("src/linkapp.asm");
    fs::write(&linkapp_dest, linkapp).unwrap();
}
