//! Reproduction rig for the real-Pi-4 **window-resize drawing artefact**.
//!
//! The gesture the user reports: dragging a TeraDesk window's *sizer* makes
//! square holes appear in the desktop background and travel with the drag;
//! dragging the same window by its *mover* is clean.
//!
//! The rig drives the AES exactly as the board does — a [`ScriptHost`] whose
//! `pump_input` is the only thing that advances the pointer (the USB-HID mouse
//! is polled from `Host::pump_input`, `kernel/src/host_env.rs:41`), feeding
//! packets through `tos_api::vdi::feed_relative` (our `mouse_int`) — and
//! snapshots the whole framebuffer once per drag frame, so every pixel the AES
//! writes during the gesture is recoverable and comparable.
//!
//! Harness shape (backend + host) copied from `aes_button_dispatch.rs`.

#![allow(clippy::too_many_arguments)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

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
    /// Op counter + "dump every Nth op to PPM" trip, so the *mid-frame*
    /// screen — what the display scans out while the AES is still drawing —
    /// can be looked at, not just the settled end-of-frame state.
    ops: AtomicUsize,
    op_dump: Mutex<Option<(String, usize)>>,
}

impl BufBackend {
    fn new() -> Self {
        BufBackend {
            px: Mutex::new(vec![0; SCREEN_W as usize * SCREEN_H as usize]),
            palette: Mutex::new([0; 256]),
            req: Mutex::new([(0, 0, 0); 256]),
            log: Mutex::new(Vec::new()),
            logging: AtomicBool::new(false),
            ops: AtomicUsize::new(0),
            op_dump: Mutex::new(None),
        }
    }

    fn note(&self, s: String) {
        if self.logging.load(Ordering::Relaxed) {
            self.log.lock().unwrap().push(s);
        }
        let n = self.ops.fetch_add(1, Ordering::Relaxed);
        let dump = self.op_dump.lock().unwrap().clone();
        if let Some((dir, every)) = dump {
            if every > 0 && n.is_multiple_of(every) {
                dump_ppm(&format!("{dir}/op-{n:05}.ppm"), &self.snapshot());
            }
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
    /// One framebuffer snapshot per `pump_input` while `filming` is set — a
    /// drag frame is exactly one loop turn of `drag_size`/`drag_move`.
    film: Mutex<Vec<(Vec<u32>, i16, i16)>>,
    filming: AtomicBool,
}

impl ScriptHost {
    fn new(script: &[Packet]) -> Self {
        ScriptHost {
            raster: BufBackend::new(),
            script: Mutex::new(script.iter().copied().collect()),
            clock: AtomicU64::new(0),
            film: Mutex::new(Vec::new()),
            filming: AtomicBool::new(false),
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
        if self.filming.load(Ordering::Relaxed) {
            let s = self.raster.snapshot();
            let (mx, my) = tos_api::vdi::mouse_position();
            self.film.lock().unwrap().push((s, mx, my));
        }
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

/// Seed the backend's pseudo-palette the way the VDI dispatcher does before a
/// colour-sensitive op (`ACTIVE_PALETTE`, `kernel/src/vdi_backend.rs:11-21`):
/// EmuTOS' `videl_palette1` for pens 0..15, as RGB555-in-565 words. Without
/// this every pen resolves to 0 and the whole frame renders black.
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

fn dump_ppm(path: &str, px: &[u32]) {
    use std::io::Write;
    let mut f = std::fs::File::create(path).unwrap();
    write!(f, "P6\n{} {}\n255\n", SCREEN_W, SCREEN_H).unwrap();
    let mut buf = Vec::with_capacity(px.len() * 3);
    for &v in px {
        buf.push(((v >> 16) & 0xFF) as u8);
        buf.push(((v >> 8) & 0xFF) as u8);
        buf.push((v & 0xFF) as u8);
    }
    f.write_all(&buf).unwrap();
}

fn dump_dir() -> Option<String> {
    std::env::var("DRAG_DUMP_DIR").ok()
}

/// Consume every packet still queued, so a later `move_to` is not driving the
/// pointer through a backlog.
fn drain(host: &'static ScriptHost, pb: &mut Aespb) {
    for _ in 0..64 {
        if host.script.lock().unwrap().is_empty() {
            return;
        }
        pb.call(79, 0, 5, 0);
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
        pb.call(79, 0, 5, 0); // graf_mkstate — ends in dsptch() → pump_input()
    }
}

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

/// The desktop-background tree an application registers with
/// `wind_set(0, WF_NEWDESK, tree, root)` — TeraDesk does exactly this
/// (`sdk/apps/teradesk/icon.c:2511`). Root = a patterned box over the whole
/// screen, plus three icon-sized boxes that sit *behind* the test window.
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

struct Rig {
    host: &'static ScriptHost,
    pb: Aespb,
    wh: i16,
}

/// The window under test, in border coordinates.
const WX: i16 = 120;
const WY: i16 = 80;
const WW: i16 = 320;
const WH: i16 = 220;

fn setup(kind: i16) -> Rig {
    let host: &'static ScriptHost = Box::leak(Box::new(ScriptHost::new(&[])));
    install(host);
    let mut pb = Aespb::new();
    pb.call(10, 0, 1, 0); // appl_init
    seed_palette(&host.raster);

    // Register the desktop background tree, then let the AES paint it.
    let tree = desk_tree();
    pb.intin.fill(0);
    pb.intin[0] = 0; // DESKWH
    pb.intin[1] = 14; // WF_NEWDESK
    pb.intin[2] = 0; // root object
    pb.addrin[0] = tree.as_ptr() as u64;
    pb.call(105, 6, 1, 1); // wind_set

    pb.intin.fill(0);
    pb.intin[0] = kind;
    pb.intin[1] = 0;
    pb.intin[2] = 0;
    pb.intin[3] = SCREEN_W as i16;
    pb.intin[4] = SCREEN_H as i16;
    pb.call(100, 5, 1, 0); // wind_create
    let wh = pb.intout[0];
    assert!(wh > 0, "wind_create must give a handle, got {wh}");

    pb.intin.fill(0);
    pb.intin[0] = wh;
    pb.intin[1] = WX;
    pb.intin[2] = WY;
    pb.intin[3] = WW;
    pb.intin[4] = WH;
    pb.call(101, 5, 1, 0); // wind_open

    pb.intin.fill(0);
    pb.intin[0] = 257; // graf_mouse(M_ON)
    pb.call(78, 1, 1, 0);

    Rig { host, pb, wh }
}

fn evnt_multi(pb: &mut Aespb) {
    pb.intin.fill(0);
    pb.intin[0] = MU_MESAG | MU_BUTTON | MU_KEYBD;
    pb.intin[1] = 0x0102;
    pb.intin[2] = 3;
    pb.intin[3] = 0;
    pb.addrin[0] = 0;
    pb.call(25, 16, 7, 1);
}

/// EmuTOS' sizer feedback is two XOR outlines and nothing else: `gr_rubwind`
/// (`aes/gemgrlib.c:199-220`) loops `gr_clamp` + `gr_wait`, and `gr_wait` draws
/// `gr_draw` (`:155-166`: the rubber box, plus the work-area box offset by
/// `poff`), waits, and complements it away again. Nothing is repainted and the
/// window is not resized — the owner does that when it answers `WM_SIZED`.
///
/// The regression this pins down: the former live-resize repainted the desktop
/// background (`w_drawdesk` — the background box **and every icon**) over the
/// whole window rectangle on every drag frame, then redrew the frame on top.
/// The window's area therefore showed the background-behind-the-window, icons
/// included, for the length of each frame's redraw — the square "holes" that
/// travel with the drag, reported from a real Pi 4 with TeraDesk. Here that
/// shows up as ~33 000 changed pixels per frame inside the window, all of them
/// off the rubber band.
#[test]
fn sizer_drag_draws_only_the_rubber_band() {
    let kind = NAME | CLOSER | FULLER | MOVER | SIZER;
    let mut rig = setup(kind);
    let host = rig.host;

    let dd = dump_dir();
    if let Some(d) = &dd {
        dump_ppm(&format!("{d}/00-open.ppm"), &host.raster.snapshot());
    }

    // The two rectangles the rubber band is built from: the window's border
    // rect (WF_CURRXYWH) and its work rect (WF_WORKXYWH); `hctl_window` derives
    // `poff = work - border` from exactly these (`aes/gemctrl.c:247-251`).
    let curr = wind_get_rect(&mut rig.pb, rig.wh, 5);
    let work = wind_get_rect(&mut rig.pb, rig.wh, 4);
    let poff = (work.0 - curr.0, work.1 - curr.1, work.2 - curr.2, work.3 - curr.3);
    // Minimum size (`gemctrl.c:252-257`): the window has top gadgets, so
    // `wm = gl_wbox * 4`; it has no vertical gadgets, so `hm = gl_hchar`.
    let (wmin, hmin) = (19 * 4, 16);

    move_to(host, &mut rig.pb, WX + WW - 4, WY + WH - 4, 0);
    host.push(Packet { dx: 0, dy: 0, buttons: 1 });
    rig.pb.call(79, 0, 5, 0); // graf_mkstate — observe the press

    let before = host.raster.snapshot();
    let (start_mx, start_my) = tos_api::vdi::mouse_position();
    if let Some(d) = &dd {
        dump_ppm(&format!("{d}/01-armed.ppm"), &before);
    }

    for _ in 0..6 {
        host.push(Packet { dx: 10, dy: 8, buttons: 1 });
    }
    for _ in 0..6 {
        host.push(Packet { dx: -10, dy: -8, buttons: 1 });
    }
    host.push(Packet { dx: 0, dy: 0, buttons: 0 }); // release
    for _ in 0..8 {
        host.push(Packet { dx: 0, dy: 0, buttons: 0 });
    }

    host.filming.store(true, Ordering::Relaxed);
    if let Some(d) = &dd {
        std::fs::create_dir_all(format!("{d}/size")).unwrap();
        *host.raster.op_dump.lock().unwrap() = Some((format!("{d}/size"), 4));
    }
    evnt_multi(&mut rig.pb);
    *host.raster.op_dump.lock().unwrap() = None;
    host.filming.store(false, Ordering::Relaxed);

    let film: Vec<(Vec<u32>, i16, i16)> = host.film.lock().unwrap().drain(..).collect();
    eprintln!("== sizer drag: {} frames", film.len());
    let changed = report("size", &before, &film, &dd);
    assert!(film.len() > 4, "the scripted drag must produce several frames");

    // Every pixel that differs from the pre-drag screen must sit on one of the
    // two rubber-band outlines, or under the mouse sprite (16x16 from the
    // hotspot, which `gr_wait`'s `gsx_mon` redraws on top of the band).
    // Every position the mouse sprite occupied during the gesture, including
    // the one it was drawn at in `before`: those 16x16 boxes hold sprite
    // pixels in one image and background in the other, which is the sprite
    // moving, not a repaint. (ARROW's hotspot is (0,0), `aes/mform.rsc`.)
    let mut sprite: Vec<(i16, i16)> = vec![(start_mx, start_my)];
    sprite.extend(film.iter().map(|(_, mx, my)| (*mx, *my)));
    let mut stray: Vec<(usize, i16, i16)> = Vec::new();
    for (fi, x, y) in changed {
        let (mx, my) = (film[fi].1, film[fi].2);
        let bw = (mx - curr.0 + 1).max(wmin);
        let bh = (my - curr.1 + 1).max(hmin);
        let on_band = on_outline(x, y, curr.0, curr.1, bw, bh)
            || on_outline(x, y, curr.0 + poff.0, curr.1 + poff.1, bw + poff.2, bh + poff.3);
        let on_cursor = sprite.iter().any(|(sx, sy)| x >= *sx && x < *sx + 16 && y >= *sy && y < *sy + 16);
        if !on_band && !on_cursor {
            stray.push((fi, x, y));
        }
    }
    if !stray.is_empty() {
        for e in stray.iter().take(20) {
            eprintln!("  stray pixel: frame {} ({},{})", e.0, e.1, e.2);
        }
    }
    assert!(
        stray.is_empty(),
        "the sizer drag painted {} pixels that are not part of the XOR rubber band: \
         EmuTOS `gr_rubwind`/`gr_wait` (aes/gemgrlib.c:169-220) touches nothing but the \
         two outline boxes — no desktop background, no window frame, no work area",
        stray.len()
    );

    // `gr_wait` complements the band away before it returns, and nothing else
    // was drawn: on release the screen is bit-identical to what it was.
    let after = host.raster.snapshot();
    let diff = after.iter().zip(before.iter()).filter(|(a, b)| a != b).count();
    if let Some(d) = &dd {
        dump_ppm(&format!("{d}/99-after.ppm"), &after);
    }
    assert_eq!(
        diff, 0,
        "after the button came up the rubber band must be fully erased and the \
         screen unchanged (the window is resized by the owner's wind_set(WF_CURRXYWH) \
         in answer to WM_SIZED, not by the drag)"
    );

    // And the window itself is untouched by the drag — EmuTOS' AES never
    // resizes it; `ct_msgup` only reports the dragged size to the owner.
    let after_curr = wind_get_rect(&mut rig.pb, rig.wh, 5);
    assert_eq!(
        after_curr, curr,
        "gr_rubwind must not move or resize the window (aes/gemgrlib.c:199-220 \
         writes only *pwend/*phend); the owner does that on WM_SIZED"
    );
}

/// `wind_get(handle, field)` → `(x, y, w, h)`.
fn wind_get_rect(pb: &mut Aespb, handle: i16, field: i16) -> (i16, i16, i16, i16) {
    pb.intin.fill(0);
    pb.intin[0] = handle;
    pb.intin[1] = field;
    pb.call(104, 2, 5, 0);
    (pb.intout[1], pb.intout[2], pb.intout[3], pb.intout[4])
}

/// The 1-pixel outline of `(x, y, w, h)`, as [`set`] membership.
fn on_outline(px: i16, py: i16, x: i16, y: i16, w: i16, h: i16) -> bool {
    if w <= 0 || h <= 0 {
        return false;
    }
    let (xr, yb) = (x + w - 1, y + h - 1);
    let in_x = px >= x && px <= xr;
    let in_y = py >= y && py <= yb;
    (in_x && in_y) && (px == x || px == xr || py == y || py == yb)
}

/// Per drag frame: how many pixels changed against the pre-drag screen, and
/// where. `film` entries are `(screen, mouse_x, mouse_y)` sampled at the top
/// of each `pump_input`, i.e. once per drag-loop turn.
fn report(tag: &str, before: &[u32], film: &[(Vec<u32>, i16, i16)], dd: &Option<String>) -> Vec<(usize, i16, i16)> {
    let mut all_changed = Vec::new();
    for (i, (f, mx, my)) in film.iter().enumerate() {
        let mut changed = 0usize;
        let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
        for y in 0..SCREEN_H as i16 {
            for x in 0..SCREEN_W as i16 {
                let k = y as usize * SCREEN_W as usize + x as usize;
                if f[k] != before[k] {
                    changed += 1;
                    all_changed.push((i, x, y));
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        eprintln!(
            "  [{tag}] frame {i:2}: mouse=({mx},{my}) changed={changed:6} bbox=({x0},{y0})-({x1},{y1})"
        );
        if let Some(d) = dd {
            dump_ppm(&format!("{d}/{tag}-frame-{i:02}.ppm"), f);
        }
    }
    all_changed
}
