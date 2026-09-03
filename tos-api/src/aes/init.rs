//! AES boot-time initialisation — the parts of EmuTOS `aes/geminit.c` that
//! set up AES state an application can observe **before** it ever calls a
//! manager: the AES's own built-in `DESKTOP` object tree and the desktop
//! background byte read from the `.INF` file.
//!
//! Spec: `docs/api/aes/shell-loop.md` §1 (`gem_main`), §2 (`process_inf1`),
//! §11 (the `ap_private` chain).
//!
//! Three EmuTOS steps live here, in EmuTOS's order:
//!
//! 1. **`process_inf1`** (`geminit.c:343-392`, called from `:776`) scans the
//!    first `INF_SIZE` bytes of the `.INF` file for a `#Q` line and stores
//!    the three desktop background bytes in `bg[]`, setting `bgfound`.
//! 2. **`gem_rsc_fixit`** (`gem_rsc.c:636-641`) converts the built-in
//!    resource's char-grid coordinates to pixels once the VDI workstation is
//!    open, and `run_accs_and_desktop` then forces the `DESKTOP` root to the
//!    full screen height (`geminit.c:715-717`).
//! 3. **`set_aes_background`** (`geminit.c:635-641`), called from
//!    `geminit.c:719-728` when `#Q` was found, ORs the plane-appropriate
//!    background byte into the low byte of that root's `ob_spec`.
//!
//! The result is the value every `appl_init` reports as `ap_private`
//! (`gemsuper.c:94-96`) — `0x1143` (= 4419) with no `#Q` line, `0x1100 |
//! patcol` with one. It is **not** derived from any desktop-supplied tree:
//! `appl_init` indexes the AES's own compiled-in `gem.rsc`
//! (`gem_rsc.c:574-578`, index `DESKTOP` = 2 per `gem_rsc.h:48`), and the
//! tree a desktop registers with `wind_set(WF_NEWDESK)` lands in a different
//! variable entirely (`gemwmlib.c:1687`).

use core::cell::UnsafeCell;

use super::object::{Object, TedInfo};
use crate::vdi::font::{CELL_H, CELL_W};

/// `ROOT` — object 0 of any tree (`include/obdefs.h:20`).
const ROOT: usize = 0;
/// `DTNAME` — object 2 of the built-in `DESKTOP` tree, the `G_TEXT` that
/// `sh_draw` fills with the name of the program being launched
/// (`gem_rsc.h:49`, `gemshlib.c:284-286`).
const DTNAME: usize = 2;

/// `G_BOX` / `G_TEXT` graphic types (`obdefs.h:67-68`).
const G_BOX: u16 = 20;
const G_TEXT: u16 = 21;
/// `LASTOB` object flag (`obdefs.h:88`).
const LASTOB: u16 = 0x0020;
/// `IBM` font id / `TE_CNTR` justification (`obdefs.h:62,255`).
const IBM: i16 = 3;
const TE_CNTR: i16 = 2;

/// Number of bytes of the `.INF` file `process_inf1` looks at — EmuTOS
/// `INF_SIZE` (`geminit.c:99`, used at `:769`). Anything past it is invisible
/// to the AES; the desktop writes `#R`/`#Z`/`#E`/`#Q` first, so the `#Q` line
/// is always well inside it.
const INF_SIZE: usize = 300;

/// The `.INF` file the AES reads at boot. EmuTOS uses `EMUDESK.INF`
/// (`INF_FILE_NAME`); NewTOS uses the Atari-classic name, the same file the
/// desktop reads for its own `#Q` copy (`kernel/src/desk/inf.rs:51`).
const INF_PATH: &str = "C:\\NEWDESK.INF";

/// `ob_spec` of the built-in `DESKTOP` tree's ROOT — the literal `4419L` of
/// `gem_rsc.c:551-556`. `4419 == 0x1143`; a real TT running EmuTOS reports
/// exactly this in `appl_init`'s `ap_private`. `set_aes_background` only ever
/// rewrites its low byte, so `0x1100` is a compiled-in constant.
const DESKTOP_ROOT_OBSPEC: u64 = 4419;

/// `ob_spec` of object 1, the background box (`gem_rsc.c:558-563`).
const DESKTOP_BOX_OBSPEC: u64 = 16_716_032;

/// The `TEDINFO` object 2 (`DTNAME`) points at — `rs_tedinfo[12]`
/// (`gem_rsc.c:88-91` in the ROM table: `{0L, "", "", IBM, 1, TE_CNTR, 4352,
/// 0, 1, 2, 1}`). `te_ptext` is NULL until `sh_draw` points it at the command
/// name (`gemshlib.c:284-285`); the two string fields are the empty string.
static DTNAME_TEXT: [u8; 1] = [0];

/// Wrapper giving the built-in `DESKTOP` tree interior mutability. EmuTOS
/// mutates `rs_trees[DESKTOP]` in place (`gem_rsc_fixit`, `set_aes_background`,
/// the `ob_height` fixup); so do we.
struct DesktopTree(UnsafeCell<[Object; 3]>);

// SAFETY: single boot CPU and a cooperative (non-preemptive) trap path — the
// same discipline as every other mutable AES/BIOS singleton in this crate
// (e.g. `xbios::system::KBDVECS`). Writers are the boot-time [`init`] and
// [`set_aes_background`]; readers are `appl_init` and the drawing path.
unsafe impl Sync for DesktopTree {}

/// `rs_trees[DESKTOP]` — the AES's own built-in `DESKTOP` tree, three objects,
/// transcribed from `aes/gem_rsc.c:550-570`. Coordinates are still in the
/// resource's packed char-grid form until [`init`] runs `fix_chpos` over them,
/// exactly as EmuTOS leaves them until `gem_rsc_fixit` (`gem_rsc.c:636-641`).
static DESKTOP: DesktopTree = DesktopTree(UnsafeCell::new([
    // [0] ROOT: { -1, 1, 2, G_BOX, NONE, NORMAL, 4419L, 0,0,80,25 }
    Object {
        ob_next: -1,
        ob_head: 1,
        ob_tail: 2,
        ob_type: G_BOX,
        ob_flags: 0,
        ob_state: 0,
        ob_spec: DESKTOP_ROOT_OBSPEC,
        ob_x: 0,
        ob_y: 0,
        ob_width: 80,
        ob_height: 25,
    },
    // [1]: { 2, -1, -1, G_BOX, NONE, NORMAL, 16716032L, 0,0,80,513 }
    Object {
        ob_next: 2,
        ob_head: -1,
        ob_tail: -1,
        ob_type: G_BOX,
        ob_flags: 0,
        ob_state: 0,
        ob_spec: DESKTOP_BOX_OBSPEC,
        ob_x: 0,
        ob_y: 0,
        ob_width: 80,
        ob_height: 513,
    },
    // [2] DTNAME: { 0, -1, -1, G_TEXT, LASTOB, NORMAL, &rs_tedinfo[12],
    //               0,0,80,769 } — `ob_spec` is patched in `init` (a static
    //               cannot take the address of another static here).
    Object {
        ob_next: 0,
        ob_head: -1,
        ob_tail: -1,
        ob_type: G_TEXT,
        ob_flags: LASTOB,
        ob_state: 0,
        ob_spec: 0,
        ob_x: 0,
        ob_y: 0,
        ob_width: 80,
        ob_height: 769,
    },
]));

/// `infbuf` (`geminit.c:110`) — the first [`INF_SIZE`] bytes of the `.INF`
/// file, read **once** in `gem_main` (`:766-773`) and used by both
/// [`process_inf1_q`] (`:776`) and [`process_inf2`] (`:734`).
struct InfBuf(UnsafeCell<[u8; INF_SIZE + 1]>);
// SAFETY: as [`DesktopTree`] — single boot CPU, cooperative trap path.
unsafe impl Sync for InfBuf {}
static INFBUF: InfBuf = InfBuf(UnsafeCell::new([0; INF_SIZE + 1]));

/// `rs_tedinfo[12]`, the `TEDINFO` `DTNAME` points at (`gem_rsc.c:88-91`).
struct DtNameTed(UnsafeCell<TedInfo>);
// SAFETY: as [`DesktopTree`] — single CPU, cooperative trap path.
unsafe impl Sync for DtNameTed {}

static DTNAME_TED: DtNameTed = DtNameTed(UnsafeCell::new(TedInfo {
    te_ptext: 0, // `0L` — `sh_draw` points it at the command name
    te_ptmplt: 0,
    te_pvalid: 0,
    te_font: IBM,
    te_junk1: 1,
    te_just: TE_CNTR,
    te_color: 4352,
    te_junk2: 0,
    te_thickness: 1,
    te_txtlen: 2,
    te_tmplen: 1,
}));

/// Base pointer of the built-in `DESKTOP` tree — EmuTOS `rs_trees[DESKTOP]`.
#[inline]
pub(crate) fn desktop_tree() -> *mut Object {
    DESKTOP.0.get() as *mut Object
}

/// Point `DTNAME`'s `TEDINFO.te_ptext` at `p` — `sh_draw(lcmd, FALSE)`'s
/// `ted->te_ptext = (char *)lcmd` (`gemshlib.c:284-285`), the program name
/// shown while a program starts.
pub(crate) fn set_dtname_text(p: u64) {
    // SAFETY: single-CPU AES; `DTNAME_TED` is an always-initialised static.
    unsafe { (*DTNAME_TED.0.get()).te_ptext = p };
}

/// `rs_trees[DESKTOP][ROOT].ob_spec` — the word `appl_init` hands back as
/// `ap_private` (`gemsuper.c:95-96`).
pub(crate) fn desktop_root_obspec() -> u64 {
    // SAFETY: the static tree is always initialised and three objects long;
    // single-CPU cooperative access (see [`DesktopTree`]).
    unsafe { (*desktop_tree().add(ROOT)).ob_spec }
}

/// `set_aes_background` (`geminit.c:635-641`) — replace the low byte of the
/// `DESKTOP` root's `ob_spec` with the desktop background pattern/colour
/// byte. The upper 24 bits of the resource constant are preserved verbatim,
/// which is what keeps `ap_private`'s high half at `0x1100`.
pub(crate) fn set_aes_background(patcol: u8) {
    // SAFETY: as [`desktop_root_obspec`].
    unsafe {
        let root = &mut *desktop_tree().add(ROOT);
        root.ob_spec &= 0xffff_ff00;
        root.ob_spec |= patcol as u64;
    }
}

/// `scan_2` (`util/optimize.c:202-217`) — skip blanks, then convert the next
/// two hex digits to a value; a `\r` before them yields `0` and consumes
/// nothing. `0xff` decodes to `-1`. Returns `(value, bytes consumed)`.
fn scan_2(buf: &[u8]) -> (i16, usize) {
    let mut i = 0;
    while i < buf.len() && buf[i] == b' ' {
        i += 1;
    }
    if i >= buf.len() || buf[i] == b'\r' {
        return (0, i);
    }
    // `hex_dig` (`optimize.c:181-187`): `c >= 'A' ? c + 9 : c`, then `& 0x0f`.
    let hex_dig = |c: u8| -> i16 { (if c >= b'A' { c.wrapping_add(9) } else { c } & 0x0f) as i16 };
    let hi = hex_dig(buf[i]);
    i += 1;
    let lo = if i < buf.len() { hex_dig(buf[i]) } else { 0 };
    i += 1;
    let v = (hi << 4) | lo;
    (if v == 0x00ff { -1 } else { v }, i)
}

/// `process_inf1`'s `#Q` arm (`geminit.c:384-388`) — the AES's own read of the
/// desktop background line, `#Q <desktop> <window>` three times, one pair per
/// plane-count bucket. Only the *desktop* byte of each pair is taken: EmuTOS
/// advances `scan_2(...) + 3`, skipping the space and the two window-colour
/// digits. Returns `Some(bg)` when a `#Q` line was present (`bgfound`).
///
/// This is deliberately a **second** reader of the same line — the desktop
/// parses `#Q` again for its own `G.g_patcol` (`desk/deskapp.c:953-960`).
fn process_inf1_q(buf: &[u8]) -> Option<[i16; 3]> {
    let mut found = None;
    let mut i = 0;
    // `for (pcurr = infbuf; *pcurr; )` with the `'#'` + tag lookahead
    // (`geminit.c:356-361`).
    while i + 1 < buf.len() && buf[i] != 0 {
        if buf[i] != b'#' {
            i += 1;
            continue;
        }
        i += 1;
        let tag = buf[i];
        i += 1;
        if tag != b'Q' {
            continue;
        }
        let mut bg = [0i16; 3];
        for slot in bg.iter_mut() {
            let (v, used) = scan_2(&buf[i.min(buf.len())..]);
            *slot = v;
            i = (i + used + 3).min(buf.len());
        }
        found = Some(bg);
    }
    found
}

/// AES boot initialisation. Runs once, before any process can call
/// `appl_init`, and performs the three `geminit.c` steps this module
/// documents. Idempotent in effect except for the background byte, which is
/// re-derived from the `.INF` file each time.
pub fn init() {
    // ── 1. `process_inf1` — read the first INF_SIZE bytes and look for `#Q`.
    //    A missing/unreadable file is EmuTOS's `n < 0 → n = 0` case
    //    (`geminit.c:771-773`): no `#Q`, so `bgfound` stays FALSE. The buffer
    //    is the shared `infbuf` static (`geminit.c:110`), read **once** —
    //    `process_inf2` reads the same bytes later in the boot.
    // SAFETY: single-CPU boot path; exclusive access to the static here.
    let infbuf = unsafe { &mut *INFBUF.0.get() };
    infbuf.fill(0);
    let n = crate::bdos::fs::read_file(INF_PATH, &mut infbuf[..INF_SIZE]).unwrap_or(0);
    infbuf[n] = 0; // `infbuf[n] = '\0'` (geminit.c:773)
    let bg = process_inf1_q(&infbuf[..=n]);

    // ── 2. `gem_rsc_fixit` (`gem_rsc.c:636-641`) + the DTNAME TEDINFO link.
    //    The tree's coordinates are still packed char-grid words; convert all
    //    four of every object, exactly as `rs_obfix` does (`gemrslib.c:89-99`).
    let gl_width = crate::host::host()
        .and_then(|h| h.raster())
        .map(|r| r.width() as i16)
        .unwrap_or(80 * CELL_W);
    // SAFETY: single-CPU boot path; the statics are initialised and in scope
    // for the whole program (see [`DesktopTree`]).
    unsafe {
        let ted = DTNAME_TED.0.get();
        (*ted).te_ptmplt = DTNAME_TEXT.as_ptr() as u64;
        (*ted).te_pvalid = DTNAME_TEXT.as_ptr() as u64;
        let tree = desktop_tree();
        for i in 0..3 {
            let o = &mut *tree.add(i);
            o.ob_x = super::resource::fix_chpos(o.ob_x, CELL_W, false, gl_width);
            o.ob_y = super::resource::fix_chpos(o.ob_y, CELL_H, false, gl_width);
            o.ob_width = super::resource::fix_chpos(o.ob_width, CELL_W, true, gl_width);
            o.ob_height = super::resource::fix_chpos(o.ob_height, CELL_H, false, gl_width);
        }
        (*tree.add(DTNAME)).ob_spec = ted as u64;

        // `tree[ROOT].ob_height = gl_rscreen.g_h` (`geminit.c:716-717`) — the
        // full screen height, overriding the resource's 25 character rows.
        if let Some(h) = crate::host::host().and_then(|h| h.raster()).map(|r| r.height() as i16) {
            (*tree.add(ROOT)).ob_height = h;
        }
    }

    // ── 3. `set_aes_background` (`geminit.c:719-728`), only if `#Q` was found.
    //    `n = (gl_nplanes > 2) ? 2 : gl_nplanes - 1` picks the pair for this
    //    screen depth; `gl_nplanes` is the VDI workstation's plane count.
    if let Some(bg) = bg {
        let planes = crate::host::host()
            .and_then(|h| h.raster())
            .map(|r| r.planes() as i16)
            .unwrap_or(4);
        let idx = if planes > 2 { 2 } else { (planes - 1).max(0) } as usize;
        set_aes_background((bg[idx] & 0xff) as u8);
    }
}

/// `process_inf2` (`geminit.c:407-464`) — the **second** `.INF` pass, run
/// after the AES's own init and before the shell loop (`geminit.c:734`,
/// inside `run_accs_and_desktop`). It reads two lines of the buffer
/// [`init`] filled:
///
/// * **`#E`** (`:421-436`) — `ev_dclick(env & 0x07, TRUE)` sets the
///   double-click speed from the low three bits of the first byte
///   ([`super::event::ev_dclick`]). EmuTOS then reads further bytes on the
///   same line for the blitter (`Blitmode`, `#if CONF_WITH_BLITTER`) and the
///   68030 cache (`set_cache`, `#if CONF_WITH_CACHE_CONTROL`). Both are
///   **m68k hardware** with no BCM2711 equivalent — the hardware-layer
///   carve-out of `docs/COMPATIBILITY.md`. The bytes are still consumed so
///   the scan cursor advances exactly as EmuTOS's does; only the two
///   hardware actions are absent.
/// * **`#Z`** (`:438-460`), format `#Z <isgem-byte> <path>@` — the autorun
///   entry. `scan_2` reads the flag byte (`00` = character mode, else GEM),
///   the path up to `@` is split by `sh_name` into a directory and a bare
///   program name, and the pair goes to [`super::shell::sh_wdef`] with
///   `isauto = TRUE`.
///
/// Returns `(isauto, isgem)`; `isgem` is EmuTOS's return value
/// `(*isauto && !isgem) ? FALSE : TRUE` (`:463`), i.e. character mode only
/// when an autorun entry exists *and* its flag byte is `00`. Both are passed
/// straight into `sh_main(isauto, isgem)` (`geminit.c:739`).
///
/// **Only the first [`INF_SIZE`] (300) bytes are visible**, because that is
/// all `gem_main` reads (`geminit.c:769`). EmuTOS states the resulting
/// constraint on the file itself: "the `#Z` and `#E` lines must be completely
/// contained within the first INF_SIZE (currently 300) bytes of EMUDESK.INF"
/// (`desk/deskapp.c:161-162`), which is why the desktop's writer emits `#Z`
/// between `#R` and `#E` (`deskapp.c:1128-1133`). A `#Z` line placed after the
/// `#W`/ANODE bulk is invisible to the AES.
///
/// **`BOOTFLAG_SKIP_AUTO_ACC` (`geminit.c:452`) is not reproduced**: NewTOS
/// has no boot-time modifier-key scan (EmuTOS sets the flag when Control is
/// held on the initinfo screen, `bios/bios.c:819-820`), so there is no such
/// flag to test. Nothing is faked — the guard is simply absent, and the
/// autorun entry is always honoured.
pub fn process_inf2() -> (bool, bool) {
    // SAFETY: single-CPU boot path; the static was filled by `init`.
    let buf = unsafe { &*INFBUF.0.get() };
    let mut isauto = false;
    let mut isgem: i16 = 1; // `WORD env, isgem = TRUE;` (:409)

    let mut i = 0usize;
    // `for (pcurr = infbuf; *pcurr; )` with the `'#'` + tag lookahead (:415-419)
    while i < buf.len() && buf[i] != 0 {
        if buf[i] != b'#' {
            i += 1;
            continue;
        }
        i += 1;
        if i >= buf.len() {
            break;
        }
        let tag = buf[i];
        if tag == b'E' {
            // `pcurr += 2` — skip the tag and the following blank (:423).
            i += 2;
            let (env, used) = scan_2(&buf[i.min(buf.len())..]);
            i = (i + used).min(buf.len());
            super::event::ev_dclick(env & 0x07, true); // (:425)
            // The blitter byte (:426) and, with CONF_WITH_CACHE_CONTROL, the
            // two video bytes plus the cache byte (:431-434). Consumed to
            // keep the cursor in step; no RPi4 equivalent to act on.
            for _ in 0..4 {
                let (_v, used) = scan_2(&buf[i.min(buf.len())..]);
                i = (i + used).min(buf.len());
            }
        } else if tag == b'Z' {
            // `pcurr += 2; scan_2(pcurr, &isgem); pcurr += 3;` (:441-444) —
            // note `scan_2`'s return is discarded here, the cursor advances
            // by a fixed three bytes (two hex digits + one blank).
            i += 2;
            let (v, _used) = scan_2(&buf[i.min(buf.len())..]);
            isgem = v;
            i = (i + 3).min(buf.len());
            // `while (*pcurr && *pcurr != '@') ++pcurr; *pcurr = 0;` (:445-447)
            let start = i;
            while i < buf.len() && buf[i] != 0 && buf[i] != b'@' {
                i += 1;
            }
            let path = &buf[start..i];
            // `tmpptr2 = sh_name(tmpptr1); *(tmpptr2-1) = 0;` (:448-449) —
            // split the one string into (directory, program) by writing a NUL
            // over the byte before the filename. When the filename starts at
            // offset 0 there is no such byte inside the path, so the
            // directory is empty.
            let off = path.len() - super::shell::sh_name(path).len();
            let dir: &[u8] = if off > 0 { &path[..off - 1] } else { &[] };
            let prog = &path[off..];
            super::shell::sh_wdef(prog, dir); // (:455)
            isauto = true; // (:456)
            i += 1; // `++pcurr;` (:458)
        }
    }

    // `return (*isauto && !isgem) ? FALSE : TRUE;` (:463)
    (isauto, !(isauto && isgem == 0))
}
