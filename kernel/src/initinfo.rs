//! NewTOS boot information screen — the EmuTOS-style "initinfo" screen,
//! rebuilt with our own VDI primitives.
//!
//! Like EmuTOS, it shows a centred banner, a separator, a block of
//! `label : value` system-info pairs (version, CPU, machine, RAM), a
//! second separator and a couple of message lines — but every pixel is
//! drawn **through the VDI** (`svc #2`), exercising the Phase-12 text,
//! fill and line primitives. This is the screen shown after boot.
//!
//! [`run`] also owns the **hold** — EmuTOS' `initinfo` does not just draw the
//! screen, it waits on it (`references/emutos/bios/initinfo.c:376-442`) and
//! returns the modifier keys the user was holding, which `biosmain` turns into
//! boot flags (`bios/bios.c:816-820`). See [`wait_for_user`].

use alloc::format;

use crate::bios::machine::frames::{stats, Pool};
use crate::vdi_client::Vdi;

// VDI palette indices (see vdi_backend::PALETTE).
const WHITE: i16 = 0;
const BLACK: i16 = 1;
const RED: i16 = 2;
const BLUE: i16 = 4;

/// EmuTOS `INITINFO_DURATION` — how long the welcome screen is held, in
/// **seconds** (`references/emutos/include/config.h:1392-1405`):
///
/// > By default, the EmuTOS welcome screen (initinfo) is displayed for 3
/// > seconds. On emulators, this is enough to read the text, and optionally
/// > to press Shift to keep the screen displayed. But on real hardware, it
/// > can take several seconds for the monitor to recover from stand-by mode,
/// > so the welcome screen may never be seen. In such cases, it is wise to
/// > increase the welcome screen duration.
///
/// EmuTOS makes it a **per-target build-time constant** and raises it for
/// machines whose display is slow to come up: the FireBee target overrides it
/// to 8 with the comment *"Longer time for LCD monitors startup"*
/// (`include/config.h:173-175`). NewTOS' build-time targets are the boards, so
/// the same per-target override lives here — the Pi-4's HDMI monitor spends
/// several seconds locking the mode (its own sync banner covers the first part
/// of our boot screen), so `board-rpi4` gets 15 s; qemu-virt keeps EmuTOS'
/// default 3, which is exactly the "on emulators, this is enough" case.
///
/// A long duration is not a long boot: this is a *wait*, not a delay. Any key
/// — or Shift/Control/Alternate — ends it immediately ([`wait_for_user`]), so
/// the cost of the 15 s on a machine that is being rebooted repeatedly is one
/// keypress.
///
/// EmuTOS' `INITINFO_DURATION == 0` case (`bios/bios.c:801-802`: do not show
/// the welcome screen at all) has no NewTOS counterpart — neither board sets 0.
#[cfg(feature = "board-rpi4")]
const INITINFO_DURATION: u64 = 15;
/// See the `board-rpi4` arm above.
#[cfg(not(feature = "board-rpi4"))]
const INITINFO_DURATION: u64 = 3;

/// `MODE_RSHIFT|MODE_LSHIFT` = `MODE_SHIFT` (`references/emutos/bios/ikbd.h:26`,
/// `include/biosdefs.h:90-91`) — a Shift key is down, which *holds* the screen.
const MODE_SHIFT: u8 = 0x03;

/// `MODE_SCA` (`references/emutos/include/biosdefs.h:94`) — Shift, Control or
/// Alternate is down; any of them ends the timed wait.
const MODE_SCA: u8 = 0x0F;

/// `DEL_ASCII` (`references/emutos/bios/initinfo.c:48`) — the spurious Delete
/// keypress left over from a Ctrl+Alt+Shift+Delete reboot, which the wait loop
/// swallows instead of treating as "user pressed a key".
const DEL_ASCII: u8 = 0x7F;

/// `INFO_LENGTH` (`references/emutos/bios/initinfo.c:50`) — the width of the
/// info lines in characters; also the width of the inverse-video bar.
const INFO_LENGTH: i16 = 40;

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

/// Draw the boot information screen, hold it, and return the keyboard
/// modifier state the user was holding when it ended — EmuTOS
/// `initinfo(&shiftbits)` (`references/emutos/bios/initinfo.c:267-457`).
///
/// The returned byte is EmuTOS' `*pshiftbits` in `Kbshift` / `MODE_*`
/// encoding; `biosmain` turns it into boot flags (`bios/bios.c:816-820`) —
/// see [`crate::launch::set_bootflags_from_shiftbits`]. EmuTOS also returns
/// the selected boot device; NewTOS does not (see [`wait_for_user`]).
///
/// **Without a screen** there is nothing to hold, so we take EmuTOS' own
/// no-welcome-screen path, `*pshiftbits = kbshift(-1)` with no wait
/// (`initinfo.c:463-469`, the `#else FULL_INITINFO` variant; the same
/// assignment `biosmain` makes when the screen is suppressed,
/// `bios/bios.c:812`).
pub fn run() -> u8 {
    // Every input the wait loop needs — `kbshift`, `bconstat2`, `bconin2` and
    // the 200 Hz tick — reaches us through the installed host, exactly as
    // EmuTOS' `initinfo` calls those BIOS functions directly. No host means no
    // keyboard and no clock, so there is nothing to wait *for*.
    let Some(host) = tos_api::host::host() else {
        return 0;
    };
    let (sw, sh) = crate::vdi_backend::dims();
    if sw == 0 || sh == 0 {
        return host.keyboard_shift_state();
    }
    let (sw, sh) = (sw as i16, sh as i16);

    let mut v = Vdi::new();
    if !v.open() {
        return host.keyboard_shift_state();
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

    // ── the key hints EmuTOS prints below the second separator
    //    (`bios/initinfo.c:353-364`) ──
    //
    // EmuTOS prints five; NewTOS prints the two whose effect this build
    // actually has. The other three are honestly absent, because the machinery
    // behind them is:
    //
    // * `"Hold <Alternate> to skip HDD boot"` (:355) sets
    //   `BOOTFLAG_SKIP_HDD_BOOT`, which suppresses *executing the hard disk's
    //   boot sector* in `blkdev_boot` (`bios/blkdev.c:231`). NewTOS never
    //   executes a boot sector (there is no m68k to run it on), so there is
    //   nothing for the flag to skip.
    // * `"Press key 'X' to boot from X:"` (:357) selects the boot device, which
    //   EmuTOS then feeds to `blkdev_boot`, `Dsetdrv(bootdev)` and `autoexec`
    //   (`bios/bios.c:825-846`). NewTOS has no boot-device variable: the AUTO
    //   scan is hardwired to `C:\AUTO\` (`launch::run_auto_folder`). Wiring a
    //   real `bootdev` through those callers is a separate change; faking the
    //   key without it would only pretend to select a drive.
    // * `"Press <Esc> to run an early console"` (:359) is `#if WITH_CLI` and
    //   sets `BOOTFLAG_EARLY_CLI`, which `biosmain` uses to `Pexec` EmuCON
    //   *before* `autoexec` (`bios/bios.c:834-844`). NewTOS' console shell is
    //   reachable as the shell loop's `CONSOLE_APP`, but there is no
    //   run-a-shell-then-continue-booting entry, so the flag has no consumer.
    y += 10;
    v.set_text_align(1, 5);
    v.set_text_color(BLACK);
    // `display_message(_("Hold <Control> to skip AUTO/ACC"))` (:353)
    v.gtext(cx, y, b"Hold <Control> to skip AUTO/ACC");
    y += 22;
    // `display_inverse(_("Hold <Shift> to pause this screen"),0)` (:364)
    display_inverse(&mut v, cx, y, b"Hold <Shift> to pause this screen");

    // ── footer + booting ──
    y += 26;
    v.set_text_align(1, 5);
    v.set_text_color(BLACK);
    v.gtext(cx, y, b"NewTOS is an independent project - not original Atari software.");
    y += 22;
    v.set_text_color(BLUE);
    v.gtext(cx, y, b"Adapted by Ingo Uhlemann with Claude AI...");

    v.close();

    // EmuTOS' `initinfo` ends in the wait loop, not in the last `cprintf`.
    wait_for_user(host)
}

/// `display_inverse(s, FALSE)` (`references/emutos/bios/initinfo.c:134-150`) —
/// the message centred in an inverse-video bar.
///
/// EmuTOS pads the string with spaces to [`INFO_LENGTH`] characters and
/// brackets the whole run with the VT52 inverse-video pair `\033p` … `\033q`
/// (`:142-146`). Our screen is VDI-drawn rather than a character grid, so the
/// same appearance is a filled `INFO_LENGTH`-cell bar in the foreground colour
/// with the text drawn on top in the background colour. The text goes down in
/// **transparent** writing mode: the VDI's default replace mode paints a
/// glyph's clear pixels with pen 0 (`vdi_backend::draw_glyph_sized`, EmuTOS
/// `vdi_textblit.c` `WM_REPLACE`), which would repaint the bar white under
/// every character cell.
fn display_inverse(v: &mut Vdi, cx: i16, y: i16, s: &[u8]) {
    use tos_api::vdi::font::{CELL_H, CELL_W};

    let half = INFO_LENGTH * CELL_W / 2;
    v.set_fill(BLACK);
    v.fill_rect(cx - half, y, cx + half - 1, y + CELL_H - 1);
    v.set_text_align(1, 5); // centre, top
    v.set_text_color(WHITE);
    v.set_write_mode(2); // transparent
    v.gtext(cx, y, s);
    v.set_write_mode(1); // back to replace, the workstation default
}

/// Hold the boot screen — EmuTOS `initinfo`'s wait loop
/// (`references/emutos/bios/initinfo.c:376-442`). Returns the keyboard
/// modifier state (`kbshift(-1)`) as it was when the wait ended.
///
/// ```c
/// while (1) {
///     long end = hz_200 + INITINFO_DURATION * 200UL;
///     olddev = dev;
///     do {
///         shiftbits = kbshift(-1);
///         if ((shiftbits & MODE_SCA) || bconstat2()) break;
///     } while (hz_200 < end);
///     while ((shiftbits & MODE_SHIFT) && !bconstat2()) shiftbits = kbshift(-1);
///     if (bconstat2()) {
///         int c = toupper(LOBYTE(bconin2()));
///         if (c == DEL_ASCII) continue;      /* eat spurious Delete */
///         ...                                /* Esc / drive letter */
///     }
///     if (dev == olddev) break;
///     cprint_devices(dev);
/// }
/// ```
///
/// So the screen stays up for [`INITINFO_DURATION`] seconds, **but**:
///
/// * pressing any key ends it at once — and the key is *consumed* here
///   (`bconin2`), so it does not leak into the AUTO programs or the desktop;
/// * holding Shift, Control or Alternate ends the timed part at once, and
///   holding **Shift** then keeps the screen up for as long as it is held
///   (`:407-413`) — this is the "Hold `<Shift>` to pause this screen" affordance;
/// * a spurious `Delete` (the tail of a Ctrl+Alt+Shift+Delete reboot) is eaten
///   and the whole wait starts over (`:420-422`).
///
/// **Two things from the C are deliberately not here.**
///
/// 1. The boot-device selection (`olddev`/`dev`, `:390`, `:429-441`), which is
///    what makes the outer `while (1)` loop more than one pass: EmuTOS
///    redisplays the drive line and waits again when the user picks another
///    drive. NewTOS has no boot-device variable to select (see the hint-line
///    comment in [`run`]), so the loop always leaves after one pass — except
///    for the `Delete` `continue`, which is kept.
/// 2. `stop_until_interrupt()` (`:400-402`, `#if USE_STOP_INSN_TO_FREE_HOST_CPU`)
///    — an m68k `stop` that idles the CPU until the next interrupt. On this
///    hardware that would be a *hang*, not an idle: the real Pi-4 keyboard is a
///    polled USB-HID device (`drivers::usb::poll`, the PHASE-16 stack), so
///    nothing would ever wake a `wfi` and the wait could not be skipped at all.
///    The loop therefore drives [`tos_api::Host::pump_input`] — the same hook
///    the AES event loop and the blocking console reads use — on every pass.
///    That call *is* the keyboard on real hardware; on qemu-virt it is
///    redundant (the PL011 RX IRQ fills the same ring) but harmless.
///
/// The 200 Hz `hz_200` tick becomes the host's monotonic millisecond clock —
/// the same source the process loader snapshots into the `_hz_200` sysvar
/// (`tos_api::bdos::process::loader`, `$4BA`).
fn wait_for_user(host: &dyn tos_api::Host) -> u8 {
    let mut shiftbits;
    loop {
        // `long end = hz_200 + INITINFO_DURATION * 200UL;` (:388)
        let end = host.monotonic_millis() + INITINFO_DURATION * 1000;

        // `do { … } while (hz_200 < end);` (:392-404)
        loop {
            host.pump_input();
            shiftbits = host.keyboard_shift_state(); // `kbshift(-1)` (:394)
            // "If Shift, Control, Alt or normal key is pressed, stop waiting"
            // (:396-398)
            if shiftbits & MODE_SCA != 0 || host.console_has_input() {
                break;
            }
            if host.monotonic_millis() >= end {
                break;
            }
        }

        // "Wait while Shift is pressed, and normal key is not pressed" (:406-413)
        while shiftbits & MODE_SHIFT != 0 && !host.console_has_input() {
            host.pump_input();
            shiftbits = host.keyboard_shift_state();
        }

        // "if a non-modifier key was pressed, examine it" (:415-435). The
        // `bconstat2()` test and the `bconin2()` read collapse into one
        // non-blocking pop; only the ASCII byte matters (`LOBYTE`, :417).
        if let Some(key) = host.console_try_read_key() {
            if (key as u8).to_ascii_uppercase() == DEL_ASCII {
                continue; // "eat spurious Delete key press" (:420-422)
            }
        }

        break; // `if (dev == olddev) break;` (:436-437)
    }
    shiftbits
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
