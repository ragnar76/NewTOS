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
use tos_api::bdos::fs::file::{fclose, fcreate, fopen, fread, fwrite};
use tos_api::trap::TrapArgs;
use tos_api::vdi::font::{CELL_H, CELL_W};

use crate::sync::SpinMutex;

use super::global::{self, Grect, DROOT, NIL};
use super::model::{AType, Sort, View};

// ── GEMDOS file access ──────────────────────────────────────────────────────

const TRAP_GEMDOS: u16 = 1;
const FCREATE: u16 = 0x3C;
const FOPEN: u16 = 0x3D;
const FCLOSE: u16 = 0x3E;
const FREAD: u16 = 0x3F;
const FWRITE: u16 = 0x40;

/// The desktop configuration file, on the boot drive's root (EmuTOS keeps its
/// own `EMUDESK.INF` here; NewTOS uses the Atari-classic `NEWDESK.INF` name the
/// user's SD card carries).
const INF_PATH: &str = "C:\\NEWDESK.INF";

/// Upper bound on the file we will read (a full 64-`ANODE` file with strings is
/// a few KiB; this guards a runaway read).
const MAX_INF_BYTES: usize = 64 * 1024;

/// Build a native GEMDOS [`TrapArgs`].
fn gargs(func: u16, regs: [u64; 8]) -> TrapArgs {
    TrapArgs { regs, func: func as u64, svc_imm: TRAP_GEMDOS }
}

/// A NUL-terminated path buffer built from `s`.
fn cpath(s: &str) -> [u8; 130] {
    let mut b = [0u8; 130];
    let src = s.as_bytes();
    let n = src.len().min(b.len() - 1);
    b[..n].copy_from_slice(&src[..n]);
    b
}

/// Read the whole `NEWDESK.INF` into a heap buffer, or [`None`] if it cannot be
/// opened (a missing file is **not** an error — the caller keeps defaults).
fn read_inf_file() -> Option<Vec<u8>> {
    let p = cpath(INF_PATH);
    let rc = fopen(&gargs(FOPEN, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]));
    if rc < 0 {
        return None;
    }
    let handle = rc as i16;
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let n = fread(&gargs(FREAD, [handle as u64, chunk.len() as u64, chunk.as_mut_ptr() as u64, 0, 0, 0, 0, 0]));
        if n <= 0 {
            break;
        }
        let got = n as usize;
        buf.extend_from_slice(&chunk[..got.min(chunk.len())]);
        if got < chunk.len() || buf.len() >= MAX_INF_BYTES {
            break;
        }
    }
    fclose(&gargs(FCLOSE, [handle as u64, 0, 0, 0, 0, 0, 0, 0]));
    Some(buf)
}

/// Create/truncate `NEWDESK.INF` and write `bytes`. Returns whether the full
/// buffer was written.
fn write_inf_file(bytes: &[u8]) -> bool {
    let p = cpath(INF_PATH);
    let rc = fcreate(&gargs(FCREATE, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]));
    if rc < 0 {
        return false;
    }
    let handle = rc as i16;
    let mut off = 0usize;
    let mut ok = true;
    while off < bytes.len() {
        let n = fwrite(&gargs(FWRITE, [handle as u64, (bytes.len() - off) as u64, bytes[off..].as_ptr() as u64, 0, 0, 0, 0, 0]));
        if n <= 0 {
            ok = false;
            break;
        }
        off += n as usize;
    }
    fclose(&gargs(FCLOSE, [handle as u64, 0, 0, 0, 0, 0, 0, 0]));
    ok
}

// ── `#E` environment flag bits (`deskinf.md` appendix, `deskapp.c`) ──────────

const INF_E1_VIEWTEXT: u8 = 0x80;
const INF_E1_SORTMASK: u8 = 0x60;
const INF_E1_SORTSHIFT: u8 = 5;
const INF_E5_NOSORT: u8 = 0x80;
const INF_E5_NOSIZE: u8 = 0x10;

/// `INF_E1_DEFAULT` = `CONFDEL|CONFCPY|dclick(2)` (`deskapp.c` `desk_inf_data1`).
const INF_E1_DEFAULT: u8 = 0x1A;
/// `INF_E2_DEFAULT` = `BLITTER|IDTDATE|IDTTIME`.
const INF_E2_DEFAULT: u8 = 0xE0;
/// `INF_E5_DEFAULT` = `APPDIR|ISFULL` (size-to-fit enabled, sorting on).
const INF_E5_DEFAULT: u8 = 0x60;

/// Number of `#K` menu-shortcut bytes to skip on read (`NUM_SHORTCUTS`,
/// `deskmain.h:18`). NewTOS's shortcuts are fixed ([`super::menu`]), so the
/// `#K` line is parsed-and-ignored and never written (as if
/// `CONF_WITH_DESKTOP_CONFIG` were off).
const NUM_SHORTCUTS: usize = 33;

// ── `ANODE`-tag `a_type` codes (`deskapp.h:43-49`), shared with `app_blddesk`
//    for the desktop-icon-position lookup. ─────────────────────────────────
/// `AT_ISDISK` — disk/media icon (`#M`).
pub(super) const AT_ISDISK: u8 = 2;
/// `AT_ISTRSH` — trash icon (`#T`).
pub(super) const AT_ISTRSH: u8 = 3;
/// `AT_ISPRNT` — printer icon (`#O`).
pub(super) const AT_ISPRNT: u8 = 4;

// ── Installed-application extra-block bits + icon indices ────────────────────
/// `INF_AT_APPDIR` (`deskapp.c:126`) — 1 ⇒ set the current directory to the
/// application's directory (else to the top window's).
const INF_AT_APPDIR: u8 = 0x01;
/// `INF_AT_ISFULL` (`deskapp.c:127`) — 1 ⇒ pass the document's full path in the
/// command tail (else just the file name).
const INF_AT_ISFULL: u8 = 0x02;
/// `IG_APPL` (`deskapp.h:60`) — the generic application icon index.
const IG_APPL: i16 = 6;
/// `IG_DOCU` (`deskapp.h:61`) — the generic document icon index.
const IG_DOCU: i16 = 7;

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
            b'K' => {
                for _ in 0..NUM_SHORTCUTS {
                    let _ = scan_2(&mut c);
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
    let data = parse_inf(&bytes);
    apply(&data);
    *SESSION.lock() = Some(data);
    true
}

/// Apply a parsed [`InfData`] to the live desktop state.
fn apply(data: &InfData) {
    // #E → view / sort / size-to-fit.
    let (view, sort, size_to_fit) = env_to_state(data.e1, data.e5);

    // #M/#T/#O → cached desktop-icon grid positions (grid units on disk).
    let mut desk_icons: Vec<(u8, u8, u8, u8)> = Vec::new();
    for a in &data.anodes {
        let code = match a.tag {
            b'M' => AT_ISDISK,
            b'T' => AT_ISTRSH,
            b'O' => AT_ISPRNT,
            _ => continue,
        };
        desk_icons.push((code, a.letter, a.xspot, a.yspot));
    }

    // #W → cached window rectangles (character cells → pixels).
    let windows: Vec<Grect> = data
        .windows
        .iter()
        .map(|w| Grect {
            x: w.xc as i16 * CELL_W,
            y: w.yc as i16 * CELL_H,
            w: w.wc as i16 * CELL_W,
            h: w.hc as i16 * CELL_H,
        })
        .collect();

    global::with(|g| {
        g.view = view;
        g.sort = sort;
        g.size_to_fit = size_to_fit;
        g.inf_desk_icons = desk_icons;
        g.inf_windows = windows;
    });

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

/// The pixel rectangle to open the window in slot `slot` at, from a saved `#W`
/// line, or [`None`] when there is no (usable) saved geometry for that slot.
pub(super) fn window_rect(slot: usize) -> Option<Grect> {
    global::with(|g| g.inf_windows.get(slot).copied())
        .flatten()
        .filter(|r| r.w > 0 && r.h > 0)
}

// ── Save (`cnx_put` + `app_save(TRUE)`) ─────────────────────────────────────

/// Build an [`InfData`] snapshot of the current desktop state: env from the
/// live view/sort/size-to-fit (other prefs preserved from [`SESSION`]),
/// `g_patcol`, the live desktop-icon positions read back from the object tree,
/// the currently-open windows' geometry, and the preserved file-type `ANODE`s.
fn build_from_state() -> InfData {
    let session = SESSION.lock().clone();

    // Env + desktop-icon anodes read from the live object tree, under one lock.
    let (view, sort, size_to_fit, desk_anodes) = global::with(|g| {
        let mut anodes = Vec::new();
        let icw = g.g_icw.max(1);
        let ich = g.g_ich.max(1);
        let mut child = g.g_screen[DROOT as usize].ob_head;
        while child != NIL && child != DROOT {
            let si = &g.g_screeninfo[child as usize];
            let tag = match si.atype {
                AType::Disk => Some(b'M'),
                AType::Trash => Some(b'T'),
                AType::Printer => Some(b'O'),
                _ => None,
            };
            if let Some(tag) = tag {
                let letter = (si.block.ib_char & 0xFF) as u8;
                let letter = if letter.is_ascii_uppercase() { letter } else { 0 };
                let xs = (g.g_screen[child as usize].ob_x / icw).clamp(0, 255) as u8;
                let ys = (g.g_screen[child as usize].ob_y / ich).clamp(0, 255) as u8;
                anodes.push(InfAnode {
                    tag,
                    is_desk: true,
                    has_extra: false,
                    xspot: xs,
                    yspot: ys,
                    aicon: -1,
                    dicon: -1,
                    letter,
                    pappl: label_of(&si.label),
                    pdata: String::new(),
                    flags: 0,
                    funkey: 0,
                    pargs: String::new(),
                });
            }
            child = g.g_screen[child as usize].ob_next;
        }
        (g.view, g.sort, g.size_to_fit, anodes)
    })
    .unwrap_or((View::Icon, Sort::Name, true, Vec::new()));

    // Base env / patcol / file-type anodes: preserved from the session, or the
    // built-in defaults on a first-ever save.
    let base = session.unwrap_or_default();
    let (e1, e5) = state_to_env(view, sort, size_to_fit, base.e1, base.e5);

    let mut anodes = desk_anodes;
    let type_anodes: Vec<InfAnode> = if base.anodes.iter().any(|a| !a.is_desk) {
        base.anodes.iter().filter(|a| !a.is_desk).cloned().collect()
    } else {
        InfData::default_type_anodes()
    };
    anodes.extend(type_anodes);

    InfData {
        e1,
        e2: base.e2,
        e5,
        patcol: {
            let p = global::patcol_all();
            [(p[0], p[1]), (p[2], p[3]), (p[4], p[5])]
        },
        autorun: base.autorun,
        autorun_gem: base.autorun_gem,
        windows: collect_open_windows(),
        anodes,
    }
}

/// The label text of a `ScreenInfo::label` buffer as an owned string.
fn label_of(label: &[u8]) -> String {
    let end = label.iter().position(|&b| b == 0).unwrap_or(label.len());
    String::from_utf8_lossy(&label[..end]).into_owned()
}

/// Snapshot the currently-open windows' frame geometry into `#W` lines
/// (character-cell units). Slots are gathered under the desktop lock; the AES
/// geometry query runs with the lock released.
fn collect_open_windows() -> Vec<InfWindow> {
    // (slot, handle, title) for every open window, in slot order.
    let mut open: Vec<(usize, i16, String)> = Vec::new();
    global::with(|g| {
        for (slot, w) in g.windows.slots.iter().enumerate() {
            if w.w_id != super::win::DESKWH {
                open.push((slot, w.w_id, w.w_pnode.p_spec.clone()));
            }
        }
    });

    let mut out = Vec::new();
    for (_slot, handle, path) in open {
        let r = super::wind_get_grect(handle, super::WF_CXYWH);
        out.push(InfWindow {
            hsl: 0,
            vsl: 0,
            xc: (r.x / CELL_W).clamp(0, 255) as u8,
            yc: (r.y / CELL_H).clamp(0, 255) as u8,
            wc: (r.w / CELL_W).clamp(0, 255) as u8,
            hc: (r.h / CELL_H).clamp(0, 255) as u8,
            path,
        });
    }
    out
}

/// Options → Save desktop: serialise the current state and write
/// `NEWDESK.INF`. Returns whether the write succeeded.
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

// ── Show-Info dialog (`inf_file_folder`, `deskinf.c:493-639`) ───────────────

// obdefs.h object graphic types.
const G_BOX: u16 = 20;
const G_BOXTEXT: u16 = 22;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;

// obdefs.h ob_flags bits.
const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const EDITABLE: u16 = 0x0008;
const LASTOB: u16 = 0x0020;
// obdefs.h ob_state bit.
const OB_SELECTED: u16 = 0x0001;

/// Opaque dialog box colour word (black border + solid-white interior), as in
/// [`super::dlg`] and the menu drop-downs.
const BOX_SPEC: u64 = (1 << 16) | 0x1070;

/// The longest edited name (a 12-char 8.3 name plus one slack byte).
const NAME_MAX: usize = 13;

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

/// The current screen dimensions.
fn screen_dims() -> (i16, i16) {
    let r = super::screen_rect();
    (r.w, r.h)
}

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

/// FAT attribute bits (`bdosdefs.h`).
const FA_RO: u8 = 0x01;
const FA_HIDDEN: u8 = 0x02;
const FA_SYSTEM: u8 = 0x04;
const FA_SUBDIR: u8 = 0x10;
const FA_ARCHIVE: u8 = 0x20;

/// The Show-Info / rename dialog for a single selected item (`inf_file_folder`,
/// `deskinf.c:493-639`): show size / date / time / attributes and offer an
/// editable name. Returns the new name on OK when it changed (an empty name is
/// rejected), otherwise [`None`]. The caller repaints the screen underneath.
///
/// `size`/`date`/`time`/`attr` are the selected [`super::model::FNode`]'s own
/// fields (DOS-packed date/time, FAT attribute byte). The date/time text layout
/// is a display choice, not TOS contract (`deskinf.md`: "no program reads this").
pub(super) fn inf_file_folder(name: &str, size: u32, date: u16, time: u16, attr: u8) -> Option<String> {
    let (sw, sh) = screen_dims();
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
        te_color: 1 << 8,
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

    // Date/time line: "Date: MM/DD/YYYY  HH:MM".
    let mut datebuf = [0u8; 40];
    {
        let year = 1980 + (date >> 9) as u32;
        let month = ((date >> 5) & 0x0F) as u32;
        let day = (date & 0x1F) as u32;
        let hour = (time >> 11) as u32;
        let min = ((time >> 5) & 0x3F) as u32;
        let mut p = 0usize;
        push_str(&mut datebuf, &mut p, b"Date: ");
        push_dec2(&mut datebuf, &mut p, month);
        push_str(&mut datebuf, &mut p, b"/");
        push_dec2(&mut datebuf, &mut p, day);
        push_str(&mut datebuf, &mut p, b"/");
        push_dec(&mut datebuf, &mut p, year as u64);
        push_str(&mut datebuf, &mut p, b"  ");
        push_dec2(&mut datebuf, &mut p, hour);
        push_str(&mut datebuf, &mut p, b":");
        push_dec2(&mut datebuf, &mut p, min);
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

    let (dw, dh) = (320i16, 200i16);
    let bx = ((sw - dw) / 2).max(0);
    let by = ((sh - dh) / 2).max(0);
    let ob_ok = 7i16;

    let tree = [
        mk(G_BOX, 0, BOX_SPEC, bx, by, dw, dh, -1, 1, 8),
        mk(G_STRING, 0, TITLE.as_ptr() as u64, 24, 14, dw - 48, 16, 2, -1, -1),
        mk(G_STRING, 0, NAME_LBL.as_ptr() as u64, 24, 44, 48, 16, 3, -1, -1),
        mk(G_BOXTEXT, EDITABLE, &ted as *const _ as u64, 80, 40, 200, 22, 4, -1, -1),
        mk(G_STRING, 0, sizebuf.as_ptr() as u64, 24, 74, dw - 48, 16, 5, -1, -1),
        mk(G_STRING, 0, datebuf.as_ptr() as u64, 24, 96, dw - 48, 16, 6, -1, -1),
        mk(G_STRING, 0, attrbuf.as_ptr() as u64, 24, 118, dw - 48, 16, 7, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT, OK.as_ptr() as u64, 40, 156, 90, 28, 8, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB, CANCEL.as_ptr() as u64, 190, 156, 90, 28, 0, -1, -1),
    ];

    let clip = Grect { x: 0, y: 0, w: sw, h: sh };
    super::objc_draw_dialog(tree.as_ptr() as u64, clip);
    let clicked = super::form_do(tree.as_ptr() as u64);
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

// ── Set-background dialog (`inf_backgrounds`, `deskinf.c:825-921`) ───────────

/// Options → Set background (`inf_backgrounds`): step the desktop background
/// pattern (0-7) and colour (0-15); on OK apply to `g_patcol`, repaint the
/// desktop, and persist to `NEWDESK.INF`. Returns whether the background
/// changed. The caller repaints the screen the dialog covered.
pub(super) fn set_background() -> bool {
    let (sw, sh) = screen_dims();
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

    let (dw, dh) = (300i16, 180i16);
    let bx = ((sw - dw) / 2).max(0);
    let by = ((sh - dh) / 2).max(0);

    // Object indices.
    const PAT_DN: i16 = 4;
    const PAT_UP: i16 = 5;
    const COL_DN: i16 = 8;
    const COL_UP: i16 = 9;
    const OK_OBJ: i16 = 10;

    let mut tree = [
        mk(G_BOX, 0, BOX_SPEC, bx, by, dw, dh, -1, 1, 11),
        mk(G_STRING, 0, TITLE.as_ptr() as u64, 24, 14, dw - 48, 16, 2, -1, -1),
        mk(G_STRING, 0, PAT_LBL.as_ptr() as u64, 24, 48, 72, 16, 3, -1, -1),
        mk(G_STRING, 0, patnum.as_ptr() as u64, 104, 48, 40, 16, 4, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, MINUS.as_ptr() as u64, 160, 44, 44, 24, 5, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, PLUS.as_ptr() as u64, 214, 44, 44, 24, 6, -1, -1),
        mk(G_STRING, 0, COL_LBL.as_ptr() as u64, 24, 84, 72, 16, 7, -1, -1),
        mk(G_STRING, 0, colnum.as_ptr() as u64, 104, 84, 40, 16, 8, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, MINUS.as_ptr() as u64, 160, 80, 44, 24, 9, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, PLUS.as_ptr() as u64, 214, 80, 44, 24, 10, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT, OK.as_ptr() as u64, 30, 138, 90, 28, 11, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB, CANCEL.as_ptr() as u64, 178, 138, 90, 28, 0, -1, -1),
    ];
    let clip = Grect { x: 0, y: 0, w: sw, h: sh };

    let committed = loop {
        // Refresh the two number fields, then repaint + run the modal loop once.
        let mut p = 0usize;
        push_dec(&mut patnum, &mut p, pat as u64);
        patnum[p.min(patnum.len() - 1)] = 0;
        let mut q = 0usize;
        push_dec(&mut colnum, &mut q, col as u64);
        colnum[q.min(colnum.len() - 1)] = 0;

        super::objc_draw_dialog(tree.as_ptr() as u64, clip);
        let clicked = super::form_do(tree.as_ptr() as u64);
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
    };

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
    save_desktop();
    true
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
    original.e1 = e1;
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
