// build.rs — selects the linker script that matches the active board feature
// and exposes its directory to rustc via -C link-arg; generates the table of
// the programs the kernel image embeds (DESK43 U6b, decision D3: `embed.rs`),
// failing the build when one of them has not been built for the board.

use std::env;
use std::path::PathBuf;

#[path = "embed.rs"]
mod embed;

fn main() {
    let rpi4 = env::var_os("CARGO_FEATURE_BOARD_RPI4").is_some();
    let qemu = env::var_os("CARGO_FEATURE_BOARD_QEMU_VIRT").is_some();

    let (script, board) = match (rpi4, qemu) {
        (true, false) => ("link-rpi4.ld", "rpi4"),
        (false, true) => ("link-qemu-virt.ld", "qemu-virt"),
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

    // The embedded programs (`embed.rs`): one blob per board, from the
    // directory `cargo xtask build-programs --board <board>` writes. A missing
    // blob stops the build here, before anything is compiled against it.
    let workspace = manifest_dir.parent().expect("kernel/ has a parent").to_path_buf();
    let dir = embed::programs_dir(&workspace, board);
    println!("cargo:rerun-if-changed={}", manifest_dir.join("embed.rs").display());
    for e in embed::EMBEDDED {
        println!("cargo:rerun-if-changed={}", dir.join(e.file).display());
    }
    let src = match embed::generate(embed::EMBEDDED, &dir, board) {
        Ok(src) => src,
        Err(msg) => panic!("{msg}"),
    };
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded_programs.rs");
    std::fs::write(&out, src).unwrap_or_else(|e| panic!("kernel build: write {}: {e}", out.display()));
}
