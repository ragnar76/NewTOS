// build.rs — selects the linker script that matches the active board feature
// and exposes its directory to rustc via -C link-arg.

use std::env;
use std::path::PathBuf;

fn main() {
    let rpi4 = env::var_os("CARGO_FEATURE_BOARD_RPI4").is_some();
    let qemu = env::var_os("CARGO_FEATURE_BOARD_QEMU_VIRT").is_some();

    let script = match (rpi4, qemu) {
        (true, false) => "link-rpi4.ld",
        (false, true) => "link-qemu-virt.ld",
        (false, false) => panic!(
            "kernel build: no board feature selected. Enable exactly one of \
             `board-rpi4` or `board-qemu-virt`."
        ),
        (true, true) => panic!(
            "kernel build: both `board-rpi4` and `board-qemu-virt` are \
             enabled. Pick exactly one."
        ),
    };

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let script_path = manifest_dir.join("src/arch/aarch64").join(script);

    println!("cargo:rerun-if-changed={}", script_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg=-T{}", script_path.display());
}
