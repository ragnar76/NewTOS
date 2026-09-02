//! Reproduction rig for the **desktop-background over-paint**: the AES repaints
//! `w_drawdesk` over the whole damage rectangle instead of over the desktop
//! window's owner rectangle list, so it paints the background *inside* windows
//! it must never touch.
//!
//! What EmuTOS does. `w_drawdesk(pc)` is one call to
//! `do_walk(DESKWH, tree, root, depth, pc)` (`aes/gemwmlib.c:340-359`), and
//! `do_walk` (`:286-312`) is:
//!
//! ```c
//! if (pc) rc_intersect(&gl_rfull, pc); else pc = &gl_rfull;
//! for (po = D.w_win[wh].w_rlist; po; po = po->o_link) {
//!     rc_copy(&po->o_gr, &t);
//!     if (rc_intersect(pc, &t)) { gsx_sclip(&t); ob_draw(tree, obj, depth); }
//! }
//! ```
//!
//! Two clips, not one:
//!
//! * `gl_rfull` — the screen **minus the menu row** (`aes/gemgraf.c:423-424`,
//!   set unconditionally at `gsx_start`, menu bar installed or not). The AES
//!   never repaints the menu strip from `w_drawdesk`; removing a menu bar does
//!   not repaint it either (`mn_bar`, `aes/gemmnlib.c:724-728`).
//! * `D.w_win[DESKWH].w_rlist` — the desktop window's **owner rectangle list**:
//!   its own rectangle with every window above it subtracted, rebuilt by
//!   `newrect`/`mkrect`/`brkrct` (`aes/gemwrect.c:64-205`) at the top of every
//!   `draw_change` (`aes/gemwmlib.c:1129`) *before* anything is drawn. A window
//!   that is open owns its rectangle; the desktop does not, so `w_drawdesk`
//!   cannot paint into it. What the window shows is restored by its owner in
//!   answer to the `WM_REDRAW` that `w_update`/`w_redraw` post
//!   (`:1004-1048,887-907`) — the AES never paints a window's work area.
//!
//! The five entries into the background repaint are exercised here, one test
//! each: `wind_open`, `wind_close`, `wind_set(WF_CURRXYWH)`, the mover drag and
//! `form_dial(FMD_FINISH)`. Every one asserts the same two things — *nothing*
//! changed inside a covering window's work area, and the bare desktop inside
//! the damage rectangle *was* repainted (so a fix cannot be "paint less").
//!
//! Harness shape (backend + host + parameter block) copied from
//! `aes_window_content.rs`, which copied it from `aes_window_drag.rs`.

#![allow(clippy::too_many_arguments)]


use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tos_api::bdos::process::Abi;
use tos_api::host::Host;
use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};

const SCREEN_W: u16 = 640;
const SCREEN_H: u16 = 480;

// ── a raster backend over a real pixel buffer, with an op log ──────────────

struct BufBackend {
    px: Mutex<Vec<u32>>,
    palette: Mutex<[u16; 256]>,
    req: Mutex<[(u16, u16, u16); 256]>,
    log: Mutex<Vec<String>>,
    logging: AtomicBool,
}

impl BufBackend {
    fn new() -> Self {
        BufBackend {
            px: Mutex::new(vec![0; SCREEN_W as usize * SCREEN_H as usize]),
            palette: Mutex::new([0; 256]),
            req: Mutex::new([(0, 0, 0); 256]),
            log: Mutex::new(Vec::new()),
            logging: AtomicBool::new(false),
        }
    }

    fn note(&self, s: String) {
        if self.logging.load(Ordering::Relaxed) {
            self.log.lock().unwrap().push(s);
        }
    }

    fn inside(x: i16, y: i16) -> bool {
        x >= 0 && y >= 0 && (x as i32) < SCREEN_W as i32 && (y as i32) < SCREEN_H as i32
    }

    fn idx(x: i16, y: i16) -> usize {
        y as usize * SCREEN_W as usize + x as usize
    }

    fn put(&self, x: i16, y: i16, v: u32, clip: ClipRect) {
        if !Self::inside(x, y) {
            return;
        }
        if let Some((x0, y0, x1, y1)) = clip.clamp_to(SCREEN_W, SCREEN_H) {
            let (xi, yi) = (x as i32, y as i32);
            if xi < x0 || xi > x1 || yi < y0 || yi > y1 {
                return;
            }
        }
        self.px.lock().unwrap()[Self::idx(x, y)] = v;
    }

    fn snapshot(&self) -> Vec<u32> {
        self.px.lock().unwrap().clone()
    }
}

impl RasterBackend for BufBackend {
    fn width(&self) -> u16 {
        SCREEN_W
    }
    fn height(&self) -> u16 {
        SCREEN_H
    }
    fn num_colors(&self) -> u16 {
        256
    }
    fn planes(&self) -> u16 {
        16
    }
    fn set_color(&self, index: u16, r: u16, g: u16, b: u16) {
        if (index as usize) < 256 {
            self.req.lock().unwrap()[index as usize] = (r, g, b);
        }
    }
    fn query_color(&self, index: u16) -> (u16, u16, u16) {
        if (index as usize) < 256 {
            self.req.lock().unwrap()[index as usize]
        } else {
            (0, 0, 0)
        }
    }
    fn set_active_palette(&self, palette: &[u16; 256]) {
        *self.palette.lock().unwrap() = *palette;
    }
    fn pel_to_device16(&self, pel: Pel) -> u16 {
        self.palette.lock().unwrap()[pel as usize]
    }
    fn device_to_rgb565(&self, argb: u32) -> u16 {
        let (r, g, b) = ((argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF);
        (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16
    }
    fn rgb565_to_device(&self, px: u16) -> u32 {
        let (r5, g6, b5) = ((px >> 11) & 0x1F, (px >> 5) & 0x3F, px & 0x1F);
        let r = ((r5 << 3) | (r5 >> 2)) as u32;
        let g = ((g6 << 2) | (g6 >> 4)) as u32;
        let b = ((b5 << 3) | (b5 >> 2)) as u32;
        (r << 16) | (g << 8) | b
    }
    fn read_pixel(&self, x: i16, y: i16) -> Pel {
        if !Self::inside(x, y) {
            return 0;
        }
        let v = self.px.lock().unwrap()[Self::idx(x, y)];
        let want = self.device_to_rgb565(v);
        let pal = *self.palette.lock().unwrap();
        pal.iter().position(|&p| p == want).unwrap_or(0) as Pel
    }
    fn fill_rect(&self, x: i16, y: i16, w: i16, h: i16, pel: Pel, clip: ClipRect) {
        self.note(format!("fill_rect x={x} y={y} w={w} h={h} pel={pel} clip={:?}", (clip.xmn, clip.ymn, clip.xmx, clip.ymx)));
        let v = self.rgb565_to_device(self.pel_to_device16(pel));
        for yy in y..y.saturating_add(h) {
            for xx in x..x.saturating_add(w) {
                self.put(xx, yy, v, clip);
            }
        }
    }
    /// Honours the VDI write mode, like the board's backend
    /// (`kernel/src/vdi_backend.rs` `abline`/`combine`/`xor_pixel`): XOR
    /// feedback (the AES rubber band) only restores the screen if the second
    /// draw really complements what the first one wrote.
    fn abline(
        &self,
        x0: i16,
        y0: i16,
        x1: i16,
        y1: i16,
        pel: Pel,
        mode: WriteMode,
        clip: ClipRect,
        mask: u16,
        last: bool,
    ) -> u16 {
        let fg = self.rgb565_to_device(self.pel_to_device16(pel));
        let bg = self.rgb565_to_device(self.pel_to_device16(0));
        // Always draw left→right (EmuTOS `abline:2300`).
        let (ax, ay, mut bx, mut by) = if x1 < x0 { (x1, y1, x0, y0) } else { (x0, y0, x1, y1) };
        // XOR `LSTLIN` kludge (`abline:2319`): shorten all but the last segment
        // so a shared vertex is not complemented twice.
        if mode == WriteMode::Xor && !last {
            if ax != bx {
                bx -= 1;
            } else if ay != by {
                by -= 1;
            }
        }
        let combine = |x: i16, y: i16, set: bool| match mode {
            WriteMode::Replace => self.put(x, y, if set { fg } else { bg }, clip),
            WriteMode::Transparent => {
                if set {
                    self.put(x, y, fg, clip)
                }
            }
            WriteMode::Xor => {
                if set {
                    let v = self.get_device_pixel(x, y);
                    self.put(x, y, v ^ 0x00FF_FFFF, clip);
                }
            }
            WriteMode::RevTransparent => {
                if !set {
                    self.put(x, y, fg, clip)
                }
            }
        };
        // Horizontal: screen-aligned dash, `LN_MASK` not advanced.
        if ay == by {
            let (xlo, xhi) = (ax.min(bx), ax.max(bx));
            for x in xlo..=xhi {
                combine(x, ay, mask & (0x8000u16 >> (x & 0xf)) != 0);
            }
            return mask;
        }
        let (mut x, mut y) = (ax as i32, ay as i32);
        let (xe, ye) = (bx as i32, by as i32);
        let (dx, dy) = ((xe - x).abs(), -(ye - y).abs());
        let sy = if y < ye { 1 } else { -1 };
        let mut err = dx + dy;
        let mut m = mask;
        loop {
            combine(x as i16, y as i16, m & 0x8000 != 0);
            m = m.rotate_left(1);
            if x == xe && y == ye {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += 1;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
        m
    }
    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill, clip: ClipRect) {
        self.note(format!("fill_pattern x={x} y={y} w={w} h={h} fg={} bg={}", fill.fg, fill.bg));
        let fg = self.rgb565_to_device(self.pel_to_device16(fill.fg));
        let bg = self.rgb565_to_device(self.pel_to_device16(fill.bg));
        for yy in y..y.saturating_add(h) {
            let row = fill.pattern[(yy.rem_euclid(16)) as usize % fill.pattern.len()];
            for xx in x..x.saturating_add(w) {
                let bit = row & (0x8000u16 >> (xx.rem_euclid(16) as u16));
                self.put(xx, yy, if bit != 0 { fg } else { bg }, clip);
            }
        }
    }
    fn draw_glyph_sized(
        &self,
        x: i16,
        y: i16,
        glyph: &[u8],
        cell_w: i16,
        cell_h: i16,
        fg: Pel,
        _mode: WriteMode,
        clip: ClipRect,
    ) {
        let v = self.rgb565_to_device(self.pel_to_device16(fg));
        for row in 0..cell_h {
            let byte = glyph.get(row as usize).copied().unwrap_or(0);
            for col in 0..cell_w.min(8) {
                if byte & (0x80 >> col) != 0 {
                    self.put(x + col, y + row, v, clip);
                }
            }
        }
    }
    fn blit_mono(&self, dst: Rect, src: MonoSrc, fg: Pel, bg: Pel, _m: WriteMode, clip: ClipRect) {
        let f = self.rgb565_to_device(self.pel_to_device16(fg));
        let b = self.rgb565_to_device(self.pel_to_device16(bg));
        for row in 0..dst.h {
            for col in 0..dst.w {
                let (sx, sy) = (src.x + col, src.y + row);
                let wi = (sy as usize) * src.stride_words as usize + (sx as usize) / 16;
                let w = src.bits.get(wi).copied().unwrap_or(0);
                let bit = w & (0x8000u16 >> (sx.rem_euclid(16)) as u16);
                self.put(dst.x + col, dst.y + row, if bit != 0 { f } else { b }, clip);
            }
        }
    }
    fn copy_rect(&self, src: Rect, dx: i16, dy: i16, _op: u8, clip: ClipRect) {
        self.note(format!("copy_rect src=({},{},{},{}) -> ({dx},{dy})", src.x, src.y, src.w, src.h));
        let mut tmp = Vec::with_capacity((src.w as usize) * (src.h as usize));
        {
            let px = self.px.lock().unwrap();
            for row in 0..src.h {
                for col in 0..src.w {
                    let (sx, sy) = (src.x + col, src.y + row);
                    tmp.push(if Self::inside(sx, sy) { px[Self::idx(sx, sy)] } else { 0 });
                }
            }
        }
        for row in 0..src.h {
            for col in 0..src.w {
                let v = tmp[(row as usize) * (src.w as usize) + col as usize];
                self.put(dx + col, dy + row, v, clip);
            }
        }
    }
    fn get_device_pixel(&self, x: i16, y: i16) -> u32 {
        if Self::inside(x, y) {
            self.px.lock().unwrap()[Self::idx(x, y)]
        } else {
            0
        }
    }
    fn put_device_pixel(&self, x: i16, y: i16, argb: u32) {
        if Self::inside(x, y) {
            self.px.lock().unwrap()[Self::idx(x, y)] = argb;
        }
    }
    fn flush_rect(&self, _x0: i16, _y0: i16, _x1: i16, _y1: i16) {}
}

// ── the host ──────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug)]
struct Packet {
    dx: i16,
    dy: i16,
    buttons: u8,
}

struct ScriptHost {
    raster: BufBackend,
    script: Mutex<VecDeque<Packet>>,
    clock: AtomicU64,
}

impl ScriptHost {
    fn new(script: &[Packet]) -> Self {
        ScriptHost {
            raster: BufBackend::new(),
            script: Mutex::new(script.iter().copied().collect()),
            clock: AtomicU64::new(0),
        }
    }
    fn push(&self, p: Packet) {
        self.script.lock().unwrap().push_back(p);
    }
}

impl Host for ScriptHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn console_try_read_key(&self) -> Option<u16> {
        None
    }
    fn raster(&self) -> Option<&dyn RasterBackend> {
        Some(&self.raster)
    }
    fn monotonic_millis(&self) -> u64 {
        self.clock.load(Ordering::Relaxed)
    }
    fn pump_input(&self) {
        self.clock.fetch_add(1, Ordering::Relaxed);
        let next = self.script.lock().unwrap().pop_front();
        if let Some(p) = next {
            tos_api::vdi::feed_relative(p.dx, p.dy, p.buttons, &self.raster);
        }
    }
}

unsafe impl Sync for ScriptHost {}
unsafe impl Send for ScriptHost {}

// ── AES parameter block ───────────────────────────────────────────────────

const ABI: Abi = Abi::Lp64;

struct Aespb {
    contrl: Box<[i16; 8]>,
    global: Box<[i16; 16]>,
    intin: Box<[i16; 16]>,
    intout: Box<[i16; 16]>,
    addrin: Box<[u64; 4]>,
    addrout: Box<[u64; 4]>,
    block: Box<[u64; 6]>,
}

impl Aespb {
    fn new() -> Aespb {
        let mut pb = Aespb {
            contrl: Box::new([0; 8]),
            global: Box::new([0; 16]),
            intin: Box::new([0; 16]),
            intout: Box::new([0; 16]),
            addrin: Box::new([0; 4]),
            addrout: Box::new([0; 4]),
            block: Box::new([0; 6]),
        };
        pb.block[0] = pb.contrl.as_ptr() as u64;
        pb.block[1] = pb.global.as_ptr() as u64;
        pb.block[2] = pb.intin.as_ptr() as u64;
        pb.block[3] = pb.intout.as_ptr() as u64;
        pb.block[4] = pb.addrin.as_ptr() as u64;
        pb.block[5] = pb.addrout.as_ptr() as u64;
        pb
    }

    fn call(&mut self, opcode: i16, in_len: i16, out_len: i16, ain_len: i16) {
        self.contrl[0] = opcode;
        self.contrl[1] = in_len;
        self.contrl[2] = out_len;
        self.contrl[3] = ain_len;
        self.contrl[4] = 0;
        self.intout.fill(0);
        unsafe { tos_api::aes::dispatch(self.block.as_ptr() as u64, ABI) };
    }
}

fn install(host: &'static ScriptHost) {
    let href: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
    tos_api::host::install_host(href);
}

// window kinds
const NAME: i16 = 0x0001;
const CLOSER: i16 = 0x0002;
const FULLER: i16 = 0x0004;
const MOVER: i16 = 0x0008;
const SIZER: i16 = 0x0020;


const MU_KEYBD: i16 = 0x0001;
const MU_BUTTON: i16 = 0x0002;
const MU_MESAG: i16 = 0x0010;

// AES opcodes used here.
const OP_APPL_INIT: i16 = 10;
const OP_EVNT_MULTI: i16 = 25;
const OP_FORM_DIAL: i16 = 51;
const OP_GRAF_MKSTATE: i16 = 79;
const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_CLOSE: i16 = 102;
const OP_WIND_GET: i16 = 104;
const OP_WIND_SET: i16 = 105;
const OP_WIND_NEW: i16 = 109;

const WF_WXYWH: i16 = 4;
const WF_CXYWH: i16 = 5;
const WF_NEWDESK: i16 = 14;

const FMD_FINISH: i16 = 3;

/// EmuTOS `gl_hbox` for the 8x16 system font: `gl_hchar + 3`
/// (`aes/gemgraf.c:412`), and therefore the height of `gl_rmenu` and the top
/// edge of `gl_rfull` (`:423-424,427`).
const GL_HBOX: i16 = 19;

/// A marker colour written straight into the framebuffer, standing in for
/// "application content the owner drew into its work area". Not a palette
/// colour, so nothing the AES paints can produce it by accident.
const MARK: u32 = 0x0011_2233;

/// Seed the backend's pseudo-palette the way the VDI dispatcher does before a
/// colour-sensitive op (`ACTIVE_PALETTE`, `kernel/src/vdi_backend.rs:11-21`):
/// EmuTOS' `videl_palette1` for pens 0..15, as RGB555-in-565 words.
fn seed_palette(b: &BufBackend) {
    const VIDEL1: [(u16, u16, u16); 16] = [
        (1000, 1000, 1000), (0, 0, 0), (1000, 0, 0), (0, 1000, 0),
        (0, 0, 1000), (0, 1000, 1000), (1000, 1000, 0), (1000, 0, 1000),
        (733, 733, 733), (533, 533, 533), (667, 0, 0), (0, 667, 0),
        (0, 0, 667), (0, 667, 667), (667, 667, 0), (667, 0, 667),
    ];
    let mut pal = [0u16; 256];
    for (i, (r, g, bl)) in VIDEL1.iter().enumerate() {
        let c5 = |v: u16| ((v as u32 * 31 + 500) / 1000) as u16;
        pal[i] = (c5(*r) << 11) | (c5(*g) << 6) | c5(*bl);
    }
    *b.palette.lock().unwrap() = pal;
}

/// A GEM `OBJECT` at the kernel/LP64 record width (`tos-api/src/aes/object.rs`).
#[repr(C)]
#[derive(Copy, Clone)]
struct Obj {
    next: i16,
    head: i16,
    tail: i16,
    otype: u16,
    flags: u16,
    state: u16,
    spec: u64,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
}

const G_BOX: u16 = 20;

/// The desktop-background tree an application registers with
/// `wind_set(0, WF_NEWDESK, tree, root)` — TeraDesk does exactly this
/// (`sdk/apps/teradesk/icon.c:2511`). Root = a patterned box over the whole
/// screen, plus three icon-sized boxes, i.e. the tree whose icons showed up as
/// travelling square holes when the AES painted it where it must not.
fn desk_tree() -> &'static [Obj] {
    // colour word: (border<<12)|(text<<8)|(tmode<<7)|(pattern<<4)|interior
    let bg = 0x0000_0043u64; // pattern 4, interior 3 (green) — a dithered desktop
    let ic = 0x0000_0002u64; // solid interior 2 (red) — an "icon"
    Box::leak(Box::new([
        Obj { next: -1, head: 1, tail: 3, otype: G_BOX, flags: 0, state: 0, spec: bg, x: 0, y: 0, w: SCREEN_W as i16, h: SCREEN_H as i16 },
        Obj { next: 2, head: -1, tail: -1, otype: G_BOX, flags: 0, state: 0, spec: ic, x: 150, y: 110, w: 32, h: 32 },
        Obj { next: 3, head: -1, tail: -1, otype: G_BOX, flags: 0, state: 0, spec: ic, x: 250, y: 180, w: 32, h: 32 },
        Obj { next: -1, head: -1, tail: -1, otype: G_BOX, flags: 0, state: 0, spec: ic, x: 350, y: 240, w: 32, h: 32 },
    ]))
}

impl BufBackend {
    /// Fill `(x, y, w, h)` with a raw device value, bypassing the AES — the
    /// stand-in for the owner's own content drawing.
    fn paint(&self, r: (i16, i16, i16, i16), argb: u32) {
        let mut px = self.px.lock().unwrap();
        for y in r.1..r.1 + r.3 {
            for x in r.0..r.0 + r.2 {
                if Self::inside(x, y) {
                    px[Self::idx(x, y)] = argb;
                }
            }
        }
    }

    /// Number of pixels inside `r` that differ from `before`, and the first
    /// such pixel (for the failure message).
    fn diff_in(&self, before: &[u32], r: (i16, i16, i16, i16)) -> (usize, Option<(i16, i16, u32, u32)>) {
        let px = self.px.lock().unwrap();
        let mut n = 0usize;
        let mut first = None;
        for y in r.1..r.1 + r.3 {
            for x in r.0..r.0 + r.2 {
                if !Self::inside(x, y) {
                    continue;
                }
                let k = Self::idx(x, y);
                if px[k] != before[k] {
                    if first.is_none() {
                        first = Some((x, y, before[k], px[k]));
                    }
                    n += 1;
                }
            }
        }
        (n, first)
    }

    /// Number of pixels inside `r` still holding [`MARK`] — used as
    /// "the AES did *not* repaint here".
    fn marked_in(&self, r: (i16, i16, i16, i16)) -> usize {
        let px = self.px.lock().unwrap();
        let mut n = 0;
        for y in r.1..r.1 + r.3 {
            for x in r.0..r.0 + r.2 {
                if Self::inside(x, y) && px[Self::idx(x, y)] == MARK {
                    n += 1;
                }
            }
        }
        n
    }
}

// ── the rig ───────────────────────────────────────────────────────────────

static SERIAL: Mutex<()> = Mutex::new(());
static RIG: OnceLock<&'static ScriptHost> = OnceLock::new();

/// The one host for this binary (`install_host` accepts a single install).
fn rig() -> &'static ScriptHost {
    RIG.get_or_init(|| {
        let host: &'static ScriptHost = Box::leak(Box::new(ScriptHost::new(&[])));
        install(host);
        seed_palette(&host.raster);
        host
    })
}

/// Fresh AES window state + the registered desktop background tree. `wind_new`
/// (EmuTOS `wm_new`, `aes/gemwmlib.c:1875-1893`) closes and deletes every
/// window and drains the update locks, so each test starts from the same table.
fn fresh(host: &'static ScriptHost) -> Aespb {
    let mut pb = Aespb::new();
    pb.call(OP_APPL_INIT, 0, 1, 0);
    host.script.lock().unwrap().clear();
    pb.intin.fill(0);
    pb.call(OP_WIND_NEW, 0, 1, 0);

    let tree = desk_tree();
    pb.intin.fill(0);
    pb.intin[0] = 0; // DESKWH
    pb.intin[1] = WF_NEWDESK;
    pb.intin[2] = 0; // root object
    pb.addrin[0] = tree.as_ptr() as u64;
    pb.call(OP_WIND_SET, 6, 1, 1);
    pb.addrin[0] = 0;
    pb
}

/// `wind_create` + `wind_open` at `r`, returning the handle.
fn open_window(pb: &mut Aespb, kind: i16, r: (i16, i16, i16, i16)) -> i16 {
    pb.intin.fill(0);
    pb.intin[0] = kind;
    pb.intin[1] = 0;
    pb.intin[2] = 0;
    pb.intin[3] = SCREEN_W as i16;
    pb.intin[4] = SCREEN_H as i16;
    pb.call(OP_WIND_CREATE, 5, 1, 0);
    let h = pb.intout[0];
    assert!(h > 0, "wind_create must give a handle, got {h}");
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.intin[1] = r.0;
    pb.intin[2] = r.1;
    pb.intin[3] = r.2;
    pb.intin[4] = r.3;
    pb.call(OP_WIND_OPEN, 5, 1, 0);
    h
}

/// `wind_get(handle, field)` → `(x, y, w, h)`.
fn wind_get_rect(pb: &mut Aespb, handle: i16, field: i16) -> (i16, i16, i16, i16) {
    pb.intin.fill(0);
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(OP_WIND_GET, 2, 5, 0);
    (pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4])
}

fn form_dial(pb: &mut Aespb, fmd: i16, little: (i16, i16, i16, i16), big: (i16, i16, i16, i16)) {
    pb.intin.fill(0);
    pb.intin[0] = fmd;
    pb.intin[1] = little.0;
    pb.intin[2] = little.1;
    pb.intin[3] = little.2;
    pb.intin[4] = little.3;
    pb.intin[5] = big.0;
    pb.intin[6] = big.1;
    pb.intin[7] = big.2;
    pb.intin[8] = big.3;
    pb.call(OP_FORM_DIAL, 9, 1, 0);
}

/// Consume every packet still queued, so a later `move_to` is not driving the
/// pointer through a backlog.
fn drain(host: &'static ScriptHost, pb: &mut Aespb) {
    for _ in 0..64 {
        if host.script.lock().unwrap().is_empty() {
            return;
        }
        pb.call(OP_GRAF_MKSTATE, 0, 5, 0);
    }
}

/// Walk the pointer to `(tx, ty)` with `buttons`, one packet per AES call.
fn move_to(host: &'static ScriptHost, pb: &mut Aespb, tx: i16, ty: i16, buttons: u8) {
    drain(host, pb);
    for _ in 0..200 {
        let (cx, cy) = tos_api::vdi::mouse_position();
        if cx == tx && cy == ty {
            break;
        }
        let dx = (tx - cx).clamp(-32, 32);
        let dy = (ty - cy).clamp(-32, 32);
        host.push(Packet { dx, dy, buttons });
        pb.call(OP_GRAF_MKSTATE, 0, 5, 0);
    }
}

// ── geometry ──────────────────────────────────────────────────────────────

/// Window A — opened first, so it ends up *below* B.
const RA: (i16, i16, i16, i16) = (40, 60, 400, 300);
/// Window B — opened second, overlapping A's bottom-right corner and reaching
/// out onto the bare desktop to A's right.
const RB: (i16, i16, i16, i16) = (240, 180, 340, 250);
/// A patch of bare desktop inside RB but outside RA, clear of the background
/// tree's icon boxes: what the AES *must* repaint.
const BARE: (i16, i16, i16, i16) = (460, 300, 110, 120);

const KIND: i16 = NAME | CLOSER | FULLER | MOVER | SIZER;

/// `r` shrunk by `n` pixels on every side — the interior of a work area, so a
/// 3D frame's inner bevel row is never counted as "the work area was painted".
fn inset(r: (i16, i16, i16, i16), n: i16) -> (i16, i16, i16, i16) {
    (r.0 + n, r.1 + n, r.2 - 2 * n, r.3 - 2 * n)
}

/// Assert that not one pixel of `work` changed, and that `bare` did change —
/// the two halves of `do_walk`'s clip (`aes/gemwmlib.c:286-312`).
fn assert_clipped(
    b: &BufBackend,
    before: &[u32],
    work: (i16, i16, i16, i16),
    bare: (i16, i16, i16, i16),
    what: &str,
) {
    let (n, first) = b.diff_in(before, work);
    assert_eq!(
        n, 0,
        "{what}: {n} of {} pixels inside the covering window's work area changed \
         (first {first:?}); EmuTOS clips w_drawdesk to the DESKTOP window's owner \
         rectangle list, which does not contain a rectangle another open window \
         owns (do_walk, aes/gemwmlib.c:286-312; newrect/mkrect, aes/gemwrect.c:136-205). \
         The window's content is restored by its owner on WM_REDRAW, not by the AES \
         (w_update/w_redraw, aes/gemwmlib.c:1004-1048,887-907)",
        work.2 as i32 * work.3 as i32,
    );
    let still = b.marked_in(bare);
    assert_eq!(
        still, 0,
        "{what}: {still} of {} pixels of *bare desktop* inside the damage rectangle \
         were left unpainted; the desktop window owns them, so w_drawdesk must draw \
         the background tree there (aes/gemwmlib.c:340-359)",
        bare.2 as i32 * bare.3 as i32,
    );
}

// ── 1. wind_close ─────────────────────────────────────────────────────────

/// Closing the top window repaints the desktop it uncovered — and nothing else.
///
/// `wm_close` → `draw_change` (`aes/gemwmlib.c:1438-1459,1058-1223`) rebuilds
/// the rectangle lists first (`:1129`), so by the time `w_drawdesk(&c)` runs
/// (`:1217-1219`) the closed window's rectangle has gone back to whoever is
/// underneath: the desktop where nothing else covers it, window A where A does.
/// A gets its part back as a `WM_REDRAW` (`w_update`/`w_redraw`,
/// `:1004-1048,887-907`), not as a background repaint.
#[test]
fn closing_a_window_does_not_paint_the_window_below() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let host = rig();
    let b = &host.raster;
    let mut pb = fresh(host);

    let ha = open_window(&mut pb, KIND, RA);
    let hb = open_window(&mut pb, KIND, RB);
    let work_a = wind_get_rect(&mut pb, ha, WF_WXYWH);
    assert!(work_a.2 > 0 && work_a.3 > 0, "A must have a work area");

    // A's owner has drawn its content; the bare desktop is marked so the test
    // can tell "repainted" from "left alone".
    b.paint(inset(work_a, 2), MARK);
    b.paint(BARE, MARK);
    let before = b.snapshot();

    pb.intin.fill(0);
    pb.intin[0] = hb;
    pb.call(OP_WIND_CLOSE, 1, 1, 0);

    assert_clipped(b, &before, inset(work_a, 2), BARE, "wind_close");
}

// ── 2. wind_open ──────────────────────────────────────────────────────────

/// Opening a window paints its frame and asks the owner for the inside; it
/// never paints the inside itself.
///
/// On an open `draw_change` takes the "grow that is a superset or an open"
/// branch (`aes/gemwmlib.c:1160-1170`), which leaves `start == w_handle`, so
/// the `if (start == DESKWH) w_drawdesk(&c)` at `:1217-1219` does not even run.
/// `w_update` then draws the frame (`w_cpwalk`) and posts `WM_REDRAW`
/// (`:1004-1048`). The work area keeps whatever was on the screen until the
/// owner answers.
#[test]
fn opening_a_window_does_not_paint_its_work_area() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let host = rig();
    let b = &host.raster;
    let mut pb = fresh(host);

    let _ha = open_window(&mut pb, KIND, RA);
    // Mark the whole rectangle B is about to occupy.
    b.paint(RB, MARK);
    let before = b.snapshot();

    let hb = open_window(&mut pb, KIND, RB);
    let work_b = wind_get_rect(&mut pb, hb, WF_WXYWH);
    assert!(work_b.2 > 0 && work_b.3 > 0, "B must have a work area");

    let (n, first) = b.diff_in(&before, inset(work_b, 2));
    assert_eq!(
        n, 0,
        "wind_open painted {n} of {} work-area pixels (first {first:?}); on an open \
         EmuTOS never calls w_drawdesk at all — `start` stays at the opening window \
         (aes/gemwmlib.c:1160-1170,1217-1219) — and w_cpwalk draws only the frame \
         objects, W_WORK being a borderless G_IBOX (aes/gemwmlib.c:67-68,87-88; \
         aes/gemoblib.c:663-670)",
        work_b.2 as i32 * work_b.3 as i32,
    );
}

// ── 3. wind_set(WF_CURRXYWH) ──────────────────────────────────────────────

/// The programmatic move: the window leaves, the desktop it uncovered is
/// repainted, the window it uncovered is not.
#[test]
fn a_programmatic_move_does_not_paint_the_window_below() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let host = rig();
    let b = &host.raster;
    let mut pb = fresh(host);

    let ha = open_window(&mut pb, KIND, RA);
    let hb = open_window(&mut pb, KIND, RB);
    let work_a = wind_get_rect(&mut pb, ha, WF_WXYWH);

    b.paint(inset(work_a, 2), MARK);
    b.paint((RB.0 + 5, RA.1 + RA.3 + 5, 50, 60), MARK);
    let before = b.snapshot();

    // Move B down-right, off A entirely; the damage is old ∪ new.
    pb.intin.fill(0);
    pb.intin[0] = hb;
    pb.intin[1] = WF_CXYWH;
    pb.intin[2] = 300;
    pb.intin[3] = 220;
    pb.intin[4] = RB.2;
    pb.intin[5] = RB.3;
    pb.call(OP_WIND_SET, 6, 1, 0);

    // B's new rectangle covers part of A again; only the part of A's work area
    // that no window covers is asserted untouched. The bare-desktop probe has
    // to be somewhere the *new* B does not cover either: the bottom-left slice
    // of B's old rectangle, which is below A and left of the new B.
    let uncovered = (work_a.0, work_a.1, (300 - work_a.0).max(0), work_a.3);
    let bare = (RB.0 + 5, RA.1 + RA.3 + 5, 50, 60);
    assert_clipped(b, &before, inset(uncovered, 2), bare, "wind_set(WF_CURRXYWH)");
}

// ── 4. form_dial(FMD_FINISH) ──────────────────────────────────────────────

/// Taking a dialog down repaints the desktop under it, not the windows under
/// it: `fm_dial(FMD_FINISH)` is `w_drawdesk(pt)` + `w_update(DESKWH, pt, …)`
/// (`aes/gemfmlib.c:375-378`), i.e. the same `do_walk` clip plus a `WM_REDRAW`
/// fan-out to every window the dialog covered.
#[test]
fn form_dial_finish_does_not_paint_the_windows_under_the_dialog() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let host = rig();
    let b = &host.raster;
    let mut pb = fresh(host);

    let ha = open_window(&mut pb, KIND, RA);
    let work_a = wind_get_rect(&mut pb, ha, WF_WXYWH);

    b.paint(inset(work_a, 2), MARK);
    b.paint(BARE, MARK);
    let before = b.snapshot();

    // A dialog rectangle that spans A and the bare desktop to its right.
    form_dial(&mut pb, FMD_FINISH, (300, 220, 40, 40), (20, 40, 600, 420));

    assert_clipped(b, &before, inset(work_a, 2), BARE, "form_dial(FMD_FINISH)");
}

/// `do_walk`'s *first* clip: `rc_intersect(&gl_rfull, pc)`
/// (`aes/gemwmlib.c:294-298`). `gl_rfull` is the screen minus the menu row
/// (`aes/gemgraf.c:423-424`, set at `gsx_start` whether or not a menu bar is
/// installed), so `w_drawdesk` never touches the top `gl_hbox` rows — not on a
/// full-screen `form_dial(FMD_FINISH)` either. Removing a menu bar does not
/// repaint that strip (`mn_bar`, `aes/gemmnlib.c:724-728`); installing one
/// does.
#[test]
fn the_background_repaint_stops_at_the_menu_row() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let host = rig();
    let b = &host.raster;
    let mut pb = fresh(host);

    b.paint((0, 0, SCREEN_W as i16, SCREEN_H as i16), MARK);
    let before = b.snapshot();

    form_dial(&mut pb, FMD_FINISH, (300, 220, 40, 40), (0, 0, SCREEN_W as i16, SCREEN_H as i16));

    let menu = (0, 0, SCREEN_W as i16, GL_HBOX);
    let (n, first) = b.diff_in(&before, menu);
    assert_eq!(
        n, 0,
        "form_dial(FMD_FINISH) painted {n} pixels of the menu row (first {first:?}); \
         do_walk intersects its clip with gl_rfull first (aes/gemwmlib.c:294-298), and \
         gl_rfull starts at gl_hbox = {GL_HBOX} (aes/gemgraf.c:412,423-424)",
    );
    // Below the menu row the background *is* repainted.
    let still = b.marked_in((0, GL_HBOX, SCREEN_W as i16, SCREEN_H as i16 - GL_HBOX));
    assert_eq!(still, 0, "everything below the menu row must be repainted ({still} px left)");
}

// ── 5. the mover drag ─────────────────────────────────────────────────────

/// Window A, and window B straddling A's left edge: the strip B vacates when it
/// is dragged to the right falls half on the bare desktop and half on A.
const DA: (i16, i16, i16, i16) = (220, 40, 390, 400);
const DB: (i16, i16, i16, i16) = (200, 150, 260, 180);
/// How far the drag moves B to the right.
const DRAG: i16 = 40;

/// Moving a window by its mover repaints the strip it vacated — but only the
/// part of that strip the desktop still owns.
///
/// The mover drag itself paints nothing at all: it is an XOR outline
/// (`gr_dragbox`, `aes/gemgrlib.c:243-267`) that `gr_wait` complements away
/// again, and the window is moved **once, at the end**, when the owner answers
/// `WM_MOVED` with `wind_set(WF_CURRXYWH)` (`hctl_window` → `ct_msgup`,
/// `aes/gemctrl.c:237-238,314`). That `wm_set` runs `draw_change`
/// (`aes/gemwmlib.c:1673-1675`), whose `w_drawdesk(&c)` (`:1217-1219`) comes
/// *after* the rectangle lists were rebuilt for the new position (`:1129`) and
/// is clipped by `do_walk` to what the desktop window owns. Half this strip is
/// desktop and gets the background; the other half is window A's and must be
/// left for A's `WM_REDRAW`.
#[test]
fn the_mover_drag_repaints_only_desktop_it_uncovered() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let host = rig();
    let b = &host.raster;
    let mut pb = fresh(host);

    let ha = open_window(&mut pb, KIND, DA);
    let hb = open_window(&mut pb, KIND, DB);
    let work_b = wind_get_rect(&mut pb, hb, WF_WXYWH);
    assert!(
        DB.1 > DA.1 && DB.1 + DB.3 < DA.1 + DA.3 && DB.0 < DA.0 && DA.0 < DB.0 + DRAG,
        "B must straddle A's left edge and sit inside A vertically"
    );

    // Grab the mover well to the right of the strip B will vacate, so the
    // mouse sprite never lands in the measured region.
    let (gx, gy) = (DB.0 + DB.2 - 40, DB.1 + 6);
    move_to(host, &mut pb, gx, gy, 0);
    host.push(Packet { dx: 0, dy: 0, buttons: 1 });
    pb.call(OP_GRAF_MKSTATE, 0, 5, 0); // observe the press

    // B's owner has drawn its content, so a background repaint into the strip
    // B leaves behind is unmistakable.
    b.paint(work_b, MARK);
    let before = b.snapshot();

    host.push(Packet { dx: DRAG, dy: 0, buttons: 1 });
    host.push(Packet { dx: 0, dy: 0, buttons: 0 }); // release
    for _ in 0..8 {
        host.push(Packet { dx: 0, dy: 0, buttons: 0 });
    }

    // The first call runs the drag (the button is down, so it cannot return
    // until `intercept` has handled it); the loop then walks past whatever
    // `wind_open` left in the FIFO to the `WM_MOVED` the drag posted.
    let mut msg = Box::new([0i16; 8]);
    for _ in 0..8 {
        pb.intin.fill(0);
        pb.intin[0] = MU_MESAG | MU_BUTTON | MU_KEYBD;
        pb.intin[1] = 0x0102;
        pb.intin[2] = 3;
        pb.intin[3] = 0;
        pb.addrin[0] = msg.as_mut_ptr() as u64;
        pb.call(OP_EVNT_MULTI, 16, 7, 1);
        pb.addrin[0] = 0;
        if msg[0] == 28 {
            break;
        }
    }
    assert_eq!(msg[0], 28, "the drag must post WM_MOVED (28), got {msg:?}");

    // Nothing may have been painted yet — the drag draws the XOR outline and
    // erases it again (`gr_wait`, aes/gemgrlib.c:169-191).
    let (during, at) = b.diff_in(&before, work_b);
    assert_eq!(
        during, 0,
        "the mover drag itself changed {during} pixels of the window's work area \
         (first {at:?}); `gr_dragbox` moves nothing and paints nothing"
    );

    // The owner answers WM_MOVED — this is where the screen changes.
    pb.intin.fill(0);
    pb.intin[0] = hb;
    pb.intin[1] = WF_CXYWH;
    pb.intin[2] = msg[4];
    pb.intin[3] = msg[5];
    pb.intin[4] = msg[6];
    pb.intin[5] = msg[7];
    pb.call(OP_WIND_SET, 6, 1, 0);

    // The strip B vacated: the columns left of B's *new* border rectangle,
    // restricted to B's former work area. Left of A's border it is the
    // desktop's; from A's *work* area on it is A's. A's border columns belong to
    // neither half — the AES draws its frame there itself (`w_cpwalk` per window
    // inside the damage rectangle, `aes/gemwmlib.c:1029-1030`).
    let work_a = wind_get_rect(&mut pb, ha, WF_WXYWH);
    let desk_strip = (work_b.0, work_b.1, DA.0 - work_b.0, work_b.3);
    let a_strip = (work_a.0, work_b.1, DB.0 + DRAG - work_a.0, work_b.3);
    assert!(desk_strip.2 > 0 && a_strip.2 > 0, "both halves of the strip must be non-empty");

    let (n, first) = b.diff_in(&before, a_strip);
    assert_eq!(
        n, 0,
        "the move painted {n} of {} pixels into the half of the vacated strip \
         that window A owns (first {first:?}). w_drawdesk walks the DESKTOP window's \
         owner rectangle list, which after `newrect` no longer contains anything an \
         open window covers (aes/gemwmlib.c:286-312,340-359; aes/gemwrect.c:136-205); \
         A gets that strip back as a WM_REDRAW, not as a background repaint",
        a_strip.2 as i32 * a_strip.3 as i32,
    );

    let still = b.marked_in(desk_strip);
    assert_eq!(
        still, 0,
        "{still} of {} pixels of the vacated strip that the *desktop* owns were left \
         unpainted; that half is exactly what w_drawdesk is for \
         (aes/gemwmlib.c:340-359)",
        desk_strip.2 as i32 * desk_strip.3 as i32,
    );
}
