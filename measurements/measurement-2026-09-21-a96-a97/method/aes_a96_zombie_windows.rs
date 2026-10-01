//! A96 — **what survives a program: nothing of its windows.**
//!
//! EmuTOS sweeps the window table after every program the shell loop ran:
//! `sh_ldapp` → `dos_exec(PE_LOADGO, …)` returns → `if (gl_shgem) wm_new();`
//! (`references/emutos/aes/gemshlib.c:645,676-677`), and `wm_new`
//! (`aes/gemwmlib.c:1875-1893`) drains the `wind_update` locks, then closes
//! every open window and deletes every window in use, handles 1..NUM_WIN. A
//! program that crashed or never bothered to `wind_close`/`wind_delete`
//! leaves nothing behind for the next occupant of the AES.
//!
//! NewTOS's shell loop does that (`kernel/src/shell_loop.rs`, the `#Z`
//! autorun path). The built-in desktop's double-click launch does **not** go
//! through the shell loop: `run_gem_app` (`kernel/src/desk/main.rs`) runs the
//! program nested in the desktop's event loop and `desktop_resume` repaints
//! the desktop from the shared window table — with the dead program's windows
//! still in it, composited over the desktop as zombies (A83 finding b).
//!
//! The fix is the same sweep at the point the desktop regains control —
//! `tos_api::aes::window::wm_new_keep_desktop()`, `wm_new` minus the windows
//! the desktop identity owns: EmuTOS's `wm_new` takes those too, but there
//! the desktop restarts through `sh_main` and rebuilds them from `cnx_get`
//! (`desk/deskmain.c:2055`); this desktop resumes in place (gap G2) and keeps
//! them. The sweep also drops a `WF_NEWDESK` tree the program registered
//! (EmuTOS resets `gl_newdesk` in `wm_init` before the desktop restarts,
//! `gemwmlib.c:1300`; the tree is a VA of an address space that is gone, A97).
//!
//! The tests drive the launch boundary through the LP64 AES dispatcher in
//! the order `desktop_relinquish` / the launcher / `desktop_resume` execute
//! it (`kernel/src/desk/main.rs`, `kernel/src/launch.rs`) and then ask:
//! after the program is gone, **is its window gone** — out of the table,
//! invisible to `wind_find`, and not one pixel of it in the desktop's
//! whole-screen composite — while the desktop's own window is still there?
//!
//! Harness: the process fixture of `aes_pd_by_slot.rs`, a pixel-keeping
//! backend so the composite can be inspected.

#![allow(clippy::too_many_arguments)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, Once};

use tos_api::aes::object::Object;
use tos_api::bdos::process::{self, Abi, MAX_PROCESSES, PROCESS_TABLE};
use tos_api::host::Host;
use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};
use tos_types::BasePage;

const SCREEN_W: u16 = 640;
const SCREEN_H: u16 = 480;

// ── a raster backend that keeps its pixels ─────────────────────────────────

/// The pseudo-palette the VDI dispatcher installs before a colour-sensitive
/// op (`ACTIVE_PALETTE`, `kernel/src/vdi_backend.rs`): EmuTOS' `videl_palette1`
/// for pens 0..15 as RGB555-in-565 words (`aes_window_content.rs`'s seed).
/// Seeded at construction so pen 0 (white) and pen 1 (black) are distinct
/// device values from the first fill on, whichever test runs first.
fn seed_palette() -> [u16; 256] {
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
    pal
}

struct BufBackend {
    px: Mutex<Vec<u32>>,
    palette: Mutex<[u16; 256]>,
}

impl BufBackend {
    fn new() -> Self {
        BufBackend {
            px: Mutex::new(vec![0xFFFF_FFFF; SCREEN_W as usize * SCREEN_H as usize]),
            palette: Mutex::new(seed_palette()),
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

    /// The device value of VDI pen `pel`.
    fn device(&self, pel: Pel) -> u32 {
        self.rgb565_to_device(self.pel_to_device16(pel))
    }

    /// How many pixels of `r` are **not** `v`.
    fn count_not(&self, r: (i16, i16, i16, i16), v: u32) -> usize {
        let px = self.px.lock().unwrap();
        let mut n = 0;
        for y in r.1..r.1 + r.3 {
            for x in r.0..r.0 + r.2 {
                if Self::inside(x, y) && px[Self::idx(x, y)] != v {
                    n += 1;
                }
            }
        }
        n
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
    fn set_color(&self, _index: u16, _r: u16, _g: u16, _b: u16) {}
    fn query_color(&self, _index: u16) -> (u16, u16, u16) {
        (0, 0, 0)
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
        let v = self.device(pel);
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
        mode: WriteMode,
        clip: ClipRect,
        mask: u16,
        _last: bool,
    ) -> u16 {
        let fg = self.device(pel);
        let (ax, ay, bx, by) = if x1 < x0 { (x1, y1, x0, y0) } else { (x0, y0, x1, y1) };
        let put = |x: i16, y: i16| match mode {
            WriteMode::Xor => {
                let v = self.get_device_pixel(x, y);
                self.put(x, y, v ^ 0x00FF_FFFF, clip);
            }
            _ => self.put(x, y, fg, clip),
        };
        if ay == by {
            for x in ax.min(bx)..=ax.max(bx) {
                put(x, ay);
            }
        } else if ax == bx {
            for y in ay.min(by)..=ay.max(by) {
                put(ax, y);
            }
        } else {
            let (mut x, mut y) = (ax as i32, ay as i32);
            let (xe, ye) = (bx as i32, by as i32);
            let (dx, dy) = ((xe - x).abs(), -(ye - y).abs());
            let sy = if y < ye { 1 } else { -1 };
            let mut err = dx + dy;
            loop {
                put(x as i16, y as i16);
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
        }
        mask
    }
    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill, clip: ClipRect) {
        let fg = self.device(fill.fg);
        let bg = self.device(fill.bg);
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
        let v = self.device(fg);
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
        let f = self.device(fg);
        let b = self.device(bg);
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

// ── the host ──────────────────────────────────────────────────────────────

struct PixelHost {
    raster: BufBackend,
    clock: AtomicU64,
}

impl Host for PixelHost {
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
        let n = self.clock.fetch_add(1, Ordering::Relaxed);
        assert!(n < 1_000_000, "a trap is spinning");
    }
}

unsafe impl Sync for PixelHost {}
unsafe impl Send for PixelHost {}

static HOST_ONCE: Once = Once::new();
static HOST: Mutex<Option<&'static PixelHost>> = Mutex::new(None);

fn install_host() {
    HOST_ONCE.call_once(|| {
        let host: &'static PixelHost =
            Box::leak(Box::new(PixelHost { raster: BufBackend::new(), clock: AtomicU64::new(0) }));
        let href: &'static &'static dyn Host = Box::leak(Box::new(host as &'static dyn Host));
        tos_api::host::install_host(href);
        *HOST.lock().unwrap() = Some(host);
    });
}

fn host() -> &'static PixelHost {
    HOST.lock().unwrap().expect("host installed by lock()")
}

/// One lock over every process-wide static the tests touch.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    install_host();
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

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

    fn call(&mut self, opcode: i16, in_len: i16, out_len: i16, ain_len: i16) -> i16 {
        self.contrl[0] = opcode;
        self.contrl[1] = in_len;
        self.contrl[2] = out_len;
        self.contrl[3] = ain_len;
        self.contrl[4] = 0;
        self.intout.fill(0);
        // SAFETY: all six arrays are live, LP64, and cover the declared lengths.
        assert!(unsafe { tos_api::aes::dispatch(self.block.as_ptr() as u64, ABI) }.is_none());
        self.intout[0]
    }
}

// AES opcodes used here.
const APPL_INIT: i16 = 10;
const APPL_EXIT: i16 = 19;
const FORM_DIAL: i16 = 51;
const WIND_CREATE: i16 = 100;
const WIND_OPEN: i16 = 101;
const WIND_CLOSE: i16 = 102;
const WIND_DELETE: i16 = 103;
const WIND_SET: i16 = 105;
const WIND_FIND: i16 = 106;
const WIND_UPDATE: i16 = 107;

/// Window kinds (`aesdefs.h`).
const NAME: i16 = 0x0001;
const CLOSER: i16 = 0x0002;
const MOVER: i16 = 0x0008;

/// `wind_set` field / `form_dial` type / `wind_update` mode (`aesdefs.h`).
const WF_NEWDESK: i16 = 14;
const FMD_FINISH: i16 = 3;
const BEG_UPDATE: i16 = 1;
const BEG_MCTRL: i16 = 3;

/// The desktop window handle, and the desktop's AES identity — the reserved
/// descriptor index past the GEMDOS slots (`pd::DESKTOP`).
const DESKWH: i16 = 0;
const DESKTOP_IDENTITY: usize = MAX_PROCESSES;

/// `G_BOX` (`obdefs.h`) and the `ob_spec` of a solid white box (`gr_crack`:
/// pattern 7 in bits 4..6, interior colour 0, thickness 0).
const G_BOX: u16 = 20;
const WHITE_SOLID: u64 = 0x70;

/// The desktop's window and the program's: disjoint, both clear of the menu
/// row, the program's on the bare desktop.
const DESK_WIN: (i16, i16, i16, i16) = (330, 260, 240, 160);
const PROG_WIN: (i16, i16, i16, i16) = (40, 60, 240, 160);

fn centre(r: (i16, i16, i16, i16)) -> (i16, i16) {
    (r.0 + r.2 / 2, r.1 + r.3 / 2)
}

/// `r` shrunk by `n` on every side.
fn inset(r: (i16, i16, i16, i16), n: i16) -> (i16, i16, i16, i16) {
    (r.0 + n, r.1 + n, r.2 - 2 * n, r.3 - 2 * n)
}

// ── AES calls ─────────────────────────────────────────────────────────────

fn appl_init(pb: &mut Aespb) -> i16 {
    pb.intin.fill(0);
    pb.call(APPL_INIT, 0, 1, 0)
}

fn appl_exit(pb: &mut Aespb) -> i16 {
    pb.intin.fill(0);
    pb.call(APPL_EXIT, 0, 1, 0)
}

fn wind_create(pb: &mut Aespb) -> i16 {
    pb.intin.fill(0);
    pb.intin[0] = NAME | CLOSER | MOVER;
    pb.intin[1] = 0;
    pb.intin[2] = 19;
    pb.intin[3] = SCREEN_W as i16;
    pb.intin[4] = SCREEN_H as i16 - 19;
    let h = pb.call(WIND_CREATE, 5, 1, 0);
    assert!(h > 0, "wind_create must give a handle, got {h}");
    h
}

fn wind_open(pb: &mut Aespb, h: i16, r: (i16, i16, i16, i16)) {
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.intin[1] = r.0;
    pb.intin[2] = r.1;
    pb.intin[3] = r.2;
    pb.intin[4] = r.3;
    assert_eq!(pb.call(WIND_OPEN, 5, 1, 0), 1, "wind_open failed");
}

fn wind_close(pb: &mut Aespb, h: i16) {
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.call(WIND_CLOSE, 1, 1, 0);
}

fn wind_delete(pb: &mut Aespb, h: i16) {
    pb.intin.fill(0);
    pb.intin[0] = h;
    pb.call(WIND_DELETE, 1, 1, 0);
}

fn wind_find(pb: &mut Aespb, x: i16, y: i16) -> i16 {
    pb.intin.fill(0);
    pb.intin[0] = x;
    pb.intin[1] = y;
    pb.call(WIND_FIND, 2, 1, 0)
}

fn wind_update(pb: &mut Aespb, mode: i16) {
    pb.intin.fill(0);
    pb.intin[0] = mode;
    pb.call(WIND_UPDATE, 1, 1, 0);
}

/// `wind_set(DESKWH, WF_NEWDESK, tree, root)` in the native form the kernel
/// desktop sends (tree in `addr_in[0]`, root in `int_in[2]`); a null tree
/// un-registers.
fn wind_set_newdesk(pb: &mut Aespb, tree: u64, root: i16) {
    pb.intin.fill(0);
    pb.intin[0] = DESKWH;
    pb.intin[1] = WF_NEWDESK;
    pb.intin[2] = root;
    pb.addrin[0] = tree;
    pb.call(WIND_SET, 3, 1, 1);
    pb.addrin[0] = 0;
}

/// `form_dial(FMD_FINISH, 0,0,0,0, 0,0,W,H)` — the desktop's whole-screen
/// composite (`redraw_desktop`, `kernel/src/desk/redraw.rs`).
fn form_dial_finish(pb: &mut Aespb) {
    pb.intin.fill(0);
    pb.intin[0] = FMD_FINISH;
    pb.intin[5] = 0;
    pb.intin[6] = 0;
    pb.intin[7] = SCREEN_W as i16;
    pb.intin[8] = SCREEN_H as i16;
    pb.call(FORM_DIAL, 9, 1, 0);
}

// ── a process slot the host can "run" ─────────────────────────────────────

/// A claimed GEMDOS process slot with a basepage page of its own (the
/// `aes_pd_by_slot.rs` fixture). Released on drop through the kernel's own
/// slot release, so a failing test leaves nothing behind.
struct Proc {
    slot: usize,
    page: *mut [u8; 4096],
}

impl Proc {
    fn new() -> Self {
        let slot = process::allocate_slot().expect("a free process slot");
        let page = Box::into_raw(Box::new([0u8; 4096]));
        PROCESS_TABLE[slot].basepage_kva.store(page.cast::<BasePage>(), Ordering::Release);
        Proc { slot, page }
    }

    fn bp(&self) -> *mut BasePage {
        self.page.cast::<BasePage>()
    }

    /// Make this the running process: every AES call now carries its identity.
    fn run(&self) {
        process::set_current(self.bp());
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if process::current() == self.bp() {
            process::set_current(std::ptr::null_mut());
        }
        // SAFETY: single-threaded under `LOCK`; no frame snapshot is consumed
        // on the host. Releasing an already-released slot only re-zeroes it.
        unsafe { process::pterm::release_slot(self.slot) };
        // SAFETY: `page` came from `Box::into_raw` above and is not aliased
        // once the slot no longer names it.
        unsafe { drop(Box::from_raw(self.page)) };
    }
}

/// The kernel boot context: no running process — the built-in desktop.
fn desktop_context() {
    process::set_current(std::ptr::null_mut());
}

/// `Pterm` of the running process — the real `terminate` path followed by the
/// handler's `release_slot`. What is under test is what it leaves in the AES
/// window table: nothing is touched there (`pterm.rs`, `release_client`).
fn pterm(p: &Proc) {
    PROCESS_TABLE[p.slot].parent_frame.elr().store(1, Ordering::Release);
    // SAFETY: single-threaded under `LOCK`; `RUN` is `p`'s basepage.
    let resume = unsafe { process::pterm::terminate(0) }.expect("Pterm of the running process");
    assert_eq!(resume.parent_frame_slot, p.slot);
    // SAFETY: the frame snapshot is not needed on the host.
    unsafe { process::pterm::release_slot(p.slot) };
}

// ── the launch boundary, as the kernel drives it ──────────────────────────

/// The desktop's own background tree: one white `G_BOX` over the screen, so
/// every pixel the composite paints as *desktop* is white and anything else
/// inside a rectangle is a window.
fn desktop_tree() -> Box<Object> {
    Box::new(Object {
        ob_next: -1,
        ob_head: -1,
        ob_tail: -1,
        ob_type: G_BOX,
        ob_flags: 0,
        ob_state: 0,
        ob_spec: WHITE_SOLID,
        ob_x: 0,
        ob_y: 0,
        ob_width: SCREEN_W as i16,
        ob_height: SCREEN_H as i16,
    })
}

/// `desktop_relinquish` (`kernel/src/desk/main.rs`): un-register the
/// background tree, leave the AES (`deskmain.c:2159-2166`). The menu bar is
/// not part of this test.
fn desktop_relinquish(pb: &mut Aespb) {
    desktop_context();
    wind_set_newdesk(pb, 0, 0);
    appl_exit(pb);
}

/// The launcher's post-run step (`kernel/src/launch.rs`
/// `close_client_if_open(child.slot)` — `sh_ldapp`'s `AP_OPEN` cleanup,
/// `aes/gemshlib.c:650-657`).
fn launcher_cleanup(slot: usize) {
    tos_api::aes::close_client_if_open(slot);
}

/// `desktop_resume` (`kernel/src/desk/main.rs`), step for step: the
/// `sh_ldapp` window sweep (A96), `appl_init`, re-register the background
/// tree (`deskmain.c:2049`), the whole-screen composite (`redraw_desktop`).
fn desktop_resume(pb: &mut Aespb, tree: &Object) {
    desktop_context();
    tos_api::aes::window::wm_new_keep_desktop();
    appl_init(pb);
    wind_set_newdesk(pb, tree as *const Object as u64, 0);
    form_dial_finish(pb);
}

/// Tear down what a test built, whether or not it passed.
fn cleanup(pb: &mut Aespb, desk_win: i16) {
    desktop_context();
    wind_close(pb, desk_win);
    wind_delete(pb, desk_win);
    wind_set_newdesk(pb, 0, 0);
    appl_exit(pb);
    tos_api::aes::window::wm_new();
}

// ── (1) the program's window is gone, the desktop's is not ─────────────────

/// The desktop has a directory window open. The program it launches opens a
/// window and `Pterm`s without `wind_close`/`wind_delete`. When the desktop
/// resumes, the program's window must be out of the table (its slot free),
/// `wind_find` at its centre must answer the desktop, and its rectangle in
/// the whole-screen composite must be bare desktop — while the desktop's own
/// window is still in the table, still found, and still painted.
#[test]
fn a_window_the_program_left_open_is_gone_when_the_desktop_resumes() {
    let _g = lock();
    let mut pb = Aespb::new();
    let tree = desktop_tree();
    tos_api::aes::window::wm_new();

    // The desktop: in the AES, background registered, one window open.
    desktop_context();
    appl_init(&mut pb);
    wind_set_newdesk(&mut pb, &*tree as *const Object as u64, 0);
    let desk_win = wind_create(&mut pb);
    wind_open(&mut pb, desk_win, DESK_WIN);
    assert_eq!(tos_api::aes::window::test_window_owner(desk_win), Some(DESKTOP_IDENTITY));

    // The launch: relinquish, the program runs, opens a window, dies.
    desktop_relinquish(&mut pb);
    let prog = Proc::new();
    prog.run();
    appl_init(&mut pb);
    let prog_win = wind_create(&mut pb);
    wind_open(&mut pb, prog_win, PROG_WIN);
    assert_eq!(tos_api::aes::window::test_window_owner(prog_win), Some(prog.slot));
    assert_eq!(wind_find(&mut pb, centre(PROG_WIN).0, centre(PROG_WIN).1), prog_win);
    // …and holds the screen locks, as a program in the middle of a redraw.
    wind_update(&mut pb, BEG_UPDATE);
    wind_update(&mut pb, BEG_MCTRL);
    pterm(&prog);
    launcher_cleanup(prog.slot);

    // The desktop regains control.
    desktop_resume(&mut pb, &tree);

    let white = host().raster.device(0);
    let (px, py) = centre(PROG_WIN);
    let (dx, dy) = centre(DESK_WIN);
    let verdict = (
        tos_api::aes::window::test_window_owner(prog_win),
        wind_find(&mut pb, px, py),
        host().raster.count_not(PROG_WIN, white),
        tos_api::aes::window::test_window_owner(desk_win),
        wind_find(&mut pb, dx, dy),
        host().raster.count_not(inset(DESK_WIN, 1), white),
    );
    cleanup(&mut pb, desk_win);

    assert_eq!(
        verdict.0, None,
        "the dead program's window must be out of the table (wm_new, gemwmlib.c:1886-1890)"
    );
    assert_eq!(verdict.1, DESKWH, "wind_find inside the dead program's window must answer the desktop");
    assert_eq!(
        verdict.2, 0,
        "the whole-screen composite must paint the dead program's rectangle as bare desktop: {} pixels are not",
        verdict.2
    );
    assert_eq!(verdict.3, Some(DESKTOP_IDENTITY), "the desktop's own window must survive the sweep (gap G2)");
    assert_eq!(verdict.4, desk_win, "wind_find inside the desktop's window must still answer it");
    assert!(verdict.5 > 0, "the desktop's window must still be painted");
}

// ── (2) the program's background tree goes with it ─────────────────────────

/// A program that registered its own `WF_NEWDESK` tree (a desktop
/// replacement) and died leaves a pointer into an address space that no
/// longer exists. The sweep drops it — before the desktop re-registers its
/// own, nothing must be registered (EmuTOS `wm_init`, `gl_newdesk = NULL`,
/// `gemwmlib.c:1300`).
#[test]
fn the_programs_background_tree_is_dropped_with_it() {
    let _g = lock();
    let mut pb = Aespb::new();
    let tree = desktop_tree();
    tos_api::aes::window::wm_new();

    desktop_context();
    appl_init(&mut pb);
    wind_set_newdesk(&mut pb, &*tree as *const Object as u64, 0);
    let desk_win = wind_create(&mut pb);
    wind_open(&mut pb, desk_win, DESK_WIN);

    desktop_relinquish(&mut pb);
    let prog = Proc::new();
    prog.run();
    appl_init(&mut pb);
    // The program's own background tree, in its own memory.
    let prog_tree = desktop_tree();
    wind_set_newdesk(&mut pb, &*prog_tree as *const Object as u64, 0);
    assert_eq!(tos_api::aes::window::test_newdesk_tree(), &*prog_tree as *const Object as u64);
    pterm(&prog);
    launcher_cleanup(prog.slot);

    desktop_context();
    tos_api::aes::window::wm_new_keep_desktop();
    let after_sweep = tos_api::aes::window::test_newdesk_tree();
    // The rest of `desktop_resume`, then tear down.
    appl_init(&mut pb);
    wind_set_newdesk(&mut pb, &*tree as *const Object as u64, 0);
    form_dial_finish(&mut pb);
    cleanup(&mut pb, desk_win);

    assert_eq!(after_sweep, 0, "the dead program's WF_NEWDESK tree must be dropped by the sweep");
}
