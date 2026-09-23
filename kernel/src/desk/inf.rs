//! `desk::inf` — `NEWDESK.INF` read/save and the `deskinf.c` Show-Info /
//! Set-background dialogs.
//!
//! Reimplements two EmuTOS pieces that share the `NEWDESK.INF`/`EMUDESK.INF`
//! feature area, per [`docs/api/desktop/deskinf.md`] (the file-format appendix
//! and the Show-Info dialogs) and `deskapp.md` (`app_start`'s INF-position
//! apply loop, `:979-987`):
//!
//! * the **file format** — `desk/deskapp.c`'s `app_parse`/`app_start` (load) and
//!   `app_save` (save): the `#R`/`#Z`/`#E`/`#Q`/`#W` inline lines plus the
//!   `#M`/`#T`/`#O`/`#D`/`#G`/`#Y`/`#F`/`#P`/`#I`/`#N`/`#X`/`#V` `ANODE` lines,
//!   with the grid-unit ↔ pixel lifecycle for desktop-icon spots;
//! * the **dialogs** — `deskinf.c`'s `inf_file_folder` (Show Info + rename) and
//!   `inf_backgrounds` (Set Background).
//!
//! ## What is applied at boot
//!
//! Per the lead decision (EmuTOS-faithful): INF desktop-icon positions are
//! **grid-rounded via [`DeskState::snap_icon_grid`] with no collision check**
//! (`app_start` does exactly this — colliding INF positions overlap silently,
//! `deskins.md`). Collision avoidance is the install path's job (#20), not here.
//! View/sort ([`View`]/[`Sort`]), the background (`g_patcol`), the desktop-icon
//! grid positions, and the saved window rectangles are applied; a missing file
//! is not an error (the desktop keeps its built-in defaults).

use alloc::string::String;
use alloc::vec::Vec;

use tos_api::aes::object::{Object, TedInfo};
use tos_api::vdi::font::{CELL_H, CELL_W};

use crate::sync::SpinMutex;

use super::dosbind::{dos_close, dos_create, dos_gdrv, dos_open, dos_read, dos_sdrv, dos_write};
use super::global::{self, Anode, Grect, WSave, NIL, NUM_WNODES};
use super::icons;
use super::model::{AType, DateFmt, Sort, TimeFmt, View};

// ── GEMDOS file access (through the trap, [`super::dosbind`]) ────────────

/// The desktop configuration file, on the boot drive's root (EmuTOS keeps its
/// own `EMUDESK.INF` here; NewTOS uses the Atari-classic `NEWDESK.INF` name the
/// user's SD card carries).
const INF_PATH: &str = "C:\\NEWDESK.INF";

/// Upper bound on the file we will read (a full 64-`ANODE` file with strings is
/// a few KiB; this guards a runaway read).
const MAX_INF_BYTES: usize = 64 * 1024;

/// Read the whole `NEWDESK.INF` into a heap buffer, or [`None`] if it cannot be
/// opened (a missing file is **not** an error — the caller keeps defaults).
fn read_inf_file() -> Option<Vec<u8>> {
    let rc = dos_open(INF_PATH, 0);
    if rc < 0 {
        return None;
    }
    let handle = rc as i16;
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let n = dos_read(handle, chunk.len() as i32, chunk.as_mut_ptr());
        if n <= 0 {
            break;
        }
        let got = n as usize;
        buf.extend_from_slice(&chunk[..got.min(chunk.len())]);
        if got < chunk.len() || buf.len() >= MAX_INF_BYTES {
            break;
        }
    }
    dos_close(handle);
    Some(buf)
}

/// Create/truncate `NEWDESK.INF` and write `bytes`. Returns whether the full
/// buffer was written.
fn write_inf_file(bytes: &[u8]) -> bool {
    let rc = dos_create(INF_PATH, 0);
    if rc < 0 {
        return false;
    }
    let handle = rc as i16;
    let mut off = 0usize;
    let mut ok = true;
    while off < bytes.len() {
        let n = dos_write(handle, (bytes.len() - off) as i32, bytes[off..].as_ptr());
        if n <= 0 {
            ok = false;
            break;
        }
        off += n as usize;
    }
    dos_close(handle);
    ok
}

// ── `#E` environment flag bits (`deskinf.md` appendix, `deskapp.c`) ──────────

const INF_E1_VIEWTEXT: u8 = 0x80;
const INF_E1_SORTMASK: u8 = 0x60;
const INF_E1_SORTSHIFT: u8 = 5;
/// `INF_E1_CONFDEL` (`deskapp.c:89`) — 1 ⇒ confirm deletes.
const INF_E1_CONFDEL: u8 = 0x10;
/// `INF_E1_DCMASK` (`deskapp.c:91`) — double-click speed index, 0..4 valid.
const INF_E1_DCMASK: u8 = 0x07;
/// `INF_E2_IDTDATE` (`deskapp.c:96`) — 1 ⇒ take the date format from the `_IDT`
/// cookie and ignore [`INF_E2_DAYMONTH`].
const INF_E2_IDTDATE: u8 = 0x40;
/// `INF_E2_IDTTIME` (`deskapp.c:97`) — 1 ⇒ take the time format from `_IDT` and
/// ignore [`INF_E2_24HCLOCK`].
const INF_E2_IDTTIME: u8 = 0x20;
/// `INF_E2_DAYMONTH` (`deskapp.c:100`) — 1 ⇒ day before month.
const INF_E2_DAYMONTH: u8 = 0x04;
/// `INF_E2_24HCLOCK` (`deskapp.c:101`) — 1 ⇒ 24-hour clock.
const INF_E2_24HCLOCK: u8 = 0x02;
const INF_E5_NOSORT: u8 = 0x80;
const INF_E5_NOSIZE: u8 = 0x10;

/// `INF_E1_DEFAULT` = `CONFDEL|CONFCPY|dclick(2)` (`deskapp.c` `desk_inf_data1`).
const INF_E1_DEFAULT: u8 = 0x1A;
/// `INF_E2_DEFAULT` = `BLITTER|IDTDATE|IDTTIME`.
const INF_E2_DEFAULT: u8 = 0xE0;
/// `INF_E5_DEFAULT` = `APPDIR|ISFULL` (size-to-fit enabled, sorting on).
const INF_E5_DEFAULT: u8 = 0x60;

/// Number of `#K` menu-shortcut bytes (`NUM_SHORTCUTS`, `deskmain.h:18`).
/// The array itself lives in [`super::menu`], which owns the menu strings the
/// letters are rendered into; this module only moves it in and out of the file
/// (`deskapp.c:964-975` / `:1184-1192`).
use super::menu::NUM_SHORTCUTS;

// ── Installed-application extra-block bits + icon indices ────────────────────
/// `INF_AT_APPDIR` (`deskapp.c:126`) — 1 ⇒ set the current directory to the
/// application's directory (else to the top window's).
const INF_AT_APPDIR: u8 = 0x01;
/// `INF_AT_ISFULL` (`deskapp.c:127`) — 1 ⇒ pass the document's full path in the
/// command tail (else just the file name).
const INF_AT_ISFULL: u8 = 0x02;

use super::icons::{IG_APPL, IG_DOCU};

// ── The serializable INF model ──────────────────────────────────────────────

/// One `ANODE` line (`#M`/`#T`/`#O`/`#D`/`#G`/`#Y`/`#F`/`#P`/`#I`/`#N`/`#X`/
/// `#V`). Desktop icons (`is_desk`) carry a grid spot + drive letter; file
/// types (`has_extra`) carry a function-key/flags/args block.
#[derive(Clone, PartialEq, Eq, Default)]
struct InfAnode {
    /// One-character tag byte (`b'M'`, `b'T'`, …).
    tag: u8,
    /// `AF_ISDESK` implied by the tag (has `xx`/`yy` + drive letter).
    is_desk: bool,
    /// `AT_ISFILE && !AF_ISDESK` implied by the tag (has the extra block).
    has_extra: bool,
    /// Desktop grid column (`a_xspot`, grid units on disk).
    xspot: u8,
    /// Desktop grid row (`a_yspot`, grid units on disk).
    yspot: u8,
    /// `a_aicon` index (`FF` → `-1`).
    aicon: i16,
    /// `a_dicon` index (`FF` → `-1`).
    dicon: i16,
    /// Drive letter (`0` = none).
    letter: u8,
    /// `a_pappl` — application path / wildcard / label.
    pappl: String,
    /// `a_pdata` — data path / wildcard.
    pdata: String,
    /// Extra-block flags nibble (`INF_AT_APPDIR|INF_AT_ISFULL`, 0-3).
    flags: u8,
    /// Extra-block function-key number.
    funkey: u8,
    /// `a_pargs` — command-line arguments.
    pargs: String,
}

/// One `#W` saved-window-geometry line (character-cell units on disk).
#[derive(Clone, PartialEq, Eq, Default)]
struct InfWindow {
    /// Horizontal slider position (opaque slider units).
    hsl: u8,
    /// Vertical slider position.
    vsl: u8,
    /// X position in character cells.
    xc: u8,
    /// Y position in character cells.
    yc: u8,
    /// Width in character cells.
    wc: u8,
    /// Height in character cells.
    hc: u8,
    /// Saved directory path (`@`-terminated on disk; empty for a default slot).
    path: String,
}

/// The whole parsed / serialisable `NEWDESK.INF` state.
#[derive(Clone, PartialEq, Eq)]
struct InfData {
    /// `#E` env byte 1 (view/sort/confirm/dclick).
    e1: u8,
    /// `#E` env byte 2 (blitter/idt/overwrite/date-time format).
    e2: u8,
    /// `#E` env byte 5 (nosort/appdir/isfull/nosize/nocache).
    e5: u8,
    /// `#Q` background pattern/colour: three `(desktop, window)` pairs, one per
    /// plane-count bucket.
    patcol: [(u8, u8); 3],
    /// `#K` menu-shortcut letters, one per [`NUM_SHORTCUTS`] slot (`0` = none).
    /// EmuTOS lets **every** build read the line and only
    /// `CONF_WITH_DESKTOP_CONFIG` builds write it (`deskapp.c:961-965`,
    /// `:1183-1193`); ours has the dialog, so it does both.
    shortcuts: [u8; NUM_SHORTCUTS],
    /// `#Z` autorun program path (empty = none).
    autorun: String,
    /// `#Z` flag byte — EmuTOS writes `pa->a_flags & AF_ISCRYS`
    /// (`deskapp.c:1133`), i.e. `01` for a GEM application and `00` for a
    /// character-mode one. The AES reads it back as `isgem` in `process_inf2`
    /// (`geminit.c:443`) and it decides whether the autorun program starts in
    /// graphics or alpha mode, so it must survive a load/save round trip.
    autorun_gem: u8,
    /// `#W` saved window geometries, in file order.
    windows: Vec<InfWindow>,
    /// Every `ANODE` line, in file order.
    anodes: Vec<InfAnode>,
}

impl Default for InfData {
    fn default() -> Self {
        Self {
            e1: INF_E1_DEFAULT,
            e2: INF_E2_DEFAULT,
            e5: INF_E5_DEFAULT,
            patcol: [(0x41, 0x40), (0x43, 0x40), (0x43, 0x40)],
            // `memcpy(menu_shortcuts, default_shortcuts, NUM_SHORTCUTS)`
            // before `app_start` (`deskmain.c:2013`): a file with no `#K` line
            // leaves the built-in letters in place.
            shortcuts: super::menu::DEFAULT_SHORTCUTS,
            autorun: String::new(),
            autorun_gem: 0,
            windows: Vec::new(),
            anodes: Vec::new(),
        }
    }
}

/// The session INF: the last-parsed file, kept so a Save preserves the
/// file-type `ANODE`s and confirm/format preferences NewTOS does not yet edit
/// (mirroring EmuTOS keeping the whole `G.g_ahead` list live for the session).
static SESSION: SpinMutex<Option<InfData>> = SpinMutex::new(None);

// ── Field-scan primitives (`util/optimize.c` `scan_2`, `deskapp.c` `scan_str`)

/// A forward byte cursor over the INF buffer.
struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    /// The byte at the cursor (`0` past the end).
    fn peek(&self) -> u8 {
        self.b.get(self.i).copied().unwrap_or(0)
    }

    /// Consume and return the byte at the cursor.
    fn bump(&mut self) -> u8 {
        let c = self.peek();
        if self.i < self.b.len() {
            self.i += 1;
        }
        c
    }

    /// Skip a run of literal spaces (`scan_2`/`scan_str` both do this first).
    fn skip_spaces(&mut self) {
        while self.peek() == b' ' {
            self.i += 1;
        }
    }

    /// Whether the cursor sits at a line end (`\r`/`\n`/NUL) — the extra-block
    /// present-or-absent test.
    fn at_line_end(&self) -> bool {
        matches!(self.peek(), b'\r' | b'\n' | 0)
    }
}

/// One hex digit's value (`0` for a non-hex byte).
fn hexval(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'A'..=b'F' => c - b'A' + 10,
        b'a'..=b'f' => c - b'a' + 10,
        _ => 0,
    }
}

/// `scan_2` — a 2-hex-digit field: skip leading spaces; a `\r` at the field
/// start yields `0` with nothing consumed; otherwise read exactly two hex
/// digits (`util/optimize.c:202-219`).
fn scan_2(c: &mut Cursor) -> u8 {
    c.skip_spaces();
    if c.peek() == b'\r' {
        return 0;
    }
    let hi = hexval(c.bump());
    let lo = hexval(c.bump());
    (hi << 4) | lo
}

/// A `scan_2` icon field: `0xFF` → `-1` (the "no icon override" sentinel,
/// `app_parse:359-364`).
fn scan_icon(c: &mut Cursor) -> i16 {
    let v = scan_2(c);
    if v == 0xFF {
        -1
    } else {
        v as i16
    }
}

/// `scan_str` — an `@`-terminated string: skip leading spaces, copy until `@`
/// (consumed) or `\r`/`\n`/NUL (left in place, so the caller can detect the
/// line end), `deskapp.c:258-289`.
fn scan_str(c: &mut Cursor) -> String {
    c.skip_spaces();
    let mut s = String::new();
    loop {
        match c.peek() {
            b'@' => {
                c.i += 1;
                break;
            }
            b'\r' | b'\n' | 0 => break,
            ch => {
                s.push(ch as char);
                c.i += 1;
            }
        }
    }
    s
}

// ── Parser (`app_start`'s line loop + `app_parse`) ──────────────────────────

/// Parse a whole `NEWDESK.INF` buffer (`app_start`, `deskapp.c:851-976`): scan
/// forward for each `#`, read the one-character tag, dispatch. Bytes between a
/// field's end and the next `#` are skipped, matching EmuTOS's `if (*pcurr++ !=
/// '#') continue;`.
fn parse_inf(bytes: &[u8]) -> InfData {
    let mut d = InfData::default();
    let mut c = Cursor { b: bytes, i: 0 };
    while c.i < c.b.len() {
        if c.bump() != b'#' {
            continue;
        }
        let tag = c.bump();
        match tag {
            b'R' => {
                let _ = scan_2(&mut c); // revision level — always upgraded on save
            }
            b'Z' => {
                c.skip_spaces();
                // `AF_ISCRYS` (`deskapp.h:28`) — kept so `serialize` writes the
                // same byte back; the AES reads it as `isgem` (`geminit.c:443`).
                d.autorun_gem = scan_2(&mut c);
                d.autorun = scan_str(&mut c);
            }
            b'E' => {
                d.e1 = scan_2(&mut c);
                d.e2 = scan_2(&mut c);
                let _vh = scan_2(&mut c); // video mode high — read but discarded
                let _vl = scan_2(&mut c); // video mode low  — read but discarded
                d.e5 = scan_2(&mut c);
            }
            b'Q' => {
                for pair in d.patcol.iter_mut() {
                    pair.0 = scan_2(&mut c);
                    pair.1 = scan_2(&mut c);
                }
            }
            // `case 'K'` (`deskapp.c:961-975`) — all 33 slots, overwriting the
            // defaults. EmuTOS notes that *reading* the line is unconditional
            // even in a build that cannot save it (`:962-965`).
            b'K' => {
                for slot in d.shortcuts.iter_mut() {
                    *slot = scan_2(&mut c);
                }
            }
            b'W' => {
                d.windows.push(parse_window(&mut c));
            }
            b'T' | b'M' | b'O' | b'X' | b'V' | b'D' | b'G' | b'Y' | b'F' | b'P' | b'I' | b'N' => {
                d.anodes.push(parse_anode(&mut c, tag));
            }
            _ => {}
        }
    }
    d
}

/// Parse one `#W` line: `<hsl> <vsl> <xc> <yc> <wc> <hc> <00> <path>@`
/// (`app_start:889-913`). The 7th field is the unused placeholder.
fn parse_window(c: &mut Cursor) -> InfWindow {
    let hsl = scan_2(c);
    let vsl = scan_2(c);
    let xc = scan_2(c);
    let yc = scan_2(c);
    let wc = scan_2(c);
    let hc = scan_2(c);
    let _unused = scan_2(c);
    let path = scan_str(c);
    InfWindow { hsl, vsl, xc, yc, wc, hc, path }
}

/// Parse one `ANODE` line after its tag (`app_parse`, `deskapp.c:295-436`).
/// Desktop icons (`T`/`M`/`O`/`X`/`V`) carry `xx yy … L`; file types
/// (`G`/`Y`/`F`/`P`/`I`/`N`) carry the trailing `f kk pargs@` block when present.
fn parse_anode(c: &mut Cursor, tag: u8) -> InfAnode {
    let is_desk = matches!(tag, b'T' | b'M' | b'O' | b'X' | b'V');
    let has_extra = matches!(tag, b'G' | b'Y' | b'F' | b'P' | b'I' | b'N');

    let (xspot, yspot) = if is_desk { (scan_2(c), scan_2(c)) } else { (0, 0) };
    let aicon = scan_icon(c);
    let dicon = scan_icon(c);

    // The `<L>` field is a single literal character (space = no letter): consume
    // exactly one separator space, then one char — do NOT skip a run of spaces,
    // or a blank `L` would swallow the first char of `pappl`.
    let letter = if is_desk {
        if c.peek() == b' ' {
            c.i += 1;
        }
        let l = c.bump();
        if l == b' ' || l == b'\r' || l == b'\n' || l == 0 {
            0
        } else {
            l
        }
    } else {
        0
    };

    let pappl = scan_str(c);
    let pdata = scan_str(c);

    // The extra `f kk pargs@` block is present only when a non-space, non-EOL
    // byte follows `pdata` (the `desk_inf_data2` type defaults omit it).
    let (flags, funkey, pargs) = if has_extra {
        c.skip_spaces();
        if c.at_line_end() {
            (0, 0, String::new())
        } else {
            let flags = hexval(c.bump());
            let funkey = scan_2(c);
            let pargs = scan_str(c);
            (flags, funkey, pargs)
        }
    } else {
        (0, 0, String::new())
    };

    InfAnode { tag, is_desk, has_extra, xspot, yspot, aicon, dicon, letter, pappl, pdata, flags, funkey, pargs }
}

// ── Serialiser (`app_save`, `deskapp.c:1106-1300`) ──────────────────────────

/// Append two upper-case hex digits of `v` to `out`.
fn push_hex2(out: &mut Vec<u8>, v: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    out.push(HEX[(v >> 4) as usize]);
    out.push(HEX[(v & 0x0F) as usize]);
}

/// Append the hex byte for an icon index (`-1` → `FF`).
fn push_icon(out: &mut Vec<u8>, v: i16) {
    push_hex2(out, if v < 0 { 0xFF } else { (v & 0xFF) as u8 });
}

impl InfData {
    /// Serialise to the `NEWDESK.INF` byte grammar in `app_save`'s line order:
    /// `#R`, `#Z`, `#E`, `#Q`, all `#W`, then every `ANODE` line.
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();

        // #R — always written at the current revision level (2).
        out.extend_from_slice(b"#R 02\r\n");

        // #Z — only when an autorun program is set. The flag byte is
        // `pa->a_flags & AF_ISCRYS` (`deskapp.c:1133`), round-tripped from the
        // file so a Save does not silently downgrade a GEM autorun program to
        // character mode.
        if !self.autorun.is_empty() {
            out.extend_from_slice(b"#Z ");
            push_hex2(&mut out, self.autorun_gem);
            out.push(b' ');
            out.extend_from_slice(self.autorun.as_bytes());
            out.extend_from_slice(b"@\r\n");
        }

        // #E — the video-mode bytes are informational only; written as 00 00.
        out.extend_from_slice(b"#E ");
        push_hex2(&mut out, self.e1);
        out.push(b' ');
        push_hex2(&mut out, self.e2);
        out.extend_from_slice(b" 00 00 ");
        push_hex2(&mut out, self.e5);
        out.extend_from_slice(b"\r\n");

        // #Q — three (desktop, window) pattern/colour pairs.
        out.extend_from_slice(b"#Q ");
        for (i, pair) in self.patcol.iter().enumerate() {
            if i > 0 {
                out.push(b' ');
            }
            push_hex2(&mut out, pair.0);
            out.push(b' ');
            push_hex2(&mut out, pair.1);
        }
        out.extend_from_slice(b"\r\n");

        // #K — the menu-shortcut letters, one two-digit byte per slot, written
        // between `#Q` and the `#W` lines exactly as `app_save` orders them
        // (`deskapp.c:1176-1193`). The whole array is always written, including
        // the `00` slots, because the reader takes it positionally.
        out.extend_from_slice(b"#K");
        for k in self.shortcuts {
            out.push(b' ');
            push_hex2(&mut out, k);
        }
        out.extend_from_slice(b"\r\n");

        // #W — every saved window geometry.
        for w in &self.windows {
            out.extend_from_slice(b"#W ");
            for v in [w.hsl, w.vsl, w.xc, w.yc, w.wc, w.hc] {
                push_hex2(&mut out, v);
                out.push(b' ');
            }
            out.extend_from_slice(b"00 ");
            out.extend_from_slice(w.path.as_bytes());
            out.extend_from_slice(b"@\r\n");
        }

        // ANODE lines.
        for a in &self.anodes {
            out.push(b'#');
            out.push(a.tag);
            out.push(b' ');
            if a.is_desk {
                push_hex2(&mut out, a.xspot);
                out.push(b' ');
                push_hex2(&mut out, a.yspot);
                out.push(b' ');
            }
            push_icon(&mut out, a.aicon);
            out.push(b' ');
            push_icon(&mut out, a.dicon);
            out.push(b' ');
            if a.is_desk {
                out.push(if a.letter == 0 { b' ' } else { a.letter });
                out.push(b' ');
            }
            out.extend_from_slice(a.pappl.as_bytes());
            out.extend_from_slice(b"@ ");
            out.extend_from_slice(a.pdata.as_bytes());
            out.push(b'@');
            // Extra `<f><kk> <pargs>@` block only when it carries data (matches
            // the compact `desk_inf_data2` defaults, which omit it entirely):
            // one flags nibble, then the 2-digit funkey, then the args string.
            if a.has_extra && (a.flags != 0 || a.funkey != 0 || !a.pargs.is_empty()) {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push(b' ');
                out.push(HEX[(a.flags & 0x0F) as usize]);
                push_hex2(&mut out, a.funkey);
                out.push(b' ');
                out.extend_from_slice(a.pargs.as_bytes());
                out.push(b'@');
            }
            out.extend_from_slice(b"\r\n");
        }

        out
    }

    /// The built-in file-type default `ANODE`s (`desk_inf_data2`,
    /// `deskapp.c:181-189`) — generic per-extension entries. Used when no file
    /// has been read, so a fresh Save still writes a complete, valid file.
    fn default_type_anodes() -> Vec<InfAnode> {
        // (tag, aicon, dicon, pappl, pdata)
        let table: [(u8, i16, i16, &str, &str); 8] = [
            (b'F', -1, 0x07, "", "*.*"),
            (b'N', -1, 0x07, "", "*.*"),
            (b'D', -1, 0x02, "", "*.*"),
            (b'Y', 0x06, -1, "*.GTP", ""),
            (b'G', 0x06, -1, "*.APP", ""),
            (b'G', 0x06, -1, "*.PRG", ""),
            (b'P', 0x06, -1, "*.TTP", ""),
            (b'F', 0x06, -1, "*.TOS", ""),
        ];
        table
            .iter()
            .map(|&(tag, aicon, dicon, pappl, pdata)| InfAnode {
                tag,
                is_desk: false,
                has_extra: matches!(tag, b'G' | b'Y' | b'F' | b'P' | b'I' | b'N'),
                xspot: 0,
                yspot: 0,
                aicon,
                dicon,
                letter: 0,
                pappl: String::from(pappl),
                pdata: String::from(pdata),
                flags: 0,
                funkey: 0,
                pargs: String::new(),
            })
            .collect()
    }
}

// ── Sort ↔ `#E`-bit mapping ─────────────────────────────────────────────────

/// Decode the `#E` env bytes to `(view, sort, size_to_fit)`.
fn env_to_state(e1: u8, e5: u8) -> (View, Sort, bool) {
    let view = if e1 & INF_E1_VIEWTEXT != 0 { View::Text } else { View::Icon };
    let sort = if e5 & INF_E5_NOSORT != 0 {
        Sort::NoSort
    } else {
        match (e1 & INF_E1_SORTMASK) >> INF_E1_SORTSHIFT {
            1 => Sort::Type,
            2 => Sort::Size,
            3 => Sort::Date,
            _ => Sort::Name,
        }
    };
    let size_to_fit = e5 & INF_E5_NOSIZE == 0;
    (view, sort, size_to_fit)
}

/// Fold `(view, sort, size_to_fit)` into the `#E` `e1`/`e5` bytes, keeping every
/// other bit of the base bytes (confirm/dclick/format prefs) unchanged.
fn state_to_env(view: View, sort: Sort, size_to_fit: bool, base_e1: u8, base_e5: u8) -> (u8, u8) {
    let mut e1 = base_e1 & !(INF_E1_VIEWTEXT | INF_E1_SORTMASK);
    if view == View::Text {
        e1 |= INF_E1_VIEWTEXT;
    }
    let mut e5 = base_e5 & !(INF_E5_NOSORT | INF_E5_NOSIZE);
    match sort {
        Sort::Name => {}
        Sort::Type => e1 |= 1 << INF_E1_SORTSHIFT,
        Sort::Size => e1 |= 2 << INF_E1_SORTSHIFT,
        Sort::Date => e1 |= 3 << INF_E1_SORTSHIFT,
        Sort::NoSort => e5 |= INF_E5_NOSORT,
    }
    if !size_to_fit {
        e5 |= INF_E5_NOSIZE;
    }
    (e1, e5)
}

/// The Preferences-dialog half of the `#E` line, decoded from env bytes 1 and 2
/// — EmuTOS `app_start`'s `'E'` arm (`deskapp.c:917-934`):
///
/// ```c
/// cnxsave->cs_confdel  = ((envr & INF_E1_CONFDEL) != 0);
/// cnxsave->cs_dblclick =  (envr & INF_E1_DCMASK);
/// …
/// cnxsave->cs_datefmt = (envr & INF_E2_IDTDATE) ? DATEFORM_IDT
///                     : (envr & INF_E2_DAYMONTH) ? DATEFORM_DMY : DATEFORM_MDY;
/// cnxsave->cs_timefmt = (envr & INF_E2_IDTTIME) ? TIMEFORM_IDT
///                     : (envr & INF_E2_24HCLOCK) ? TIMEFORM_24H : TIMEFORM_12H;
/// ```
///
/// Returns `(confirm_deletes, dclick_index, time_format, date_format)`. The
/// `IDT` values are decoded faithfully even though the desktop cannot render
/// them ([`TimeFmt::Idt`] explains why) — they are what a `NEWDESK.INF` written
/// by EmuTOS carries, and losing them on read would silently rewrite the user's
/// file on the next save.
fn env_to_prefs(e1: u8, e2: u8) -> (bool, u8, TimeFmt, DateFmt) {
    let confdel = e1 & INF_E1_CONFDEL != 0;
    let dclick = e1 & INF_E1_DCMASK;
    let timefmt = if e2 & INF_E2_IDTTIME != 0 {
        TimeFmt::Idt
    } else if e2 & INF_E2_24HCLOCK != 0 {
        TimeFmt::H24
    } else {
        TimeFmt::H12
    };
    let datefmt = if e2 & INF_E2_IDTDATE != 0 {
        DateFmt::Idt
    } else if e2 & INF_E2_DAYMONTH != 0 {
        DateFmt::Dmy
    } else {
        DateFmt::Mdy
    };
    (confdel, dclick, timefmt, datefmt)
}

/// The inverse of [`env_to_prefs`] — EmuTOS `app_save`'s env assembly
/// (`deskapp.c:1138-1160`). Every bit outside this dialog's own is taken from
/// `base_e1`/`base_e2` unchanged, so the confirm-copy / blitter / cache bits
/// NewTOS does not edit survive a load/save round trip.
fn prefs_to_env(
    confdel: bool,
    dclick: u8,
    timefmt: TimeFmt,
    datefmt: DateFmt,
    base_e1: u8,
    base_e2: u8,
) -> (u8, u8) {
    let mut e1 = base_e1 & !(INF_E1_CONFDEL | INF_E1_DCMASK);
    if confdel {
        e1 |= INF_E1_CONFDEL;
    }
    e1 |= dclick & INF_E1_DCMASK;

    let mut e2 = base_e2 & !(INF_E2_IDTDATE | INF_E2_IDTTIME | INF_E2_DAYMONTH | INF_E2_24HCLOCK);
    match timefmt {
        TimeFmt::Idt => e2 |= INF_E2_IDTTIME,
        TimeFmt::H24 => e2 |= INF_E2_24HCLOCK,
        TimeFmt::H12 => {}
    }
    match datefmt {
        DateFmt::Idt => e2 |= INF_E2_IDTDATE,
        DateFmt::Dmy => e2 |= INF_E2_DAYMONTH,
        DateFmt::Mdy => {}
    }
    (e1, e2)
}

// ── Boot / runtime load + apply ─────────────────────────────────────────────

/// Read `NEWDESK.INF` and apply it to the live desktop state
/// (`app_start`'s INF-position apply loop + the env/background application).
///
/// Applies: the `#E` view/sort/size-to-fit, the `#Q` background (`g_patcol`),
/// the `#M`/`#T`/`#O` desktop-icon grid positions (cached for `app_blddesk`),
/// and the `#W` window rectangles (cached for `open_dir_window`). The parsed
/// file is stashed in [`SESSION`] so a later Save preserves the file-type
/// `ANODE`s and the confirm/format prefs. A missing file leaves every default
/// in place and is **not** an error.
///
/// Must run **after** `app_start` (so `g_icw`/`g_ich` exist for the grid
/// rounding) and **before** `win_view`/`app_blddesk` (so the view and icon
/// positions take effect). Returns whether a file was actually loaded.
/// Forget the parsed `.INF` of the session that has ended — the `inf` half of
/// [`super::reset`] (A106). [`load_and_apply`] fills it again at the next
/// desktop start; dropping it here means a file that has since disappeared
/// cannot be re-saved from a stale in-memory copy.
pub(super) fn reset() {
    *SESSION.lock() = None;
}

pub(super) fn load_and_apply() -> bool {
    let Some(bytes) = read_inf_file() else {
        return false;
    };
    // `if (inf_data[0] != '#') build_inf(inf_data, …);` (`deskapp.c:833-834`).
    // EmuTOS decides "is there INF data?" on the **signature**, not on whether
    // a file exists — `shel_get` hands it a buffer either way, and a buffer
    // that does not start with `#` is not an INF. Our test image ships a
    // 21-byte `newdesk placeholder\r\n` for exactly that case, so the check is
    // not theoretical: without it the desktop comes up with no icons at all.
    if bytes.first() != Some(&b'#') {
        return false;
    }
    let data = parse_inf(&bytes);
    apply(&data);
    *SESSION.lock() = Some(data);
    true
}

/// `build_inf` (`deskapp.c:696-752`) — synthesise the default `EMUDESK.INF`
/// and apply it, for the case [`load_and_apply`] found no INF data.
///
/// EmuTOS assembles three pieces of *text* and lets its own parser read them
/// back: `desk_inf_data1`'s `#R`/`#E`/`#Q`/`#W` lines (`deskapp.c:171-180`),
/// the drive map's `#M` lines, and `desk_inf_data2`'s file-type lines, then the
/// `#T` (and `#O`) fixtures. Here the first piece **is** [`InfData::default`]
/// — the same `#E 1A E0 00 00 60` and `#Q 41 40 43 40 43 40` byte for byte —
/// and the third is [`InfData::default_type_anodes`], already reached by
/// `build_from_state` on a first-ever save; only the icon half has to be built,
/// which [`build_desk_icons`] does.
pub(super) fn build_inf() {
    apply(&InfData::default());
    build_desk_icons();
}

/// Apply a parsed [`InfData`] — EmuTOS `app_start`'s dispatch loop
/// (`deskapp.c:845-976`).
///
/// EmuTOS splits this deliberately: the `#M`/`#T`/`#O` icon spots and the `#Q`
/// pattern/colours go straight into `GLOBES` (they belong to the desktop's own
/// object tree), while the `#E` preferences and the `#W` window geometries go
/// into the **context save area** `G.g_cnxsave` and reach the live desktop only
/// later, through `cnx_get` (`deskmain.c:2055`). The same split is kept here.
fn apply(data: &InfData) {
    // #E → cnxsave->cs_view / cs_sort / cs_sizefit (`deskapp.c:994-1013`) and
    // the Preferences half (`deskapp.c:917-934`).
    let (view, sort, size_to_fit) = env_to_state(data.e1, data.e5);
    let (confdel, dclick, timefmt, datefmt) = env_to_prefs(data.e1, data.e2);

    // #M/#T/#O → `G.g_ahead` (`app_parse`'s tag switch, `deskapp.c:300-346`).
    // The spot is still in grid units here; `app_start` scales it and snaps it
    // once the whole file is parsed (`:979-987`), which is what the closure
    // below does — it is the first point at which `g_icw`/`g_ich` are in hand.
    let parsed: Vec<InfAnode> = data.anodes.iter().filter(|a| a.is_desk).cloned().collect();

    // #W → cnxsave->cs_wnode[] (character cells → pixels, `deskapp.c:889-913`:
    // `pws->x_save *= gl_wchar` and so on for each of the four).
    global::with(|g| {
        g.g_cnxsave.cs_view = view;
        g.g_cnxsave.cs_sort = sort;
        g.g_cnxsave.cs_sizefit = size_to_fit;
        g.g_cnxsave.cs_confdel = confdel;
        g.g_cnxsave.cs_dblclick = dclick;
        g.g_cnxsave.cs_timefmt = timefmt;
        g.g_cnxsave.cs_datefmt = datefmt;
        g.g_cnxsave.cs_wnode = Default::default();
        for (slot, w) in data.windows.iter().take(NUM_WNODES).enumerate() {
            g.g_cnxsave.cs_wnode[slot] = WSave {
                x_save: w.xc as i16 * CELL_W,
                y_save: w.yc as i16 * CELL_H,
                w_save: w.wc as i16 * CELL_W,
                h_save: w.hc as i16 * CELL_H,
                hsl_save: w.hsl as i16,
                vsl_save: w.vsl as i16,
                pth_save: w.path.clone(),
            };
        }
        g.g_ahead = parsed
            .iter()
            .map(|a| {
                let a_type = match a.tag {
                    b'T' => AType::Trash,
                    b'O' => AType::Printer,
                    _ => AType::Disk,
                };
                // `x = pa->a_xspot * G.g_icw; y = pa->a_yspot * G.g_ich + …;
                //  snap_icon(x, y, &pa->a_xspot, &pa->a_yspot, 0, 0);`
                // (`deskapp.c:981-984`) — grid units on disk, pixels in memory.
                let (x, y) = g.snap_icon(a.xspot as i16 * g.g_icw, a.yspot as i16 * g.g_ich);
                Anode {
                    a_type,
                    a_letter: a.letter,
                    a_aicon: a.aicon,
                    a_pappl: a.pappl.clone(),
                    a_xspot: x,
                    a_yspot: y,
                    a_obid: NIL,
                }
            })
            .collect();
    });
    // #K → `menu_shortcuts[]` (`deskapp.c:967-974`). The array is the desktop's
    // live shortcut table; the *rendering* of it into the menu strings happens
    // later, in `install_shortcuts` — EmuTOS runs that once `app_start` has
    // returned (`deskmain.c:2023`), which for NewTOS is
    // `super::menu::install_menu_bar`.
    super::menu::set_shortcuts(data.shortcuts);

    // #Q → g_patcol (the >2-plane bucket drives the live desktop background).
    let patcol = [
        data.patcol[0].0,
        data.patcol[0].1,
        data.patcol[1].0,
        data.patcol[1].1,
        data.patcol[2].0,
        data.patcol[2].1,
    ];
    global::set_patcol_all(patcol);
}

/// The desktop-icon half of `build_inf` (`deskapp.c:696-752`) — synthesise
/// `G.g_ahead` from the live drive map plus the Trash and Printer fixtures.
///
/// EmuTOS reaches this when neither `shel_get` nor the disk produced an
/// `EMUDESK.INF`: it *writes the `#M`/`#T`/`#O` text* into a buffer and lets its
/// own parser read it back. NewTOS goes straight to the nodes — the text
/// detour exists only because EmuTOS has one parser and wants one code path,
/// and a round trip through a string buffer is not observable. The **layout
/// rules are its**: drives fill the grid left-to-right, `icon_index % xcnt`
/// columns and `icon_index / xcnt` rows (`:719-721`); the Trash goes to the
/// bottom-left, moved to the bottom-**right** if the last drive icon reached
/// that row (`:738-744`); the Printer takes the bottom-right only if the Trash
/// did not (`:747-754`).
///
/// A no-op when `G.g_ahead` is already populated, so the two callers — the
/// tail of [`apply`] and the no-file arm of [`load_and_apply`] — are the same
/// call.
fn build_desk_icons() {
    if global::with(|g| !g.g_ahead.is_empty()).unwrap_or(true) {
        return;
    }
    // `drivemask = dos_sdrv(dos_gdrv());` (`deskapp.c:714`). Read outside the
    // state lock: it is a GEMDOS trap, not desktop state.
    let drivemask = dos_sdrv(dos_gdrv());

    global::with(|g| {
        let icw = g.g_icw.max(1);
        let ich = g.g_ich.max(1);
        // `build_inf`'s grid counts are `app_start`'s locals, derived from the
        // cell size and the inter-icon gap — **not** from `g_icw`/`g_ich`
        // (`deskapp.c:813-817`, `xcnt = G.g_desk.g_w / (G.g_wicon+MIN_WINT)`).
        // The two agree whenever the division is exact and can differ by one
        // column when it is not, so the source formula is the one used here.
        let screen_h = g.g_desk.y + g.g_desk.h;
        let xcnt = (g.g_desk.w / (g.g_wicon + global::min_wint(screen_h))).max(1);
        let ycnt = (g.g_desk.h / (g.g_hicon + global::MIN_HINT)).max(1);

        let mut icon_index = 0i16;
        let mut drive_y = 0i16;
        for drive in 0..26u32 {
            if drivemask & (1 << drive) == 0 {
                continue;
            }
            let drive_x = icon_index % xcnt;
            drive_y = icon_index / xcnt;
            let letter = b'A' + drive as u8;
            // `icon_type = (i > 1) ? IG_HARD : IG_FLOPPY;` (`deskapp.c:721`).
            let aicon = if drive > 1 { icons::IG_HARD } else { icons::IG_FLOPPY };
            g.g_ahead.push(Anode {
                a_type: AType::Disk,
                a_letter: letter,
                a_aicon: aicon,
                a_pappl: disk_label(letter),
                a_xspot: drive_x * icw,
                a_yspot: drive_y * ich,
                a_obid: NIL,
            });
            icon_index += 1;
        }

        // Trash: bottom-left, or bottom-right when the drives already reached
        // the bottom row (`deskapp.c:738-744`).
        let icon_y = ycnt - 1;
        let trash_x = if drive_y >= icon_y { xcnt - 1 } else { 0 };
        g.g_ahead.push(Anode {
            a_type: AType::Trash,
            a_letter: 0,
            a_aicon: icons::IG_TRASH,
            a_pappl: String::from(TRASH_LABEL),
            a_xspot: trash_x * icw,
            a_yspot: icon_y * ich,
            a_obid: NIL,
        });

        // Printer: only "if room", i.e. only when the Trash stayed on the left
        // (`deskapp.c:747-754`, `CONF_WITH_PRINTER_ICON`).
        if trash_x == 0 {
            g.g_ahead.push(Anode {
                a_type: AType::Printer,
                a_letter: 0,
                a_aicon: icons::IG_PRINT,
                a_pappl: String::from(PRINTER_LABEL),
                a_xspot: (xcnt - 1) * icw,
                a_yspot: icon_y * ich,
                a_obid: NIL,
            });
        }
    });
}

// ── The built-in desktop-icon labels ────────────────────────────
//
// EmuTOS's wording, `desk_rsc.c:2481-2483` (`STDISK`/`STTRASH`/`STPRINT`). Both
// a hard disk and a floppy get `STDISK` plus the drive letter — `build_inf`
// prints `"%s %c"` (`deskapp.c:723`) and `install_drive` builds the same string
// (`deskins.c:247`), so an icon added by *Install devices* is labelled exactly
// like one the desktop built at boot. The icon *picture* is what distinguishes
// the two (`IG_HARD` vs `IG_FLOPPY`), not the text.

/// `STDISK` — the word a drive icon's label starts with; [`disk_label`] appends
/// the drive letter.
pub(super) const DISK_LABEL: &str = "DISK";
/// Label of the Trash icon, `STTRASH`.
pub(super) const TRASH_LABEL: &str = "TRASH";
/// Label of the Printer icon, `STPRINT`.
pub(super) const PRINTER_LABEL: &str = "PRINTER";

/// `sprintf(G.g_work, "%s %c", desktop_str_addr(STDISK), pa->a_letter)`
/// (`deskins.c:247`, and the same text from `build_inf`, `deskapp.c:723`).
pub(super) fn disk_label(letter: u8) -> String {
    let mut s = String::from(DISK_LABEL);
    s.push(' ');
    s.push(letter as char);
    s
}

/// The pixel rectangle to open the window in slot `slot` at, taken from that
/// slot's [`WSave`] — EmuTOS `win_alloc`, which reads the new window's default
/// geometry straight out of `G.g_cnxsave->cs_wnode[G.g_wcnt]`
/// (`deskwin.c:165-177`). [`None`] when the slot holds no saved window, and the
/// caller then uses its own staggered fallback.
///
/// **Named deviation.** EmuTOS takes that rectangle *unconditionally*, because
/// its built-in `EMUDESK.INF` always supplies four sensible default `#W` lines
/// (`desk_inf_data1`, `deskapp.c:177-180`: `02 06 26 0C`, `02 08 26 0C`, …)
/// with empty paths — geometry without a window. NewTOS ships no such defaults,
/// and `cnx_get`'s validation turns an all-zero slot into the **whole desktop**
/// (`w_save <= 0` → `g_desk.g_w`, `deskmain.c:1476-1483`), so taking it
/// unconditionally would open every fresh window full-screen. Until the built-in
/// `#W` defaults are adopted — a visible change to the boot picture, and a
/// decision of its own — an unused slot supplies no rectangle.
pub(super) fn window_rect(slot: usize) -> Option<Grect> {
    global::with(|g| {
        let ws = g.g_cnxsave.cs_wnode.get(slot)?;
        (!ws.pth_save.is_empty() && ws.w_save > 0 && ws.h_save > 0)
            .then_some(Grect { x: ws.x_save, y: ws.y_save, w: ws.w_save, h: ws.h_save })
    })
    .flatten()
}

// ── Save (`app_save`, `deskapp.c:1106-1290`) ────────────────────────────────

/// Build an [`InfData`] snapshot — EmuTOS `app_save`.
///
/// `app_save` reads the **context save area**, not the live desktop: the
/// `#E` preference bits come from `cnxsave->cs_view`/`cs_sort`/`cs_sizefit`
/// (`deskapp.c:1136-1170`) and every `#W` line from `cnxsave->cs_wnode[]`
/// (`:1193-1203`). `G.g_cnxsave` is brought up to date by the `cnx_put()` its
/// callers run first (`deskmain.c:891,2154`). The desktop-icon spots and
/// `g_patcol` are read from the live `GLOBES`, exactly as EmuTOS does.
fn build_from_state() -> InfData {
    let session = SESSION.lock().clone();

    // Env + desktop-icon anodes, under one lock.
    #[allow(clippy::type_complexity)]
    let (view, sort, size_to_fit, prefs, desk_anodes): (View, Sort, bool, (bool, u8, TimeFmt, DateFmt), Vec<InfAnode>) = global::with(|g| {
        // The `ANODE` half of `app_save` (`deskapp.c:1216-1262`), for the
        // `AF_ISDESK` nodes: one line per entry of `G.g_ahead`, in list order,
        // with the spot converted back to grid units
        // (`pa->a_xspot/G.g_icw`, `:1258-1259`). Reading the **list** and not
        // the object tree is what EmuTOS does, and it is why a node whose icon
        // could not be allocated (`a_obid == NIL`) still survives a save.
        let icw = g.g_icw.max(1);
        let ich = g.g_ich.max(1);
        let anodes: Vec<InfAnode> = g
            .g_ahead
            .iter()
            .map(|pa| InfAnode {
                tag: match pa.a_type {
                    AType::Trash => b'T',
                    AType::Printer => b'O',
                    _ => b'M',
                },
                is_desk: true,
                has_extra: false,
                xspot: (pa.a_xspot / icw).clamp(0, 255) as u8,
                yspot: (pa.a_yspot / ich).clamp(0, 255) as u8,
                aicon: pa.a_aicon,
                // A desktop icon has no data icon: EmuTOS's `install_drive`
                // and `install_desktop_icon` both set `a_dicon = NIL`
                // (`deskins.c:250`, `:854`), which `app_save` writes as `FF`.
                dicon: -1,
                letter: pa.a_letter,
                pappl: pa.a_pappl.clone(),
                pdata: String::new(),
                flags: 0,
                funkey: 0,
                pargs: String::new(),
            })
            .collect();
        (
            g.g_cnxsave.cs_view,
            g.g_cnxsave.cs_sort,
            g.g_cnxsave.cs_sizefit,
            (
                g.g_cnxsave.cs_confdel,
                g.g_cnxsave.cs_dblclick,
                g.g_cnxsave.cs_timefmt,
                g.g_cnxsave.cs_datefmt,
            ),
            anodes,
        )
    })
    .unwrap_or((
        View::Icon,
        Sort::Name,
        true,
        (true, global::DEF_DCLICK, TimeFmt::H24, DateFmt::Mdy),
        Vec::new(),
    ));

    // Base env / patcol / file-type anodes: preserved from the session, or the
    // built-in defaults on a first-ever save.
    let base = session.unwrap_or_default();
    let (e1, e5) = state_to_env(view, sort, size_to_fit, base.e1, base.e5);
    // The Preferences bits go into the *same* `e1` byte, in a disjoint bit
    // range (`CONFDEL|DCMASK` vs `VIEWTEXT|SORTMASK`), so they are folded in on
    // top of the result rather than from `base.e1` again.
    let (confdel, dclick, timefmt, datefmt) = prefs;
    let (e1, e2) = prefs_to_env(confdel, dclick, timefmt, datefmt, e1, base.e2);

    let mut anodes = desk_anodes;
    let type_anodes: Vec<InfAnode> = if base.anodes.iter().any(|a| !a.is_desk) {
        base.anodes.iter().filter(|a| !a.is_desk).cloned().collect()
    } else {
        InfData::default_type_anodes()
    };
    anodes.extend(type_anodes);

    InfData {
        e1,
        e2,
        e5,
        patcol: {
            let p = global::patcol_all();
            [(p[0], p[1]), (p[2], p[3]), (p[4], p[5])]
        },
        // The live array, not the session copy: the Desktop-configuration
        // dialog edits it in place, and `app_save` writes `menu_shortcuts[]`
        // (`deskapp.c:1188-1189`).
        shortcuts: super::menu::shortcuts(),
        autorun: base.autorun,
        autorun_gem: base.autorun_gem,
        windows: collect_open_windows(),
        anodes,
    }
}

/// The `#W` lines of `app_save` (`deskapp.c:1192-1203`): **every** slot of the
/// context save area, in slot order, converted from pixels to character cells.
/// EmuTOS writes all `NUM_WNODES` lines unconditionally — an unused slot is a
/// line with an empty path — because the rectangles of the unused slots are the
/// default geometry `win_alloc` hands the next window that opens.
fn collect_open_windows() -> Vec<InfWindow> {
    global::with(|g| {
        g.g_cnxsave
            .cs_wnode
            .iter()
            .map(|ws| InfWindow {
                hsl: ws.hsl_save.clamp(0, 255) as u8,
                vsl: ws.vsl_save.clamp(0, 255) as u8,
                xc: (ws.x_save / CELL_W).clamp(0, 255) as u8,
                yc: (ws.y_save / CELL_H).clamp(0, 255) as u8,
                wc: (ws.w_save / CELL_W).clamp(0, 255) as u8,
                hc: (ws.h_save / CELL_H).clamp(0, 255) as u8,
                path: ws.pth_save.clone(),
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Options → Save desktop: serialise the context save area + the live icon
/// positions and write `NEWDESK.INF` — EmuTOS `app_save(TRUE)`. The caller runs
/// `cnx_put()` first (`deskmain.c:889-893`, `SAVEITEM`). Returns whether the
/// write succeeded.
pub(super) fn save_desktop() -> bool {
    let data = build_from_state();
    let bytes = data.serialize();
    let ok = write_inf_file(&bytes);
    if ok {
        *SESSION.lock() = Some(data);
    }
    ok
}

// ── Install Application: the `ANODE` application-association model ────────────
//
// EmuTOS `deskins.c` `ins_app_dialog` creates/edits an `ANODE` whose `a_pappl`
// is the full path of an application and whose `a_pdata` is a document wildcard
// (`*.EXT`), so that later double-clicking a matching document launches the
// application with the document as its command tail (`desksupp.c` `do_aopen`'s
// `installed_datafile` branch). NewTOS keeps that association in the same
// `NEWDESK.INF` `#G`/`#Y`/`#P`/`#F` line an `ANODE` serialises to (see
// [`InfData::serialize`] and EmuTOS `deskapp.c:1216-1273` `app_save`), so it
// survives Save Desktop + reboot exactly like the built-in type defaults.

/// The application flavour chosen in the Install-Application dialog — selects
/// the `NEWDESK.INF` line tag exactly as EmuTOS `app_save` (`deskapp.c:1232-
/// 1235`) derives it from `AF_ISCRYS`/`AF_ISPARM`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AppFlavor {
    /// GEM application, no parameters (`AF_ISCRYS`) → `#G`.
    Gem,
    /// Plain TOS application, no parameters → `#F`.
    Tos,
    /// GEM application that takes parameters (`AF_ISCRYS|AF_ISPARM`) → `#Y`.
    Gtp,
    /// TOS application that takes parameters (`AF_ISPARM`) → `#P`.
    Ttp,
}

impl AppFlavor {
    /// The `NEWDESK.INF` `ANODE` tag byte for this flavour (`app_save`).
    fn tag(self) -> u8 {
        match self {
            AppFlavor::Gem => b'G',
            AppFlavor::Tos => b'F',
            AppFlavor::Gtp => b'Y',
            AppFlavor::Ttp => b'P',
        }
    }

    /// Recover the flavour from a serialised `ANODE` tag byte (the load path).
    fn from_tag(tag: u8) -> AppFlavor {
        match tag {
            b'G' => AppFlavor::Gem,
            b'Y' => AppFlavor::Gtp,
            b'P' => AppFlavor::Ttp,
            _ => AppFlavor::Tos, // 'F' and anything else
        }
    }
}

/// A resolved document→application association, returned by
/// [`lookup_association`] for the desktop's document-open path.
#[derive(Clone)]
pub(super) struct Association {
    /// Full `C:` path of the associated application (`a_pappl`).
    pub app_path: String,
    /// `AF_ISFULL`: pass the document's full path in the tail (else file name).
    pub full_path: bool,
    /// The stored fixed argument prefix (`a_pargs`), prepended to the tail.
    pub args: String,
}

/// Turn a document-type field (the extension the user types in the dialog, e.g.
/// `TXT`) into the stored `a_pdata` wildcard, mirroring EmuTOS
/// `ins_app_dialog` (`deskins.c:559-562`): prefix `*.`, upper-case, and collapse
/// a leading `*` to the all-files `*.*` spec.
fn doc_mask_from_ext(ext: &str) -> String {
    let e = ext.trim().trim_start_matches("*.").to_ascii_uppercase();
    if e.starts_with('*') || e.is_empty() {
        String::from("*.*")
    } else {
        let mut s = String::from("*.");
        s.push_str(&e);
        s
    }
}

/// The document-type extension currently associated with `app_path` (the
/// `a_pdata` wildcard minus its `*.` prefix), for pre-filling the dialog when an
/// application is already installed. [`None`] if it is not installed.
pub(super) fn installed_doctype(app_path: &str) -> Option<(String, AppFlavor, bool)> {
    let guard = SESSION.lock();
    let data = guard.as_ref()?;
    let a = data
        .anodes
        .iter()
        .find(|a| !a.is_desk && is_real_path(&a.pappl) && a.pappl.eq_ignore_ascii_case(app_path))?;
    let ext = a.pdata.strip_prefix("*.").unwrap_or(&a.pdata);
    Some((String::from(ext), AppFlavor::from_tag(a.tag), a.flags & INF_AT_ISFULL != 0))
}

/// Whether `pappl` is a concrete file path rather than a wildcard/empty
/// placeholder — EmuTOS `is_installed` (`deskins.c:71-80`).
fn is_real_path(pappl: &str) -> bool {
    !matches!(pappl.as_bytes().first(), None | Some(b'*') | Some(b'?'))
}

/// Ensure [`SESSION`] exists and carries the built-in file-type default
/// `ANODE`s, so a subsequently-added association is preserved by
/// [`build_from_state`]'s "any non-desk anode" branch on the next Save.
fn session_with_type_defaults(guard: &mut Option<InfData>) -> &mut InfData {
    let data = guard.get_or_insert_with(InfData::default);
    if !data.anodes.iter().any(|a| !a.is_desk) {
        data.anodes.extend(InfData::default_type_anodes());
    }
    data
}

/// Install (or re-install) an application association into the in-memory INF
/// model — EmuTOS `ins_app_dialog`'s `APINSTAL` branch (`deskins.c:511-582`).
/// `app_path` is the application's full `C:` path, `doc_ext` the document-type
/// extension the user entered, `flavor` the GEM/TOS/(G)TP flavour, `full_path`
/// the `AF_ISFULL` choice, `appdir` the `AF_APPDIR` choice, and `args` the fixed
/// argument prefix. Re-installing the same application updates its existing
/// record in place (one association record per application, as EmuTOS
/// `app_afind_by_name` resolves). The change is persisted by Save Desktop.
pub(super) fn install_application(
    app_path: &str,
    doc_ext: &str,
    flavor: AppFlavor,
    full_path: bool,
    appdir: bool,
    args: &str,
) {
    let mut guard = SESSION.lock();
    let data = session_with_type_defaults(&mut guard);

    let tag = flavor.tag();
    let pdata = doc_mask_from_ext(doc_ext);
    let flags = (if appdir { INF_AT_APPDIR } else { 0 }) | (if full_path { INF_AT_ISFULL } else { 0 });

    if let Some(a) = data
        .anodes
        .iter_mut()
        .find(|a| !a.is_desk && is_real_path(&a.pappl) && a.pappl.eq_ignore_ascii_case(app_path))
    {
        a.tag = tag;
        a.has_extra = true;
        a.aicon = IG_APPL;
        a.dicon = IG_DOCU;
        a.pdata = pdata;
        a.flags = flags;
        a.pargs = String::from(args);
    } else {
        // New application: insert ahead of the generic type defaults so a
        // document matches the specific installed app first (EmuTOS `app_alloc`
        // links new ANODEs at the head; `app_save` reverses so generics trail).
        data.anodes.insert(
            0,
            InfAnode {
                tag,
                is_desk: false,
                has_extra: true,
                xspot: 0,
                yspot: 0,
                aicon: IG_APPL,
                dicon: IG_DOCU,
                letter: 0,
                pappl: String::from(app_path),
                pdata,
                flags,
                funkey: 0,
                pargs: String::from(args),
            },
        );
    }
}

/// Remove an installed application association — EmuTOS `ins_app_dialog`'s
/// `APREMOVE` branch (`deskins.c:584-589`). Returns whether a record was
/// removed. The change is persisted by Save Desktop.
pub(super) fn remove_application(app_path: &str) -> bool {
    let mut guard = SESSION.lock();
    let Some(data) = guard.as_mut() else {
        return false;
    };
    let before = data.anodes.len();
    data.anodes
        .retain(|a| !(!a.is_desk && is_real_path(&a.pappl) && a.pappl.eq_ignore_ascii_case(app_path)));
    data.anodes.len() != before
}

/// Resolve a document `name` (a bare 8.3 file name, e.g. `README.TXT`) to its
/// installed application, if any — the lookup EmuTOS performs implicitly when it
/// binds an `FNODE` to its matching `ANODE` during a directory scan and then
/// `do_aopen` sees `is_installed(pa) && !isapp`. Walks the session `ANODE`s for
/// the first *installed* file record whose `a_pdata` wildcard matches `name`.
pub(super) fn lookup_association(name: &[u8]) -> Option<Association> {
    let guard = SESSION.lock();
    let data = guard.as_ref()?;
    for a in &data.anodes {
        if a.is_desk || !is_real_path(&a.pappl) || a.pdata.is_empty() {
            continue;
        }
        if super::model::wildcmp(a.pdata.as_bytes(), name) {
            return Some(Association {
                app_path: a.pappl.clone(),
                full_path: a.flags & INF_AT_ISFULL != 0,
                args: a.pargs.clone(),
            });
        }
    }
    None
}

// ── Shared dialog open/close plumbing (`deskinf.c:345-402`) ─────────────────
//
// Every desktop dialog goes through the same three AES calls, and they are the
// reason a dialog neither lands under the menu bar nor has to repaint the whole
// screen when it goes away:
//
//   form_center(tree, &x,&y,&w,&h);            centre the ROOT, report the
//                                              outer rect (outline + shadow)
//   form_dial(FMD_START, 0,0,0,0, x,y,w,h);    reserve that rect
//   objc_draw(tree, ROOT, MAX_DEPTH, x,y,w,h); paint the dialog into it
//   ... form_do(tree, start) ...
//   form_dial(FMD_FINISH, 0,0,0,0, x,y,w,h);   release + repaint that rect
//   drain the redraw messages FMD_FINISH queued
//
// EmuTOS' `fm_dial` does **not** photograph the screen on FMD_START
// (`aes/gemfmlib.c:365-369` is a bare `break`): the restore is the FMD_FINISH
// repaint — `w_drawdesk(pt)` plus a `WM_REDRAW` to every window the rectangle
// touches (`:375-379`). Those messages are why `end_dialog` has to drain the
// pipe here instead of leaving them for the next `evnt_multi`: the desktop
// answers its own messages directly, so an undrained redraw would arrive out of
// sequence and show up as a partial refresh (`deskinf.c:371-378`, verbatim).

/// `start_dialog` (EmuTOS `deskinf.c:349-357`) — centre `tree`, constrain it to
/// the desktop rectangle (`rc_intersect(&gl_rfull, &t)`), reserve that area with
/// `form_dial(FMD_START)` and paint the tree into it. Returns the rectangle, to
/// be handed back to [`end_dialog`]-equivalent callers that keep it.
pub(super) fn start_dialog(tree: u64) -> Grect {
    let mut t = super::form_center(tree);
    // `rc_intersect(&gl_rfull, &t)` — `gl_rfull` is the screen minus the menu
    // row, i.e. exactly `G.g_desk` (`aes/gemgraf.c:423-424`, `deskmain.c:1959`).
    if let Some(clipped) = super::redraw::rc_intersect(super::g_desk(), t) {
        t = clipped;
    }
    super::form_dial_start(t);
    super::objc_draw_dialog(tree, t);
    t
}

/// `end_dialog` (EmuTOS `deskinf.c:363-390`) — release the dialog's rectangle
/// with `form_dial(FMD_FINISH)` (which repaints the desktop under it and fans a
/// `WM_REDRAW` out to every window it covered), then drain the messages that
/// just queued through the desktop's own `hndl_msg`, exactly as EmuTOS does and
/// for the reason its comment gives.
pub(super) fn end_dialog(tree: u64) {
    let t = super::form_center(tree);
    super::form_dial_finish(t);

    // `while (evnt_multi(MU_TIMER|MU_MESAG, …) & MU_MESAG) hndl_msg();`
    // The MU_TIMER with a zero timer is EmuTOS' "quick out": the call returns
    // immediately once no message is left.
    let mut msg = [0i16; 8];
    loop {
        let ev = super::evnt_multi(super::MU_TIMER | super::MU_MESAG, &mut msg);
        if ev.which & super::MU_MESAG == 0 {
            break;
        }
        super::main::hndl_msg(&msg);
    }
}

/// `inf_show` (EmuTOS `deskinf.c:396-402`) — put a dialog up, run it, take it
/// down. Returns `form_do`'s exit object.
pub(super) fn inf_show(tree: u64, start: i16) -> i16 {
    start_dialog(tree);
    let exitobj = super::form_do(tree, start);
    end_dialog(tree);
    exitobj
}

// ── Dialog-tree primitives, shared by every `desk/` dialog built here ───────

// obdefs.h object graphic types.
const G_BOX: u16 = 20;
const G_BOXTEXT: u16 = 22;
/// `G_IBOX` (`obdefs.h`) — an invisible box. It draws nothing; its job is to be
/// the **parent** of a radio-button group, because the AES's `fm_button`
/// deselects the siblings of the button that was hit by walking the parent's
/// child chain (`aes/gemfmlib.c` `select_radio`, our `tos-api/src/aes/form.rs`).
/// EmuTOS wraps every `RBUTTON` group of `ADSETPRE` in one (`desk_rsc.c:1392`
/// and the five that follow).
const G_IBOX: u16 = 25;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;
/// `G_FTEXT` (`obdefs.h`) — formatted text: a `TEDINFO` drawn through its
/// template, **without** a box around it.
const G_FTEXT: u16 = 29;
/// `G_FBOXTEXT` (`obdefs.h`) — the same, boxed.
const G_FBOXTEXT: u16 = 30;

// obdefs.h ob_flags bits.
const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const EDITABLE: u16 = 0x0008;
/// `TOUCHEXIT` (`obdefs.h`) — the object ends `form_do` on the button *down*,
/// without being selected. Every stepper and repeat gadget of `desk_rsc.c` is
/// one.
const TOUCHEXIT: u16 = 0x0040;
/// `RBUTTON` (`obdefs.h`) — radio button: selecting it deselects every other
/// `RBUTTON` child of the same parent.
const RBUTTON: u16 = 0x0010;
const LASTOB: u16 = 0x0020;
// obdefs.h ob_state bits.
const OB_SELECTED: u16 = 0x0001;
/// `OUTLINED` (`obdefs.h:105`) — the ROOT state every `desk_rsc.c` dialog box
/// carries. On an [`FL3DBAK`] panel the AES draws EmuTOS's `draw_3d_outline`
/// instead of the flat GEM double frame (`aes/gemoblib.c:869-887`), i.e. the
/// raised-panel optic. `ob_center` grows the rectangle it reports by 3 px per
/// side for it (`aes/gemobed.c:81-93`), so `form_dial` reserves and restores
/// the outline too.
pub(super) const OUTLINED: u16 = 0x0010;

// ── The EmuTOS 3D flag set (`obdefs.h:92-97`) ───────────────────────────────
//
// `desk_rsc.c` uses exactly three roles, and uses them the same way in every
// one of its twenty trees (the whole file was extracted object by object; the
// table is in `docs/api/desktop/deskinf.md`):
//
// | object                                         | ob_flags     |
// |------------------------------------------------|--------------|
// | ROOT `G_BOX` (with `OUTLINED` state)           | `FL3DBAK`    |
// | `G_FTEXT`/`G_FBOXTEXT` field                   | `FL3DBAK`    |
// | `G_BUTTON` with `RBUTTON` (± `EXIT`)           | `FL3DIND`    |
// | `G_BUTTON` with `EXIT` (OK/Cancel/Install/…)   | `FL3DACT`    |
// | `G_BUTTON` with `TOUCHEXIT` (arrow/stepper)    | `FL3DACT`    |
// | `G_STRING`, `G_TITLE`, `G_IBOX`, `G_IMAGE`     | none         |
//
// Two rows are worth naming because they are not what a guess would produce:
// a radio button that *also* carries `EXIT` is still an indicator
// (`ADBKGND`'s `BGDESK`/`BGWIN`, `desk_rsc.c:2254,2260`), and a stepper gadget
// is an activator (`ADINSDSK`'s `ID_UP`/`ID_DOWN`, `:990,996`).
//
// (`desk_rsc.c` spells `FL3DACT` as `FL3DBAK | FL3DIND`, which is the same
// 0x0600 bit pattern — `obdefs.h:93-97`.)

/// `FL3DIND` (`obdefs.h:95`) — 3D indicator: grown by `ADJ3DSTD`, beveled, and
/// it changes colour when pressed. Every `RBUTTON` in `desk_rsc.c` has it.
pub(super) const FL3DIND: u16 = 0x0200;
/// `FL3DBAK` (`obdefs.h:97`) — 3D background panel: **not** grown and **not**
/// beveled (it does not carry the `FL3DOBJ` bit), but a hollow-white interior
/// takes the `objc_sysvar` `backgrcol` face. Every dialog ROOT and every framed
/// text field in `desk_rsc.c` has it.
pub(super) const FL3DBAK: u16 = 0x0400;
/// `FL3DACT` (`obdefs.h:96`) — 3D activator: grown and beveled like an
/// indicator, but it keeps its colour and nudges its text when pressed. Every
/// exit button in `desk_rsc.c` has it.
pub(super) const FL3DACT: u16 = 0x0600;

/// The dialog-box `ob_spec` of `desk_rsc.c`: a 1-pixel **inner** border over the
/// colour word `0x1100` — black border, black text, `IP_HOLLOW` pattern,
/// white interior (`gr_crack`). Used by every `desk_rsc.c` dialog ROOT bar
/// `ADDINFO`, which is two pixels thick (`desk_rsc.c:622,709,877,922,1027,
/// 1198,1237,1282,1315,1369,1570,1603,2230` all `69888L` = `(1<<16)|0x1100`).
///
/// **Hollow is load-bearing.** The AES only substitutes the 3D face colour when
/// the box is `IP_HOLLOW` *and* white (EmuTOS `just_draw`, `gemoblib.c:619-636`;
/// `tos-api/src/aes/object.rs` `takes_3d_face`). The previous NewTOS value
/// `(1<<16)|0x1070` was pattern 7 = `IP_SOLID`, so an [`FL3DBAK`] flag on it
/// would have been silently ignored and the box would have stayed flat white.
/// The sign is unchanged: a dialog box's border is *inner* (positive), unlike
/// the desktop's drop-down boxes, which A100 measured as outer (`0xFF1100`).
pub(super) const BOX_SPEC: u64 = (1 << 16) | 0x1100;

/// `TEDINFO.te_color` of every editable field in `desk_rsc.c`
/// (`desk_rs_tedinfo[0,18,24,25,…]`, all `4352` = `0x1100`): black border,
/// black text, hollow white interior — the same colour word as [`BOX_SPEC`],
/// so an [`FL3DBAK`] field takes the `backgrcol` face too
/// (`tos-api/src/aes/object.rs`, TEDINFO arm). The previous NewTOS value
/// `1 << 8` painted the frame in pen 0 (WHITE), which under `FL3DBAK` would
/// have drawn a white ring on the grey field.
pub(super) const TED_COLOR: i16 = 0x1100;

/// The longest edited name (a 12-char 8.3 name plus one slack byte).
const NAME_MAX: usize = 13;

/// `ROOT` (`obdefs.h`) — the tree's first object; also `form_do`'s "no editable
/// field preselected" start value (EmuTOS passes `ROOT` for these dialogs).
const ROOT: i16 = 0;

/// Build one dialog `OBJECT`.
#[allow(clippy::too_many_arguments)]
fn mk(ty: u16, fl: u16, spec: u64, x: i16, y: i16, w: i16, h: i16, next: i16, head: i16, tail: i16) -> Object {
    Object {
        ob_next: next,
        ob_head: head,
        ob_tail: tail,
        ob_type: ty,
        ob_flags: fl,
        ob_state: 0,
        ob_spec: spec,
        ob_x: x,
        ob_y: y,
        ob_width: w,
        ob_height: h,
    }
}

/// Build the ROOT box of a desktop dialog exactly as `desk_rsc.c` declares one:
/// a `G_BOX` at the origin (`form_center` writes the centred position), with
/// [`FL3DBAK`] flags, [`OUTLINED`] state and a `(thickness << 16) | 0x1100`
/// `ob_spec` — the three pieces that together give EmuTOS's raised grey panel.
/// `spec` is normally [`BOX_SPEC`]; `ADDINFO` is the one tree with a 2-pixel
/// border (`desk_rsc.c:774-777`).
pub(super) fn mk_root(spec: u64, w: i16, h: i16, tail: i16) -> Object {
    let mut root = mk(G_BOX, FL3DBAK, spec, 0, 0, w, h, -1, 1, tail);
    root.ob_state = OUTLINED;
    root
}

/// One NUL-terminated `G_STRING` line held on the dialog's own stack frame.
///
/// A `G_STRING`'s `ob_spec` is a C string pointer, and the AES draws it
/// **left-aligned at `ob_x`**, vertically centred in `ob_h`
/// (`aes/gemoblib.c:817-845`, reproduced in `tos-api/src/aes/object.rs`'s
/// `G_STRING` arm) — there is no centring attribute to set, so a line that must
/// appear centred has to be *placed* centred, which is what [`Line::width`] is
/// for. EmuTOS's resource strings are `.rsc` literals with the NUL already in
/// them; a Rust `&str` has none, hence the copy.
struct Line {
    buf: [u8; LINE_MAX + 1],
    len: usize,
}

/// Longest dialog line, in characters (72 · 8 px = 576, still inside a 640-px
/// screen with margins).
const LINE_MAX: usize = 72;

impl Line {
    /// Copy `s` (truncated to [`LINE_MAX`]) into a NUL-terminated buffer.
    fn new(s: &str) -> Self {
        let mut buf = [0u8; LINE_MAX + 1];
        let src = s.as_bytes();
        let len = src.len().min(LINE_MAX);
        buf[..len].copy_from_slice(&src[..len]);
        Self { buf, len }
    }
    /// The `ob_spec` value for a `G_STRING` showing this line.
    fn spec(&self) -> u64 {
        self.buf.as_ptr() as u64
    }
    /// The drawn width in pixels (the AES normal font is fixed-pitch).
    fn width(&self) -> i16 {
        self.len as i16 * CELL_W
    }
}

// ── Desktop-info dialog (`do_deskmenu`'s `ABOUITEM`, `deskmain.c:612-646`) ──

/// `do_aboutdesk` — the Desk menu's *Desktop info...* box.
///
/// EmuTOS shows the `ADDINFO` resource tree (`desk/desk_rsc.c:772-873`): an
/// `OUTLINED` root box 40 characters wide, a `- EmuTOS -` title flanked by two
/// `G_IMAGE` logos, a `Version` label with the value `desk_xlate_fix` patched in
/// from the BIOS `version[]` string (`deskmain.c:1880`), the copyright block,
/// the project URL and the GPL notice, and a single `DEOK` button
/// (`desk_rsc.c:869-873`). The **structure** is reproduced here — centred box,
/// title line, a `Version`/`Machine` pair block, the credit paragraphs, one
/// default OK button.
///
/// ## The content, and where each line comes from
///
/// | row | line | source |
/// |-----|------|--------|
/// | 0 | `- NewTOS -` | `ADDINFO`'s `- EmuTOS -` title (`desk_rsc.c:785-789`) |
/// | 2 | `Version  …` | [`crate::initinfo::VERSION_STRING`] (`DELABEL`/`DEVERSN`) |
/// | 3 | `Machine  …` | [`crate::initinfo::machine_name`] |
/// | 5-6 | the independence notice | [`crate::initinfo::INDEPENDENCE_NOTICE`] |
/// | 8-9 | the EmuTOS credit | [`CREDIT_REIMPL`] / [`CREDIT_EMUTOS`] |
/// | 11-14 | the `'GPLed'` chain | [`CREDIT_GPL`] / [`CREDIT_DRI`] / [`CREDIT_CALDERA`] / [`CREDIT_LINEO`] |
/// | 16 | the authorship line | [`crate::initinfo::AUTHORSHIP_LINE`] |
///
/// The blank rows 1/4/7/10/15 are `ADDINFO`'s own: it leaves char rows 3, 6, 8
/// and 13 empty between its blocks (`desk_rsc.c:790-866`).
///
/// **Two readers, one source, in two places — on purpose.** The version, the
/// machine name, the independence notice and the authorship line are also
/// printed by the boot screen, so they live in [`crate::initinfo`] and are read
/// from there; that is EmuTOS's own arrangement (`desk_xlate_fix` points
/// `DEVERSN->ob_spec` at the BIOS `version[]` the boot screen prints,
/// `deskmain.c:1880`). The six credit lines are *not* on the boot screen —
/// EmuTOS keeps its `'GPLed'` chain in `desk_rsc.c` alone and `bios/initinfo.c`
/// prints none of it — so they stay local to this dialog.
///
/// **Three named deviations.**
///
/// * EmuTOS runs `start_dialog` → `form_do` loop → `end_dialog` rather than
///   [`inf_show`], solely because `DEICON` is a `TOUCHEXIT` that plays the
///   Easter-egg tune and loops instead of closing (`deskmain.c:626-641`,
///   `CONF_WITH_EASTER_EGG`). With no such object the loop would run exactly
///   once, so this is `inf_show(tree, ROOT)` — the same three calls.
/// * The box is sized to its text instead of the resource's fixed 40 columns:
///   the machine line is board-dependent (`initinfo::machine_name`), so there is
///   no build-time width to bake in. EmuTOS solves the same problem for its
///   menus with the run-time `adjust_menu` (`deskmain.c:1608-1635`).
/// * `ADDINFO`'s copyright year is patched in at start-up from
///   `COPYRIGHT_YEAR` (`deskmain.c:1892`, `obj/header.h:19`, generated by
///   `tools/mkheader.awk`). The span in [`CREDIT_EMUTOS`] stays a literal even
///   though NewTOS now has a build year ([`tos_api::build::BUILD_YEAR`]):
///   EmuTOS's year comes from *its* build, and ours is pinned source, so
///   stamping our build year onto their line would extend their copyright by
///   the calendar.
///
/// **On the build date: this box shows none, and neither does EmuTOS's.**
/// `ADDINFO` carries `DECOPYRT`, patched from `COPYRIGHT_YEAR` — a *year*, not
/// a build date. EmuTOS's build date goes into the OS header alone
/// (`tools/mkheader.awk` generates `obj/header.h`'s `OS_DATE`/`OS_DOSDATE`),
/// and its boot screen prints only `EmuTOS Version %s` (`bios/initinfo.c:321`).
/// NewTOS now has that stamp too ([`tos_api::build`], DESK12): it feeds the
/// OSHEADER the same way, and the boot screen prints it as an own addition.
/// It is not repeated here — besides having no EmuTOS counterpart, the box is
/// 436 px tall at 640x480 with 10 px to spare, and one more row is 20.
///
pub(super) fn do_aboutdesk() {
    // Column layout: a `label   value` pair block, as `ADDINFO` pairs DELABEL
    // with DEVERSN (`desk_rsc.c:803-813`).
    const LABEL_COLS: i16 = 9;
    const MARGIN: i16 = 24;
    const ROW: i16 = CELL_H + 4;

    let title = Line::new("- NewTOS -");
    let ver_lbl = Line::new("Version");
    let ver_val = Line::new(crate::initinfo::VERSION_STRING);
    let mac_lbl = Line::new("Machine");
    let mac_val = Line::new(crate::initinfo::machine_name());
    // The independence notice is one sentence in `initinfo`; split it at its
    // own dash so the box stays close to the resource's proportions instead of
    // growing to the full 63-character line.
    let notice = crate::initinfo::INDEPENDENCE_NOTICE;
    let (head, tail) = match notice.find("- ") {
        Some(i) => (&notice[..i + 1], &notice[i + 2..]),
        None => (notice, ""),
    };
    let note1 = Line::new(head);
    let note2 = Line::new(tail);
    let reimpl = Line::new(CREDIT_REIMPL);
    let emuteam = Line::new(CREDIT_EMUTOS);
    let gpl = Line::new(CREDIT_GPL);
    let dri = Line::new(CREDIT_DRI);
    let caldera = Line::new(CREDIT_CALDERA);
    let lineo = Line::new(CREDIT_LINEO);
    let author = Line::new(crate::initinfo::AUTHORSHIP_LINE);

    static OK: &[u8] = b" OK \0";
    const OK_W: i16 = 90;
    const OK_H: i16 = 28;

    // Pair block: label column + the wider of the two values.
    let pair_w = LABEL_COLS * CELL_W + ver_val.width().max(mac_val.width());
    let text_w = pair_w
        .max(title.width())
        .max(note1.width())
        .max(note2.width())
        .max(reimpl.width())
        .max(emuteam.width())
        .max(gpl.width())
        .max(dri.width())
        .max(caldera.width())
        .max(lineo.width())
        .max(author.width());
    let dw = text_w + 2 * MARGIN;
    let pair_x = (dw - pair_w) / 2;
    let val_x = pair_x + LABEL_COLS * CELL_W;
    let ctr = |l: &Line| (dw - l.width()) / 2;

    // Row grid, with `ADDINFO`'s blank rows at 1, 4, 7, 10 and 15.
    let y0 = MARGIN;
    let row = |n: i16| y0 + n * ROW;
    let y_ok = row(18);
    let dh = y_ok + OK_H + MARGIN;

    /// Object index of the OK button — `ADDINFO`'s `DEOK` (`desk_rsc.h:91`).
    const DEOK: i16 = 15;
    let str_obj = |l: &Line, x: i16, n: i16, next: i16| mk(G_STRING, 0, l.spec(), x, row(n), l.width(), CELL_H, next, -1, -1);
    let mut tree = [
        mk_root(ADDINFO_BOX_SPEC, dw, dh, DEOK),
        str_obj(&title, ctr(&title), 0, 2),
        str_obj(&ver_lbl, pair_x, 2, 3),
        str_obj(&ver_val, val_x, 2, 4),
        str_obj(&mac_lbl, pair_x, 3, 5),
        str_obj(&mac_val, val_x, 3, 6),
        str_obj(&note1, ctr(&note1), 5, 7),
        str_obj(&note2, ctr(&note2), 6, 8),
        str_obj(&reimpl, ctr(&reimpl), 8, 9),
        str_obj(&emuteam, ctr(&emuteam), 9, 10),
        str_obj(&gpl, ctr(&gpl), 11, 11),
        str_obj(&dri, ctr(&dri), 12, 12),
        str_obj(&caldera, ctr(&caldera), 13, 13),
        str_obj(&lineo, ctr(&lineo), 14, 14),
        str_obj(&author, ctr(&author), 16, DEOK),
        // `DEOK`: `SELECTABLE | DEFAULT | EXIT | LASTOB | FL3DBAK | FL3DIND`
        // (`desk_rsc.c:869-873`) — the last two bits are `FL3DACT`.
        mk(
            G_BUTTON,
            SELECTABLE | DEFAULT | EXIT | LASTOB | FL3DACT,
            OK.as_ptr() as u64,
            (dw - OK_W) / 2,
            y_ok,
            OK_W,
            OK_H,
            0,
            -1,
            -1,
        ),
    ];

    // `inf_show(tree, ROOT)` (`deskinf.c:396-402`). `as_mut_ptr`, not `as_ptr`:
    // the AES writes `ob_state` through this pointer while the dialog runs.
    let clicked = inf_show(tree.as_mut_ptr() as u64, ROOT);
    // `obj = tree + DEOK; obj->ob_state = NORMAL;` (`deskmain.c:643-644`) — the
    // resource tree is reused on the next open, so the pressed button must not
    // stay inverted. Ours is rebuilt each time; done anyway, for the same reason
    // `set_background` does it.
    if let Some(o) = tree.get_mut(clicked.max(0) as usize) {
        o.ob_state &= !OB_SELECTED;
    }
}

/// `ADDINFO`'s ROOT `ob_spec` — `135424L` = `(2 << 16) | 0x1100`
/// (`desk_rsc.c:774-777`): the same colour word as [`BOX_SPEC`], but a
/// **two**-pixel inner border. `ADDINFO` is the only `desk_rsc.c` tree with it.
const ADDINFO_BOX_SPEC: u64 = (2 << 16) | 0x1100;

// ── The About box's credit lines ────────────────────────────────────────────
//
// Local to this dialog, not in `crate::initinfo`, because EmuTOS keeps the
// same split: `bios/initinfo.c` prints none of the `'GPLed'` chain, only
// `desk_rsc.c`'s `ADDINFO` carries it.

/// What NewTOS is, in one line. EmuTOS's `ADDINFO` has no counterpart claim —
/// it *is* the code it credits; NewTOS is a reimplementation of it, which is
/// the fact this line states.
const CREDIT_REIMPL: &str = "Reimplemented from the EmuTOS sources";
/// EmuTOS's own copyright line, as `ADDINFO` assembles it from
/// `"Copyright \275 2001-"` + `COPYRIGHT_YEAR` + `"The EmuTOS development team"`
/// (`desk_rsc.c:806-828`, `deskmain.c:1892`). `\275` is the Atari ST charset's
/// `(c)` glyph, written out here. The year span stays a literal even though
/// NewTOS now has a build year ([`tos_api::build::BUILD_YEAR`]): EmuTOS's year
/// comes from *its* build, ours is a pinned source tree, and stamping our build
/// year onto their line would extend their copyright by the calendar.
const CREDIT_EMUTOS: &str = "(c) 2001-2026 The EmuTOS development team";
/// `ADDINFO`'s `"Based on 'GPLed' sources"` heading, verbatim
/// (`desk_rsc.c:834-838`).
const CREDIT_GPL: &str = "Based on 'GPLed' sources";
/// `ADDINFO`'s `"\275 1987 Digital Research, Inc."` (`desk_rsc.c:840-844`).
const CREDIT_DRI: &str = "(c) 1987 Digital Research, Inc.";
/// `ADDINFO`'s `"\275 1999 Caldera Thin Clients, Inc."` (`desk_rsc.c:846-850`).
const CREDIT_CALDERA: &str = "(c) 1999 Caldera Thin Clients, Inc.";
/// `ADDINFO`'s `"\275 2001 Lineo, Inc."` (`desk_rsc.c:852-856`).
const CREDIT_LINEO: &str = "(c) 2001 Lineo, Inc.";

/// Append a decimal rendering of `v` to `buf` at `*p` (bounded by `buf.len()`).
fn push_dec(buf: &mut [u8], p: &mut usize, mut v: u64) {
    if v == 0 {
        if *p < buf.len() {
            buf[*p] = b'0';
            *p += 1;
        }
        return;
    }
    let mut tmp = [0u8; 20];
    let mut i = 20;
    while v > 0 {
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    for &d in &tmp[i..] {
        if *p < buf.len() {
            buf[*p] = d;
            *p += 1;
        }
    }
}

/// Append the bytes of `s` to `buf` at `*p`.
fn push_str(buf: &mut [u8], p: &mut usize, s: &[u8]) {
    for &b in s {
        if *p < buf.len() {
            buf[*p] = b;
            *p += 1;
        }
    }
}

/// Append `v` as two zero-padded decimal digits to `buf`.
fn push_dec2(buf: &mut [u8], p: &mut usize, v: u32) {
    if *p + 1 < buf.len() {
        buf[*p] = b'0' + (v / 10 % 10) as u8;
        buf[*p + 1] = b'0' + (v % 10) as u8;
        *p += 2;
    }
}

// ── Date / time formatting (`fmt_date`/`fmt_time`, `deskinf.c:74-180`) ──────
//
// Both readers of the date/time preference live here, as they do in EmuTOS:
// the text-view line builder (`format_fnode`, `deskinf.c:220-307`, ours in
// `super::win`) and the Show-Info box (`inf_dttm`, `deskinf.c:460-468`).
// Neither takes the desktop lock — the caller passes the format in, because
// `format_fnode` already runs inside `global::with`.

/// `DATEFORM_SEP` (`deskbind.h:64`) — the separator EmuTOS uses when the format
/// does not come from the `_IDT` cookie.
const DATEFORM_SEP: u8 = b'/';

/// `fmt_date` (`deskinf.c:124-180`) — a DOS-packed date word written into `out`
/// as `xx<sep>xx<sep>xx`, with a four-digit year when `fourdigit`. The field
/// order is `datefmt`'s: month first for [`DateFmt::Mdy`], day first for
/// [`DateFmt::Dmy`]. Returns the number of bytes written.
///
/// [`DateFmt::Idt`] renders as [`DateFmt::Mdy`]: the `_IDT` cookie it names is
/// out of the desktop's reach (see [`DateFmt`]), and MDY is what the text view
/// printed before the preference existed, so an unreachable value changes
/// nothing on screen.
///
/// **`sep` is a parameter, and that is a divergence, not a design.** EmuTOS
/// passes `DATEFORM_SEP` ('/') from both call sites; NewTOS's text view has
/// printed `MM-DD-YY` with a dash since it was written (`super::win`'s
/// `format_fnode`). DESK02 binds the *order* to the preference and leaves the
/// separator exactly as each call site had it, so no screen changes that this
/// ticket did not ask to change; the dash is reported as its own finding.
pub(super) fn fmt_date(date: u16, fourdigit: bool, sep: u8, datefmt: DateFmt, out: &mut [u8]) -> usize {
    let mut year = 1980u32 + ((date >> 9) & 0x7F) as u32;
    if !fourdigit {
        year %= 100;
    }
    let month = ((date >> 5) & 0x0F) as u32;
    let day = (date & 0x1F) as u32;

    let (v1, v2) = match datefmt.rendered() {
        DateFmt::Dmy => (day, month),
        // `default:` — MDY, which is also where `rendered()` sends the
        // unreachable IDT value.
        _ => (month, day),
    };

    let mut p = 0usize;
    push_dec2(out, &mut p, v1);
    push_str(out, &mut p, &[sep]);
    push_dec2(out, &mut p, v2);
    push_str(out, &mut p, &[sep]);
    if fourdigit {
        push_dec(out, &mut p, year as u64);
    } else {
        push_dec2(out, &mut p, year);
    }
    p
}

/// `fmt_time` (`deskinf.c:74-105`) — a DOS-packed time word written into `out`
/// as `hh:mm` plus a **two-character suffix**: `am`/`pm` for [`TimeFmt::H12`],
/// two blanks for [`TimeFmt::H24`] (EmuTOS's `char *suffix = "  ";`,
/// `deskinf.c:77`). The suffix keeps the field width constant at 7 whichever
/// format is chosen, which is why EmuTOS can lay the text view out in fixed
/// columns. Returns the number of bytes written (always 7).
///
/// [`TimeFmt::Idt`] renders as [`TimeFmt::H24`] — same reasoning as
/// [`fmt_date`]'s `Idt` arm.
pub(super) fn fmt_time(time: u16, timefmt: TimeFmt, out: &mut [u8]) -> usize {
    let mut hh = ((time >> 11) & 0x1F) as u32;
    let mm = ((time >> 5) & 0x3F) as u32;
    let suffix: &[u8] = match timefmt.rendered() {
        TimeFmt::H12 => {
            let pm = hh >= 12;
            if pm {
                hh -= 12;
            }
            if hh == 0 {
                hh = 12; // `if (hh == 0) hh = 12;` (`deskinf.c:99-100`)
            }
            if pm {
                b"pm"
            } else {
                b"am"
            }
        }
        _ => b"  ",
    };
    let mut p = 0usize;
    push_dec2(out, &mut p, hh);
    push_str(out, &mut p, b":");
    push_dec2(out, &mut p, mm);
    push_str(out, &mut p, suffix);
    p
}

/// FAT attribute bits (`bdosdefs.h`).
const FA_RO: u8 = 0x01;
const FA_HIDDEN: u8 = 0x02;
const FA_SYSTEM: u8 = 0x04;
const FA_SUBDIR: u8 = 0x10;
const FA_ARCHIVE: u8 = 0x20;

/// The Show-Info / rename dialog for a single selected item (`inf_file_folder`,
/// `deskinf.c:493-639`): show size / date / time / attributes and offer an
/// editable name. Returns the new name on OK when it changed (an empty name is
/// rejected), otherwise [`None`]. The screen underneath is restored by
/// `end_dialog`'s `form_dial(FMD_FINISH)` inside [`inf_show`].
///
/// `size`/`date`/`time`/`attr` are the selected [`super::model::FNode`]'s own
/// fields (DOS-packed date/time, FAT attribute byte). The date/time text layout
/// is a display choice, not TOS contract (`deskinf.md`: "no program reads this").
pub(super) fn inf_file_folder(name: &str, size: u32, date: u16, time: u16, attr: u8) -> Option<String> {
    let is_dir = attr & FA_SUBDIR != 0;

    // Editable name buffer — `form_do` edits this in place through the TEDINFO.
    let mut namebuf = [0u8; NAME_MAX + 2];
    let n = name.len().min(NAME_MAX);
    namebuf[..n].copy_from_slice(&name.as_bytes()[..n]);

    static EMPTY: [u8; 1] = [0];
    static TITLE: &[u8] = b" Show Info \0";
    static NAME_LBL: &[u8] = b"Name:\0";
    static OK: &[u8] = b" OK \0";
    static CANCEL: &[u8] = b" Cancel \0";

    let ted = TedInfo {
        te_ptext: namebuf.as_mut_ptr() as u64,
        te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: NAME_MAX as i16,
        te_tmplen: 0,
    };

    // Size line: "Size: N bytes" (a folder shows no byte count of its own).
    let mut sizebuf = [0u8; 40];
    {
        let mut p = 0usize;
        if is_dir {
            push_str(&mut sizebuf, &mut p, b"Size: (folder)");
        } else {
            push_str(&mut sizebuf, &mut p, b"Size: ");
            push_dec(&mut sizebuf, &mut p, size as u64);
            push_str(&mut sizebuf, &mut p, b" bytes");
        }
        sizebuf[p.min(sizebuf.len() - 1)] = 0;
    }

    // Date/time line: "Date: MM/DD/YYYY  HH:MM" — `inf_dttm` (`deskinf.c:460-
    // 468`), which formats the date with a four-digit year always (`:464`) and
    // the time through the same `fmt_time` the text view uses (`:467`). Both
    // take their order/clock from the Preferences dialog's values.
    let (datefmt, timefmt) =
        global::with(|g| (g.g_cdateform, g.g_ctimeform)).unwrap_or((DateFmt::Mdy, TimeFmt::H24));
    let mut datebuf = [0u8; 40];
    {
        let mut p = 0usize;
        push_str(&mut datebuf, &mut p, b"Date: ");
        p += fmt_date(date, true, DATEFORM_SEP, datefmt, &mut datebuf[p..]);
        push_str(&mut datebuf, &mut p, b"  ");
        p += fmt_time(time, timefmt, &mut datebuf[p..]);
        // Trim `fmt_time`'s two-blank 24-hour suffix off the end of the line.
        while p > 0 && datebuf[p - 1] == b' ' {
            p -= 1;
        }
        datebuf[p.min(datebuf.len() - 1)] = 0;
    }

    // Attributes line.
    let mut attrbuf = [0u8; 40];
    {
        let mut p = 0usize;
        push_str(&mut attrbuf, &mut p, b"Attributes: ");
        if is_dir {
            push_str(&mut attrbuf, &mut p, b"DIR ");
        }
        if attr & FA_RO != 0 {
            push_str(&mut attrbuf, &mut p, b"R ");
        }
        if attr & FA_HIDDEN != 0 {
            push_str(&mut attrbuf, &mut p, b"H ");
        }
        if attr & FA_SYSTEM != 0 {
            push_str(&mut attrbuf, &mut p, b"S ");
        }
        if attr & FA_ARCHIVE != 0 {
            push_str(&mut attrbuf, &mut p, b"A ");
        }
        if p == "Attributes: ".len() {
            push_str(&mut attrbuf, &mut p, b"none");
        }
        attrbuf[p.min(attrbuf.len() - 1)] = 0;
    }

    // Built at the origin; `form_center` inside `inf_show` writes the centred
    // `ob_x`/`ob_y` into the ROOT.
    let (dw, dh) = (320i16, 200i16);
    let ob_ok = 7i16;

    // The 3D roles follow `ADFFINFO` object by object (`desk_rsc.c:622-703`):
    // the ROOT is `FL3DBAK` + `OUTLINED`, `FFNAME` is an `EDITABLE | FL3DBAK`
    // field, `FFOK`/`FFCNCL` are `FL3DBAK | FL3DIND` = `FL3DACT`, and the
    // `G_STRING`s carry none. `ADFFINFO` shows size/date/time as `FL3DBAK`
    // `G_FBOXTEXT` fields with an underscore template; NewTOS builds those
    // three as plain `G_STRING` lines (no template), and a `G_STRING` has no
    // interior for a 3D background to fill — so they stay flag-free.
    let tree = [
        mk_root(BOX_SPEC, dw, dh, 8),
        mk(G_STRING, 0, TITLE.as_ptr() as u64, 24, 14, dw - 48, 16, 2, -1, -1),
        mk(G_STRING, 0, NAME_LBL.as_ptr() as u64, 24, 44, 48, 16, 3, -1, -1),
        mk(G_BOXTEXT, EDITABLE | FL3DBAK, &ted as *const _ as u64, 80, 40, 200, 22, 4, -1, -1),
        mk(G_STRING, 0, sizebuf.as_ptr() as u64, 24, 74, dw - 48, 16, 5, -1, -1),
        mk(G_STRING, 0, datebuf.as_ptr() as u64, 24, 96, dw - 48, 16, 6, -1, -1),
        mk(G_STRING, 0, attrbuf.as_ptr() as u64, 24, 118, dw - 48, 16, 7, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT | FL3DACT, OK.as_ptr() as u64, 40, 156, 90, 28, 8, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, 190, 156, 90, 28, 0, -1, -1),
    ];

    // `inf_show(tree, ROOT)` (`deskinf.c:396-402`).
    let clicked = inf_show(tree.as_ptr() as u64, ROOT);
    if clicked != ob_ok {
        return None;
    }
    let m = namebuf.iter().position(|&b| b == 0).unwrap_or(namebuf.len());
    if m == 0 {
        return None; // empty name rejected
    }
    let new = core::str::from_utf8(&namebuf[..m]).ok()?;
    if new == name {
        return None; // no change
    }
    Some(String::from(new))
}

// ── Preferences dialog (`inf_pref`, `deskinf.c:721-818`) ────────────────────

/// `inf_which` (`deskinf.c:705-716`) — the index, **counted from the end**, of
/// the first `SELECTED` object in `tree[baseobj .. baseobj+numobj]`, or `-1`
/// when none is selected. For a two-button Yes/No pair that is `1` for Yes and
/// `0` for No; for the three-button format groups it is EmuTOS's
/// `TIMEFORM_*`/`DATEFORM_*` value read straight off the left-to-right button
/// order (`deskinf.c:793-805`, whose comment says exactly that).
fn inf_which(tree: &[Object], baseobj: usize, numobj: usize) -> i16 {
    let mut i = numobj as i16 - 1;
    for obj in tree.iter().skip(baseobj).take(numobj) {
        if obj.ob_state & OB_SELECTED != 0 {
            break;
        }
        i -= 1;
    }
    i
}

/// `inf_gindex` (`util/optimize.c:134-147`) — the index, **counted from the
/// start**, of the first `SELECTED` object in `tree[baseobj .. baseobj+numobj]`,
/// or `-1` when none is selected. The reverse of [`inf_which`].
fn inf_gindex(tree: &[Object], baseobj: usize, numobj: usize) -> i16 {
    for (n, obj) in tree.iter().skip(baseobj).take(numobj).enumerate() {
        if obj.ob_state & OB_SELECTED != 0 {
            return n as i16;
        }
    }
    -1
}

/// `inf_what` (`util/optimize.c:157-170`) — `1` if `ok` was the selected
/// button, `0` if `ok + 1` (its Cancel neighbour) was, `-1` otherwise; the
/// selected button is deselected on the way out.
fn inf_what(tree: &mut [Object], ok: usize) -> i16 {
    let field = inf_gindex(tree, ok, 2);
    if field == -1 {
        return -1;
    }
    tree[ok + field as usize].ob_state = 0; // `objptr->ob_state = NORMAL;`
    i16::from(field == 0)
}

/// Options → Set preferences (`inf_pref`, `deskinf.c:721-818`).
///
/// Pre-selects every radio group from the current preference values, runs the
/// dialog through [`inf_show`], and on OK reads each group back with
/// [`inf_which`] / [`inf_gindex`] and commits it. Returns EmuTOS's narrow
/// rebuild signal: `true` **only** when the view is `V_TEXT` *and* the time or
/// date format actually changed (`deskinf.c:807-814`) — nothing else in this
/// dialog alters anything already on screen.
///
/// ## The field set, and what is missing from it
///
/// `ADSETPRE` (`desk/desk_rsc.c:1369-1560`) offers six groups in this order:
/// confirm deletes, confirm copies, confirm overwrites, double-click speed,
/// time format, date format. NewTOS shows **three** of them, in EmuTOS's order,
/// and the three it leaves out are left out rather than shown-and-ignored:
///
/// * **Confirm copies** (`SPCCYES`/`SPCCNO`) — EmuTOS's switch turns on the
///   per-item copy dialog inside `d_doop` (`deskdir.c:939`). NewTOS's copy
///   engine has no per-item dialog at all ([`super::dir`]'s module header
///   records that as a deliberate deviation), so there is nothing for the
///   switch to turn on.
/// * **Confirm overwrites** (`SPCOWYES`/`SPCOWNO`) — same reason, one level
///   deeper: EmuTOS asks before replacing an existing file (`deskdir.c:325`,
///   `if (!G.g_covwrpref)`), NewTOS's copy always overwrites (GEMDOS `Fcreate`
///   truncates).
/// * **The `Default` button of each format group** (`SPTF_DEF`/`SPDF_DEF`) —
///   "default" means "take the format from the `_IDT` cookie", which EmuTOS
///   reads with `Supexec(get_idt_cookie)` (`desksupp.c:219`). XBIOS `Supexec`
///   (0x26) is not implemented (`tos-api/src/xbios/mod.rs:129`) and the desktop
///   may not reach kernel state except through a trap (DESK10), so the value
///   cannot be honoured. It is still *decoded* from the `#E` line and written
///   back unchanged ([`env_to_prefs`]), so a file carrying it survives; it just
///   has no button. A value of `Idt` pre-selects the NewTOS fallback
///   (`24H` / `MDY`), which is what it renders as.
///
/// Each of the three fields that is shown has a live consumer: *Confirm
/// deletes* gates [`super::dir`]'s delete question, the speed is the AES's own
/// `evnt_dclick` rate, and the two formats drive `format_fnode` and the
/// Show-Info box through [`fmt_date`]/[`fmt_time`].
pub(super) fn inf_pref() -> bool {
    // ── current values ──────────────────────────────────────────────────
    let (confdel, timefmt, datefmt, view) =
        global::with(|g| (g.g_cdelepref, g.g_ctimeform, g.g_cdateform, g.view))
            .unwrap_or((true, TimeFmt::H24, DateFmt::Mdy, View::Icon));
    // `G.g_cdclkpref = evnt_dclick(0, FALSE);` (`deskinf.c:751`) — the rate is
    // the AES's, so it is asked for rather than remembered.
    let dclick = super::evnt_dclick(0, false).clamp(0, 4);

    static TITLE: &[u8] = b" SET PREFERENCES \0";
    static CONFDEL_LBL: &[u8] = b"Confirm deletes:\0";
    static DCLICK_LBL: &[u8] = b"Double-click speed:\0";
    static TIME_LBL: &[u8] = b"Time format:\0";
    static DATE_LBL: &[u8] = b"Date format:\0";
    static YES: &[u8] = b" Yes \0";
    static NO: &[u8] = b" No \0";
    static DC: [&[u8]; 5] = [b"1\0", b"2\0", b"3\0", b"4\0", b"5\0"];
    static H12: &[u8] = b" 12H \0";
    static H24: &[u8] = b" 24H \0";
    static MDY: &[u8] = b" MDY \0";
    static DMY: &[u8] = b" DMY \0";
    static OK: &[u8] = b" OK \0";
    static CANCEL: &[u8] = b" Cancel \0";

    // ── geometry ────────────────────────────────────────────────────────
    //
    // The pitch and the gap are `ADSETPRE`'s own, because a 3D indicator is
    // **grown** before it is drawn: the AES adds `ADJ3DSTD` (2 px) per side and
    // then lays the `th = -1` border outside that (`aes/gemoblib.c:504`,
    // `tos-api/src/aes/object.rs`), so every `FL3DIND` button claims 3 px more
    // in each direction than its `ob_*` rectangle says. `ADSETPRE` spaces its
    // rows two character rows apart (`desk_rsc.c:1386,1406,1426,1450` — char
    // rows 3, 5, 7, 9 → 32 px) and its speed buttons 23 px apart for a 15 px
    // button (`:1462-1493`, an 8 px gap). With the previous 26 px pitch and
    // 6 px gap the grown borders of neighbouring buttons would have met.
    const MARGIN: i16 = 24;
    /// `ADSETPRE`'s two-character row pitch (`desk_rsc.c:1386` ff.).
    const ROW_H: i16 = 2 * CELL_H;
    const BTN_H: i16 = 22;
    const YESNO_W: i16 = 52;
    const DC_W: i16 = 30;
    const FMT_W: i16 = 56;
    /// `ADSETPRE`'s inter-button gap (`desk_rsc.c:1462-1493`: pitch 23 px,
    /// width 15 px).
    const GAP: i16 = 8;
    const LBL_W: i16 = 20 * CELL_W; // "Double-click speed:" + a blank
    const GROUP_W: i16 = 5 * DC_W + 4 * GAP; // the widest group (five speeds)
    let dw = MARGIN + LBL_W + GROUP_W + MARGIN;
    let gx = MARGIN + LBL_W;

    let y_del = MARGIN + ROW_H;
    let y_dc = y_del + ROW_H;
    let y_tf = y_dc + ROW_H;
    let y_df = y_tf + ROW_H;
    let y_btn = y_df + ROW_H + GAP;
    let dh = y_btn + 28 + MARGIN;
    // Vertical offset of a `BTN_H` button inside its `CELL_H` label row.
    let boff = (CELL_H - BTN_H) / 2;

    // ── object indices (`desk_rsc.h:164-184`, minus the omitted groups) ──
    /// `SPCDYES` — the *Confirm deletes* `Yes` button; `SPCDNO` follows it.
    const SPCDYES: usize = 4;
    /// `SPDC1` — the first of the five double-click speed buttons.
    const SPDC1: usize = 8;
    /// `SPTF12HR` — the first *shown* time-format button (EmuTOS's `SPTF_DEF`
    /// precedes it and has no NewTOS counterpart).
    const SPTF12HR: usize = 15;
    /// `SPDFMMDD` — the first *shown* date-format button.
    const SPDFMMDD: usize = 19;
    /// `SPOK` — the OK button; `SPCNCL` follows it, as [`inf_what`] requires.
    const SPOK: usize = 21;

    // Every `RBUTTON` of `ADSETPRE` is `SELECTABLE | RBUTTON | FL3DIND`
    // (`desk_rsc.c:1396` ff.) — the 3D indicator role: grown, beveled, and it
    // changes colour when selected.
    let rb = |spec: &'static [u8], x: i16, w: i16, next: i16| {
        mk(G_BUTTON, SELECTABLE | RBUTTON | FL3DIND, spec.as_ptr() as u64, x, 0, w, BTN_H, next, -1, -1)
    };

    // Each radio group hangs under its own `G_IBOX`, because that is what makes
    // the AES deselect the siblings of the button that was hit (see [`G_IBOX`]).
    // `ADSETPRE`'s `G_IBOX`es carry no 3D flag and `ob_spec = 0` (`:1390` ff.).
    #[rustfmt::skip]
    let mut tree = [
        // 0 ROOT
        mk_root(BOX_SPEC, dw, dh, SPOK as i16 + 1),
        // 1 title
        mk(G_STRING, 0, TITLE.as_ptr() as u64, MARGIN, MARGIN, dw - 2 * MARGIN, CELL_H, 2, -1, -1),
        // 2 "Confirm deletes:"          3 its G_IBOX      4/5 Yes/No
        mk(G_STRING, 0, CONFDEL_LBL.as_ptr() as u64, MARGIN, y_del, LBL_W, CELL_H, 3, -1, -1),
        mk(G_IBOX, 0, 0, gx, y_del + boff, 2 * YESNO_W + GAP, BTN_H, 6, 4, 5),
        rb(YES, 0, YESNO_W, 5),
        rb(NO, YESNO_W + GAP, YESNO_W, 3),
        // 6 "Double-click speed:"       7 its G_IBOX      8..12 the five speeds
        mk(G_STRING, 0, DCLICK_LBL.as_ptr() as u64, MARGIN, y_dc, LBL_W, CELL_H, 7, -1, -1),
        mk(G_IBOX, 0, 0, gx, y_dc + boff, GROUP_W, BTN_H, 13, 8, 12),
        rb(DC[0], 0, DC_W, 9),
        rb(DC[1], DC_W + GAP, DC_W, 10),
        rb(DC[2], 2 * (DC_W + GAP), DC_W, 11),
        rb(DC[3], 3 * (DC_W + GAP), DC_W, 12),
        rb(DC[4], 4 * (DC_W + GAP), DC_W, 7),
        // 13 "Time format:"             14 its G_IBOX     15/16 12H/24H
        mk(G_STRING, 0, TIME_LBL.as_ptr() as u64, MARGIN, y_tf, LBL_W, CELL_H, 14, -1, -1),
        mk(G_IBOX, 0, 0, gx, y_tf + boff, 2 * FMT_W + GAP, BTN_H, 17, 15, 16),
        rb(H12, 0, FMT_W, 16),
        rb(H24, FMT_W + GAP, FMT_W, 14),
        // 17 "Date format:"             18 its G_IBOX     19/20 MDY/DMY
        mk(G_STRING, 0, DATE_LBL.as_ptr() as u64, MARGIN, y_df, LBL_W, CELL_H, 18, -1, -1),
        mk(G_IBOX, 0, 0, gx, y_df + boff, 2 * FMT_W + GAP, BTN_H, 21, 19, 20),
        rb(MDY, 0, FMT_W, 20),
        rb(DMY, FMT_W + GAP, FMT_W, 18),
        // 21 OK                         22 Cancel  (`SPOK`/`SPCNCL`: FL3DACT)
        mk(G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, OK.as_ptr() as u64, dw / 2 - 100, y_btn, 90, 28, SPOK as i16 + 1, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, dw / 2 + 10, y_btn, 90, 28, 0, -1, -1),
    ];

    // ── pre-select the current state (`deskinf.c:731-777`) ──────────────
    // `deselect_all(tree1)` is implicit: `mk` leaves every `ob_state` at 0.
    tree[SPCDYES + usize::from(!confdel)].ob_state |= OB_SELECTED;
    tree[SPDC1 + dclick as usize].ob_state |= OB_SELECTED;
    // `.rendered()`, not the raw value: a `#E` line whose `INF_E2_IDT*` bits are
    // set (EmuTOS's `INF_E2_DEFAULT`, and therefore every file NewTOS has ever
    // written before DESK02) decodes to `Idt`, which this dialog has no button
    // for. Pre-selecting the button the **renderer** actually uses is what keeps
    // the box honest — with the raw value the fallbacks disagreed, and the
    // measured run showed `DMY` selected while `format_fnode` printed MDY.
    tree[SPTF12HR + usize::from(timefmt.rendered() != TimeFmt::H12)].ob_state |= OB_SELECTED;
    tree[SPDFMMDD + usize::from(datefmt.rendered() != DateFmt::Mdy)].ob_state |= OB_SELECTED;

    // ── run it ──────────────────────────────────────────────────────────
    inf_show(tree.as_mut_ptr() as u64, ROOT);
    if inf_what(&mut tree, SPOK) != 1 {
        return false;
    }

    // ── commit (`deskinf.c:784-814`) ────────────────────────────────────
    let new_confdel = inf_which(&tree, SPCDYES, 2) == 1;
    // `G.g_cdclkpref = evnt_dclick(inf_gindex(tree1, SPDC1, 5), TRUE);`
    let picked = inf_gindex(&tree, SPDC1, 5).max(0);
    let new_dclick = super::evnt_dclick(picked, true).clamp(0, 4) as u8;
    // The two format groups keep EmuTOS's "read the index off the button order"
    // trick, minus its `Default` button: with `12H` then `24H` on screen,
    // `inf_which` yields 1 for the first and 0 for the second, which is exactly
    // `TIMEFORM_12H` / `TIMEFORM_24H` (`deskbind.h:68-69`). Same for `MDY`/`DMY`
    // against `DATEFORM_MDY` / `DATEFORM_DMY` (`:65-66`).
    let new_timefmt = if inf_which(&tree, SPTF12HR, 2) == 1 { TimeFmt::H12 } else { TimeFmt::H24 };
    let new_datefmt = if inf_which(&tree, SPDFMMDD, 2) == 1 { DateFmt::Mdy } else { DateFmt::Dmy };

    global::with(|g| {
        g.g_cdelepref = new_confdel;
        g.g_cdclkpref = new_dclick;
        g.g_ctimeform = new_timefmt;
        g.g_cdateform = new_datefmt;
    });

    // "if the current view is as text, and the date or time format has changed,
    // we need to tell the caller so he can rebuild the windows if necessary"
    // (`deskinf.c:807-814`).
    // Compared against the **rendered** old value, for the same reason the
    // pre-selection uses it: leaving `Idt` for the `H24` the screen already
    // showed changes no pixel and must not order a rebuild.
    view == View::Text && (new_timefmt != timefmt.rendered() || new_datefmt != datefmt.rendered())
}

// ── Set-background dialog (`inf_backgrounds`, `deskinf.c:825-921`) ───────────

/// Options → Set background (`inf_backgrounds`): step the desktop background
/// pattern (0-7) and colour (0-15); on OK apply to `g_patcol`, repaint the
/// desktop, and persist to `NEWDESK.INF`. Returns whether the background
/// changed. The screen the dialog covered is restored by `end_dialog`'s
/// `form_dial(FMD_FINISH)`.
pub(super) fn set_background() -> bool {
    let cur = global::patcol_desktop();
    let mut pat = (cur >> 4) & 0x07;
    let mut col = cur & 0x0F;

    static TITLE: &[u8] = b" Set Background \0";
    static PAT_LBL: &[u8] = b"Pattern:\0";
    static COL_LBL: &[u8] = b"Colour:\0";
    static MINUS: &[u8] = b" - \0";
    static PLUS: &[u8] = b" + \0";
    static OK: &[u8] = b" OK \0";
    static CANCEL: &[u8] = b" Cancel \0";

    // Mutable number buffers, updated + redrawn each step.
    let mut patnum = [0u8; 8];
    let mut colnum = [0u8; 8];

    // Built at the origin; `start_dialog`'s `form_center` writes the centred
    // `ob_x`/`ob_y` into the ROOT.
    let (dw, dh) = (300i16, 180i16);

    // Object indices.
    const PAT_DN: i16 = 4;
    const PAT_UP: i16 = 5;
    const COL_DN: i16 = 8;
    const COL_UP: i16 = 9;
    const OK_OBJ: i16 = 10;

    // 3D roles after `ADBKGND` (`desk_rsc.c:2230-2452`): `FL3DBAK` + `OUTLINED`
    // ROOT, `FL3DACT` on `BGOK`/`BGCANCEL`, no flag on the `G_STRING` labels.
    // `ADBKGND` has no stepper pair — it offers eight pattern and sixteen
    // colour swatches as `TOUCHEXIT` `G_BOX`es — but `desk_rsc.c` does have
    // stepper *gadgets*, and they are activators: `ADINSDSK`'s `ID_UP`/
    // `ID_DOWN` and `ADINSWIN`'s `IW_UP`/`IW_DOWN` are
    // `SELECTABLE | TOUCHEXIT | FL3DBAK | FL3DIND` = `FL3DACT`
    // (`desk_rsc.c:990,996,2094,2100`). The four `- / +` buttons take that.
    let mut tree = [
        mk_root(BOX_SPEC, dw, dh, 11),
        mk(G_STRING, 0, TITLE.as_ptr() as u64, 24, 14, dw - 48, 16, 2, -1, -1),
        mk(G_STRING, 0, PAT_LBL.as_ptr() as u64, 24, 48, 72, 16, 3, -1, -1),
        mk(G_STRING, 0, patnum.as_ptr() as u64, 104, 48, 40, 16, 4, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, MINUS.as_ptr() as u64, 160, 44, 44, 24, 5, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, PLUS.as_ptr() as u64, 214, 44, 44, 24, 6, -1, -1),
        mk(G_STRING, 0, COL_LBL.as_ptr() as u64, 24, 84, 72, 16, 7, -1, -1),
        mk(G_STRING, 0, colnum.as_ptr() as u64, 104, 84, 40, 16, 8, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, MINUS.as_ptr() as u64, 160, 80, 44, 24, 9, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, PLUS.as_ptr() as u64, 214, 80, 44, 24, 10, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT | FL3DACT, OK.as_ptr() as u64, 30, 138, 90, 28, 11, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, 178, 138, 90, 28, 0, -1, -1),
    ];
    // The two number fields, refreshed whenever a step button was pressed.
    let set_numbers = |pat: u8, col: u8, patnum: &mut [u8; 8], colnum: &mut [u8; 8]| {
        let mut p = 0usize;
        push_dec(patnum, &mut p, pat as u64);
        patnum[p.min(patnum.len() - 1)] = 0;
        let mut q = 0usize;
        push_dec(colnum, &mut q, col as u64);
        colnum[q.min(colnum.len() - 1)] = 0;
    };
    set_numbers(pat, col, &mut patnum, &mut colnum);

    // `start_dialog(tree)` … loop of `form_do` … `end_dialog(tree)` — EmuTOS
    // `inf_backgrounds` (`deskinf.c:862-896`). EmuTOS redraws only the changed
    // sample field inside the loop (`draw_fld(tree, BGSAMPLE)`); with no
    // per-field draw helper here the whole dialog is redrawn between turns, the
    // way `ins_app_dialog` does on its retry path (`deskins.c:527`).
    let tree_ptr = tree.as_ptr() as u64;
    start_dialog(tree_ptr);
    let committed = loop {
        let clicked = super::form_do(tree_ptr, ROOT);
        // Clear the pressed button's SELECTED bit so it does not stay inverted.
        if let Some(o) = tree.get_mut(clicked as usize) {
            o.ob_state &= !OB_SELECTED;
        }
        match clicked {
            PAT_DN => pat = pat.saturating_sub(1),
            PAT_UP => pat = (pat + 1).min(7),
            COL_DN => col = col.saturating_sub(1),
            COL_UP => col = (col + 1).min(15),
            OK_OBJ => break true,
            _ => break false, // Cancel / default
        }
        set_numbers(pat, col, &mut patnum, &mut colnum);
        super::dir::draw_dial(tree_ptr);
    };
    end_dialog(tree_ptr);

    if !committed {
        return false;
    }
    let new = (pat << 4) | col;
    if new == cur {
        return false;
    }
    global::set_patcol_desktop(new);
    // Push the new pattern/colour into DROOT's box `ob_spec` (the AES draws that
    // box as the background), then repaint the exposed desktop and persist.
    global::with(|g| g.sync_desk_background());
    let desk = global::with(|g| g.g_desk).unwrap_or_default();
    super::redraw::do_wredraw(super::win::DESKWH, desk);
    // NewTOS persists the new background straight away; EmuTOS leaves that to
    // the user's next Save desktop (`inf_backgrounds` ends at
    // `deskinf.c:919`). Keep the invariant every EmuTOS `app_save` call has
    // anyway — `cnx_put()` first (`deskmain.c:891,2154`) — so the `#W` lines
    // written here describe the windows that are actually on screen.
    super::main::cnx_put();
    save_desktop();
    true
}

// ── Desktop-configuration dialog (`inf_conf`, `deskinf.c:925-1246`) ─────────

/// `FIRST_FUNKEY` (`deskins.h:16`) — `a_funkey` numbering starts at 1.
const FIRST_FUNKEY: u8 = 1;
/// `NUM_FUNKEYS` (`deskins.h:18`) — F1…F20.
const NUM_FUNKEYS: usize = 20;

/// `inf_sset` (`util/optimize.c:103-112`) — `strlcpy(ted->te_ptext, pstr,
/// ted->te_txtlen)`: copy `src` into a `TEDINFO` text buffer, truncating to the
/// field's capacity and always NUL-terminating.
fn inf_sset(buf: &mut [u8], src: &[u8]) {
    let n = src.len().min(buf.len() - 1);
    buf[..n].copy_from_slice(&src[..n]);
    buf[n..].fill(0);
}

/// One function key the desktop knows about: its number (1…20) and the
/// application path bound to it — EmuTOS's `pa[]` array of `ANODE` pointers
/// (`get_all_funkeys`, `deskinf.c:928-948`).
struct FunKey {
    num: u8,
    path: String,
}

/// `get_all_funkeys` (`deskinf.c:928-948`) — the installed applications that
/// carry a function key, indexed by `a_funkey - FIRST_FUNKEY`.
///
/// EmuTOS walks `G.g_ahead`, the one list that holds both desktop icons and
/// installed-application associations. NewTOS splits those two ([`Anode`]'s
/// doc-comment): the association half — the half that can carry `a_funkey` at
/// all — is this module's [`SESSION`] model, so that is what is walked here.
/// The `NEWDESK.INF` extra block round-trips the number ([`InfAnode::funkey`]),
/// so a file written by EmuTOS, or by a NewTOS that later learns to assign
/// them, shows up in this dialog.
fn get_all_funkeys() -> [Option<FunKey>; NUM_FUNKEYS] {
    const NONE: Option<FunKey> = None;
    let mut out = [NONE; NUM_FUNKEYS];
    let guard = SESSION.lock();
    let Some(data) = guard.as_ref() else {
        return out;
    };
    for a in &data.anodes {
        if a.is_desk {
            continue;
        }
        let i = a.funkey.wrapping_sub(FIRST_FUNKEY) as usize;
        if a.funkey >= FIRST_FUNKEY && i < NUM_FUNKEYS {
            out[i] = Some(FunKey { num: a.funkey, path: a.pappl.clone() });
        }
    }
    out
}

/// Options → Desktop configuration (`inf_conf`, `deskinf.c:1096-1246`,
/// `CONF_WITH_DESKTOP_CONFIG` — on by default,
/// `include/config.h:1774-1776`, and therefore on for our Falcon030/TOS4
/// profile).
///
/// Two independent "step through a list" panels in one dialog, plus a free-RAM
/// read-out:
///
/// * **Function keys** — a *viewer*. `DCFUNPRV`/`DCFUNNXT` walk the installed
///   applications that have a function key bound; `DCFUNLT`/`DCFUNRT` scroll
///   the path sideways inside the fixed-width field (`scroll_conf_funkeys`,
///   `:952-969`). Nothing here assigns a key — EmuTOS assigns them in the
///   Install-Application dialog (`deskins.c` `APFUNKEY`), and the display is
///   read-only there too. With no key bound the panel shows its empty template,
///   which is exactly what EmuTOS shows on a fresh `EMUDESK.INF`.
/// * **Menu shortcut keys** — the editor. `DCMNUPRV`/`DCMNUNXT` step through
///   the [`super::menu::NUM_SHORTCUTS`] shortcut slots, showing the menu item's
///   text and its current key; the key is typed into `DCMNUKEY` and committed
///   by `save_shortcut` when the slot is left or the dialog OK'd, with the
///   duplicate-key question of `:1027-1057`. `DCMNUCLR` clears all 33 at once,
///   with no confirmation (`:1218-1222`).
/// * **Free RAM** — `Mxalloc(-1)` per pool (`:1119-1124`).
///
/// On OK the shortcut array is rendered into the menu strings
/// (`install_shortcuts`); on any other exit it is rebuilt *from* them
/// (`recover_shortcuts`), which undoes the edits. Like EmuTOS, nothing here
/// writes `NEWDESK.INF` — the new shortcuts reach the file through
/// *Save desktop*, which now emits the `#K` line.
///
/// ## What this dialog does **not** offer, and why
///
/// `ADDESKCF` (`desk_rsc.c:1603-1772`) starts with two further radio groups,
/// both of which NewTOS leaves out rather than showing inert:
///
/// * **Default dir** (`DCDEFAPP`/`DCDEFWIN` → `G.g_appdir`) selects the current
///   directory a *non-installed* program is launched with:
///   `do_aopen` picks the top window's path or the program's own and calls
///   `set_default_path()` (`desksupp.c:896-912`). NewTOS's launch path
///   ([`super::main`]'s `launch_native` → [`crate::launch::run_program`]) never
///   sets a current directory at all, so there is nothing for the switch to
///   steer.
/// * **Parameter** (`DCPMFULL`/`DCPMFILE` → `G.g_fullpath`) selects whether a
///   file dropped on a program icon is passed as a full path or a bare name
///   (`build_selected_path`, `deskfun.c:903-931`), reached from
///   `fun_win2win`'s drop-on-an-application branch (`deskfun.c:960-970`).
///   NewTOS's `fun_win2win` has no such branch — a drop on a program is a
///   copy — so again there is nothing behind the switch.
///
/// Both are missing *consumers* in other functions, not missing pieces of this
/// one; their `#E` byte-5 bits (`INF_E5_APPDIR`/`INF_E5_ISFULL`) keep
/// round-tripping untouched meanwhile. Same rule as the three groups
/// [`inf_pref`] leaves out (DESK02).
pub(super) fn inf_conf() {
    use super::menu;

    // ── object indices (`desk_rsc.h:191-211`, minus the two omitted groups) ──
    /// `DCFUNBOX` — the panel holding the function-key number, path and the two
    /// scroll arrows; the field `draw_fld` refreshes after a step (`:1163`).
    const DCFUNBOX: i16 = 3;
    /// `DCFUNNUM` — the `F__` number field.
    const DCFUNNUM: i16 = 4;
    /// `DCFUNLT` — scroll the path text left.
    const DCFUNLT: i16 = 5;
    /// `DCFUNPTH` — the scrollable path field.
    const DCFUNPTH: i16 = 6;
    /// `DCFUNRT` — scroll the path text right.
    const DCFUNRT: i16 = 7;
    /// `DCFUNPRV` — previous bound function key.
    const DCFUNPRV: i16 = 8;
    /// `DCFUNNXT` — next bound function key.
    const DCFUNNXT: i16 = 9;
    /// `DCMNUBOX` — the panel holding the menu-item text, its key and *Clear
    /// all*.
    const DCMNUBOX: i16 = 11;
    /// `DCMNUKEY` — the editable `Key: ^_` field.
    const DCMNUKEY: i16 = 12;
    /// `DCMNUCLR` — clear every shortcut.
    const DCMNUCLR: i16 = 13;
    /// `DCMNUTXT` — the menu item the current slot names.
    const DCMNUTXT: i16 = 14;
    /// `DCMNUPRV` — previous shortcut slot.
    const DCMNUPRV: i16 = 15;
    /// `DCMNUNXT` — next shortcut slot.
    const DCMNUNXT: i16 = 16;
    /// `DCFREMEM` — the free-RAM read-out.
    const DCFREMEM: i16 = 17;
    /// `DCOK` — the OK button; `DC_CNCL` follows it.
    const DCOK: i16 = 18;
    /// `DC_CNCL` — Cancel.
    const DC_CNCL: i16 = 19;

    // ── geometry ────────────────────────────────────────────────────────
    //
    // `ADDESKCF`'s own, character cell for character cell (`desk_rsc.c:1603-
    // 1772`, `rsrc_obfix`-resolved at 8x16): a 39x22-cell box whose rows are
    // title 1, *Default dir* 3, *Parameter* 5, *Function keys* 7, the function
    // panel 8+6px, *Menu shortcut keys* 11+8px, the shortcut panel 12+14px,
    // free RAM 17+8px, the buttons 20. Dropping the two omitted radio rows
    // takes four character rows out of the middle, so everything from
    // *Function keys* down moves up by `4 * CELL_H` and the box loses the same
    // 4 cells of height — the spacing between the rows that remain is
    // untouched, which matters because every `FL3DACT` gadget here is grown by
    // `ADJ3DSTD` (2 px per side) and then laid out one pixel of border wider
    // still (`aes/gemoblib.c:504`, `sst_th` `G_BUTTON` → `-1`).
    /// The four character rows the omitted *Default dir* / *Parameter* groups
    /// occupied.
    const DROPPED: i16 = 4 * CELL_H;
    let dw = 39 * CELL_W; // 312
    let dh = 22 * CELL_H - DROPPED; // 288
    let y_fun_lbl = 7 * CELL_H - DROPPED; // 48
    let y_fun_box = 8 * CELL_H + 6 - DROPPED; // 70
    let y_mnu_lbl = 11 * CELL_H + 8 - DROPPED; // 120
    let y_mnu_box = 12 * CELL_H + 14 - DROPPED; // 142
    let y_freemem = 17 * CELL_H + 8 - DROPPED; // 216
    let y_btn = 20 * CELL_H - DROPPED; // 256
    // `adjust_3d_positions` (`deskmain.c:1647-1656`): "avoid button overlap" —
    // the two `PRV` gadgets sit flush against their `NXT` neighbour in the
    // resource, so a 3D build moves them left by `3 * ADJ3DSTD`.
    const ADJ3DSTD: i16 = 2;
    let x_prv = 30 * CELL_W - 3 * ADJ3DSTD; // 234
    let x_nxt = 34 * CELL_W; // 272

    // ── strings ─────────────────────────────────────────────────────────
    static TITLE: &[u8] = b"DESKTOP CONFIGURATION\0";
    static FUN_LBL: &[u8] = b"Function keys:\0";
    static MNU_LBL: &[u8] = b"Menu shortcut keys:\0";
    static CLEAR_ALL: &[u8] = b"Clear all\0";
    // Characters 1-4 of the system font are the up / down / right / left
    // arrows — `ADDESKCF` labels its gadgets `"\001"`, `"\002"`, `"\003"`,
    // `"\004"` (`desk_rsc.c:1682,1697,1704,1710`).
    static UP: &[u8] = b"\x01\0";
    static DOWN: &[u8] = b"\x02\0";
    static RIGHT: &[u8] = b"\x03\0";
    static LEFT: &[u8] = b"\x04\0";
    static OK: &[u8] = b" OK \0";
    static CANCEL: &[u8] = b" Cancel \0";
    /// `STDUPCUT` (`desk_rsc.c:2563-2564`) — the duplicate-shortcut question.
    /// Two buttons, so it goes to `form_alert` verbatim rather than through
    /// `dir::fun_alert`'s one-OK-button wrapper.
    static STDUPCUT: &[u8] =
        b"[2][Last key is already assigned.|Do you want to reassign it?][  OK  |Cancel]\0";

    // ── TEDINFO templates + buffers (`desk_rs_tedinfo[28..32]`) ─────────
    static T_FUNNUM: &[u8] = b"F__\0";
    static T_FUNPTH: &[u8] = b"______________________\0";
    static T_MNUKEY: &[u8] = b"Key: ^_\0";
    static T_MNUTXT: &[u8] = b"________________________________\0";
    static T_FREMEM: &[u8] = b"Free RAM: _____________________\0";
    static V_DIGIT: &[u8] = b"9\0";
    static V_ANY: &[u8] = b"X\0";
    static V_ALPHA: &[u8] = b"a\0";

    /// `te_txtlen` of `DCFUNNUM` (`desk_rs_tedinfo[28]`).
    const FUNNUM_LEN: usize = 3;
    /// `te_txtlen` of `DCFUNPTH` (`[29]`) — the scroll window's width + NUL.
    const FUNPTH_LEN: usize = 23;
    /// `te_txtlen` of `DCMNUKEY` (`[30]`) — one character + NUL.
    const MNUKEY_LEN: usize = 2;
    /// `te_txtlen` of `DCMNUTXT` (`[31]`).
    const MNUTXT_LEN: usize = 33;
    /// `te_txtlen` of `DCFREMEM` (`[32]`).
    const FREMEM_LEN: usize = 22;

    let mut b_funnum = [0u8; FUNNUM_LEN];
    let mut b_funpth = [0u8; FUNPTH_LEN];
    let mut b_mnukey = [0u8; MNUKEY_LEN];
    let mut b_mnutxt = [0u8; MNUTXT_LEN];
    let mut b_fremem = [0u8; FREMEM_LEN];

    // `te_color` and `te_thickness` are the resource's own, per field: `4480`
    // (`0x1180`) is REPLACE-mode text on the panel face, `4352` (`0x1100`) the
    // TRANSPARENT one ([`TED_COLOR`]); a `te_thickness` of `-1` frames the
    // field, `0` leaves it unframed (`gr_crack` / `sst_th`).
    const TED_OPAQUE: i16 = 0x1180;
    let ted = |text: &mut [u8], tmpl: &[u8], valid: &[u8], just: i16, color: i16, th: i16| TedInfo {
        te_ptext: text.as_mut_ptr() as u64,
        te_ptmplt: tmpl.as_ptr() as u64,
        te_pvalid: valid.as_ptr() as u64,
        te_font: 3, // IBM (`obdefs.h:62`)
        te_junk1: 0,
        te_just: just,
        te_color: color,
        te_junk2: 0,
        te_thickness: th,
        te_txtlen: text.len() as i16,
        te_tmplen: tmpl.len() as i16,
    };
    let mut ted_funnum = ted(&mut b_funnum, T_FUNNUM, V_DIGIT, 2 /* TE_CNTR */, TED_OPAQUE, -1);
    let mut ted_funpth = ted(&mut b_funpth, T_FUNPTH, V_ANY, 0, TED_COLOR, 0);
    let mut ted_mnukey = ted(&mut b_mnukey, T_MNUKEY, V_ALPHA, 0, TED_OPAQUE, -1);
    let mut ted_mnutxt = ted(&mut b_mnutxt, T_MNUTXT, V_ANY, 0, TED_COLOR, -1);
    let mut ted_fremem = ted(&mut b_fremem, T_FREMEM, V_ANY, 0, TED_OPAQUE, 0);

    // `DCFUNBOX`/`DCMNUBOX` `ob_spec` = `16716032L` = `0xFF1100`
    // (`desk_rsc.c:1673,1721`): thickness byte `0xFF` = `th = -1`, a one-pixel
    // border **outside** the rectangle, over the hollow-white colour word that
    // lets [`FL3DBAK`] substitute the panel face (DESK01b).
    const PANEL_SPEC: u64 = 0x00FF_1100;

    #[rustfmt::skip]
    let mut tree = [
        // 0 ROOT
        mk_root(BOX_SPEC, dw, dh, DC_CNCL),
        // 1 title — `align_title` centres it on the box (`deskmain.c:1739-1747`).
        mk(G_STRING, 0, TITLE.as_ptr() as u64, (dw - 21 * CELL_W) / 2, CELL_H, 21 * CELL_W, CELL_H, 2, -1, -1),
        // 2 "Function keys:"
        mk(G_STRING, 0, FUN_LBL.as_ptr() as u64, CELL_W, y_fun_lbl, 14 * CELL_W, CELL_H, DCFUNBOX, -1, -1),
        // 3 DCFUNBOX            4..7 its children
        mk(G_BOX, FL3DBAK, PANEL_SPEC, CELL_W, y_fun_box, 37 * CELL_W, CELL_H + 8, DCFUNPRV, DCFUNNUM, DCFUNRT),
        mk(G_FTEXT, FL3DBAK, &mut ted_funnum as *mut _ as u64, 4, 4, 3 * CELL_W, CELL_H, DCFUNLT, -1, -1),
        mk(G_BUTTON, TOUCHEXIT | FL3DACT, LEFT.as_ptr() as u64, 5 * CELL_W + 4, 4, 3 * CELL_W + 4, CELL_H, DCFUNPTH, -1, -1),
        mk(G_FBOXTEXT, FL3DBAK, &mut ted_funpth as *mut _ as u64, 10 * CELL_W, 4, 22 * CELL_W, CELL_H, DCFUNRT, -1, -1),
        mk(G_BUTTON, TOUCHEXIT | FL3DACT, RIGHT.as_ptr() as u64, 33 * CELL_W, 4, 3 * CELL_W + 4, CELL_H, DCFUNBOX, -1, -1),
        // 8/9 DCFUNPRV / DCFUNNXT
        mk(G_BUTTON, TOUCHEXIT | FL3DACT, UP.as_ptr() as u64, x_prv, y_fun_lbl, 4 * CELL_W, CELL_H, DCFUNNXT, -1, -1),
        mk(G_BUTTON, TOUCHEXIT | FL3DACT, DOWN.as_ptr() as u64, x_nxt, y_fun_lbl, 4 * CELL_W, CELL_H, 10, -1, -1),
        // 10 "Menu shortcut keys:"
        mk(G_STRING, 0, MNU_LBL.as_ptr() as u64, CELL_W, y_mnu_lbl, 19 * CELL_W, CELL_H, DCMNUBOX, -1, -1),
        // 11 DCMNUBOX           12..14 its children
        mk(G_BOX, FL3DBAK, PANEL_SPEC, CELL_W, y_mnu_box, 37 * CELL_W, 3 * CELL_H + 4, DCMNUPRV, DCMNUKEY, DCMNUTXT),
        mk(G_FTEXT, EDITABLE | FL3DBAK, &mut ted_mnukey as *mut _ as u64, 4, 2 * CELL_H, 15 * CELL_W, CELL_H, DCMNUCLR, -1, -1),
        mk(G_BUTTON, TOUCHEXIT | FL3DACT, CLEAR_ALL.as_ptr() as u64, 17 * CELL_W + 4, 2 * CELL_H, 19 * CELL_W, CELL_H, DCMNUTXT, -1, -1),
        mk(G_FBOXTEXT, 0, &mut ted_mnutxt as *mut _ as u64, 4, 4, 36 * CELL_W, CELL_H, DCMNUBOX, -1, -1),
        // 15/16 DCMNUPRV / DCMNUNXT
        mk(G_BUTTON, TOUCHEXIT | FL3DACT, UP.as_ptr() as u64, x_prv, y_mnu_lbl, 4 * CELL_W, CELL_H, DCMNUNXT, -1, -1),
        mk(G_BUTTON, TOUCHEXIT | FL3DACT, DOWN.as_ptr() as u64, x_nxt, y_mnu_lbl, 4 * CELL_W, CELL_H, DCFREMEM, -1, -1),
        // 17 DCFREMEM
        mk(G_FTEXT, FL3DBAK, &mut ted_fremem as *mut _ as u64, CELL_W, y_freemem, 36 * CELL_W, CELL_H, DCOK, -1, -1),
        // 18/19 OK / Cancel (`DCOK` is `SELECTABLE|DEFAULT|EXIT|FL3DACT`,
        // `DC_CNCL` the same without `DEFAULT`, `desk_rsc.c:1759-1771`).
        mk(G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, OK.as_ptr() as u64, 7 * CELL_W, y_btn, 9 * CELL_W, CELL_H, DC_CNCL, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, 23 * CELL_W, y_btn, 9 * CELL_W, CELL_H, 0, -1, -1),
    ];
    // `DCFUNBOX`/`DCMNUBOX` carry `OUTLINED` like the ROOT (`desk_rsc.c:1672,
    // 1720`), which on an `FL3DBAK` panel is EmuTOS's `draw_3d_outline`.
    tree[DCFUNBOX as usize].ob_state = OUTLINED;
    tree[DCMNUBOX as usize].ob_state = OUTLINED;

    // ── free-RAM line (`deskinf.c:1118-1125`) ───────────────────────────
    // `sprintf("%ld KB", dos_avail_stram()/1024)`, plus `" + %ld KB"` of
    // alt-RAM when the machine has any (`has_alt_ram`; here: a non-zero
    // `Mxalloc(-1, MX_TTRAM)`). The trailing blanks are EmuTOS's
    // `memset(G.g_work+n, ' ', 40)` — enough to fill the object's text field.
    {
        let mut work = [b' '; FREMEM_LEN - 1];
        let mut p = 0usize;
        push_dec(&mut work, &mut p, (super::dosbind::dos_avail_stram() / 1024) as u64);
        push_str(&mut work, &mut p, b" KB");
        let alt = super::dosbind::dos_avail_altram();
        if alt > 0 {
            push_str(&mut work, &mut p, b" + ");
            push_dec(&mut work, &mut p, (alt / 1024) as u64);
            push_str(&mut work, &mut p, b" KB");
        }
        work[p..].fill(b' ');
        inf_sset(&mut b_fremem, &work);
    }

    // ── function-key setup (`deskinf.c:1127-1143`) ──────────────────────
    let funkeys = get_all_funkeys();
    // `textpos`/`maxpos`, EmuTOS's two file-scope scroll variables (`:950`).
    let mut textpos: i16 = 0;
    let mut maxpos: i16 = 0;
    // `init_conf_funkeys` (`:974-991`): the number into `DCFUNNUM`, the path
    // into `DCFUNPTH` at scroll position 0.
    let init_funkeys = |fk: &FunKey, b_num: &mut [u8], b_pth: &mut [u8], textpos: &mut i16, maxpos: &mut i16| {
        let mut num = [b' '; FUNNUM_LEN - 1];
        let mut p = 0usize;
        push_dec(&mut num, &mut p, fk.num as u64);
        inf_sset(b_num, &num);
        *maxpos = (fk.path.len() as i16 - FUNPTH_LEN as i16 + 1).max(0);
        *textpos = 0;
        inf_sset(b_pth, fk.path.as_bytes());
    };
    // `scroll_conf_funkeys` (`:957-969`) — returns whether the text moved.
    let scroll_funkeys = |fk: &FunKey, b_pth: &mut [u8], textpos: &mut i16, maxpos: i16, amount: i16| -> bool {
        let newpos = *textpos + amount;
        if newpos < 0 || newpos > maxpos {
            return false;
        }
        *textpos = newpos;
        inf_sset(b_pth, &fk.path.as_bytes()[newpos as usize..]);
        true
    };

    // "find lowest function key" (`:1136-1142`).
    let mut current_funkey = funkeys.iter().position(Option::is_some).unwrap_or(usize::MAX);
    if let Some(fk) = funkeys.get(current_funkey).and_then(Option::as_ref) {
        init_funkeys(fk, &mut b_funnum, &mut b_funpth, &mut textpos, &mut maxpos);
    } else {
        current_funkey = 0; // "ensure legal value for array index" (`:1143`)
    }

    // ── menu-shortcut setup (`deskinf.c:1145-1154`) ─────────────────────
    // `init_conf_shortcuts` (`:997-1018`): the slot's key into `DCMNUKEY`, and
    // the menu item's text — minus its two leading blanks and minus the
    // `SHORTCUT_SIZE` reserve at its right edge — into `DCMNUTXT`.
    let init_shortcuts = |n: usize, b_key: &mut [u8], b_txt: &mut [u8]| {
        let key = menu::shortcut_key(n);
        inf_sset(b_key, &[if key == 0 { b' ' } else { key }]);
        let mut work = [b' '; MNUTXT_LEN - 1];
        menu::shortcut_item_text(n, &mut work);
        inf_sset(b_txt, &work);
    };
    // `save_shortcut` (`:1027-1057`): commit the edited key into the array,
    // asking before stealing it from another item.
    let save_shortcut = |n: usize, b_key: &[u8]| {
        let mut key = b_key[0];
        key.make_ascii_uppercase();
        if key.is_ascii_uppercase() {
            for i in 0..menu::NUM_SHORTCUTS {
                if i == n {
                    continue; // "ignore ourselves"
                }
                if menu::shortcut_key(i) == key {
                    if super::form_alert(1, STDUPCUT.as_ptr()) != 1 {
                        return; // user cancelled — leave both slots alone
                    }
                    menu::set_shortcut_key(i, 0);
                    break;
                }
            }
        } else {
            key = 0;
        }
        menu::set_shortcut_key(n, key);
    };

    // "find lowest available menu item" (`:1147-1154`).
    let mut current_shortcut = (0..menu::NUM_SHORTCUTS)
        .find(|&i| menu::shortcut_item(i).is_some())
        .unwrap_or(0);
    init_shortcuts(current_shortcut, &mut b_mnukey, &mut b_mnutxt);

    // ── interact (`deskinf.c:1156-1240`) ────────────────────────────────
    let tree_ptr = tree.as_mut_ptr() as u64;
    start_dialog(tree_ptr);
    let mut exitobj;
    loop {
        let mut redraw: i16 = -1; // "by default, no redraw"
        exitobj = super::form_do(tree_ptr, ROOT) & 0x7fff;
        let mut done = false;
        match exitobj {
            // "look for next lower" / "next higher" (`:1160-1186`).
            DCFUNPRV | DCFUNNXT => {
                let found = if exitobj == DCFUNPRV {
                    (0..current_funkey).rev().find_map(|i| funkeys[i].as_ref().map(|fk| (i, fk)))
                } else {
                    (current_funkey + 1..NUM_FUNKEYS).find_map(|i| funkeys[i].as_ref().map(|fk| (i, fk)))
                };
                if let Some((i, fk)) = found {
                    current_funkey = i;
                    init_funkeys(fk, &mut b_funnum, &mut b_funpth, &mut textpos, &mut maxpos);
                    redraw = DCFUNBOX;
                }
            }
            DCFUNLT | DCFUNRT => {
                let amount = if exitobj == DCFUNLT { -1 } else { 1 };
                if let Some(fk) = funkeys[current_funkey].as_ref() {
                    if scroll_funkeys(fk, &mut b_funpth, &mut textpos, maxpos, amount) {
                        redraw = DCFUNPTH;
                    }
                }
            }
            DCMNUPRV => {
                save_shortcut(current_shortcut, &b_mnukey);
                if let Some(i) = (0..current_shortcut).rev().find(|&i| menu::shortcut_item(i).is_some()) {
                    current_shortcut = i;
                    init_shortcuts(i, &mut b_mnukey, &mut b_mnutxt);
                    redraw = DCMNUBOX;
                }
            }
            DCMNUNXT => {
                save_shortcut(current_shortcut, &b_mnukey);
                if let Some(i) =
                    (current_shortcut + 1..menu::NUM_SHORTCUTS).find(|&i| menu::shortcut_item(i).is_some())
                {
                    current_shortcut = i;
                    init_shortcuts(i, &mut b_mnukey, &mut b_mnutxt);
                    redraw = DCMNUBOX;
                }
            }
            DCMNUCLR => {
                menu::set_shortcuts([0; menu::NUM_SHORTCUTS]);
                inf_sset(&mut b_mnukey, b"");
                redraw = DCMNUBOX;
            }
            DCOK => {
                save_shortcut(current_shortcut, &b_mnukey);
                done = true;
            }
            DC_CNCL => done = true,
            _ => {}
        }
        // `tree[exitobj].ob_state &= ~SELECTED;` (`:1232`).
        if let Some(o) = tree.get_mut(exitobj as usize) {
            o.ob_state &= !OB_SELECTED;
        }
        if done {
            break;
        }
        if redraw >= 0 {
            super::dir::draw_fld(tree_ptr, redraw);
        }
    }

    // `if (exitobj == DCOK) install_shortcuts(); else recover_shortcuts();`
    // (`:1236-1241`) — commit the array into the menu strings, or rebuild it
    // from them and so discard every edit made in this dialog.
    if exitobj == DCOK {
        menu::install_shortcuts();
    } else {
        menu::recover_shortcuts();
    }

    end_dialog(tree_ptr);
}

// ── Round-trip self-test (verification (b); not run at boot) ─────────────────

/// Save→read round-trip verification against the live SD image (mirrors the
/// other `selftest_*` modules): build a representative [`InfData`], round-trip
/// it (1) purely in memory and (2) through `NEWDESK.INF` on disk, and confirm
/// the parsed-back state matches. Prints one line per check. **Mutates the SD
/// image** — run only against a fresh `/tmp/sd.img`.
pub fn selftest() {
    use crate::kprintln;

    kprintln!("[desk-inf] NEWDESK.INF round-trip self-test on C:\\ (mutates the SD image)");
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut check = |name: &str, cond: bool| {
        if cond {
            pass += 1;
            kprintln!("[desk-inf]  PASS {}", name);
        } else {
            fail += 1;
            kprintln!("[desk-inf]  FAIL {}", name);
        }
    };

    // A representative state: text view, sort by size, custom background, two
    // desktop-icon positions, one window, and the default file-type anodes.
    let mut original = InfData::default();
    let (e1, e5) = state_to_env(View::Text, Sort::Size, false, original.e1, original.e5);
    // …and a non-default value for every Preferences field, so the `#E` bytes
    // the dialog writes are exercised too (DESK02): confirm-deletes off,
    // double-click 4, 12-hour clock, day-before-month.
    let (e1, e2) = prefs_to_env(false, 4, TimeFmt::H12, DateFmt::Dmy, e1, original.e2);
    original.e1 = e1;
    original.e2 = e2;
    original.e5 = e5;
    original.patcol = [(0x51, 0x40), (0x63, 0x41), (0x72, 0x0A)];
    original.windows.push(InfWindow { hsl: 0, vsl: 0, xc: 2, yc: 6, wc: 0x26, hc: 0x0C, path: String::from("C:\\") });
    original.anodes.push(InfAnode {
        tag: b'M',
        is_desk: true,
        has_extra: false,
        xspot: 3,
        yspot: 5,
        aicon: -1,
        dicon: -1,
        letter: b'C',
        pappl: String::from("Disk C"),
        pdata: String::new(),
        flags: 0,
        funkey: 0,
        pargs: String::new(),
    });
    original.anodes.push(InfAnode {
        tag: b'T',
        is_desk: true,
        has_extra: false,
        xspot: 7,
        yspot: 1,
        aicon: -1,
        dicon: -1,
        letter: 0,
        pappl: String::from("Trash"),
        pdata: String::new(),
        flags: 0,
        funkey: 0,
        pargs: String::new(),
    });
    original.anodes.extend(InfData::default_type_anodes());

    // (1) In-memory round-trip: serialize → parse → compare.
    let bytes = original.serialize();
    let mem = parse_inf(&bytes);
    check("in-memory: #E view/sort", env_to_state(mem.e1, mem.e5) == env_to_state(original.e1, original.e5));
    check(
        "in-memory: #E preferences",
        env_to_prefs(mem.e1, mem.e2) == (false, 4, TimeFmt::H12, DateFmt::Dmy),
    );
    check("in-memory: #Q background", mem.patcol == original.patcol);
    check("in-memory: #W geometry", mem.windows == original.windows);
    check("in-memory: ANODE positions", mem.anodes == original.anodes);
    check("in-memory: full equality", mem == original);

    // (2) Disk round-trip: write NEWDESK.INF → read it back → parse → compare.
    let wrote = write_inf_file(&bytes);
    check("disk: write NEWDESK.INF", wrote);
    match read_inf_file() {
        Some(back) => {
            let disk = parse_inf(&back);
            check("disk: #E view/sort", env_to_state(disk.e1, disk.e5) == env_to_state(original.e1, original.e5));
            check(
                "disk: #E preferences",
                env_to_prefs(disk.e1, disk.e2) == (false, 4, TimeFmt::H12, DateFmt::Dmy),
            );
            check("disk: #Q background", disk.patcol == original.patcol);
            check("disk: #W geometry", disk.windows == original.windows);
            check("disk: ANODE positions", disk.anodes == original.anodes);
            check("disk: full equality", disk == original);
        }
        None => check("disk: read NEWDESK.INF back", false),
    }

    kprintln!("[desk-inf] round-trip self-test: {} passed, {} failed", pass, fail);
}

/// Install-Application association self-test (deskins #20): install an
/// application association, persist it to `NEWDESK.INF`, read it back off disk,
/// and confirm the document→application lookup resolves correctly. Prints the
/// written `NEWDESK.INF` and one line per check. **Mutates the SD image** — run
/// only against a fresh `/tmp/sd.img`. Verifies the parts of the feature that do
/// not need a keyboard (the dialog text entry is real-hardware-only).
pub fn selftest_install() {
    use crate::kprintln;

    kprintln!("[desk-ins] Install-Application association self-test on C:\\ (mutates the SD image)");
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut check = |name: &str, cond: bool| {
        if cond {
            pass += 1;
            kprintln!("[desk-ins]  PASS {}", name);
        } else {
            fail += 1;
            kprintln!("[desk-ins]  FAIL {}", name);
        }
    };

    // Start from a clean model, then install HELLO.NCP (a real native console
    // program on the test image) as the handler for *.TXT documents, passing the
    // document's full path (AF_ISFULL) so the persisted line carries the extra
    // flags block. A .NCP is a TOS-console flavour → the #F tag.
    *SESSION.lock() = None;
    install_application("C:\\HELLO.NCP", "TXT", AppFlavor::Tos, true, false, "");

    // The in-memory model resolves the association immediately.
    let mem = lookup_association(b"README.TXT");
    check("mem: README.TXT resolves", mem.as_ref().map(|a| a.app_path.as_str()) == Some("C:\\HELLO.NCP"));
    check("mem: full-path flag set", mem.as_ref().map(|a| a.full_path) == Some(true));
    check("mem: HELLO.BIN has no association", lookup_association(b"HELLO.BIN").is_none());

    // Persist to disk, then dump the written NEWDESK.INF so the #F line is visible.
    let wrote = save_desktop();
    check("disk: write NEWDESK.INF", wrote);
    if let Some(bytes) = read_inf_file() {
        kprintln!("[desk-ins] ---- NEWDESK.INF ({} bytes) ----", bytes.len());
        for line in bytes.split(|&b| b == b'\n') {
            let end = line.iter().position(|&b| b == b'\r').unwrap_or(line.len());
            if end == 0 {
                continue;
            }
            kprintln!("[desk-ins] | {}", core::str::from_utf8(&line[..end]).unwrap_or("<non-utf8>"));
        }
        kprintln!("[desk-ins] ---- end NEWDESK.INF ----");

        // Reload purely from the disk bytes (fresh parse into the session), then
        // confirm the association survived the write→read round-trip.
        let reparsed = parse_inf(&bytes);
        *SESSION.lock() = Some(reparsed);
        let disk = lookup_association(b"README.TXT");
        check("disk: README.TXT resolves to HELLO.NCP", disk.as_ref().map(|a| a.app_path.as_str()) == Some("C:\\HELLO.NCP"));
        check("disk: full-path flag survived", disk.as_ref().map(|a| a.full_path) == Some(true));
        check("disk: contains the #F HELLO.NCP line", contains_subslice(&bytes, b"#F 06 07 C:\\HELLO.NCP@ *.TXT@ 200 @"));
        check("disk: INSIDE.TXT (any dir) resolves", lookup_association(b"INSIDE.TXT").is_some());
        check("disk: HELLO.NCP is not its own document", lookup_association(b"HELLO.NCP").is_none());

        // Removal round-trip.
        check("model: remove HELLO.NCP association", remove_application("C:\\HELLO.NCP"));
        check("model: README.TXT no longer resolves", lookup_association(b"README.TXT").is_none());
    } else {
        check("disk: read NEWDESK.INF back", false);
    }

    kprintln!("[desk-ins] association self-test: {} passed, {} failed", pass, fail);
}

/// Whether `hay` contains the contiguous byte sequence `needle` (self-test aid).
fn contains_subslice(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > hay.len() {
        return needle.is_empty();
    }
    hay.windows(needle.len()).any(|w| w == needle)
}
