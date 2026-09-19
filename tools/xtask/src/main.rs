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

/// Resource-file invariants the build tools must keep (`tools/mkrsc.py`).
#[cfg(test)]
mod rsc_align_tests;
/// A40 — the tool leaves OBJECT coordinates as char-grid words; the loader
/// converts (`tools/mkrsc.py`, `tos-api/src/aes/resource.rs`).
#[cfg(test)]
mod rsc_obfix_tests;

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
        "gen-fsel-rsc" => cmd_gen_fsel_rsc(),
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
  build --board {{rpi4|qemu-virt}} [--release] [--features a,b]
  gen-font                       — extract the EmuTOS 6x6/8x8/8x16 ST system
                                   fonts into tos-api/src/vdi/font_data.rs
  gen-fsel-rsc                   — convert the EmuTOS AES file-selector
                                   resource (references/emutos/aes/gem.rsc)
                                   into the embedded native blob
                                   tos-api/src/aes/gem_fsel.nrsc
  run [--board {{rpi4|qemu-virt}}] [--release] [--window] [--debug]
      [--sd <img>] [--features a,b]
                                   — boot in QEMU (virt by default, or
                                     raspi4b with --board rpi4); --window
                                     shows the framebuffer, --debug halts
                                     at reset with a gdbstub on :1234;
                                     --features adds optional kernel
                                     features (e.g. aes-trace)
  test                           — run every host-runnable test in the
                                   workspace (tos-api, tos-types, xtask);
                                   the kernel is not host-linkable
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

/// Parsed common build options: board, release profile, and any extra cargo
/// features (`--features a,b`) to enable on top of the board feature. The
/// extra list is what turns optional instruments on — e.g. `--features
/// aes-trace` for the AES trap-path tracer (`tos_api::aes::trace`).
struct BuildOpts {
    board: Option<Board>,
    release: bool,
    features: Vec<String>,
}

fn parse_board_and_profile(args: &[String]) -> Result<BuildOpts, String> {
    let mut board = None;
    let mut release = false;
    let mut features: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--board" => {
                let v = it.next().ok_or("--board requires a value")?;
                board = Some(Board::from_str(v)?);
            }
            "--release" => release = true,
            "--features" => {
                let v = it.next().ok_or("--features requires a value")?;
                features.extend(
                    v.split([',', ' '])
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string()),
                );
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok(BuildOpts { board, release, features })
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
    let opts = parse_board_and_profile(args)?;
    let board = opts.board.ok_or("build: --board {rpi4|qemu-virt} is required")?;
    build_kernel(board, opts.release, &opts.features)?;
    if board == Board::Rpi4 {
        produce_kernel8_img(opts.release)?;
    }
    Ok(())
}

/// Extract the three EmuTOS Atari-ST system-font rasters (6×6, 8×8, 8×16)
/// into a committed Rust module (`tos-api/src/vdi/font_data.rs`).
///
/// Each source `dat_table` (`references/emutos/bios/fnt_st_{6x6,8x8,8x16}.c`)
/// is a monochrome raster of `form_height` scanlines, each `form_width`
/// bytes wide (big-endian UWORDs), with all 256 characters laid out
/// horizontally at `max_cell_width` px each:
///   - 8×16: 128 words/row × 16 rows, 8 px/char  → one `[u8; 16]` per char
///   - 8×8 : 128 words/row × 8 rows,  8 px/char  → one `[u8; 8]`  per char
///   - 6×6 : 96 words/row  × 6 rows,  6 px/char  → one `[u8; 6]`  per char
///
/// For the byte-aligned 8-px fonts, glyph row r of char c is simply byte c
/// of raster row r. For the 6-px font, char c occupies bits `[c*6, c*6+6)`
/// of the scanline; we pack those 6 pixels MSB-left into a byte (bits 7..2),
/// matching the `bit 7 = leftmost pixel` convention used by the raster
/// backend for every size. The 8×16 output is byte-identical to the
/// previous single-font extraction.
fn cmd_gen_font() -> Result<(), String> {
    let root = workspace_root();
    let bios = root.join("references").join("emutos").join("bios");

    // Pull the `dat_table` initialiser out of a font source and collect its
    // hex words (the array size may be spelled `[]` or `[N]`).
    let extract = |file: &str, want: usize| -> Result<Vec<u16>, String> {
        let path = bios.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e} (is the EmuTOS reference checked out?)", path.display()))?;
        let start = text
            .find("dat_table")
            .ok_or_else(|| format!("dat_table not found in {file}"))?;
        let brace = text[start..].find('{').ok_or("dat_table: no '{'")? + start;
        let end = text[brace..].find("};").ok_or("dat_table: no '};'")? + brace;
        let mut words: Vec<u16> = Vec::with_capacity(want);
        for tok in text[brace..end].split(|c: char| !c.is_ascii_hexdigit() && c != 'x' && c != 'X') {
            let t = tok.trim();
            if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                if let Ok(v) = u16::from_str_radix(hex, 16) {
                    words.push(v);
                }
            }
        }
        if words.len() != want {
            return Err(format!("expected {want} dat_table words in {file}, got {}", words.len()));
        }
        Ok(words)
    };

    // Byte-aligned fonts (8 px/char): high byte of word c/2 if c even, low
    // byte if odd (big-endian).
    let byte_at = |w: &[u16], words_per_row: usize, row: usize, c: usize| -> u8 {
        let word = w[row * words_per_row + c / 2];
        if c % 2 == 0 {
            (word >> 8) as u8
        } else {
            (word & 0xFF) as u8
        }
    };

    let f8x16 = extract("fnt_st_8x16.c", 2048)?;
    let f8x8 = extract("fnt_st_8x8.c", 1024)?;
    let f6x6 = extract("fnt_st_6x6.c", 576)?;

    let mut s = String::new();
    s.push_str("//! GENERATED by `cargo xtask gen-font` — do not edit by hand.\n");
    s.push_str("//! Atari ST system fonts (6×6, 8×8, 8×16), transposed from EmuTOS\n");
    s.push_str("//! `bios/fnt_st_{6x6,8x8,8x16}.c` (`dat_table`). One byte-array\n");
    s.push_str("//! (`form_height` rows) per character; bit 7 of each byte is the\n");
    s.push_str("//! leftmost pixel. The 6×6 glyphs pack their 6 pixels into bits 7..2.\n\n");

    s.push_str("/// 256 glyphs of the 8×16 ST system font (row-major, MSB = left).\n");
    s.push_str("pub static SYSTEM_FONT_8X16: [[u8; 16]; 256] = [\n");
    for c in 0..256usize {
        s.push_str("    [");
        for r in 0..16usize {
            s.push_str(&format!("0x{:02x},", byte_at(&f8x16, 128, r, c)));
        }
        s.push_str("],\n");
    }
    s.push_str("];\n\n");

    s.push_str("/// 256 glyphs of the 8×8 ST system font (row-major, MSB = left).\n");
    s.push_str("pub static SYSTEM_FONT_8X8: [[u8; 8]; 256] = [\n");
    for c in 0..256usize {
        s.push_str("    [");
        for r in 0..8usize {
            s.push_str(&format!("0x{:02x},", byte_at(&f8x8, 128, r, c)));
        }
        s.push_str("],\n");
    }
    s.push_str("];\n\n");

    s.push_str("/// 256 glyphs of the 6×6 ST system font (row-major, 6 px in bits 7..2).\n");
    s.push_str("pub static SYSTEM_FONT_6X6: [[u8; 6]; 256] = [\n");
    for c in 0..256usize {
        s.push_str("    [");
        for r in 0..6usize {
            // Char c occupies bits [c*6, c*6+6) of the 96-word (1536-bit) row.
            let mut val = 0u8;
            for p in 0..6usize {
                let gbit = c * 6 + p;
                let word = f6x6[r * 96 + gbit / 16];
                let pix = ((word >> (15 - (gbit % 16))) & 1) as u8;
                val |= pix << (7 - p);
            }
            s.push_str(&format!("0x{val:02x},"));
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
    println!("xtask: wrote {} (6×6 + 8×8 + 8×16 glyphs)", out.display());
    Ok(())
}

/// Convert the EmuTOS AES file-selector resource into the native NRSC blob
/// embedded by `tos-api/src/aes/fsel.rs` (`GEM_FSEL_RSC`).
///
/// Like [`cmd_gen_font`], this regenerates a committed build artifact from an
/// EmuTOS reference. The conversion itself is `tools/mkrsc.py convert` (the
/// classic big-endian `gem.rsc` → little-endian LP64 NRSC with byte-offset
/// pointers); we only drive it so the regeneration command is reproducible and
/// discoverable next to the other `gen-*` commands. The kernel then embeds the
/// resulting blob and relocates it in place — no disk read, exactly as EmuTOS
/// links `gem.rsc` into ROM.
fn cmd_gen_fsel_rsc() -> Result<(), String> {
    let root = workspace_root();
    let script = root.join("tools").join("mkrsc.py");
    let src = root.join("references").join("emutos").join("aes").join("gem.rsc");
    let out = root.join("tos-api").join("src").join("aes").join("gem_fsel.nrsc");
    let python = env::var_os("PYTHON").unwrap_or_else(|| "python3".into());
    let mut c = Command::new(python);
    c.current_dir(&root)
        .arg(&script)
        .arg("convert")
        .arg(&src)
        .arg(&out);
    println!("xtask: converting {} -> {}", src.display(), out.display());
    run(c)
}

fn build_kernel(board: Board, release: bool, extra: &[String]) -> Result<(), String> {
    // The board feature is always present; `extra` appends optional ones
    // (e.g. `aes-trace`) to the same comma-separated `--features` list.
    let mut feats = board.feature().to_string();
    for f in extra {
        feats.push(',');
        feats.push_str(f);
    }
    let mut c = cargo();
    c.current_dir(workspace_root());
    c.arg("build")
        .arg("--package")
        .arg("kernel")
        .arg("--no-default-features")
        .arg("--features")
        .arg(&feats);
    if release {
        c.arg("--release");
    }
    println!("xtask: building kernel for {} ({})",
        feats,
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
    let opts = parse_board_and_profile(&filtered)?;
    let release = opts.release;
    if opts.board == Some(Board::Rpi4) {
        return cmd_run_rpi4(release, window, debug, sd.as_deref(), &opts.features);
    }
    build_kernel(Board::QemuVirt, release, &opts.features)?;
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
        // USB host stack (PHASE-16): an xHCI controller on the PCIe bus with
        // a keyboard + mouse, so the desktop is interactive in the emulator.
        "-device", "qemu-xhci",
        "-device", "usb-kbd",
        "-device", "usb-mouse",
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
fn cmd_run_rpi4(
    release: bool,
    window: bool,
    debug: bool,
    sd: Option<&str>,
    features: &[String],
) -> Result<(), String> {
    build_kernel(Board::Rpi4, release, features)?;
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

/// The workspace packages whose tests can run on the host.
///
/// `kernel` is not one of them and cannot become one: it is `#![no_std]`,
/// pinned to `aarch64-unknown-none-softfloat` and linked against a board
/// linker script, so `cargo test -p kernel` fails in its build script before
/// it ever compiles a test. Everything else in the workspace is listed here —
/// leaving `tos-api` out (as this command did until 2026-09-04) meant the
/// command ran the two packages that have no tests at all and reported
/// success without executing a single one.
const HOST_TEST_PACKAGES: [&str; 3] = ["tos-api", "tos-types", "xtask"];

fn cmd_test(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err(format!("test: unexpected arguments: {args:?}"));
    }
    // Host-runnable tests only: kernel cannot be linked for the host.
    let mut c = cargo();
    c.current_dir(workspace_root());
    c.arg("test");
    for p in HOST_TEST_PACKAGES {
        c.args(["--package", p]);
    }
    run(c)
}
