//! Harness for the VDI reference-conformance tests.
//!
//! These tests replay measured TOS behaviour against `tos_api::vdi`. The
//! reference is a dump taken by `tosdump/` on real hardware — EmuTOS on an
//! Atari TT — and the specification in `docs/api/` was extracted from that
//! same EmuTOS source, so a difference here has exactly one reading: NewTOS
//! is wrong. See `tosdump/reference/FINDINGS-r1.md`.
//!
//! **What these tests cover.** The dump records the five VDI parameter-block
//! arrays and the trap's return value — not pixels. So they exercise
//! everything *above* the [`RasterBackend`] seam: dispatch, `Vwk` attribute
//! state, validation and clamping, clipping, and the inquiries. Whether the
//! rasterizer puts the right pixels down is a separate question that needs a
//! pixel comparison the record format does not carry.
//!
//! **Why a memory backend.** The kernel's rasterizer lives in the `kernel`
//! crate, is pinned to AArch64 and reaches the board framebuffer through the
//! HAL, so it cannot run in a host test. The backend below is a plain linear
//! buffer at the same geometry the reference was measured at (640×480). It
//! is not the kernel's rasterizer, and these tests are not a statement about
//! the kernel's rasterizer.
//!
//! **Serialisation.** The VDI keeps device-global state (handle table, `Vwk`
//! slots, palette). Cargo runs tests on parallel threads, so every case takes
//! [`VDI_LOCK`] and the workstation is opened once behind it.

#![allow(dead_code)]

use std::sync::{Mutex, OnceLock};

use tos_api::bdos::process::Abi;
use tos_api::host::Host;
use tos_api::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};

/// The geometry the reference dump was measured at: the TT in TT-medium.
/// `DEV_TAB[0..1]` in `tosdump/reference/EMUTOS14-VDI-r2.LOG` reads
/// `639,479`, i.e. 640×480.
pub const SCREEN_W: u16 = 640;
pub const SCREEN_H: u16 = 480;

// ── the memory raster backend ────────────────────────────────────────────

pub struct MemBackend {
    px: Mutex<Vec<u32>>,
    palette: Mutex<[u16; 256]>,
    req: Mutex<[(u16, u16, u16); 256]>,
}

/// EmuTOS' `videl_palette1` (`vdi/vdi_col.c:130-136`) as VDI `(r, g, b)`
/// triples in the 0..=1000 range, the same sixteen the kernel's backend
/// seeds pens 0..15 from (`kernel/src/vdi_backend.rs`). Index 0 is white
/// (background), 1 is black (foreground).
///
/// Seeding this matters: without it `vq_color` reports pen 0 as (0,0,0)
/// where every real machine reports white, and the difference is the test
/// harness's, not NewTOS's.
#[rustfmt::skip]
const VIDEL_PALETTE1: [(u16, u16, u16); 16] = [
    (1000, 1000, 1000), (0, 0, 0),       (1000, 0, 0),    (0, 1000, 0),
    (0, 0, 1000),       (0, 1000, 1000), (1000, 1000, 0), (1000, 0, 1000),
    (733, 733, 733),    (533, 533, 533), (667, 0, 0),     (0, 667, 0),
    (0, 0, 667),        (0, 667, 667),   (667, 667, 0),   (667, 0, 667),
];

impl MemBackend {
    fn new() -> Self {
        let mut req = [(0u16, 0u16, 0u16); 256];
        req[..16].copy_from_slice(&VIDEL_PALETTE1);
        MemBackend {
            px: Mutex::new(vec![0; SCREEN_W as usize * SCREEN_H as usize]),
            palette: Mutex::new([0; 256]),
            req: Mutex::new(req),
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

    /// Reported exactly as the kernel's backend reports it
    /// (`kernel/src/vdi_backend.rs`): a Falcon truecolor workstation says
    /// 256 colours over 16 planes. The TT the reference came from says 16
    /// and 4, so `DEV_TAB[13]` and `INQ_TAB[4]` differ *by design* between
    /// the reference and NewTOS. That is a deliberate profile difference,
    /// not a defect, and it is recorded as such rather than hidden here.
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

    /// The VDI reads pixels back as PELs. This backend stores device words,
    /// so it resolves back through the active palette; an unmatched colour
    /// reads as pen 0, which is what a background pixel is.
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
        // Plain Bresenham. The VDI layer above owns the line style, the
        // clipping decision and the endpoint rules; this only has to put
        // pixels somewhere deterministic.
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

pub struct TestHost {
    raster: MemBackend,
}

impl Host for TestHost {
    fn console_write_byte(&self, _byte: u8) {}
    fn raster(&self) -> Option<&dyn RasterBackend> {
        Some(&self.raster)
    }
}

// SAFETY: every field is behind a Mutex.
unsafe impl Sync for TestHost {}
unsafe impl Send for TestHost {}

pub static VDI_LOCK: Mutex<()> = Mutex::new(());
static HOST_CELL: OnceLock<&'static TestHost> = OnceLock::new();
static HOST_REF: OnceLock<&'static dyn Host> = OnceLock::new();
static WK: OnceLock<Workstation> = OnceLock::new();

#[derive(Copy, Clone)]
pub struct Workstation {
    pub phys: i16,
    pub handle: i16,
    /// `SIZ_TAB` as this workstation reported it, i.e. `v_opnvwk`'s `ptsout`.
    /// The reset reads three device-defined defaults out of it, exactly as
    /// `tosdump/core/runner.c` does.
    pub siz_tab: [i16; 12],
}

pub const ABI: Abi = Abi::Lp64;

// ── the parameter block ──────────────────────────────────────────────────

pub const CONTRL_N: usize = 12;
pub const INTIN_N: usize = 128;
pub const PTSIN_N: usize = 256;
pub const INTOUT_N: usize = 128;
pub const PTSOUT_N: usize = 256;

/// The 0xDEAD poison (`tosdump` spec section 4). Everything the call is not
/// supposed to write stays at this value, which is what makes "wrote a field
/// it must not touch" visible at all.
pub const POISON: i16 = 0xDEADu16 as i16;

pub struct Pb {
    pub contrl: Box<[i16; CONTRL_N]>,
    pub intin: Box<[i16; INTIN_N]>,
    pub ptsin: Box<[i16; PTSIN_N]>,
    pub intout: Box<[i16; INTOUT_N]>,
    pub ptsout: Box<[i16; PTSOUT_N]>,
    block: Box<[u64; 5]>,
}

impl Pb {
    pub fn new() -> Pb {
        let mut pb = Pb {
            contrl: Box::new([0; CONTRL_N]),
            intin: Box::new([0; INTIN_N]),
            ptsin: Box::new([0; PTSIN_N]),
            intout: Box::new([0; INTOUT_N]),
            ptsout: Box::new([0; PTSOUT_N]),
            block: Box::new([0; 5]),
        };
        pb.block[0] = pb.contrl.as_ptr() as u64;
        pb.block[1] = pb.intin.as_ptr() as u64;
        pb.block[2] = pb.ptsin.as_ptr() as u64;
        pb.block[3] = pb.intout.as_ptr() as u64;
        pb.block[4] = pb.ptsout.as_ptr() as u64;
        pb
    }

    pub fn poison(&mut self) {
        self.contrl.fill(POISON);
        self.intin.fill(POISON);
        self.ptsin.fill(POISON);
        self.intout.fill(POISON);
        self.ptsout.fill(POISON);
    }

    /// Issue the call. Mirrors what the trap does: the caller has already
    /// filled `contrl`, and everything comes back through the arrays.
    pub fn call(&mut self) {
        // SAFETY: `block` holds five live pointers to arrays owned by `self`
        // and large enough for every opcode under test.
        unsafe { tos_api::vdi::dispatch(self.block.as_ptr() as u64, ABI) }
    }

    fn op(&mut self, opcode: i16, sub: i16, n_pts: i16, n_int: i16, handle: i16) {
        self.contrl.fill(0);
        self.contrl[0] = opcode;
        self.contrl[1] = n_pts;
        self.contrl[3] = n_int;
        self.contrl[5] = sub;
        self.contrl[6] = handle;
        self.call();
    }

    fn int1(&mut self, opcode: i16, v: i16, handle: i16) {
        self.intin[0] = v;
        self.op(opcode, 0, 0, 1, handle);
    }
    fn int2(&mut self, opcode: i16, a: i16, b: i16, handle: i16) {
        self.intin[0] = a;
        self.intin[1] = b;
        self.op(opcode, 0, 0, 2, handle);
    }
    fn pts(&mut self, opcode: i16, x: i16, y: i16, handle: i16) {
        self.ptsin[0] = x;
        self.ptsin[1] = y;
        self.op(opcode, 0, 1, 0, handle);
    }
}

// ── workstation ──────────────────────────────────────────────────────────

/// Opens the physical workstation and one virtual workstation on it, with
/// the same `work_in` the dumper used (`tosdump/core/gem.c`): every
/// attribute index 1, raster coordinates. EmuTOS' `init_wk` derives the
/// open-time attribute state from exactly these
/// (`vdi/vdi_control.c:269-311`), so the reset below restores the same ones.
///
/// The dumper reached its handle through `graf_handle` because a GEM
/// application does; there is no AES here, so the physical workstation is
/// opened directly, which is what the AES itself does at boot.
pub fn workstation() -> &'static Workstation {
    WK.get_or_init(|| {
        let host: &'static TestHost = HOST_CELL.get_or_init(|| {
            Box::leak(Box::new(TestHost { raster: MemBackend::new() }))
        });
        let href: &'static &'static dyn Host =
            Box::leak(Box::new(*HOST_REF.get_or_init(|| host as &'static dyn Host)));
        tos_api::host::install_host(href);

        let mut pb = Pb::new();

        for i in 0..10 {
            pb.intin[i] = 1;
        }
        pb.intin[10] = 2; // raster coordinates
        pb.op(1, 0, 0, 11, 0); // v_opnwk
        let phys = pb.contrl[6];

        for i in 0..10 {
            pb.intin[i] = 1;
        }
        pb.intin[10] = 2;
        pb.op(100, 0, 0, 11, phys); // v_opnvwk
        let handle = pb.contrl[6];

        let mut siz_tab = [0i16; 12];
        siz_tab.copy_from_slice(&pb.ptsout[..12]);

        Workstation { phys, handle, siz_tab }
    })
}

/// The reset the dumper runs before every case, call for call.
///
/// This is a port of `tosdump/core/runner.c`'s `td_reset_vdi`, and it has to
/// stay a port: the reference dump was measured with exactly this sequence,
/// so any divergence here compares two different things. The three
/// device-defined values come from the `SIZ_TAB` this workstation reported
/// (EmuTOS `vdi_control.c:349-352`, slot mapping `vdi_defs.h:100-104`):
/// line width `SIZ_TAB[4]`, marker height `SIZ_TAB[9]`, character height
/// `SIZ_TAB[1]`.
pub fn reset_vdi(pb: &mut Pb, wk: &Workstation) {
    let h = wk.handle;
    pb.op(3, 0, 0, 0, h); // v_clrwk

    pb.int1(32, 1, h); // vswr_mode  replace

    pb.int1(15, 1, h); // vsl_type   solid
    pb.pts(16, wk.siz_tab[4], 0, h); // vsl_width  DEF_LWID
    pb.int1(17, 1, h); // vsl_color  pen 1
    pb.int2(108, 0, 0, h); // vsl_ends   squared
    pb.int1(113, 0xFFFFu16 as i16, h); // vsl_udsty  all ones

    pb.int1(18, 1, h); // vsm_type   1
    pb.pts(19, 0, wk.siz_tab[9], h); // vsm_height DEF_MKHT
    pb.int1(20, 1, h); // vsm_color  pen 1

    pb.int1(23, 1, h); // vsf_interior
    pb.int1(24, 1, h); // vsf_style
    pb.int1(25, 1, h); // vsf_color  pen 1
    pb.int1(104, 1, h); // vsf_perimeter on

    pb.int1(21, 1, h); // vst_font   1
    pb.pts(12, 0, wk.siz_tab[1], h); // vst_height DEF_CHHT
    pb.int1(13, 0, h); // vst_rotation 0
    pb.int1(22, 1, h); // vst_color  pen 1
    pb.int1(106, 0, h); // vst_effects none
    pb.int2(39, 0, 0, h); // vst_alignment

    pb.int1(129, 0, h); // vs_clip    off
}

// ── the reference record ─────────────────────────────────────────────────

pub const REC_SIZE: usize = 1592;
pub const HDR_SIZE: usize = 32;
const OFF_CASEID: usize = 0;
const OFF_OPCODE: usize = 2;
const OFF_RET: usize = 4;
const OFF_CONTRL: usize = 8;
const OFF_INTIN: usize = 32;
const OFF_PTSIN: usize = 288;
const OFF_INTOUT: usize = 800;
const OFF_PTSOUT: usize = 1056;
const OFF_CONTRL_BEFORE: usize = 1568;

fn be16(b: &[u8], off: usize) -> i16 {
    (((b[off] as u16) << 8) | b[off + 1] as u16) as i16
}

/// A call issued before the measured one: it establishes the state the case
/// measures, runs after the reset so it survives it, and is not recorded.
/// Without it a setter case can only prove its own echo, since the reset
/// wipes anything the previous case set.
pub struct Call {
    pub opcode: i16,
    pub sub: i16,
    pub intin: &'static [i16],
    pub ptsin: &'static [i16],
}

/// One case, as the generator emits it. `intin`/`ptsin` are the inputs from
/// the case table; the expected outputs come from the dump.
pub struct Case {
    pub prelude: &'static [Call],
    pub id: u16,
    pub rec: usize,
    pub opcode: i16,
    pub sub: i16,
    pub intin: &'static [i16],
    pub ptsin: &'static [i16],
    pub comment: &'static str,
}

struct Diff {
    field: &'static str,
    index: usize,
    got: i16,
    want: i16,
}

fn cmp_field(
    out: &mut Vec<Diff>,
    field: &'static str,
    got: &[i16],
    rec: &[u8],
    off: usize,
    count: usize,
) {
    for i in 0..count {
        let want = be16(rec, off + i * 2);
        if got[i] != want {
            out.push(Diff { field, index: i, got: got[i], want });
        }
    }
}

/// Replay one case and compare every array against the reference, field by
/// field. The input arrays are compared too: a VDI routine may not modify
/// them, and the dump captured them *after* the call precisely so that a
/// violation shows up.
pub fn check(dump: &[u8], case: &Case) {
    let _guard = VDI_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let wk = workstation();
    let mut pb = Pb::new();

    reset_vdi(&mut pb, wk);

    // The case's preludes, in order, exactly as tosdump/core/runner.c issues
    // them: after the reset, before the measured call, never recorded.
    for p in case.prelude {
        pb.contrl.fill(0);
        pb.intin[..p.intin.len()].copy_from_slice(p.intin);
        pb.ptsin[..p.ptsin.len()].copy_from_slice(p.ptsin);
        pb.contrl[0] = p.opcode;
        pb.contrl[1] = (p.ptsin.len() / 2) as i16;
        pb.contrl[3] = p.intin.len() as i16;
        pb.contrl[5] = p.sub;
        pb.contrl[6] = wk.handle;
        pb.call();
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

    pb.poison();
    pb.intin[..case.intin.len()].copy_from_slice(case.intin);
    pb.ptsin[..case.ptsin.len()].copy_from_slice(case.ptsin);
    pb.contrl[0] = case.opcode;
    pb.contrl[1] = (case.ptsin.len() / 2) as i16;
    pb.contrl[3] = case.intin.len() as i16;
    pb.contrl[5] = case.sub;
    pb.contrl[6] = wk.handle;

    let before: Vec<i16> = pb.contrl.to_vec();
    pb.call();

    // The reference must have gone in with the same contrl we did; if not,
    // the case table and the dump are out of step and nothing below means
    // anything.
    for i in 0..CONTRL_N {
        assert_eq!(
            before[i],
            be16(rec, OFF_CONTRL_BEFORE + i * 2),
            "case {}: contrl_before[{}] differs -- the dump was taken with a \
             different case table",
            case.id,
            i
        );
    }

    let mut diffs = Vec::new();
    cmp_field(&mut diffs, "contrl", &pb.contrl[..], rec, OFF_CONTRL, CONTRL_N);
    cmp_field(&mut diffs, "intin", &pb.intin[..], rec, OFF_INTIN, INTIN_N);
    cmp_field(&mut diffs, "ptsin", &pb.ptsin[..], rec, OFF_PTSIN, PTSIN_N);
    cmp_field(&mut diffs, "intout", &pb.intout[..], rec, OFF_INTOUT, INTOUT_N);
    cmp_field(&mut diffs, "ptsout", &pb.ptsout[..], rec, OFF_PTSOUT, PTSOUT_N);

    if !diffs.is_empty() {
        let mut msg = format!(
            "case {} (opcode {}, sub {}) -- {}\n  {} field(s) differ from the \
             EmuTOS reference:\n",
            case.id,
            case.opcode,
            case.sub,
            case.comment,
            diffs.len()
        );
        for d in diffs.iter().take(24) {
            msg += &format!(
                "    {}[{}]  NewTOS = 0x{:04X} ({})   reference = 0x{:04X} ({})\n",
                d.field, d.index, d.got as u16, d.got, d.want as u16, d.want
            );
        }
        if diffs.len() > 24 {
            msg += &format!("    ... and {} more\n", diffs.len() - 24);
        }
        // The trap's d0 (flip_y) is not observable through this entry point:
        // `tos_api::vdi::dispatch` returns `()`. Recorded in the dump at
        // offset 4, compared by tosdiff, out of reach here.
        let _ = OFF_RET;
        let _ = OFF_OPCODE;
        panic!("{}", msg);
    }
}
