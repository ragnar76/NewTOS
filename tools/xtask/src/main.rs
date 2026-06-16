//! NewTOS host-side build/run/test driver.
//!
//! Subcommands (PHASE-01 §D7):
//!   xtask build --board {rpi4|qemu-virt} [--release]
//!   xtask run [--release]                 — qemu-virt in QEMU, serial → stdout
//!   xtask test                             — host-side unit tests (tos-types, xtask)
//!
//! All commands shell out to `cargo`; this binary holds only the argument
//! plumbing and the QEMU invocation.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const KERNEL_TARGET: &str = "aarch64-unknown-none-softfloat";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let cmd = match args.next() {
        Some(s) => s,
        None => {
            print_usage();
            return ExitCode::from(2);
        }
    };

    let rest: Vec<String> = args.collect();
    let result = match cmd.as_str() {
        "build" => cmd_build(&rest),
        "run" => cmd_run(&rest),
        "test" => cmd_test(&rest),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown subcommand: {other}")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "\
Usage: cargo xtask <command> [options]

Commands:
  build --board {{rpi4|qemu-virt}} [--release]
  run [--release]
  test
  help
"
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Board {
    Rpi4,
    QemuVirt,
}

impl Board {
    fn feature(self) -> &'static str {
        match self {
            Board::Rpi4 => "board-rpi4",
            Board::QemuVirt => "board-qemu-virt",
        }
    }
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "rpi4" => Ok(Board::Rpi4),
            "qemu-virt" => Ok(Board::QemuVirt),
            other => Err(format!("unknown board: {other}")),
        }
    }
}

fn parse_board_and_profile(args: &[String]) -> Result<(Option<Board>, bool), String> {
    let mut board = None;
    let mut release = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--board" => {
                let v = it.next().ok_or("--board requires a value")?;
                board = Some(Board::from_str(v)?);
            }
            "--release" => release = true,
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok((board, release))
}

fn cargo() -> Command {
    let cargo_bin = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo_bin)
}

fn workspace_root() -> PathBuf {
    // tools/xtask/src/main.rs → ../../..
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().parent().unwrap().to_path_buf()
}

fn run(mut cmd: Command) -> Result<(), String> {
    let printable = format!("{cmd:?}");
    let status = cmd
        .status()
        .map_err(|e| format!("failed to spawn `{printable}`: {e}"))?;
    if !status.success() {
        return Err(format!("`{printable}` exited with {status}"));
    }
    Ok(())
}

fn cmd_build(args: &[String]) -> Result<(), String> {
    let (board, release) = parse_board_and_profile(args)?;
    let board = board.ok_or("build: --board {rpi4|qemu-virt} is required")?;
    build_kernel(board, release)?;
    if board == Board::Rpi4 {
        produce_kernel8_img(release)?;
    }
    Ok(())
}

fn build_kernel(board: Board, release: bool) -> Result<(), String> {
    let mut c = cargo();
    c.current_dir(workspace_root());
    c.arg("build")
        .arg("--package")
        .arg("kernel")
        .arg("--no-default-features")
        .arg("--features")
        .arg(board.feature());
    if release {
        c.arg("--release");
    }
    println!("xtask: building kernel for {} ({})",
        board.feature(),
        if release { "release" } else { "debug" });
    run(c)
}

fn kernel_elf_path(release: bool) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    workspace_root()
        .join("target")
        .join(KERNEL_TARGET)
        .join(profile)
        .join("kernel")
}

/// Convert the kernel ELF to `kernel8.img` (raw binary) using `rust-objcopy`
/// from the `llvm-tools-preview` component (installed via rust-toolchain).
fn produce_kernel8_img(release: bool) -> Result<(), String> {
    let elf = kernel_elf_path(release);
    if !elf.exists() {
        return Err(format!("kernel ELF not found at {}", elf.display()));
    }
    let out = workspace_root().join("kernel8.img");
    let objcopy = locate_rust_objcopy()?;
    let mut c = Command::new(objcopy);
    c.arg("-O").arg("binary").arg(&elf).arg(&out);
    println!("xtask: objcopy → {}", out.display());
    run(c)
}

fn locate_rust_objcopy() -> Result<PathBuf, String> {
    // `rustc --print sysroot` then look for
    // `lib/rustlib/<host>/bin/llvm-objcopy`.
    let out = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .map_err(|e| format!("rustc --print sysroot failed: {e}"))?;
    if !out.status.success() {
        return Err("rustc --print sysroot failed".into());
    }
    let sysroot = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let host = host_triple()?;
    let candidate = Path::new(&sysroot)
        .join("lib")
        .join("rustlib")
        .join(&host)
        .join("bin")
        .join("llvm-objcopy");
    if !candidate.exists() {
        return Err(format!(
            "llvm-objcopy not found at {}. Install with `rustup component add llvm-tools-preview`.",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn host_triple() -> Result<String, String> {
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|e| format!("rustc -vV failed: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("host: ") {
            return Ok(rest.trim().to_string());
        }
    }
    Err("could not parse host triple from rustc -vV".into())
}

fn cmd_run(args: &[String]) -> Result<(), String> {
    let (board, release) = parse_board_and_profile(args)?;
    if let Some(b) = board {
        if b != Board::QemuVirt {
            return Err("run: only `--board qemu-virt` is supported (or omit --board)".into());
        }
    }
    build_kernel(Board::QemuVirt, release)?;
    let elf = kernel_elf_path(release);

    // QEMU's `-kernel` only sets up the FDT bootloader (DTB pointer in
    // `x0`, Linux ARM64 boot protocol) when the kernel is loaded as a
    // flat binary. With an ELF the loader jumps directly to the entry
    // and `x0` is zero, so Phase 03 D8 (runtime DRAM detection) cannot
    // see the auto-generated device tree. Convert to a flat image.
    let img = workspace_root()
        .join("target")
        .join(KERNEL_TARGET)
        .join(if release { "release" } else { "debug" })
        .join("kernel.img");
    let objcopy = locate_rust_objcopy()?;
    let mut c = Command::new(objcopy);
    c.arg("-O").arg("binary").arg(&elf).arg(&img);
    println!("xtask: objcopy → {}", img.display());
    run(c)?;

    // Notes:
    //   * `gic-version=2` — our kernel/src/drivers/gic.rs targets GICv2;
    //     QEMU's `virt` machine defaults to GICv3 with cortex-a72.
    //   * `-m 272M` — 16 MiB ST-RAM slot + 256 MiB TT-RAM slot, the
    //     Atari-style additive memory map. See
    //     `kernel/src/mm/layout.rs` board constants. The kernel reads
    //     the actual size from the firmware-supplied DTB at boot
    //     (Phase 03 D8) and adjusts TT-RAM accordingly.
    //   * `-display none -serial stdio` — bind the PL011 directly to the
    //     controlling TTY without QEMU's mux chardev (`mon:stdio`),
    //     which buffers piped stdin and breaks PL011 RX.
    let mut c = Command::new("qemu-system-aarch64");
    c.args([
        "-M", "virt,gic-version=2",
        "-cpu", "cortex-a72",
        "-m", "272M",
        "-display", "none",
        "-serial", "stdio",
        "-no-reboot",
        "-kernel",
    ])
    .arg(&img);
    println!("xtask: launching QEMU (Ctrl-C to quit)");
    run(c)
}

fn cmd_test(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err(format!("test: unexpected arguments: {args:?}"));
    }
    // Host-runnable tests only: kernel cannot be linked for the host.
    let mut c = cargo();
    c.current_dir(workspace_root());
    c.args(["test", "--package", "tos-types", "--package", "xtask"]);
    run(c)
}
