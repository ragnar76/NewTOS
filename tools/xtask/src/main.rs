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
        "build-programs" => cmd_build_programs(),
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
  build-programs                 — build programs/* and regenerate the
                                   embedded blobs in kernel/src/generated/
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

/// Build the standalone `programs/*` user programs and regenerate the
/// committed Rust blobs the kernel embeds (`kernel/src/generated/`).
/// Run this whenever a program's source changes; the kernel build then
/// picks up the new bytes from the committed `.rs`.
fn cmd_build_programs() -> Result<(), String> {
    build_one_program("lineecho", "LINEECHO_ELF", "lineecho_elf.rs")
}

fn build_one_program(name: &str, static_name: &str, gen_file: &str) -> Result<(), String> {
    let root = workspace_root();
    let prog_dir = root.join("programs").join(name);

    // 1. Build the program (own workspace + own .cargo/config + linker).
    let mut c = Command::new("cargo");
    c.arg("build").arg("--release").current_dir(&prog_dir);
    println!("xtask: building program `{name}` …");
    run(c)?;

    let elf = prog_dir
        .join("target")
        .join(KERNEL_TARGET)
        .join("release")
        .join(name);
    if !elf.exists() {
        return Err(format!("program ELF not found at {}", elf.display()));
    }

    // 2. Strip symbols/debug (keeps the PT_LOAD segment + headers) so
    //    the embedded blob is small.
    let stripped = prog_dir.join("target").join(format!("{name}.stripped.elf"));
    let objcopy = locate_rust_objcopy()?;
    let mut s = Command::new(&objcopy);
    s.arg("--strip-all").arg(&elf).arg(&stripped);
    run(s)?;

    // 3. Emit a committed Rust byte array the kernel `include!`s.
    let bytes = std::fs::read(&stripped).map_err(|e| format!("read {}: {e}", stripped.display()))?;
    let gen_dir = root.join("kernel").join("src").join("generated");
    std::fs::create_dir_all(&gen_dir).map_err(|e| format!("create {}: {e}", gen_dir.display()))?;
    let out = gen_dir.join(gen_file);

    let mut src = String::new();
    src.push_str("//! GENERATED by `cargo xtask build-programs` — do not edit by hand.\n");
    src.push_str(&format!(
        "//! Stripped AArch64 ELF of the `{name}` user program, embedded\n//! for the in-kernel program registry (Phase-06 loader).\n\n"
    ));
    src.push_str(&format!(
        "/// `{name}` program image ({} bytes, stripped ET_EXEC AArch64 ELF).\n",
        bytes.len()
    ));
    src.push_str(&format!(
        "pub static {static_name}: [u8; {}] = [\n",
        bytes.len()
    ));
    for chunk in bytes.chunks(16) {
        src.push_str("    ");
        for b in chunk {
            src.push_str(&format!("0x{b:02x}, "));
        }
        src.push('\n');
    }
    src.push_str("];\n");

    std::fs::write(&out, &src).map_err(|e| format!("write {}: {e}", out.display()))?;
    println!(
        "xtask: wrote {} ({} bytes embedded)",
        out.display(),
        bytes.len()
    );
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
    let window = args.iter().any(|a| a == "--window");
    let filtered: Vec<String> = args.iter().filter(|a| *a != "--window").cloned().collect();
    let (board, release) = parse_board_and_profile(&filtered)?;
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
        // `ramfb` gives the otherwise-display-less virt machine a simple
        // linear framebuffer (configured by the kernel via fw_cfg). It
        // is inert under `-display none` but its surface can still be
        // captured with a QMP `screendump`; pass `-display cocoa`
        // (macOS) to watch it live.
        "-device", "ramfb",
    ]);
    if window {
        // Native macOS window so the framebuffer is visible. Serial
        // still goes to this terminal.
        c.args(["-display", "cocoa"]);
    } else {
        c.args(["-display", "none"]);
    }
    c.args([
        "-serial", "stdio",
        "-no-reboot",
        "-kernel",
    ])
    .arg(&img);
    println!(
        "xtask: launching QEMU{} (Ctrl-C to quit)",
        if window { " with display window" } else { "" },
    );
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
