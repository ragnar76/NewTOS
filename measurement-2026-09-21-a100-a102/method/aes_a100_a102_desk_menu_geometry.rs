//! A100 — the kernel desktop's drop-down boxes carry EmuTOS's **outside**
//! border (`ob_spec = 0xFF1100`, `desk/desk_rsc.c:294`: `th = -1`), so the
//! item XOR of a hover never reaches the frame and the AES repaints none —
//! and A102 — the kernel desktop's `THEACTIVE` is the titles' sum, as
//! `adjust_menu` sizes it (`desk/deskmain.c:1534`, `mbar->ob_width = x`), so
//! the bar row right of *Options* is the application's; `menu_popup`'s
//! no-pick branch writes `mn_keystate` alone (`aes/gemmnext.c:347`).
//!
//! **The tree here is a replica of the kernel desktop's ADMENU as
//! `kernel/src/desk/menu.rs` `build_menu` lays it out** — same labels, same
//! `adjust_menu` arithmetic (title `strlen · 8 + 4`, box `maxlen · 8 + 6`,
//! `THEACTIVE` at the RSC's 2-cell origin and as wide as the titles, boxes at
//! `THEACTIVE.x + title.x` below the 19 px bar, entries at `(0, row · 19)`
//! spanning the box). The kernel crate is `forced-target` AArch64 and has no
//! host tests, so the geometry `build_menu` really builds is checked on the
//! board (the serial dump of the registered tree in
//! `measurement-2026-09-21-a100-a102/`); what these cases pin is the AES's
//! behaviour on that geometry.
//!
//! EmuTOS: a hover moves the highlight through `menu_select` → `do_chg` →
//! `ob_change` (`aes/gemmnlib.c:170`), one `bb_fill(MD_XOR, …)` of the item's
//! rectangle each way (`aes/gemoblib.c:1295-1304`), and `mn_do` draws nothing
//! else afterwards (`:451-475`). A box with an outside border keeps its frame
//! outside every item rectangle; a box with an inside border gets its frame
//! inverted in the item's rows — EmuTOS repaints it no more than it repaints
//! anything else, which is why the desktop's RSC uses the outside border
//! and why the bar cannot start at character 0 (`deskmain.c:1537-1541`).
//!
//! NewTOS built the desktop's boxes with `(1<<16)|0x1070` (`th = +1`) and had
//! `rehilite`/`popup_shown` repaint the box frame after every toggle to hide
//! the inverted border pixels — on an outside-border box (TeraDesk's, QED's)
//! that repaint drew a second, inner frame into the box.
//!
//! Harness (raster stand-in + scripted host + parameter block) copied from
//! `aes_a87_a88_menu_ctwait_popup.rs`, plus screen snapshots and keys taken at
//! a pump: the pointer advances only in `Host::pump_input`, exactly as on the
//! board, and a drop-down is only ever on screen *inside* the track, so its
//! pixels are read from inside the `evnt_multi` call.

#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tos_api::bdos::process::Abi;
use tos_api::host::Host;
use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};

const SCREEN_W: u16 = 640;
const SCREEN_H: u16 = 480;

// ── a raster backend over a real pixel buffer ──────────────────────────────

struct BufBackend {
    px: Mutex<Vec<u32>>,
    palette: Mutex<[u16; 256]>,
    req: Mutex<[(u16, u16, u16); 256]>,
}

impl BufBackend {
    fn new() -> Self {
        BufBackend {
            px: Mutex::new(vec![0; SCREEN_W as usize * SCREEN_H as usize]),
            palette: Mutex::new([0; 256]),
            req: Mutex::new([(0, 0, 0); 256]),
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
        let v = self.rgb565_to_device(self.pel_to_device16(pel));
        for yy in y..y.saturating_add(h) {
            for xx in x..x.saturating_add(w) {
                self.put(xx, yy, v, clip);
            }
        }
    }
    fn abline(
        &self,
        x0: i16,
        y0: i16,
        x1: i16,
        y1: i16,
        pel: Pel,
        _mode: WriteMode,
        clip: ClipRect,
        mask: u16,
        _last: bool,
    ) -> u16 {
        let v = self.rgb565_to_device(self.pel_to_device16(pel));
        let (mut x, mut y) = (x0 as i32, y0 as i32);
        let (dx, dy) = ((x1 as i32 - x).abs(), -(y1 as i32 - y).abs());
        let (sx, sy) = (if x < x1 as i32 { 1 } else { -1 }, if y < y1 as i32 { 1 } else { -1 });
        let mut err = dx + dy;
        let mut m = mask;
        loop {
            if m & 0x8000 != 0 {
                self.put(x as i16, y as i16, v, clip);
            }
            m = m.rotate_left(1);
            if x == x1 as i32 && y == y1 as i32 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
        m
    }
    /// Mode-aware, as the A99 harness (`aes_screen_buffer_save_under.rs`):
    /// the hover toggle is `fill_pattern` in `WriteMode::Xor`, which must
    /// complement the target, not paint it.
    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill, clip: ClipRect) {
        let fg = self.rgb565_to_device(self.pel_to_device16(fill.fg));
        let bg = self.rgb565_to_device(self.pel_to_device16(fill.bg));
        for yy in y..y.saturating_add(h) {
            let row = fill.pattern[(yy.rem_euclid(16)) as usize % fill.pattern.len()];
            for xx in x..x.saturating_add(w) {
                let set = row & (0x8000u16 >> (xx.rem_euclid(16) as u16)) != 0;
                match fill.mode {
                    WriteMode::Replace => self.put(xx, yy, if set { fg } else { bg }, clip),
                    WriteMode::Transparent => {
                        if set {
                            self.put(xx, yy, fg, clip)
                        }
                    }
                    WriteMode::Xor => {
                        if set {
                            let v = self.get_device_pixel(xx, yy);
                            self.put(xx, yy, v ^ 0x00FF_FFFF, clip);
                        }
                    }
                    WriteMode::RevTransparent => {
                        if !set {
                            self.put(xx, yy, fg, clip)
                        }
                    }
                }
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
                let w = src.bits.word(wi).unwrap_or(0);
                let bit = w & (0x8000u16 >> (sx.rem_euclid(16)) as u16);
                self.put(dst.x + col, dst.y + row, if bit != 0 { f } else { b }, clip);
            }
        }
    }
    fn copy_rect(&self, src: Rect, dx: i16, dy: i16, _op: u8, clip: ClipRect) {
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

// ── the host: input advances only in `pump_input`, as on the board ─────────

/// One synthetic mouse packet, in the form the VDI's `mouse_int` receives it,
/// plus two harness-side side effects taken **at the pump, before the packet
/// is fed**: `snap` copies the raster into the host's snapshot list under that
/// index (the screen as the AES left it after the previous packet), and `key`
/// queues one `EV_KRET` word for `console_try_read_key` (a key struck while
/// the pointer rests where the previous packets put it).
#[derive(Copy, Clone, Debug)]
struct Packet {
    dx: i16,
    dy: i16,
    /// VDI convention: bit 0 = left, bit 1 = right.
    buttons: u8,
    snap: Option<usize>,
    key: Option<u16>,
}

/// A plain motion/button packet.
const fn pkt(dx: i16, dy: i16, buttons: u8) -> Packet {
    Packet { dx, dy, buttons, snap: None, key: None }
}

/// An idle packet that snapshots the screen under `index` first.
const fn snap(index: usize) -> Packet {
    Packet { dx: 0, dy: 0, buttons: 0, snap: Some(index), key: None }
}

/// An idle packet that queues the key `word` (`scancode << 8 | ascii`) first.
const fn key(word: u16) -> Packet {
    Packet { dx: 0, dy: 0, buttons: 0, snap: None, key: Some(word) }
}

struct ScriptHost {
    raster: BufBackend,
    script: Mutex<VecDeque<Packet>>,
    /// Screen copies taken by `snap` packets, by index.
    snaps: Mutex<Vec<(usize, Vec<u32>)>>,
    /// Keys queued by `key` packets, handed out by `console_try_read_key`.
    keys: Mutex<VecDeque<u16>>,
    /// Milliseconds; advanced by `pump_input` alone.
    clock: AtomicU64,
    /// Consecutive pumps that found the script empty. A tracking loop that
    /// never sees the event it waits for would otherwise spin forever inside
    /// the trap; the cap turns that into a failing assertion.
    idle: AtomicUsize,
}

/// Pumps on an empty script before the harness gives up. `count_clicks` may
/// legitimately run a whole double-click window (≤ 450 pumps) on an empty
/// script, so the cap sits well above that.
const IDLE_CAP: usize = 20_000;

impl ScriptHost {
    fn new() -> Self {
        ScriptHost {
            raster: BufBackend::new(),
            script: Mutex::new(VecDeque::new()),
            snaps: Mutex::new(Vec::new()),
            keys: Mutex::new(VecDeque::new()),
            clock: AtomicU64::new(0),
            idle: AtomicUsize::new(0),
        }
    }
    fn push(&self, p: Packet) {
        self.script.lock().unwrap().push_back(p);
    }
    fn clear(&self) {
        self.script.lock().unwrap().clear();
        self.snaps.lock().unwrap().clear();
        self.keys.lock().unwrap().clear();
        self.idle.store(0, Ordering::Relaxed);
    }
    /// The screen copy a `snap(index)` packet took, as device pixels (the
    /// XOR complement of the palette's RGB555 white is no palette entry, so
    /// `read_pixel`'s index mapping cannot express a hovered row) — `None` if
    /// that packet was never pumped.
    fn snapshot(&self, index: usize) -> Option<Vec<u32>> {
        let snaps = self.snaps.lock().unwrap();
        let (_, px) = snaps.iter().find(|(i, _)| *i == index)?;
        Some(px.clone())
    }
}

impl Host for ScriptHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn console_try_read_key(&self) -> Option<u16> {
        self.keys.lock().unwrap().pop_front()
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
        match next {
            Some(p) => {
                self.idle.store(0, Ordering::Relaxed);
                if let Some(i) = p.snap {
                    let px = self.raster.px.lock().unwrap().clone();
                    self.snaps.lock().unwrap().push((i, px));
                }
                if let Some(k) = p.key {
                    self.keys.lock().unwrap().push_back(k);
                }
                tos_api::vdi::feed_relative(p.dx, p.dy, p.buttons, &self.raster);
            }
            None => {
                let n = self.idle.fetch_add(1, Ordering::Relaxed) + 1;
                assert!(
                    n < IDLE_CAP,
                    "the AES is still waiting after {n} pumps on an exhausted mouse script: \
                     the event it waits for never came (pointer {:?}, buttons {:#04x})",
                    tos_api::vdi::mouse_position(),
                    tos_api::vdi::mouse_buttons()
                );
            }
        }
    }
}

// SAFETY: every mutable field is behind a Mutex / atomic.
unsafe impl Sync for ScriptHost {}
unsafe impl Send for ScriptHost {}

// ── AES parameter block ────────────────────────────────────────────────────

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

    /// Issue one AES call, exactly as the GEM-trap handler does.
    fn call(&mut self, opcode: i16, in_len: i16, out_len: i16, ain_len: i16) {
        self.contrl[0] = opcode;
        self.contrl[1] = in_len;
        self.contrl[2] = out_len;
        self.contrl[3] = ain_len;
        self.contrl[4] = 0;
        self.intout.fill(0);
        // SAFETY: `block` holds six live pointers to arrays owned by `self`,
        // each larger than any opcode issued here declares.
        unsafe { tos_api::aes::dispatch(self.block.as_ptr() as u64, ABI) };
    }
}

fn install(host: &'static ScriptHost) {
    let href: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
    tos_api::host::install_host(href);
}

/// Seed the backend's pseudo-palette the way the VDI dispatcher does
/// (`ACTIVE_PALETTE`, `kernel/src/vdi_backend.rs:11-21`): EmuTOS'
/// `videl_palette1` for pens 0..15 as RGB555-in-565 words, so `read_pixel`
/// round-trips the pens the menu draws with.
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

// ── the kernel desktop's menu tree ─────────────────────────────────────────

/// A GEM `OBJECT` at the kernel/LP64 record width (`tos-api/src/aes/object.rs:31`).
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
const G_IBOX: u16 = 25;
const G_STRING: u16 = 28;
const G_TITLE: u16 = 32;
const LASTOB: u16 = 0x0020;
const DISABLED: u16 = 0x0008;

/// `kernel/src/desk/menu.rs` `MENU_H` / `CHAR_W`.
const BAR_H: i16 = 19;
const CHAR_W: i16 = 8;

/// The RSC's drop-down box word (`desk/desk_rsc.c:294`, `16716032L`): `th =
/// -1` (outside border), border BLACK, text BLACK, IP_HOLLOW interior.
const SPEC_OUTSIDE: u64 = 0x00FF_1100;
/// What `build_menu` used before A100: `th = +1` (inside border), border
/// BLACK, IP_SOLID WHITE interior.
const SPEC_INSIDE: u64 = (1 << 16) | 0x1070;

/// One drop-down entry: a disabled separator or a selectable item, both
/// NUL-terminated (`kernel/src/desk/menu.rs` `Entry`).
enum Entry {
    Sep(&'static [u8]),
    Act(&'static [u8]),
}
use Entry::{Act, Sep};

/// The four desktop menus, labels as in `kernel/src/desk/menu.rs` `MENUS`.
const MENUS: [(&[u8], &[Entry]); 4] = [
    (b" Desk \0", &[Act(b"  Desktop info... \0"), Sep(b"------------------\0")]),
    (
        b" File \0",
        &[
            Act(b"  Open             \0"),
            Act(b"  Info/Rename...   \0"),
            Act(b"  Search...        \0"),
            Sep(b"-------------------\0"),
            Act(b"  New folder...    \0"),
            Act(b"  Close folder     \0"),
            Act(b"  Close top window \0"),
            Act(b"  Cycle windows    \0"),
            Act(b"  Select all items \0"),
            Act(b"  Set file mask... \0"),
            Sep(b"-------------------\0"),
            Act(b"  Delete...        \0"),
            Sep(b"-------------------\0"),
            Act(b"  Execute EmuCON   \0"),
            Act(b"  Shutdown         \0"),
        ],
    ),
    (
        b" View \0",
        &[
            Act(b"  Show as icons \0"),
            Act(b"  Show as text  \0"),
            Sep(b"----------------\0"),
            Act(b"  Sort by name  \0"),
            Act(b"  Sort by type  \0"),
            Act(b"  Sort by size  \0"),
            Act(b"  Sort by date  \0"),
            Act(b"  No sort       \0"),
            Sep(b"----------------\0"),
            Act(b"  Size to fit   \0"),
        ],
    ),
    (
        b" Options \0",
        &[
            Act(b"  Set background...       \0"),
            Act(b"  Install icon...         \0"),
            Act(b"  Install application...  \0"),
            Act(b"  Install devices         \0"),
            Act(b"  Remove desktop icon     \0"),
            Sep(b"--------------------------\0"),
            Act(b"  Set preferences...      \0"),
            Act(b"  Desktop configuration...\0"),
            Act(b"  Change resolution...    \0"),
            Sep(b"--------------------------\0"),
            Act(b"  Read .INF file...       \0"),
            Act(b"  Save desktop...         \0"),
        ],
    ),
];

fn label_len(s: &[u8]) -> i16 {
    s.iter().position(|&b| b == 0).unwrap_or(s.len()) as i16
}

fn entry_label(e: &Entry) -> &'static [u8] {
    match *e {
        Sep(s) | Act(s) => s,
    }
}

/// `adjust_menu` as `kernel/src/desk/menu.rs` computes it: `THEACTIVE` at
/// `2 · CHAR_W` (`desk_rsc.c:255-259`) and `Σ title_w` wide (`deskmain.c:1534`),
/// titles left to right at `strlen · 8 + 4`, boxes `maxlen · 8 + 6` wide at
/// `THEACTIVE.x + title.x` (`:1617`), shifted left to stay on screen.
struct Geom {
    active_x: i16,
    active_w: i16,
    title_x: [i16; 4],
    title_w: [i16; 4],
    box_x: [i16; 4],
    box_w: [i16; 4],
}

fn adjust_menu(sw: i16) -> Geom {
    let mut g = Geom { active_x: 2 * CHAR_W, active_w: 0, title_x: [0; 4], title_w: [0; 4], box_x: [0; 4], box_w: [0; 4] };
    let mut tx = 0i16;
    for (ti, (label, entries)) in MENUS.iter().enumerate() {
        let tw = label_len(label) * CHAR_W + 4;
        g.title_x[ti] = tx;
        g.title_w[ti] = tw;
        tx += tw;
        let maxlen = entries.iter().map(|e| label_len(entry_label(e))).max().unwrap_or(8);
        g.box_w[ti] = maxlen * CHAR_W + 6;
    }
    g.active_w = tx;
    for ti in 0..4 {
        g.box_x[ti] = (g.active_x + g.title_x[ti]).min(sw - g.box_w[ti]).max(0);
    }
    g
}

/// Object indices in the replica: `THEACTIVE` 2, titles 3..=6, `THEMENUS` 7,
/// then per menu one box followed by its entries.
const THEACTIVE: i16 = 2;
const TITLE_FILE: i16 = 4;
const TITLE_OPTIONS: i16 = 6;

/// The `THEACTIVE` rectangle the replica registers — what `menu_bar` takes as
/// `gl_ctwait` (`aes/gemmnlib.c:717`): `(16, 0, 232, 19)` for these labels.
fn active_rect() -> (i16, i16, i16, i16) {
    let g = adjust_menu(SCREEN_W as i16);
    (g.active_x, 0, g.active_w, BAR_H)
}

/// The *File* drop-down box's absolute rectangle: `(68, 19, 158, 285)`.
fn file_box_rect() -> (i16, i16, i16, i16) {
    let g = adjust_menu(SCREEN_W as i16);
    (g.box_x[1], BAR_H, g.box_w[1], MENUS[1].1.len() as i16 * BAR_H)
}

/// Build the desktop's ADMENU as `build_menu` does, with `box_spec` on every
/// drop-down box. Leaked: the AES keeps the pointer.
fn desk_menu_tree(box_spec: u64) -> *mut Obj {
    let sw = SCREEN_W as i16;
    let sh = SCREEN_H as i16;
    let g = adjust_menu(sw);
    let mk = |ty, fl, spec: u64, x, y, w, h, next, head, tail| Obj {
        next, head, tail, otype: ty, flags: fl, state: 0, spec, x, y, w, h,
    };
    let nt = MENUS.len();
    let first_title = 3usize;
    let themenus = first_title + nt;
    let n_obj = themenus + 1 + MENUS.iter().map(|(_, e)| 1 + e.len()).sum::<usize>();
    let mut o = vec![mk(0, 0, 0, 0, 0, 0, 0, -1, -1, -1); n_obj];
    o[0] = mk(G_IBOX, 0, 0, 0, 0, sw, sh, -1, 1, themenus as i16);
    o[1] = mk(G_BOX, 0, 0, 0, 0, sw, BAR_H, themenus as i16, 2, 2);
    o[2] = mk(G_IBOX, 0, 0, g.active_x, 0, g.active_w, BAR_H, 1, first_title as i16, (themenus - 1) as i16);
    for (ti, (label, _)) in MENUS.iter().enumerate() {
        let last = ti == nt - 1;
        o[first_title + ti] = mk(
            G_TITLE,
            if last { LASTOB } else { 0 },
            label.as_ptr() as u64,
            g.title_x[ti],
            0,
            g.title_w[ti],
            BAR_H,
            if last { 2 } else { (first_title + ti + 1) as i16 },
            -1,
            -1,
        );
    }
    let mut k = themenus + 1;
    let first_box = k;
    let mut last_box = k;
    for (ti, (_, entries)) in MENUS.iter().enumerate() {
        let iw = g.box_w[ti];
        let n_items = entries.len();
        let bi = k;
        last_box = bi;
        k += 1;
        let first_item = k;
        for (ii, entry) in entries.iter().enumerate() {
            let last_item = ii == n_items - 1;
            let last_overall = ti == nt - 1 && last_item;
            o[k] = mk(
                G_STRING,
                if last_overall { LASTOB } else { 0 },
                entry_label(entry).as_ptr() as u64,
                0,
                ii as i16 * BAR_H,
                iw,
                BAR_H,
                if last_item { bi as i16 } else { (k + 1) as i16 },
                -1,
                -1,
            );
            if let Sep(_) = entry {
                o[k].state |= DISABLED;
            }
            k += 1;
        }
        let next_box = if ti == nt - 1 { themenus as i16 } else { k as i16 };
        o[bi] = mk(G_BOX, 0, box_spec, g.box_x[ti], BAR_H, iw, n_items as i16 * BAR_H, next_box, first_item as i16, (k - 1) as i16);
    }
    o[themenus] = mk(G_IBOX, 0, 0, 0, 0, sw, sh, 0, first_box as i16, last_box as i16);
    Box::leak(o.into_boxed_slice()).as_mut_ptr()
}

// ── the rig ────────────────────────────────────────────────────────────────

const MU_KEYBD: i16 = 0x0001;
const MU_BUTTON: i16 = 0x0002;
const MU_M1: i16 = 0x0004;
const MU_MESAG: i16 = 0x0010;
const MU_TIMER: i16 = 0x0020;
const MN_SELECTED: i16 = 10;

/// The installed host, menu tree and AES state are process-wide statics, so
/// the cases in this binary run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());
static RIG: OnceLock<&'static ScriptHost> = OnceLock::new();

fn rig() -> &'static ScriptHost {
    RIG.get_or_init(|| {
        let host: &'static ScriptHost = Box::leak(Box::new(ScriptHost::new()));
        install(host);
        seed_palette(&host.raster);
        host
    })
}

/// Fresh application state with the desktop's bar (boxes carrying
/// `box_spec`) installed, the screen cleared to the desktop's white below a
/// freshly drawn bar, the cursor **off** (so a snapshot holds the AES's
/// pixels alone), the pointer parked on the desktop, nothing queued.
fn fresh(box_spec: u64) -> (&'static ScriptHost, Aespb) {
    let host = rig();
    host.clear();
    let mut pb = Aespb::new();
    pb.call(10, 0, 1, 0); // appl_init
    pb.intin.fill(0);
    pb.call(109, 0, 1, 0); // wind_new

    pb.intin.fill(0);
    pb.intin[0] = 256; // graf_mouse(M_OFF)
    pb.call(78, 1, 1, 0);

    // A white screen under the bar: the box interior is white too, so only
    // the frame, the glyphs and the hover inversion are non-white.
    host.raster.fill_rect(0, 0, SCREEN_W as i16, SCREEN_H as i16, 0, ClipRect::full(SCREEN_W, SCREEN_H));

    pb.intin.fill(0);
    pb.intin[0] = 1; // show
    pb.addrin[0] = desk_menu_tree(box_spec) as u64;
    pb.call(30, 1, 1, 1); // menu_bar
    pb.addrin[0] = 0;
    assert_eq!(pb.intout[0], 1, "menu_bar must install the tree");

    move_to(host, &mut pb, 320, 300, 0);
    drain_messages(&mut pb);
    (host, pb)
}

/// `graf_mkstate` — one AES call, hence one `dsptch()` → one `pump_input`.
fn graf_mkstate(pb: &mut Aespb) -> (i16, i16, i16) {
    pb.call(79, 0, 5, 0);
    (pb.intout[1], pb.intout[2], pb.intout[3])
}

/// Consume every packet still queued.
fn drain(host: &'static ScriptHost, pb: &mut Aespb) {
    for _ in 0..2000 {
        if host.script.lock().unwrap().is_empty() {
            return;
        }
        graf_mkstate(pb);
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
        host.push(pkt(dx, dy, buttons));
        graf_mkstate(pb);
    }
    let (cx, cy) = tos_api::vdi::mouse_position();
    assert_eq!((cx, cy), (tx, ty), "move_to did not reach its target");
}

/// The desktop's main-loop wait (`MU_MESAG | MU_BUTTON | MU_KEYBD`, TeraDesk's
/// click parameters) with a timer. Returns `(fired, EV_KRET, EV_BRET, msg)`.
fn evnt_multi_timed(pb: &mut Aespb, ms: u16) -> (i16, i16, i16, [i16; 8]) {
    let mut buf = Box::new([0i16; 8]);
    pb.intin.fill(0);
    pb.intin[0] = MU_MESAG | MU_BUTTON | MU_KEYBD | MU_TIMER;
    pb.intin[1] = 0x0102;
    pb.intin[2] = 3;
    pb.intin[3] = 0;
    pb.intin[14] = ms as i16;
    pb.intin[15] = 0;
    pb.addrin[0] = buf.as_mut_ptr() as u64;
    pb.call(25, 16, 7, 1);
    pb.addrin[0] = 0;
    (pb.intout[0], pb.intout[5], pb.intout[6], *buf)
}

/// The same wait measuring the pumped time.
fn evnt_multi_timed_elapsed(host: &ScriptHost, pb: &mut Aespb, ms: u16) -> (i16, i16, i16, [i16; 8], u64) {
    let t0 = host.clock.load(Ordering::Relaxed);
    let (fired, kret, bret, msg) = evnt_multi_timed(pb, ms);
    (fired, kret, bret, msg, host.clock.load(Ordering::Relaxed) - t0)
}

/// Empty the owner FIFO.
fn drain_messages(pb: &mut Aespb) {
    for _ in 0..16 {
        let mut buf = Box::new([0i16; 8]);
        pb.intin.fill(0);
        pb.intin[0] = MU_MESAG | MU_TIMER;
        pb.addrin[0] = buf.as_mut_ptr() as u64;
        pb.call(25, 16, 7, 1);
        pb.addrin[0] = 0;
        if pb.intout[0] & MU_MESAG == 0 {
            return;
        }
    }
}

/// Queue a vertical glide of the pointer by `dy_total` in 10 px packets.
fn glide_y(host: &ScriptHost, dy_total: i16, buttons: u8) {
    let step = if dy_total < 0 { -10 } else { 10 };
    let mut left = dy_total;
    while left.abs() > 10 {
        host.push(pkt(0, step, buttons));
        left -= step;
    }
    host.push(pkt(0, left, buttons));
}

/// A press held for a few packets, then released, then idle.
fn click(host: &ScriptHost) {
    host.push(pkt(0, 0, 1));
    for _ in 0..3 {
        host.push(pkt(0, 0, 1));
    }
    host.push(pkt(0, 0, 0));
    for _ in 0..8 {
        host.push(pkt(0, 0, 0));
    }
}

/// Let the pointer rest where it is for `n` packets.
fn dwell(host: &ScriptHost, n: usize) {
    for _ in 0..n {
        host.push(pkt(0, 0, 0));
    }
}

/// `ev_multi` drains the queue once before its arms (A92): one idle packet
/// so a press queued after it is post-arm input.
fn arm_gap(host: &ScriptHost) {
    host.push(pkt(0, 0, 0));
}

/// The bar row plus the tallest drop-down, for "nothing drawn" comparisons.
fn bar_and_boxes_pixels(host: &ScriptHost) -> Vec<u32> {
    let px = host.raster.px.lock().unwrap();
    px[..(SCREEN_W as usize) * (BAR_H as usize + 15 * BAR_H as usize)].to_vec()
}

/// Device black; device white is the palette's pen 0 (`seed_palette`),
/// `rgb565_to_device(0xFFDF)` = `0xFFFBFF`.
const BLACK: u32 = 0;
fn white(host: &ScriptHost) -> u32 {
    host.raster.rgb565_to_device(host.raster.pel_to_device16(0))
}

fn at(snap: &[u32], x: i16, y: i16) -> u32 {
    snap[y as usize * SCREEN_W as usize + x as usize]
}

/// The device complement `fill_pattern`'s `WriteMode::Xor` leaves.
const fn inv(v: u32) -> u32 {
    v ^ 0x00FF_FFFF
}

/// Every pixel of the 1-px ring just outside `(x, y, w, h)`.
fn ring_outside(x: i16, y: i16, w: i16, h: i16) -> Vec<(i16, i16)> {
    let mut v = Vec::new();
    for xx in x - 1..=x + w {
        v.push((xx, y - 1));
        v.push((xx, y + h));
    }
    for yy in y..y + h {
        v.push((x - 1, yy));
        v.push((x + w, yy));
    }
    v
}

/// Compare `a` against `b` over the rows `y0..y1` of columns `x0..x1`:
/// `(same, inverted, other)` pixel counts, where *inverted* means the
/// device complement (`fill_pattern`'s `WriteMode::Xor`).
fn compare_rows(a: &[u32], b: &[u32], x0: i16, x1: i16, y0: i16, y1: i16) -> (usize, usize, usize) {
    let (mut same, mut inv, mut other) = (0, 0, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            let (pa, pb) = (at(a, x, y), at(b, x, y));
            if pa == pb {
                same += 1;
            } else if pa ^ pb == 0x00FF_FFFF {
                inv += 1;
            } else {
                other += 1;
            }
        }
    }
    (same, inv, other)
}

/// Hover *File* (the box comes down), then inside the wait: snapshot 0 with
/// the box down and nothing hovered, glide onto entry 0 (*Open*), snapshot 1,
/// glide onto entry 1 (*Info/Rename…*), snapshot 2, then leave the box to
/// the right and press there — `OUTSIDE_STATE`'s cancel, consumed inside the
/// call — and let the timer end the wait. Returns the three snapshots.
fn hover_two_entries(host: &'static ScriptHost, pb: &mut Aespb) -> [Vec<u32>; 3] {
    let (bx, by, bw, _) = file_box_rect();
    let on_file = (bx + 20, 8);
    move_to(host, pb, on_file.0, on_file.1, 0);
    arm_gap(host); // the head drain
    host.push(snap(0)); // the track's first pump: box down, no item
    host.push(pkt(0, BAR_H + 9 - on_file.1, 0)); // onto entry 0 (y = 28)
    host.push(snap(1));
    host.push(pkt(0, BAR_H, 0)); // onto entry 1 (y = 47)
    host.push(snap(2));
    // Out of the box to the right, then a press there cancels the track.
    host.push(pkt(bw + 40, 0, 0));
    dwell(host, 3);
    click(host);
    let (fired, _kret, _bret, msg, elapsed) = evnt_multi_timed_elapsed(host, pb, 200);
    eprintln!("hover: fired={fired:#06x} msg={msg:?} elapsed={elapsed} box=({bx},{by},{bw})");
    assert_eq!(fired, MU_TIMER, "the cancel is consumed; fired = {fired:#06x}, msg = {msg:?}");
    let s = |i| host.snapshot(i).unwrap_or_else(|| panic!("snapshot {i} was never taken"));
    [s(0), s(1), s(2)]
}

// ── A100 ───────────────────────────────────────────────────────────────────

/// **The desktop's box: outside border, hover touches no frame pixel, and
/// nothing repaints one.** With `0xFF1100` the *File* box `(68,19,158,285)`
/// is framed by the ring one pixel outside it — columns 67 and 226, rows 18
/// and 304 — black in every snapshot. Hovering *Open* inverts the entry's
/// rows 19..37 across the box width and changes no other pixel of the box;
/// moving on to *Info/Rename…* puts rows 19..37 back exactly (the XOR round
/// trip) and inverts rows 38..56. The old `rehilite` painted black lines
/// along the box rectangle's own edges — row 19, row 303, columns 68 and 225
/// — *inside* the frame after every toggle: a second, inner frame, plainly
/// visible on the white interior.
#[test]
fn a100_hovering_an_entry_leaves_the_outside_border_and_the_other_rows_untouched() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (host, mut pb) = fresh(SPEC_OUTSIDE);
    let (bx, by, bw, bh) = file_box_rect();
    assert_eq!((bx, by, bw, bh), (68, 19, 158, 285), "the File box as build_menu lays it out");
    let [s0, s1, s2] = hover_two_entries(host, &mut pb);

    // The frame is the ring outside the box, black throughout.
    for (i, s) in [&s0, &s1, &s2].into_iter().enumerate() {
        for (x, y) in ring_outside(bx, by, bw, bh) {
            assert_eq!(at(s, x, y), BLACK, "snapshot {i}: the outside border at ({x},{y}) (th = -1, desk_rsc.c:294)");
        }
    }
    // Snapshot 0: the interior's edge rows/columns are white where no glyph
    // sits — the frame is not on them.
    let white = white(host);
    assert_eq!(at(&s0, bx + 4, by), white, "row 19 belongs to the interior, not the frame");
    assert_eq!(at(&s0, bx + 4, by + bh - 1), white, "row 303 belongs to the interior");
    assert_eq!(at(&s0, bx, by + 40), white, "column 68 belongs to the interior");
    assert_eq!(at(&s0, bx + bw - 1, by + 40), white, "column 225 belongs to the interior");

    // Snapshot 1: entry 0 inverted across the box, everything else as opened.
    for (name, sn) in [("s0", &s0), ("s1", &s1), ("s2", &s2)] {
        for y in [by - 1, by, by + 9, by + 28, by + bh - 1, by + bh] {
            let row: String = (bx - 2..bx + 60).map(|x| match at(sn, x, y) { 0 => '.', 1 => '#', _ => '?' }).collect();
            eprintln!("{name} y={y}: {row}");
        }
    }
    let (same, inv, other) = compare_rows(&s0, &s1, bx, bx + bw, by, by + BAR_H);
    assert_eq!((same, other), (0, 0), "entry 0 is one XOR of its whole rectangle: {inv} inverted, {same} same, {other} other");
    let (same, inv, other) = compare_rows(&s0, &s1, bx, bx + bw, by + BAR_H, by + bh);
    assert_eq!(
        (inv, other),
        (0, 0),
        "no pixel outside entry 0 changes on the hover: {inv} inverted, {other} other, {same} same — \
         the old frame repaint blackened row 303 and columns 68/225 (mn_do repaints nothing, gemmnlib.c:451-475)"
    );
    assert_eq!(at(&s1, bx + 4, by + bh - 1), white, "row 303 stays white while Open is hovered");
    assert_eq!(at(&s1, bx, by + 40), white, "column 68 stays white while Open is hovered");

    // Snapshot 2: entry 0 back exactly, entry 1 inverted, the rest as opened.
    let (_, inv, other) = compare_rows(&s0, &s2, bx, bx + bw, by, by + BAR_H);
    assert_eq!((inv, other), (0, 0), "entry 0 is restored pixel-exact by the second XOR");
    let (same, _, other) = compare_rows(&s0, &s2, bx, bx + bw, by + BAR_H, by + 2 * BAR_H);
    assert_eq!((same, other), (0, 0), "entry 1 is inverted");
    let (_, inv, other) = compare_rows(&s0, &s2, bx, bx + bw, by + 2 * BAR_H, by + bh);
    assert_eq!((inv, other), (0, 0), "rows below entry 1 are untouched");
}

/// **A box with an inside border gets its frame inverted by the item XOR —
/// in EmuTOS too — and nothing repaints it.** The same tree with the word
/// `build_menu` used before A100 (`(1<<16)|0x1070`, `th = +1`): the frame is
/// row 19, row 303, columns 68 and 225, black when the box opens. Entry 0's
/// rectangle covers row 19 and, in rows 19..37, the two columns; `ob_change`
/// XORs that rectangle whole (`gemoblib.c:1295-1304`: the inset is the
/// *item's* `ob_sst` thickness, 0 for a `G_STRING`), so those frame pixels
/// turn white while *Open* is hovered, and the rest of the frame stays. This
/// is the defect the RSC avoids with the outside border; the old `rehilite`
/// hid it by repainting the frame, which EmuTOS does not do.
#[test]
fn a100_an_inside_border_box_gets_its_frame_inverted_by_the_item_xor_as_in_emutos() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (host, mut pb) = fresh(SPEC_INSIDE);
    let (bx, by, bw, bh) = file_box_rect();
    let [s0, s1, s2] = hover_two_entries(host, &mut pb);

    // Snapshot 0: the frame is on the box rectangle's own edges.
    assert_eq!(at(&s0, bx + 4, by), BLACK, "row 19 is the inside frame");
    assert_eq!(at(&s0, bx + 4, by + bh - 1), BLACK, "row 303 is the inside frame");
    assert_eq!(at(&s0, bx, by + 40), BLACK, "column 68 is the inside frame");
    assert_eq!(at(&s0, bx + bw - 1, by + 40), BLACK, "column 225 is the inside frame");
    let white = white(host);
    assert_eq!(at(&s0, bx - 1, by + 40), white, "nothing outside the box");

    // Snapshot 1: entry 0's XOR took the frame pixels inside its rectangle
    // with it; the frame below entry 0 is untouched.
    assert_eq!(at(&s1, bx + 4, by), inv(BLACK), "row 19 inverted with entry 0 (no frame repaint)");
    assert_eq!(at(&s1, bx, by + 9), inv(BLACK), "column 68 inverted in entry 0's rows");
    assert_eq!(at(&s1, bx + bw - 1, by + 9), inv(BLACK), "column 225 inverted in entry 0's rows");
    assert_eq!(at(&s1, bx, by + 40), BLACK, "column 68 below entry 0 still the frame");
    assert_eq!(at(&s1, bx + 4, by + bh - 1), BLACK, "row 303 still the frame");
    let (same, _, other) = compare_rows(&s0, &s1, bx, bx + bw, by, by + BAR_H);
    assert_eq!((same, other), (0, 0), "entry 0 is one XOR of its whole rectangle, frame pixels included");
    let (_, inv, other) = compare_rows(&s0, &s1, bx, bx + bw, by + BAR_H, by + bh);
    assert_eq!((inv, other), (0, 0), "nothing below entry 0 changes");

    // Snapshot 2: the second XOR restores entry 0 — frame pixels included.
    assert_eq!(at(&s2, bx + 4, by), BLACK, "row 19 restored by the un-hilite XOR");
    let (_, inv, other) = compare_rows(&s0, &s2, bx, bx + bw, by, by + BAR_H);
    assert_eq!((inv, other), (0, 0), "entry 0 restored pixel-exact");
}

// ── A102, point 1: THEACTIVE = the titles' sum ─────────────────────────────

/// Bar row, right of the last title: past *Options*, inside the `gl_hbox` row.
const RIGHT_OF_OPTIONS: (i16, i16) = (400, 8);

/// **The bar row right of *Options* is the application's: timer and keys
/// flow, no track starts.** `THEACTIVE` is `(16, 0, 232, 19)` — the titles'
/// sum at the RSC's 2-cell origin — so `gl_ctwait` ends at x = 248 and the
/// pointer at (400, 8) never hands the mouse to `ctl_pd` (`mchange`,
/// `aes/geminput.c:388-389`). The desktop's wait with a 100 ms timer sees the
/// timer on time while the pointer rests there; a HELP key (`0x62`) struck
/// while it rests there is delivered at once. The A87 pattern on the kernel
/// desktop's tree: with `THEACTIVE` screen-wide (the old `build_menu`) the
/// rest parked the desktop in the Control Manager's track until the pointer
/// left the row.
#[test]
fn a102_the_bar_row_right_of_options_stays_with_the_application() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (host, mut pb) = fresh(SPEC_OUTSIDE);
    let (ax, _, aw, _) = active_rect();
    assert_eq!((ax, aw), (16, 232), "THEACTIVE = 2 cells in, Σ titles wide (desk_rsc.c:255-259, deskmain.c:1534)");
    assert!(RIGHT_OF_OPTIONS.0 >= ax + aw, "the rest position is right of THEACTIVE");

    move_to(host, &mut pb, RIGHT_OF_OPTIONS.0, RIGHT_OF_OPTIONS.1, 0);
    let before = bar_and_boxes_pixels(host);
    dwell(host, 300);
    glide_y(host, 100 - RIGHT_OF_OPTIONS.1, 0);
    let (fired, _kret, _bret, msg, elapsed) = evnt_multi_timed_elapsed(host, &mut pb, 100);
    let (mx, my) = tos_api::vdi::mouse_position();
    eprintln!("a102 rest right of Options: fired={fired:#06x} at=({mx},{my}) msg={msg:?} elapsed={elapsed}");
    assert_eq!(fired, MU_TIMER, "only the timer; fired = {fired:#06x}, msg = {msg:?}");
    assert!(elapsed < 150, "the desktop was parked in the track for {elapsed} ms with the pointer right of Options");
    assert!(my < BAR_H && mx >= ax + aw, "the pointer is still in the row at the timer, at ({mx},{my})");
    assert_eq!(bar_and_boxes_pixels(host), before, "no drop-down / title highlight may have been drawn");

    // HELP while resting there: delivered at once, pointer still in the row.
    drain(host, &mut pb);
    move_to(host, &mut pb, RIGHT_OF_OPTIONS.0, RIGHT_OF_OPTIONS.1, 0);
    dwell(host, 20);
    host.push(key(0x6200)); // HELP make, no ascii
    dwell(host, 300);
    let (fired, kret, _bret, msg, elapsed) = evnt_multi_timed_elapsed(host, &mut pb, 1000);
    let (mx, my) = tos_api::vdi::mouse_position();
    eprintln!("a102 HELP right of Options: fired={fired:#06x} key={kret:#06x} at=({mx},{my}) elapsed={elapsed}");
    assert_eq!(fired, MU_KEYBD, "the key reaches the desktop while the pointer rests in the row; fired = {fired:#06x}, msg = {msg:?}");
    assert_eq!(kret, 0x6200, "EV_KRET = the HELP key");
    assert!(elapsed < 60, "the key was delivered late ({elapsed} ms): a track was parking the desktop");
    assert!(my < BAR_H && mx >= ax + aw, "the pointer is still in the row, at ({mx},{my})");
    drain(host, &mut pb);
}

/// **`THEACTIVE`'s right edge is the boundary.** Resting on the last pixel
/// column of *Options* (x = 247) starts the track — the *Options* box comes
/// down and a press outside cancels it, consumed, timer late; one column
/// further (x = 248) the same rest is the application's: timer on time,
/// nothing drawn.
#[test]
fn a102_the_last_column_of_options_starts_the_track_and_the_next_does_not() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (host, mut pb) = fresh(SPEC_OUTSIDE);
    let (ax, _, aw, _) = active_rect();
    let edge = ax + aw; // 248: first column right of THEACTIVE

    // The Options box `(172, 19, 214, 228)`; its outside border is column 171.
    let g = adjust_menu(SCREEN_W as i16);
    let (ox, ow) = (g.box_x[3], g.box_w[3]);
    assert_eq!((ox, ow), (172, 214));

    move_to(host, &mut pb, edge, 8, 0);
    let before = bar_and_boxes_pixels(host);
    dwell(host, 50);
    host.push(snap(0)); // mid-rest: what is on screen while the pointer rests at x = 248
    dwell(host, 250);
    glide_y(host, 100 - 8, 0);
    let (fired, _, _, msg, elapsed) = evnt_multi_timed_elapsed(host, &mut pb, 100);
    eprintln!("a102 edge x={edge}: fired={fired:#06x} msg={msg:?} elapsed={elapsed}");
    assert_eq!(fired, MU_TIMER, "x = {edge} is outside THEACTIVE; fired = {fired:#06x}, msg = {msg:?}");
    assert!(elapsed < 150, "x = {edge} is outside THEACTIVE, no track may park the desktop ({elapsed} ms)");
    let s0 = host.snapshot(0).expect("the mid-rest snapshot");
    assert!(s0[..before.len()] == before[..], "nothing drawn at x = {edge} while the pointer rested there");
    assert_eq!(bar_and_boxes_pixels(host), before, "nothing drawn at x = {edge}");

    drain(host, &mut pb);
    drain_messages(&mut pb);
    move_to(host, &mut pb, 320, 300, 0);
    move_to(host, &mut pb, edge - 1, 8, 0); // Options' last column: the track starts
    arm_gap(host);
    host.push(snap(1)); // the track's first pump: the Options box is down
    // Down through the Options box and out onto the desktop, then a press
    // there: OUTSIDE_STATE cancel, consumed.
    glide_y(host, 300 - 8, 0);
    dwell(host, 3);
    click(host);
    let (fired2, _, _, msg2, elapsed2) = evnt_multi_timed_elapsed(host, &mut pb, 100);
    eprintln!("a102 edge x={}: fired={fired2:#06x} msg={msg2:?} elapsed={elapsed2}", edge - 1);
    assert_eq!(fired2, MU_TIMER, "x = {} is Options' last column: the track ran and consumed the cancel; msg = {msg2:?}", edge - 1);
    let s1 = host.snapshot(1).expect("the in-track snapshot");
    assert_eq!(at(&s1, ox - 1, 40), BLACK, "the Options box was down at x = {}: its outside border at column {}", edge - 1, ox - 1);
    assert_eq!(at(&s1, ox + ow, 40), BLACK, "… and at column {}", ox + ow);
    assert_eq!(at(&s1, ox, 40), white(host), "… with a white interior at column {ox}");
    assert!(s1[..before.len()] != before[..], "the Options box must have been down at x = {}", edge - 1);
    assert_eq!(bar_and_boxes_pixels(host), before, "the cancel put the screen back");
    drain(host, &mut pb);
}

/// **Guard — a title still starts the track.** Hover onto *File*, slide onto
/// *Open*, press: one `MN_SELECTED (File, Open)`, no `MU_BUTTON`.
#[test]
fn a102_a_title_inside_theactive_still_starts_the_track() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (host, mut pb) = fresh(SPEC_OUTSIDE);
    let (bx, by, _, _) = file_box_rect();
    move_to(host, &mut pb, bx + 20, 8, 0);
    glide_y(host, by + 9 - 8, 0); // onto Open
    click(host);
    let (fired, _, bret, msg, _) = evnt_multi_timed_elapsed(host, &mut pb, 200);
    eprintln!("a102 title track: fired={fired:#06x} bret={bret} msg={msg:?}");
    assert_eq!(fired & MU_MESAG, MU_MESAG, "fired = {fired:#06x}");
    let item_open = 12; // THEMENUS 7, Desk box 8 + entries 9/10, File box 11, Open 12
    assert_eq!((msg[0], msg[3], msg[4]), (MN_SELECTED, TITLE_FILE, item_open), "msg = {msg:?}");
    assert_eq!(fired & MU_BUTTON, 0, "the selecting press is the Control Manager's; fired = {fired:#06x}");
    drain(host, &mut pb);
}

// ── A102, point 3: menu_popup's no-pick branch ─────────────────────────────

/// A three-item pop-up: root `G_BOX` 112×57, items at y 0/19/38.
fn popup_tree() -> *mut Obj {
    static A: &[u8] = b"Alpha\0";
    static B: &[u8] = b"Beta\0";
    static C: &[u8] = b"Gamma\0";
    let s = |p: &[u8], y, next, last| Obj {
        next,
        head: -1,
        tail: -1,
        otype: G_STRING,
        flags: if last { LASTOB } else { 0 },
        state: 0,
        spec: p.as_ptr() as u64,
        x: 0,
        y,
        w: 112,
        h: BAR_H,
    };
    let tree: Box<[Obj; 4]> = Box::new([
        Obj { next: -1, head: 1, tail: 3, otype: G_BOX, flags: 0, state: 0, spec: SPEC_OUTSIDE, x: 0, y: 0, w: 112, h: 3 * BAR_H },
        s(A, 0, 2, false),
        s(B, BAR_H, 3, false),
        s(C, 2 * BAR_H, 0, true),
    ]);
    Box::leak(tree).as_mut_ptr()
}

/// The `MENU` struct at the LP64 layout (`mn_tree` @0, four WORDs @8).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct MenuRec {
    mn_tree: u64,
    mn_menu: i16,
    mn_item: i16,
    mn_scroll: i16,
    mn_keystate: i16,
}

/// **No pick writes `mn_keystate` alone.** `mn_popup`'s `else` branch is one
/// statement, `mdata->mn_keystate = 0; /* TOS always sets mn_keystate */`
/// (`aes/gemmnext.c:345-349`); `mn_tree`, `mn_menu`, `mn_item` and
/// `mn_scroll` of the output block are not written. The output block here is
/// filled with sentinels that nothing in the call could produce, the pointer
/// rests outside the box, and the press there is the no-pick. NewTOS wrote
/// all five fields on that branch (tree/menu/scroll as input, item −1).
#[test]
fn a102_menu_popup_no_pick_writes_only_mn_keystate() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (host, mut pb) = fresh(SPEC_OUTSIDE);
    let tree = popup_tree();
    let min = Box::new(MenuRec { mn_tree: tree as u64, mn_menu: 0, mn_item: 1, mn_scroll: 7, mn_keystate: 0x55 });
    let sentinel = MenuRec { mn_tree: 0xDEAD_BEEF_CAFE_F00D, mn_menu: 0x1111, mn_item: 0x2222, mn_scroll: 0x3333, mn_keystate: 0x5555 };
    let mut mout = Box::new(sentinel);

    // The pointer rests at (320,300); the box is asked for at (100,100), so
    // the press lands outside it: handle_popup returns on the press with
    // nothing under it (gemmnext.c:290-291) — the no-pick branch.
    click(host);
    pb.intin.fill(0);
    pb.intin[0] = 100;
    pb.intin[1] = 100;
    pb.addrin[0] = &*min as *const MenuRec as u64;
    pb.addrin[1] = &mut *mout as *mut MenuRec as u64;
    pb.call(36, 2, 1, 2);
    pb.addrin[0] = 0;
    pb.addrin[1] = 0;
    let rc = pb.intout[0];
    eprintln!("a102 popup no pick: rc={rc} out={mout:?}");
    assert_eq!(rc, 0, "a press outside the box → the no-pick branch");
    assert_eq!(
        *mout,
        MenuRec { mn_keystate: 0, ..sentinel },
        "no pick: mn_keystate = 0 and nothing else is written (gemmnext.c:347); out = {mout:?}"
    );
    drain(host, &mut pb);
}
