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
const BLACK: i16 = 1;
const RED: i16 = 2;
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

/// The "NEWTOS" logo as a 5×35 cell grid. `#` = a filled (coloured) cell,
/// space = background — rendered as coloured blocks (not text), exactly the
/// way EmuTOS' `print_art` paints its logo from a colour grid. "NEW" is
/// drawn blue, "TOS" red (columns < 18 vs ≥ 18).
const LOGO: [&[u8]; 5] = [
    b"#   # ##### #   # #####  ###   ####",
    b"##  # #     #   #   #   #   # #    ",
    b"# # # ####  # # #   #   #   #  ### ",
    b"#  ## #     ## ##   #   #   #     #",
    b"#   # ##### #   #   #    ###  #### ",
];

/// Format the GEMDOS clock as `YYYY-MM-DD HH:MM:SS` for the boot-time line.
fn boot_time_str() -> alloc::string::String {
    let (time, date) = tos_api::gemdos::time::dos_now();
    let day = date & 0x1F;
    let month = (date >> 5) & 0x0F;
    let year = 1980u16.wrapping_add(date >> 9);
    let sec = (time & 0x1F) * 2;
    let min = (time >> 5) & 0x3F;
    let hour = time >> 11;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}")
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
    let cx = sw / 2;

    // ── outer frame ──
    let (fx0, fy0, fx1, fy1) = (16, 12, sw - 17, sh - 13);
    v.set_line(BLACK);
    rect(&mut v, fx0, fy0, fx1, fy1);

    // ── NewTOS logo: a grid of coloured blocks (EmuTOS' print_art idea —
    //    each cell is a colour, not an ASCII char). "New" blue, "TOS" red. ──
    let (cw, chh) = (12i16, 13i16);
    let cols = LOGO[0].len() as i16;
    let logo_x = cx - cols * cw / 2;
    let logo_y = fy0 + 16;
    for (r, row) in LOGO.iter().enumerate() {
        let ry = logo_y + r as i16 * chh;
        for (c, &cell) in row.iter().enumerate() {
            if cell == b'#' {
                let col = c as i16;
                v.set_fill(if col < 18 { BLUE } else { RED });
                let bx = logo_x + col * cw;
                v.fill_rect(bx, ry, bx + cw - 1, ry + chh - 1);
            }
        }
    }
    let mut y = logo_y + LOGO.len() as i16 * chh;

    // ── tagline, centred ──
    y += 12;
    v.set_text_align(1, 5); // centre, top
    v.set_text_color(BLACK);
    v.gtext(cx, y, b"This operating system isnt Original");

    // separator under the tagline
    y += 24;
    v.set_line(BLACK);
    v.line(fx0 + 16, y, fx1 - 16, y);

    // ── system-info pairs (label right-aligned, value left of centre) ──
    let st = stats(Pool::Stram);
    let tt = stats(Pool::Ttram);
    y += 18;
    pair(&mut v, cx, y, b"Version", b"NewTOS 0.1");
    y += 22;
    pair(&mut v, cx, y, b"CPU type", b"ARM Cortex-A72 (AArch64)");
    y += 22;
    pair(&mut v, cx, y, b"Machine", machine_name().as_bytes());
    y += 22;
    pair(&mut v, cx, y, b"ST-RAM", size_str(st.total_bytes()).as_bytes());
    y += 22;
    pair(&mut v, cx, y, b"TT-RAM", size_str(tt.total_bytes()).as_bytes());
    y += 22;
    pair(&mut v, cx, y, b"GEMDOS drives", drives_str().as_bytes());
    y += 22;
    pair(&mut v, cx, y, b"Boot time", boot_time_str().as_bytes());

    // separator
    y += 22;
    v.set_line(BLACK);
    v.line(fx0 + 16, y, fx1 - 16, y);

    // ── USB device tree (devices are enumerated *before* this screen) ──
    y += 16;
    let mut devs = [crate::drivers::usb::DeviceInfo::default(); 4];
    let (ports, n) = crate::drivers::usb::tree(&mut devs);
    if ports > 0 {
        let tx = fx0 + 40;
        v.set_text_align(0, 5); // left
        v.set_text_color(BLACK);
        v.gtext(tx, y, format!("USB: xHCI host controller, {ports} ports").as_bytes());
        y += 18;
        if n == 0 {
            v.gtext(tx + 16, y, b"(no devices connected)");
            y += 18;
        }
        for (i, d) in devs[..n].iter().enumerate() {
            let kind = match d.protocol {
                1 => "Keyboard",
                2 => "Mouse",
                _ => "HID device",
            };
            let lead = if i + 1 == n { "\\-" } else { "|-" };
            let line = format!(
                "{} Port {}: {}  {:04x}:{:04x}",
                lead, d.port, kind, d.vid, d.pid
            );
            v.gtext(tx + 12, y, line.as_bytes());
            y += 18;
        }
    }

    // ── footer + booting ──
    y += 10;
    v.set_text_align(1, 5);
    v.set_text_color(BLACK);
    v.gtext(cx, y, b"NewTOS is an independent project - not original Atari software.");
    y += 22;
    v.set_text_color(BLUE);
    v.gtext(cx, y, b"Booting the desktop...");

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
