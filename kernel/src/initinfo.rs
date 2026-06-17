//! NewTOS boot information screen — the EmuTOS-style "initinfo" screen,
//! rebuilt with our own VDI primitives.
//!
//! Like EmuTOS, it shows a centred banner, a separator, a block of
//! `label : value` system-info pairs (version, CPU, machine, RAM), a
//! second separator and a couple of message lines — but every pixel is
//! drawn **through the VDI** (`svc #2`), exercising the Phase-12 text,
//! fill and line primitives. This is the screen shown after boot.

use alloc::format;

use crate::bios::machine::frames::{stats, Pool};
use crate::vdi_client::Vdi;

// VDI palette indices (see vdi_backend::PALETTE).
const WHITE: i16 = 0;
const BLACK: i16 = 1;
const BLUE: i16 = 4;

/// Friendly machine name for the current board.
fn machine_name() -> &'static str {
    match crate::BOARD_NAME {
        "rpi4" => "Raspberry Pi 4 (BCM2711)",
        "qemu-virt" => "QEMU aarch64-virt",
        other => other,
    }
}

/// Build a list of mounted drive letters (e.g. `"C: D:"`) from the BIOS
/// drive bitmap, as filled by `Drvmap` / the partition mount.
fn drives_str() -> alloc::string::String {
    let bits = tos_api::bios::drive::drvbits();
    if bits == 0 {
        return alloc::string::String::from("none");
    }
    let mut s = alloc::string::String::new();
    for d in 0..16u32 {
        if bits & (1 << d) != 0 {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push((b'A' + d as u8) as char);
            s.push(':');
        }
    }
    s
}

/// Format a byte count as a human-readable size string.
fn size_str(bytes: u64) -> alloc::string::String {
    if bytes >= 1 << 20 {
        format!("{} MiB", bytes >> 20)
    } else {
        format!("{} KiB", bytes >> 10)
    }
}

/// Draw the boot information screen.
pub fn run() {
    let (sw, sh) = crate::vdi_backend::dims();
    if sw == 0 || sh == 0 {
        return;
    }
    let (sw, sh) = (sw as i16, sh as i16);

    let mut v = Vdi::new();
    if !v.open() {
        return;
    }
    v.clear(); // white background

    // ── outer frame ──
    let (fx0, fy0, fx1, fy1) = (24, 16, sw - 25, sh - 17);
    v.set_line(BLACK);
    rect(&mut v, fx0, fy0, fx1, fy1);

    // ── title bar: white text on a blue bar, centred ──
    let cx = sw / 2;
    v.set_fill(BLUE);
    v.fill_rect(fx0 + 16, fy0 + 18, fx1 - 16, fy0 + 54);
    v.set_text_align(1, 5); // centre, top
    v.set_text_color(WHITE);
    v.gtext(cx, fy0 + 28, b"NewTOS - clean-room TOS for AArch64");

    // separator under the title
    let mut y = fy0 + 72;
    v.set_line(BLACK);
    v.line(fx0 + 16, y, fx1 - 16, y);

    // ── system-info pairs (label right-aligned, value left of centre) ──
    let st = stats(Pool::Stram);
    let tt = stats(Pool::Ttram);
    y += 18;
    pair(&mut v, cx, y, b"Version", b"NewTOS 0.1");
    y += 24;
    pair(&mut v, cx, y, b"CPU type", b"ARM Cortex-A72");
    y += 24;
    pair(&mut v, cx, y, b"Architecture", b"AArch64 (ARMv8-A)");
    y += 24;
    pair(&mut v, cx, y, b"Machine", machine_name().as_bytes());
    y += 24;
    pair(&mut v, cx, y, b"ST-RAM", size_str(st.total_bytes()).as_bytes());
    y += 24;
    pair(&mut v, cx, y, b"TT-RAM", size_str(tt.total_bytes()).as_bytes());
    y += 24;
    pair(&mut v, cx, y, b"GEMDOS", drives_str().as_bytes());

    // separator before the messages
    y += 26;
    v.set_line(BLACK);
    v.line(fx0 + 16, y, fx1 - 16, y);

    // ── messages, centred ──
    y += 16;
    v.set_text_align(1, 5);
    v.set_text_color(BLACK);
    v.gtext(cx, y, b"Graphics rendered through the VDI (Trap 2 / svc #2)");
    y += 22;
    v.set_text_color(BLUE);
    v.gtext(cx, y, b"This is bare metal - no emulation in the kernel.");

    v.close();
}

/// Draw a label/value pair: `label` right-aligned ending near the
/// centre, `value` left-aligned just past it.
fn pair(v: &mut Vdi, cx: i16, y: i16, label: &[u8], value: &[u8]) {
    // label + " :" right-aligned, ending 8 px left of centre
    v.set_text_align(2, 5); // right, top
    v.set_text_color(BLACK);
    let mut lbuf = [0u8; 24];
    let n = label.len().min(lbuf.len() - 2);
    lbuf[..n].copy_from_slice(&label[..n]);
    lbuf[n] = b' ';
    lbuf[n + 1] = b':';
    v.gtext(cx - 8, y, &lbuf[..n + 2]);
    // value left-aligned, 8 px right of centre
    v.set_text_align(0, 5); // left, top
    v.set_text_color(BLUE);
    v.gtext(cx + 8, y, value);
}

/// Draw a rectangle outline (four `v_pline` segments).
fn rect(v: &mut Vdi, x0: i16, y0: i16, x1: i16, y1: i16) {
    v.line(x0, y0, x1, y0);
    v.line(x1, y0, x1, y1);
    v.line(x1, y1, x0, y1);
    v.line(x0, y1, x0, y0);
}
