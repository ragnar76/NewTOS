//! Harness for the AES reference-conformance tests.
//!
//! These tests replay measured TOS behaviour against `tos_api::aes`. The
//! reference is a dump taken by `tosdump/` on real hardware — **EmuTOS 1.4 on
//! an Atari TT**, `tosdump/reference/EMUTOS14-AES.BIN`, produced by `DUMPAES`
//! from `tosdump/cases/aes-r1.tsv` — and `docs/api/aes/` was extracted from
//! that same EmuTOS source, so a difference here normally has exactly one
//! reading: NewTOS is wrong. The two exceptions (an EmuTOS *version* delta and
//! the fields that are a property of the machine rather than of the AES) are
//! named below and in [`GLOBAL_EXCLUDED`]; nothing is skipped silently.
//!
//! **What these tests cover.** The record holds the six AES parameter-block
//! arrays as they stood *after* the call — `control`, `global`, `int_in`,
//! `int_out`, `addr_in`, `addr_out` — so they exercise the dispatcher's
//! marshalling (`min(IN_LEN, I_SIZE)` words in, exactly `OUT_LEN` words back
//! out, `aes/gemsuper.c:428-448`), every opcode's computed outputs, and the
//! rule that a manager may not write the caller's input arrays. They say
//! nothing about pixels: round 1 is pointer-free and draws nothing.
//!
//! **The trap's `d0` is out of reach.** The record carries it (offset 106);
//! both machines wrote 0. `tos_api::aes::dispatch` returns
//! `Option<sched::Switch>`, not the WORD the trap stub puts in `d0`, so the
//! field cannot be compared from here — the same limitation the VDI harness
//! has, and for the same reason. `tosdump/host/build/tosdiff` compares it.
//!
//! **No reset between cases, and why the order does not matter.** The dumper
//! runs the table top to bottom in one process and resets nothing between
//! cases: the AES has no documented default state to restore
//! (`tosdump/core/runner_aes.h`). Round 1 was therefore built out of opcodes
//! that do not mutate state another case reads (`tosdump/cases/aes-r1.tsv`
//! header). That is what makes it legitimate for cargo to run these tests in
//! an arbitrary order — but only one at a time, since the AES window table,
//! the process descriptors and the installed host are process-wide statics;
//! hence [`AES_LOCK`].
//!
//! **Why the screen is 640×480 at four planes.** The reference was measured
//! in TT-medium. Several outputs are a function of the screen — `wind_get`'s
//! desktop rectangles, `appl_init`'s `ap_planes` — so replaying at the
//! machine's geometry is what turns them into a real comparison instead of an
//! excused difference. On the Pi 4 the same code answers 1920×1080 at 16
//! planes (Falcon truecolor), and that is correct there: the values are the
//! workstation's, and this harness supplies a workstation shaped like the TT's.

#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tos_api::bdos::process::Abi;
use tos_api::host::Host;
use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};

/// The geometry the reference dump was measured at: the TT in TT-medium,
/// 640×480 at four planes / sixteen pens.
pub const SCREEN_W: u16 = 640;
pub const SCREEN_H: u16 = 480;
pub const SCREEN_PLANES: u16 = 4;
pub const SCREEN_COLORS: u16 = 16;

// ── the memory raster backend ────────────────────────────────────────────

/// A plain linear pixel buffer at the reference geometry. The kernel's
/// rasterizer is pinned to AArch64 and reaches the board framebuffer through
/// the HAL, so it cannot run in a host test; nothing in round 1 draws, and
/// this exists only so that the AES sees a screen of the right size and depth.
pub struct MemBackend {
    px: Mutex<Vec<u32>>,
    palette: Mutex<[u16; 256]>,
    req: Mutex<[(u16, u16, u16); 256]>,
}

impl MemBackend {
    fn new() -> Self {
        MemBackend {
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

impl RasterBackend for MemBackend {
    fn width(&self) -> u16 {
        SCREEN_W
    }
    fn height(&self) -> u16 {
        SCREEN_H
    }
    fn num_colors(&self) -> u16 {
        SCREEN_COLORS
    }
    fn planes(&self) -> u16 {
        SCREEN_PLANES
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

    #[allow(clippy::too_many_arguments)]
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

    fn fill_pattern(&self, x: i16, y: i16, w: i16, h: i16, fill: PatternFill, clip: ClipRect) {
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

    #[allow(clippy::too_many_arguments)]
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
                let sx = src.x + col;
                let sy = src.y + row;
                let wi = (sy as usize) * src.stride_words as usize + (sx as usize) / 16;
                let w = src.bits.get(wi).copied().unwrap_or(0);
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

// ── the host ─────────────────────────────────────────────────────────────

/// The host the AES runs against. `pump_input` is what EmuTOS's `dsptch()`
/// fall-through reaches (`aes/gemsuper.c:452-464`); there is no input script
/// here, so it only advances the clock. The clock also advances on every
/// read, which is what lets the two `evnt_timer` cases terminate: a wait of
/// one millisecond is over after the harness has looked at the clock twice.
pub struct TestHost {
    raster: MemBackend,
    clock: AtomicU64,
}

impl Host for TestHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn raster(&self) -> Option<&dyn RasterBackend> {
        Some(&self.raster)
    }
    fn monotonic_millis(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed)
    }
    fn pump_input(&self) {
        self.clock.fetch_add(1, Ordering::Relaxed);
    }
}

// SAFETY: every field is behind a Mutex or an atomic.
unsafe impl Sync for TestHost {}
unsafe impl Send for TestHost {}

/// The AES keeps process-global state (window table, AES process descriptors,
/// the built-in DESKTOP tree). Cargo runs tests on parallel threads, so every
/// case takes this lock.
pub static AES_LOCK: Mutex<()> = Mutex::new(());
static HOST_CELL: OnceLock<&'static TestHost> = OnceLock::new();
static HOST_REF: OnceLock<&'static dyn Host> = OnceLock::new();
static CTX: OnceLock<Context> = OnceLock::new();

/// The ABI the parameter block is laid at. The measured NewTOS run was an
/// LP64 program (`tosdump/build-dumpaes-newtos.sh` compiles with
/// `-target aarch64-unknown-none-elf`), and LP64 is NewTOS's native
/// application ABI, so the replay uses it too. It is the one place the two
/// machines genuinely cannot agree byte for byte — see [`ap_private`] in
/// [`cmp_global`].
pub const ABI: Abi = Abi::Lp64;

// ── the AES parameter block ──────────────────────────────────────────────

/// `control[5]` — in the AES **all five** slots are inputs: the dispatcher
/// copies `control[]` onto a local buffer and never writes any of it back
/// (`aes/gemsuper.c:435`).
pub const CONTRL_N: usize = 5;
/// `global[15]` — EmuTOS `struct aesglobal`, 30 bytes (`aes/gemrslib.h:15-27`,
/// `desk/gembind.h:27` `G_SIZE 15`). There is no index 15.
pub const GLOBAL_N: usize = 15;
/// `I_SIZE` (`include/aesdefs.h`), the dispatcher's input clamp.
pub const INTIN_N: usize = 16;
/// `O_SIZE`, the largest `OUT_LEN` any opcode produces (`evnt_multi`'s seven).
pub const INTOUT_N: usize = 7;
/// `AI_SIZE` / the single `addr_out` slot.
pub const ADDRIN_N: usize = 3;
pub const ADDROUT_N: usize = 1;

/// The 0xDEAD poison (`docs/TOS-REFERENCE-DUMPER.md` section 4). Everything
/// the call is not supposed to write stays at this value, which is what makes
/// "wrote a slot it must not touch" visible at all.
pub const POISON: i16 = 0xDEADu16 as i16;
/// The pointer-array poison. `tosdump/core/runner_aes.c` builds it as a
/// *value* repeated to the target's pointer width, so the low 32 bits — the
/// only part the record carries — read `0xDEADDEAD` on the 32-bit m68k and on
/// 64-bit AArch64 alike.
pub const POISON_PTR: u64 = 0xDEAD_DEAD_DEAD_DEAD;

pub struct Pb {
    pub contrl: Box<[i16; CONTRL_N]>,
    pub intin: Box<[i16; INTIN_N]>,
    pub intout: Box<[i16; INTOUT_N]>,
    pub addrin: Box<[u64; ADDRIN_N]>,
    pub addrout: Box<[u64; ADDROUT_N]>,
    /// The application's live `AESGLOBAL` — **not** owned by the block. It is
    /// the process-wide array [`global_array`] leaked once, exactly as
    /// `tosdump/core/gem.c` keeps one file-scope `aes_global[15]` for the
    /// whole run: `appl_init` fills it and every case afterwards is measured
    /// against what it left there.
    global: &'static mut [i16; GLOBAL_N],
    block: Box<[u64; 6]>,
}

impl Pb {
    pub fn new() -> Pb {
        let mut pb = Pb {
            contrl: Box::new([0; CONTRL_N]),
            intin: Box::new([0; INTIN_N]),
            intout: Box::new([0; INTOUT_N]),
            addrin: Box::new([0; ADDRIN_N]),
            addrout: Box::new([0; ADDROUT_N]),
            global: global_array(),
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

    /// The live `AESGLOBAL` as it stands.
    pub fn global(&self) -> &[i16; GLOBAL_N] {
        self.global
    }

    /// Poison everything **except** `global`, which is live AES state and
    /// belongs to the application rather than to the case
    /// (`tosdump/core/runner_aes.c` step 2).
    pub fn poison(&mut self) {
        self.contrl.fill(POISON);
        self.intin.fill(POISON);
        self.intout.fill(POISON);
        self.addrin.fill(POISON_PTR);
        self.addrout.fill(POISON_PTR);
    }

    /// Lay `control[0..4]` and issue the call — `tosdump/core/runner_aes.c`'s
    /// `aes_call`, which the measured call and every prelude both go through
    /// so the two cannot lay the block differently.
    pub fn call(&mut self, opcode: i16, in_len: i16, out_len: i16, ain_len: i16, aout_len: i16) {
        self.contrl[0] = opcode;
        self.contrl[1] = in_len;
        self.contrl[2] = out_len;
        self.contrl[3] = ain_len;
        self.contrl[4] = aout_len;
        // SAFETY: `block` holds six live pointers to arrays owned by `self`,
        // each at least as long as the AES parameter block requires.
        unsafe { tos_api::aes::dispatch(self.block.as_ptr() as u64, ABI) };
    }
}

// ── the application context ──────────────────────────────────────────────

/// What `tosdump/core/gem.c`'s `td_gem_open` established before the first
/// case ran: `appl_init`, `graf_handle`, and `v_opnvwk` on the handle
/// `graf_handle` returned. The AES cases need none of the VDI workstation,
/// but the reference was measured with it open and the context has to be the
/// same for the comparison to mean anything.
#[derive(Copy, Clone)]
pub struct Context {
    pub ap_id: i16,
    pub phys_handle: i16,
    pub handle: i16,
}

/// `work_in` for `v_opnvwk`: every attribute index 1, raster coordinates —
/// the classic GEM application open, stated once in `tosdump/core/gem.c` and
/// repeated here call for call.
fn open_workstation(phys: i16) -> i16 {
    let contrl = Box::new([0i16; 12]);
    let intin = Box::new([0i16; 128]);
    let ptsin = Box::new([0i16; 256]);
    let intout = Box::new([0i16; 128]);
    let ptsout = Box::new([0i16; 256]);
    let mut block = [0u64; 5];
    block[0] = contrl.as_ptr() as u64;
    block[1] = intin.as_ptr() as u64;
    block[2] = ptsin.as_ptr() as u64;
    block[3] = intout.as_ptr() as u64;
    block[4] = ptsout.as_ptr() as u64;

    // SAFETY: the five arrays outlive the call and are large enough for
    // `v_opnvwk`'s `work_out` (45 words) and `SIZ_TAB` (12 words).
    unsafe {
        let c = contrl.as_ptr() as *mut i16;
        let i = intin.as_ptr() as *mut i16;
        for k in 0..10 {
            i.add(k).write(1);
        }
        i.add(10).write(2); // 2 = raster coordinates
        for k in 0..12 {
            c.add(k).write(0);
        }
        c.write(100); // v_opnvwk
        c.add(3).write(11); // 11 intin words
        c.add(6).write(phys);
        let _flip_y = tos_api::vdi::dispatch(block.as_ptr() as u64, ABI);
        c.add(6).read()
    }
}

/// The one application context of this test binary, opened once. `global` is
/// leaked deliberately: the AES filled it at `appl_init` and every case is
/// measured against it afterwards, exactly as the dumper's file-scope
/// `aes_global[15]` lives for the whole run.
fn context() -> &'static Context {
    CTX.get_or_init(|| {
        let host: &'static TestHost = HOST_CELL.get_or_init(|| {
            Box::leak(Box::new(TestHost {
                raster: MemBackend::new(),
                clock: AtomicU64::new(0),
            }))
        });
        let href: &'static &'static dyn Host =
            Box::leak(Box::new(*HOST_REF.get_or_init(|| host as &'static dyn Host)));
        tos_api::host::install_host(href);

        let mut pb = Pb::new();
        pb.poison();
        pb.call(10, 0, 1, 0, 0); // appl_init
        let ap_id = pb.intout[0];
        assert!(ap_id >= 0, "appl_init failed ({ap_id})");

        pb.poison();
        pb.call(77, 0, 5, 0, 0); // graf_handle
        let phys_handle = pb.intout[0];
        assert!(phys_handle > 0, "graf_handle returned no handle ({phys_handle})");

        let handle = open_workstation(phys_handle);
        assert!(handle > 0, "v_opnvwk returned no handle ({handle})");

        Context { ap_id, phys_handle, handle }
    })
}

/// The live `AESGLOBAL`, allocated once and shared by every parameter block.
/// Zero-initialised, which is what the dumper's `.bss` array was. Handing out
/// several `&'static mut` to it is sound only because [`AES_LOCK`] serialises
/// the cases and no two `Pb`s are alive across the lock — the same discipline
/// the AES itself runs under.
fn global_array() -> &'static mut [i16; GLOBAL_N] {
    static G: OnceLock<usize> = OnceLock::new();
    let p = *G.get_or_init(|| Box::leak(Box::new([0i16; GLOBAL_N])) as *mut _ as usize);
    // SAFETY: the array was leaked above, so it lives for the whole program;
    // access is serialised by `AES_LOCK`.
    unsafe { &mut *(p as *mut [i16; GLOBAL_N]) }
}

// ── the reference record ─────────────────────────────────────────────────

pub const HDR_SIZE: usize = 32;
pub const REC_SIZE: usize = 120;
const OFF_CASEID: usize = 0;
const OFF_OPCODE: usize = 2;
const OFF_CONTRL: usize = 4;
const OFF_GLOBAL: usize = 14;
const OFF_INTIN: usize = 44;
const OFF_INTOUT: usize = 76;
const OFF_ADDRIN: usize = 90;
const OFF_ADDROUT: usize = 102;
const OFF_RET: usize = 106;

fn be16(b: &[u8], off: usize) -> i16 {
    (((b[off] as u16) << 8) | b[off + 1] as u16) as i16
}

fn be32(b: &[u8], off: usize) -> u32 {
    ((b[off] as u32) << 24) | ((b[off + 1] as u32) << 16) | ((b[off + 2] as u32) << 8)
        | b[off + 3] as u32
}

// ── the case ─────────────────────────────────────────────────────────────

/// A call issued before the measured one: it establishes the state the case
/// measures and is not recorded. Poisoned and laid exactly like a measured
/// call (`tosdump/core/runner_aes.c` step 1).
pub struct Call {
    pub opcode: i16,
    pub in_len: i16,
    pub out_len: i16,
    pub ain_len: i16,
    pub aout_len: i16,
    pub intin: &'static [i16],
}

/// One case, as `tosdump/host/mkrusttests` emits it. The four counts are
/// `control[1..4]` **as the table declares them**, not derived from `intin`:
/// several cases exist precisely to measure a count that disagrees with the
/// values listed (`aes/gemsuper.c:428-448`).
pub struct Case {
    pub prelude: &'static [Call],
    pub id: u16,
    pub rec: usize,
    pub opcode: i16,
    pub in_len: i16,
    pub out_len: i16,
    pub ain_len: i16,
    pub aout_len: i16,
    pub intin: &'static [i16],
    pub comment: &'static str,
}

struct Diff {
    field: &'static str,
    index: usize,
    got: i64,
    want: i64,
    hex_width: usize,
}

fn push_diff(out: &mut Vec<Diff>, field: &'static str, index: usize, got: i64, want: i64, w: usize) {
    if got != want {
        out.push(Diff { field, index, got, want, hex_width: w });
    }
}

fn cmp_words(out: &mut Vec<Diff>, field: &'static str, got: &[i16], rec: &[u8], off: usize) {
    for (i, &g) in got.iter().enumerate() {
        push_diff(out, field, i, g as i64, be16(rec, off + i * 2) as i64, 4);
    }
}

// ── the `global` array: what is compared, and what is not ────────────────

/// `global` word indices that are **not** compared, each because the value is
/// a property of the machine the dump was taken on and not of the AES. Every
/// other word — and the `ap_private` LONG, see [`cmp_global`] — is compared.
///
/// * **`global[2]` — `ap_id`.** EmuTOS: `pglobal->ap_id = rlr->p_pid`
///   (`aes/gemsuper.c:94`). On the TT that is **0**, because EmuTOS's AES owns
///   a fixed pool of `AESPD`s: the initial process takes `curpid = 0`
///   (`aes/geminit.c:875-876`), the screen manager takes 1
///   (`:882`, `getpd`/`gempd.c:64-66`), and a program the desktop launches
///   runs *inside the desktop's own* `AESPD`, so it reports the desktop's
///   pid 0. NewTOS gives every launched program a GEMDOS process of its own
///   and derives the id from its process-table slot
///   (`tos-api/src/aes/appl.rs`, `current_pid`), which is **1** for the first
///   one. Both are "the id of this application"; the number is a property of
///   the process model, and no conforming program may depend on its value —
///   it passes it to `appl_write`/`menu_register` and compares it with a
///   message's `msg[1]`, all of which stay consistent.
/// * **`global[11]`, `global[12]` — `ap_3resv`.** EmuTOS:
///   `pglobal->ap_3resv = (LONG)&D` (`aes/gemsuper.c:98`), the address of the
///   AES's own global data area (`aes/gemrslib.h:25`). An address: 0x0000BE04
///   on the TT, the AES process-descriptor table's address on NewTOS
///   (`tos-api/src/aes/appl.rs`, `pd::table_addr()`). That it is an address
///   and not a value is directly measurable in the reference set — between
///   `NEWTOS-AES.BIN` and `NEWTOS-AES-r2.BIN`, two runs of the *same* binary
///   against the *same* table, `global[11]` alone moved from 0x80B0 to
///   0x80D8.
///
/// Neither is left unchecked. [`cmp_global`] compares the machine-independent
/// part of both instead of the number: `ap_id` must equal what `appl_init`
/// returned (EmuTOS `ret = ap_init()` returns `rlr->p_pid`,
/// `aes/gemaplib.c:54-63`, the same `p_pid` that goes into `ap_id` at
/// `gemsuper.c:94` — so "the two agree" holds on any machine), and `ap_3resv`
/// must be non-zero exactly when the reference's is, i.e. `appl_init` must
/// have put *an* address there.
pub const GLOBAL_EXCLUDED: [usize; 3] = [2, 11, 12];

/// Compare the caller's `AESGLOBAL` against the record.
///
/// Fifteen WORDs, but not fifteen independent values: `ap_private`
/// (`gemrslib.h:19`) is a `LONG`, and a `LONG` occupies its two WORD slots in
/// the machine's byte order. The m68k reference holds it high-word-first at
/// `global[3..4]`; a little-endian AArch64 caller holds the same *value*
/// low-word-first, and under LP64 it is eight bytes at byte 6 rather than
/// four, because gemlib's `*(long*)&aes_global[3]` (`gem.h:30`) carries no
/// pointer-size conditional. Comparing those two slots word-for-word would
/// therefore report a difference for a value that is identical. So the WORD
/// fields are compared as WORDs and `ap_private` is compared as the LONG it
/// is — the reference's big-endian 32 bits against the caller-width field
/// this ABI actually reads.
///
/// The words that carry no `appl_init` output — `ap_ptree` (5–6),
/// `ap_rscmem` (7–8), `ap_rsclen` (9), `ap_4resv` (13–14) — **are** compared:
/// EmuTOS's `APPL_INIT` arm does not name them (`gemsuper.c:92-98`), both
/// arrays start zeroed, so a difference there means somebody wrote where
/// EmuTOS does not. Under LP64 words 5 and 6 are the high half of the
/// eight-byte `ap_private`, which is zero for the ≤32-bit `gr_crack` box word
/// `ap_private` holds, so the same comparison still stands.
fn cmp_global(out: &mut Vec<Diff>, got: &[i16; GLOBAL_N], rec: &[u8], ap_id: i16) {
    // The excluded fields, checked by the property that does not depend on
    // the machine (see [`GLOBAL_EXCLUDED`]).
    push_diff(out, "ap_id == appl_init ret", 0, got[2] as i64, ap_id as i64, 4);
    let ref_3resv = ((be16(rec, OFF_GLOBAL + 11 * 2) as u16 as u32) << 16)
        | be16(rec, OFF_GLOBAL + 12 * 2) as u16 as u32;
    let my_3resv = ((got[11] as u16 as u32) as u64) | ((got[12] as u16 as u32) as u64) << 16;
    push_diff(
        out,
        "ap_3resv is an address",
        0,
        i64::from(my_3resv != 0),
        i64::from(ref_3resv != 0),
        4,
    );

    // ap_private as a value: the reference's big-endian m68k LONG at words
    // 3–4 against the caller-width LONG at byte 6.
    let want = ((be16(rec, OFF_GLOBAL + 3 * 2) as u16 as u32) << 16)
        | be16(rec, OFF_GLOBAL + 4 * 2) as u16 as u32;
    // SAFETY: `got` is 15 WORDs = 30 bytes; byte 6 + 8 = 14 is inside it. The
    // slot is only 2-byte aligned, hence the unaligned read.
    let mine = unsafe { (got.as_ptr() as *const u8).add(6).cast::<u64>().read_unaligned() };
    push_diff(out, "ap_private", 0, mine as i64, want as i64, 8);

    for (i, &g) in got.iter().enumerate() {
        if GLOBAL_EXCLUDED.contains(&i) || i == 3 || i == 4 {
            continue; // 3–4 are the ap_private LONG, compared above
        }
        push_diff(out, "global", i, g as i64, be16(rec, OFF_GLOBAL + i * 2) as i64, 4);
    }
}

// ── where the recorded reference is not the oracle ───────────────────────

/// One field of one case where **EmuTOS 1.4 — the build the dump was taken
/// from — is older than the EmuTOS source this project implements against**
/// (`references/emutos`, pinned at `1a7484ab`, 2026-04-14). The record is
/// still compared; only the value it is compared *against* changes, and the
/// entry has to say exactly why. Nothing is skipped: if NewTOS stops
/// producing [`expected`](Self::expected) the test fails, and it fails with
/// both numbers in the message.
///
/// [`Self::expected`]: Delta::expected
pub struct Delta {
    /// Case id = the physical line number in `tosdump/cases/aes-r1.tsv`.
    pub case: u16,
    pub field: &'static str,
    pub index: usize,
    /// What the TT running EmuTOS 1.4 actually wrote.
    pub recorded: i64,
    /// What the pinned EmuTOS source mandates, i.e. what NewTOS must produce.
    pub expected: i64,
    pub why: &'static str,
}

/// The one such field in round 1.
///
/// `wind_get(0, WF_OWNER)`: `WF_OWNER` (20) **did not exist** in EmuTOS 1.4.
/// It is absent from `include/aesdefs.h` (which ends at `WF_COLOR` 18) and
/// `wm_get` has no arm for it, so field 20 reaches `default: return FALSE`
/// and the AES answers `RET_CODE = 0` — which is what the TT recorded.
/// It was added on 2025-08-23 (`acb7f3b4`, "Add partial implementation of
/// WF_OWNER to wind_get"), after the `VERSION_1_4` tag of 2025-06-07, and the
/// pinned source now has it at `aes/gemwmlib.c:1578-1589` under
/// `#if AES_VERSION >= 0x330`, returning TRUE with the owner pid, the open
/// flag and the top/bottom handles in `poutwds[0..3]`. Both machines report
/// `ap_version 0x0340`, so 1.4 was answering FALSE for a field its own
/// advertised AES version has; NewTOS implements the pinned source
/// (`tos-api/src/aes/window.rs`, the `WF_OWNER` arm) and returns TRUE.
///
/// Only `int_out[0]` is affected: the case declares `OUT_LEN 1`, so the four
/// words the handler writes into the dispatcher's local `int_out` are never
/// copied back and stay poisoned on both machines.
pub const DELTAS: &[Delta] = &[Delta {
    case: 167,
    field: "int_out",
    index: 0,
    recorded: 0,
    expected: 1,
    why: "WF_OWNER did not exist in EmuTOS 1.4 (added 2025-08-23, acb7f3b4); \
          the pinned source implements it at aes/gemwmlib.c:1578-1589",
}];

/// Re-point the diffs a [`Delta`] covers at the value the pinned EmuTOS
/// source mandates, and drop the ones that then agree.
fn apply_deltas(diffs: &mut Vec<Diff>, case: u16) {
    diffs.retain_mut(|d| {
        match DELTAS
            .iter()
            .find(|x| x.case == case && x.field == d.field && x.index == d.index)
        {
            Some(x) if d.got == x.expected => false,
            Some(x) => {
                d.want = x.expected;
                true
            }
            None => true,
        }
    });
}

/// Replay one case and compare every array against the reference.
pub fn check(dump: &[u8], case: &Case) {
    let _guard = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = context();
    let mut pb = Pb::new();

    // The case's preludes, in order, exactly as `tosdump/core/runner_aes.c`
    // issues them: each poisoned and laid like a measured call, none recorded.
    for p in case.prelude {
        pb.poison();
        pb.intin[..p.intin.len()].copy_from_slice(p.intin);
        pb.call(p.opcode, p.in_len, p.out_len, p.ain_len, p.aout_len);
    }

    let rec = &dump[HDR_SIZE + case.rec * REC_SIZE..][..REC_SIZE];
    assert_eq!(
        be16(rec, OFF_CASEID) as u16,
        case.id,
        "record {} is case {}, expected case {} -- dump and generated tests disagree",
        case.rec,
        be16(rec, OFF_CASEID),
        case.id
    );
    assert_eq!(
        be16(rec, OFF_OPCODE),
        case.opcode,
        "case {}: record opcode {} vs case opcode {}",
        case.id,
        be16(rec, OFF_OPCODE),
        case.opcode
    );

    pb.poison();
    pb.intin[..case.intin.len()].copy_from_slice(case.intin);
    pb.call(case.opcode, case.in_len, case.out_len, case.ain_len, case.aout_len);

    let mut diffs = Vec::new();
    cmp_words(&mut diffs, "contrl", &pb.contrl[..], rec, OFF_CONTRL);
    cmp_global(&mut diffs, pb.global(), rec, ctx.ap_id);
    cmp_words(&mut diffs, "int_in", &pb.intin[..], rec, OFF_INTIN);
    cmp_words(&mut diffs, "int_out", &pb.intout[..], rec, OFF_INTOUT);
    // The record carries the low 32 bits of each address slot, because the
    // format's slots are 32 bit while the block's are the target's pointer
    // width (`tosdump/core/runner_aes.c`).
    for (i, &a) in pb.addrin.iter().enumerate() {
        push_diff(&mut diffs, "addr_in", i, a as u32 as i64, be32(rec, OFF_ADDRIN + i * 4) as i64, 8);
    }
    for (i, &a) in pb.addrout.iter().enumerate() {
        push_diff(&mut diffs, "addr_out", i, a as u32 as i64, be32(rec, OFF_ADDROUT + i * 4) as i64, 8);
    }
    apply_deltas(&mut diffs, case.id);

    if !diffs.is_empty() {
        let mut msg = format!(
            "case {} (opcode {}) -- {}\n  {} field(s) differ from the EmuTOS \
             1.4 reference:\n",
            case.id,
            case.opcode,
            case.comment,
            diffs.len()
        );
        for d in diffs.iter().take(24) {
            msg += &format!(
                "    {}[{}]  NewTOS = 0x{:0w$X} ({})   reference = 0x{:0w$X} ({})\n",
                d.field,
                d.index,
                d.got & ((1i64 << (d.hex_width * 4)) - 1),
                d.got,
                d.want & ((1i64 << (d.hex_width * 4)) - 1),
                d.want,
                w = d.hex_width
            );
        }
        if diffs.len() > 24 {
            msg += &format!("    ... and {} more\n", diffs.len() - 24);
        }
        // The trap's `d0` is recorded at OFF_RET and compared by
        // `tosdump/host/build/tosdiff`; `tos_api::aes::dispatch` does not
        // return it, so it is out of reach here.
        let _ = OFF_RET;
        panic!("{}", msg);
    }
}
