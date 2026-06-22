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
        "gen-font" => cmd_gen_font(),
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
  gen-font                       — extract the EmuTOS 8x16 ST system font
                                   into tos-api/src/vdi/font_data.rs
  run [--board {{rpi4|qemu-virt}}] [--release] [--window] [--debug]
                                   — boot in QEMU (virt by default, or
                                     raspi4b with --board rpi4); --window
                                     shows the framebuffer, --debug halts
                                     at reset with a gdbstub on :1234
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

/// Extract the EmuTOS 8×16 ST system font into a committed Rust module
/// (`tos-api/src/vdi/font_data.rs`). The source raster (`dat_table` in
/// `references/emutos/bios/fnt_st_8x16.c`) is a 2048×16 monochrome
/// bitmap: 128 big-endian UWORDs per row × 16 rows, with all 256
/// characters laid out horizontally at 8 px each. We transpose it into
/// one `[u8; 16]` glyph per character (row r of char c = byte c of the
/// raster row r).
fn cmd_gen_font() -> Result<(), String> {
    let root = workspace_root();
    let src_c = root
        .join("references")
        .join("emutos")
        .join("bios")
        .join("fnt_st_8x16.c");
    let text = std::fs::read_to_string(&src_c)
        .map_err(|e| format!("read {}: {e} (is the EmuTOS reference checked out?)", src_c.display()))?;

    // Slice out the `dat_table` initialiser and collect its hex words.
    let start = text
        .find("dat_table[]")
        .ok_or("dat_table[] not found in font source")?;
    let brace = text[start..].find('{').ok_or("dat_table: no '{'")? + start;
    let end = text[brace..].find("};").ok_or("dat_table: no '};'")? + brace;
    let body = &text[brace..end];

    let mut words: Vec<u16> = Vec::with_capacity(2048);
    for tok in body.split(|c: char| !c.is_ascii_hexdigit() && c != 'x' && c != 'X') {
        let t = tok.trim();
        if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            if let Ok(v) = u16::from_str_radix(hex, 16) {
                words.push(v);
            }
        }
    }
    if words.len() != 2048 {
        return Err(format!("expected 2048 dat_table words, got {}", words.len()));
    }

    // Raster: 128 words/row × 16 rows. Char c, row r → byte c of row r
    // (high byte of word c/2 if c even, low byte if odd; big-endian).
    const WORDS_PER_ROW: usize = 128;
    let byte_at = |row: usize, c: usize| -> u8 {
        let w = words[row * WORDS_PER_ROW + c / 2];
        if c % 2 == 0 {
            (w >> 8) as u8
        } else {
            (w & 0xFF) as u8
        }
    };

    let mut s = String::new();
    s.push_str("//! GENERATED by `cargo xtask gen-font` — do not edit by hand.\n");
    s.push_str("//! 8×16 Atari ST system font, transposed from EmuTOS\n");
    s.push_str("//! `bios/fnt_st_8x16.c` (`dat_table`). One `[u8; 16]` glyph per\n");
    s.push_str("//! character; bit 7 of each byte is the leftmost pixel.\n\n");
    s.push_str("/// 256 glyphs of the 8×16 ST system font (row-major, MSB = left).\n");
    s.push_str("pub static SYSTEM_FONT_8X16: [[u8; 16]; 256] = [\n");
    for c in 0..256usize {
        s.push_str("    [");
        for r in 0..16usize {
            s.push_str(&format!("0x{:02x},", byte_at(r, c)));
        }
        s.push_str("],\n");
    }
    s.push_str("];\n");

    let out = root
        .join("tos-api")
        .join("src")
        .join("vdi")
        .join("font_data.rs");
    std::fs::write(&out, &s).map_err(|e| format!("write {}: {e}", out.display()))?;
    println!("xtask: wrote {} (256×16 glyphs)", out.display());
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
    let debug = args.iter().any(|a| a == "--debug");
    // `--sd <path>` attaches a raw SD-card image (the kernel's C: drive).
    // Defaults to /tmp/sd.img if present, so the desktop sees its drives.
    let sd = args
        .iter()
        .position(|a| a == "--sd")
        .and_then(|i| args.get(i + 1).cloned())
        .or_else(|| {
            let def = std::path::Path::new("/tmp/sd.img");
            def.exists().then(|| def.to_string_lossy().into_owned())
        });
    let filtered: Vec<String> = {
        let mut out = Vec::new();
        let mut skip = false;
        for a in args {
            if skip {
                skip = false;
                continue;
            }
            if a == "--window" || a == "--debug" {
                continue;
            }
            if a == "--sd" {
                skip = true;
                continue;
            }
            out.push(a.clone());
        }
        out
    };
    let (board, release) = parse_board_and_profile(&filtered)?;
    if board == Some(Board::Rpi4) {
        return cmd_run_rpi4(release, window, debug, sd.as_deref());
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
    if debug {
        // Halt at reset and expose QEMU's gdbstub on tcp::1234.
        c.args(["-S", "-s"]);
    }
    println!(
        "xtask: launching QEMU{}{} (Ctrl-C to quit)",
        if window { " with display window" } else { "" },
        if debug { " [gdbstub :1234, halted]" } else { "" },
    );
    run(c)
}

/// `xtask run --board rpi4`: boot the RPi4 kernel on QEMU's `raspi4b`
/// machine (the debuggable first vehicle for the VideoCore framebuffer).
/// `--window` shows the emulated HDMI output; `--debug` halts at reset
/// with a gdbstub on :1234. There is no serial dependency — the kernel's
/// PL011 is wired to serial0 (`-serial stdio`), but the framebuffer is
/// the intended output channel on real hardware.
fn cmd_run_rpi4(release: bool, window: bool, debug: bool, sd: Option<&str>) -> Result<(), String> {
    build_kernel(Board::Rpi4, release)?;
    produce_kernel8_img(release)?;
    let img = workspace_root().join("kernel8.img");

    let mut c = Command::new("qemu-system-aarch64");
    c.args(["-M", "raspi4b", "-no-reboot"]);
    if window {
        c.args(["-display", "cocoa"]);
    } else {
        c.args(["-display", "none"]);
    }
    // serial0 on raspi4b is the PL011 (UART0) the kernel drives.
    c.args(["-serial", "stdio", "-kernel"]).arg(&img);
    // Attach the SD-card image (the kernel's C: drive) so the desktop sees
    // its drives + directories.
    if let Some(path) = sd {
        c.args(["-drive", &format!("file={path},if=sd,format=raw")]);
    }
    if debug {
        c.args(["-S", "-s"]);
    }
    println!(
        "xtask: launching QEMU raspi4b{}{}{} (Ctrl-C to quit)",
        if window { " with display window" } else { "" },
        match sd {
            Some(p) => format!(" [sd: {p}]"),
            None => String::new(),
        },
        if debug { " [gdbstub :1234, halted]" } else { "" },
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
