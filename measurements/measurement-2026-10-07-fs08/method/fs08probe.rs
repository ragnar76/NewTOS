//! FS08 / INPUT01 measurement probe — SCRATCH ONLY, never part of the product.
//!
//! Counts, per report block, what the file path costs from the GEMDOS
//! dispatcher down to each SD command, and how long the input pump
//! (`pump_input`) was not called. Writes an appended plain-text report to
//! `C:\FS08.TXT` (and the same text to the serial console) whenever the
//! system has been idle on the file system for 1.5 s and the running caller
//! is inside a GEM (AES/VDI) call — i.e. after an image load has finished
//! and the application is back in its event loop.
//!
//! Single CPU, trap path with IRQs masked; relaxed atomics only.

#![allow(dead_code)]

use core::cell::UnsafeCell;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use crate::trap::TrapArgs;

// ── time ───────────────────────────────────────────────────────────────
#[inline(always)]
pub fn now() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let v: u64;
        unsafe { core::arch::asm!("isb", "mrs {0}, cntpct_el0", out(reg) v, options(nomem, nostack)) };
        v
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        0
    }
}
pub fn frq() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let v: u64;
        unsafe { core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) v, options(nomem, nostack)) };
        if v == 0 { 1 } else { v }
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        1
    }
}
fn us(t: u64) -> u64 {
    t.saturating_mul(1_000_000) / frq()
}
fn ms(t: u64) -> u64 {
    t.saturating_mul(1_000) / frq()
}

// ── counters ───────────────────────────────────────────────────────────
macro_rules! ctr { ($($n:ident),* $(,)?) => { $( static $n: AtomicU64 = AtomicU64::new(0); )* } }
const Z: AtomicU64 = AtomicU64::new(0);

static FLUSHING: AtomicBool = AtomicBool::new(false);
static DIRTY: AtomicBool = AtomicBool::new(false);

// per block (reset at every report)
static SVC_N: [AtomicU64; 5] = [Z; 5];
static SVC_T: [AtomicU64; 5] = [Z; 5];
static SVC_MAX: [AtomicU64; 5] = [Z; 5];
static GD_N: [AtomicU64; 256] = [Z; 256];
static GD_T: [AtomicU64; 256] = [Z; 256];
static GD_MAX: [AtomicU64; 256] = [Z; 256];
static FR_HIST: [AtomicU64; 7] = [Z; 7];
static EM_PH: [AtomicU64; 5] = [Z; 5];
ctr!(
    FR_N, FR_REQ, FR_RET, FR_T, FR_MAX,
    WALK_SKIP, WALK_ADV, FAT_CALLS, FAT_MISS, LAST_CLSIZB,
    BLK_RC, BLK_RS, BLK_WC, BLK_WS,
    EM_RN, EM_RT, EM_RMAX, EM_RMIN, EM_WN, EM_WT, EM_WMAX, EM_ERR,
    PX_N, PX_T,
    PUMP_N, PUMP_T, PUMP_MAX, GAP_MAX, GAP_MAX_AT, GAP_100, GAP_500, GAP_1000, GAP_SUM,
    IRQ_TIMER, IRQ_UART, IRQ_OTHER,
    BLOCK_START, BLOCK_NO, LAST_FLUSH_T, CACHE_BASE_H, CACHE_BASE_M, EV_LOST,
);
// monotonic (never reset) — for attribution deltas
ctr!(MONO_SECT, MONO_EMT, MONO_FRN, MONO_PXN, MONO_SVC_T, MONO_GD_T, LAST_PUMP, LAST_ACT);
// pump snapshot
ctr!(SNAP_SECT, SNAP_EMT, SNAP_FRN, SNAP_PXN, SNAP_SVC_T, SNAP_GD_T);

/// Serial-only writer installed by the kernel (`fn(&[u8])` as usize).
pub static SERIAL_OUT: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static M1: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];

fn add(c: &AtomicU64, v: u64) {
    c.fetch_add(v, Relaxed);
}
fn max(c: &AtomicU64, v: u64) {
    if v > c.load(Relaxed) {
        c.store(v, Relaxed);
    }
}
fn off() -> bool {
    FLUSHING.load(Relaxed)
}

// ── events ─────────────────────────────────────────────────────────────
#[derive(Clone, Copy)]
struct Ev {
    t: u64,
    kind: u8,
    v: [u64; 8],
    name: [u8; 48],
    nlen: u8,
}
const EV0: Ev = Ev { t: 0, kind: 0, v: [0; 8], name: [0; 48], nlen: 0 };
const NEV: usize = 160;
const K_FILE: u8 = 1;
const K_PEXEC: u8 = 2;
const K_GAP: u8 = 3;

#[derive(Clone, Copy)]
struct Hf {
    used: bool,
    h: u64,
    t_open: u64,
    n: u64,
    bytes: u64,
    ticks: u64,
    sect: u64,
    emt: u64,
    maxreq: u64,
    minreq: u64,
    name: [u8; 48],
    nlen: u8,
}
const HF0: Hf = Hf { used: false, h: 0, t_open: 0, n: 0, bytes: 0, ticks: 0, sect: 0, emt: 0, maxreq: 0, minreq: u64::MAX, name: [0; 48], nlen: 0 };

struct State {
    ev: [Ev; NEV],
    nev: usize,
    hf: [Hf; 24],
    pend_name: [u8; 48],
    pend_nlen: u8,
    px_name: [u8; 48],
    px_nlen: u8,
    out: [u8; 48 * 1024],
    olen: usize,
}
struct Cell(UnsafeCell<State>);
unsafe impl Sync for Cell {}
static ST: Cell = Cell(UnsafeCell::new(State {
    ev: [EV0; NEV],
    nev: 0,
    hf: [HF0; 24],
    pend_name: [0; 48],
    pend_nlen: 0,
    px_name: [0; 48],
    px_nlen: 0,
    out: [0; 48 * 1024],
    olen: 0,
}));
fn st() -> &'static mut State {
    unsafe { &mut *ST.0.get() }
}
fn copy_name(dst: &mut [u8; 48], src: &[u8]) -> u8 {
    // keep the tail (file name) if the path is long
    let s = if src.len() > 48 { &src[src.len() - 48..] } else { src };
    dst[..s.len()].copy_from_slice(s);
    s.len() as u8
}
fn push(kind: u8, t: u64, v: [u64; 8], name: &[u8]) {
    let s = st();
    if s.nev >= NEV {
        add(&EV_LOST, 1);
        return;
    }
    let mut e = Ev { t, kind, v, name: [0; 48], nlen: 0 };
    e.nlen = copy_name(&mut e.name, name);
    s.ev[s.nev] = e;
    s.nev += 1;
}

// ── hooks: kernel side ─────────────────────────────────────────────────
/// `cls`: 0 GEMDOS, 1 VDI, 2 AES, 3 BIOS, 4 XBIOS.
pub fn svc_done(cls: usize, t0: u64) {
    if off() {
        return;
    }
    let dt = now().wrapping_sub(t0);
    add(&SVC_N[cls], 1);
    add(&SVC_T[cls], dt);
    max(&SVC_MAX[cls], dt);
    add(&MONO_SVC_T, dt);
}
pub fn irq(kind: u8) {
    match kind {
        0 => add(&IRQ_TIMER, 1),
        1 => add(&IRQ_UART, 1),
        _ => add(&IRQ_OTHER, 1),
    }
}
pub fn set_m1(v: [u32; 6]) {
    for (i, x) in v.iter().enumerate() {
        M1[i].store(*x, Relaxed);
    }
}
pub fn note_pexec_name(p: &[u8]) {
    let s = st();
    s.px_nlen = copy_name(&mut s.px_name, p);
}
pub fn pexec_begin() -> (u64, u64, u64) {
    st().px_nlen = 0;
    (now(), MONO_SECT.load(Relaxed), MONO_EMT.load(Relaxed))
}
pub fn pexec_done(mode: u16, b: (u64, u64, u64)) {
    if off() {
        return;
    }
    let t1 = now();
    let dt = t1.wrapping_sub(b.0);
    add(&PX_N, 1);
    add(&PX_T, dt);
    add(&MONO_PXN, 1);
    // Pexec's GEMDOS time is not in the dispatcher (frame-mutating call);
    // book it as GEMDOS service time so gap attribution sees it.
    add(&MONO_GD_T, dt);
    LAST_ACT.store(t1, Relaxed);
    DIRTY.store(true, Relaxed);
    let s = st();
    let n = s.px_nlen as usize;
    let mut nm = [0u8; 48];
    nm[..n].copy_from_slice(&s.px_name[..n]);
    push(
        K_PEXEC,
        b.0,
        [mode as u64, dt, MONO_SECT.load(Relaxed) - b.1, MONO_EMT.load(Relaxed) - b.2, 0, 0, 0, 0],
        &nm[..n],
    );
}
/// Called at the END of `pump_input`, with the timestamp of its start.
pub fn on_pump(t0: u64) {
    if off() {
        return;
    }
    let t1 = now();
    add(&PUMP_N, 1);
    let d = t1.wrapping_sub(t0);
    add(&PUMP_T, d);
    max(&PUMP_MAX, d);
    let last = LAST_PUMP.swap(t1, Relaxed);
    if last != 0 {
        let gap = t0.wrapping_sub(last);
        let f = frq();
        if gap > f / 10 {
            add(&GAP_100, 1);
            add(&GAP_SUM, gap);
        }
        if gap > f / 2 {
            add(&GAP_500, 1);
        }
        if gap > f {
            add(&GAP_1000, 1);
        }
        if gap > GAP_MAX.load(Relaxed) {
            GAP_MAX.store(gap, Relaxed);
            GAP_MAX_AT.store(last, Relaxed);
        }
        if gap > f / 5 {
            push(
                K_GAP,
                last,
                [
                    gap,
                    MONO_SVC_T.load(Relaxed).wrapping_sub(SNAP_SVC_T.load(Relaxed)),
                    MONO_GD_T.load(Relaxed).wrapping_sub(SNAP_GD_T.load(Relaxed)),
                    MONO_EMT.load(Relaxed).wrapping_sub(SNAP_EMT.load(Relaxed)),
                    MONO_FRN.load(Relaxed).wrapping_sub(SNAP_FRN.load(Relaxed)),
                    MONO_PXN.load(Relaxed).wrapping_sub(SNAP_PXN.load(Relaxed)),
                    MONO_SECT.load(Relaxed).wrapping_sub(SNAP_SECT.load(Relaxed)),
                    0,
                ],
                b"",
            );
        }
    }
    SNAP_SVC_T.store(MONO_SVC_T.load(Relaxed), Relaxed);
    SNAP_GD_T.store(MONO_GD_T.load(Relaxed), Relaxed);
    SNAP_EMT.store(MONO_EMT.load(Relaxed), Relaxed);
    SNAP_FRN.store(MONO_FRN.load(Relaxed), Relaxed);
    SNAP_PXN.store(MONO_PXN.load(Relaxed), Relaxed);
    SNAP_SECT.store(MONO_SECT.load(Relaxed), Relaxed);
}
pub fn blk_read(sectors: u64) {
    if off() {
        return;
    }
    add(&BLK_RC, 1);
    add(&BLK_RS, sectors);
}
pub fn blk_write(sectors: u64) {
    if off() {
        return;
    }
    add(&BLK_WC, 1);
    add(&BLK_WS, sectors);
}
/// One `read_one`: phase ticks dat-inhibit, cmd17, read-ready, pio, data-done; total; ok.
pub fn emmc_read(ph: [u64; 5], total: u64, ok: bool) {
    if off() {
        return;
    }
    add(&EM_RN, 1);
    add(&EM_RT, total);
    max(&EM_RMAX, total);
    let mn = EM_RMIN.load(Relaxed);
    if mn == 0 || total < mn {
        EM_RMIN.store(total, Relaxed);
    }
    for i in 0..5 {
        add(&EM_PH[i], ph[i]);
    }
    if !ok {
        add(&EM_ERR, 1);
    }
    add(&MONO_SECT, 1);
    add(&MONO_EMT, total);
    LAST_ACT.store(now(), Relaxed);
    DIRTY.store(true, Relaxed);
}
pub fn emmc_write(total: u64, ok: bool) {
    if off() {
        return;
    }
    add(&EM_WN, 1);
    add(&EM_WT, total);
    max(&EM_WMAX, total);
    if !ok {
        add(&EM_ERR, 1);
    }
    add(&MONO_EMT, total);
    LAST_ACT.store(now(), Relaxed);
    DIRTY.store(true, Relaxed);
}

// ── hooks: file system side ────────────────────────────────────────────
pub fn note_open_path(p: &[u8]) {
    if off() {
        return;
    }
    let s = st();
    s.pend_nlen = copy_name(&mut s.pend_name, p);
}
pub fn walk_skip() {
    if !off() {
        add(&WALK_SKIP, 1);
    }
}
pub fn walk_adv() {
    if !off() {
        add(&WALK_ADV, 1);
    }
}
pub fn fat_entry_done(miss_delta: u32) {
    if !off() {
        add(&FAT_CALLS, 1);
        add(&FAT_MISS, miss_delta as u64);
    }
}
pub fn clsizb(v: u32) {
    LAST_CLSIZB.store(v as u64, Relaxed);
}

pub struct GdStart {
    t: u64,
    sect: u64,
    emt: u64,
}
pub fn gd_begin() -> GdStart {
    GdStart { t: now(), sect: MONO_SECT.load(Relaxed), emt: MONO_EMT.load(Relaxed) }
}
pub fn gd_end(args: &TrapArgs, s: GdStart, ret: i32) {
    if off() {
        return;
    }
    let t1 = now();
    let dt = t1.wrapping_sub(s.t);
    let op = (args.func & 0xFF) as usize;
    if args.func > 0xFF {
        return;
    }
    add(&GD_N[op], 1);
    add(&GD_T[op], dt);
    max(&GD_MAX[op], dt);
    add(&MONO_GD_T, dt);
    let st = st();
    match op {
        0x3D if ret >= 0 => {
            // bind the pending name to the handle
            let h = ret as u64;
            let slot = st.hf.iter().position(|x| x.used && x.h == h).or_else(|| st.hf.iter().position(|x| !x.used));
            if let Some(i) = slot {
                let mut f = HF0;
                f.used = true;
                f.h = h;
                f.t_open = t1;
                f.nlen = st.pend_nlen;
                f.name = st.pend_name;
                st.hf[i] = f;
            }
            LAST_ACT.store(t1, Relaxed);
            DIRTY.store(true, Relaxed);
        }
        0x3F => {
            let len = args.regs[1] as i32 as i64;
            let h = args.regs[0] as u16 as u64;
            let l = if len < 0 { 0 } else { len as u64 };
            let b = match l {
                0..=512 => 0,
                513..=2048 => 1,
                2049..=4096 => 2,
                4097..=16384 => 3,
                16385..=65536 => 4,
                65537..=1048576 => 5,
                _ => 6,
            };
            add(&FR_HIST[b], 1);
            add(&FR_N, 1);
            add(&FR_REQ, l);
            if ret > 0 {
                add(&FR_RET, ret as u64);
            }
            add(&FR_T, dt);
            max(&FR_MAX, dt);
            add(&MONO_FRN, 1);
            if let Some(f) = st.hf.iter_mut().find(|x| x.used && x.h == h) {
                f.n += 1;
                if ret > 0 {
                    f.bytes += ret as u64;
                }
                f.ticks += dt;
                f.sect += MONO_SECT.load(Relaxed) - s.sect;
                f.emt += MONO_EMT.load(Relaxed) - s.emt;
                if l > f.maxreq {
                    f.maxreq = l;
                }
                if l < f.minreq {
                    f.minreq = l;
                }
            }
            LAST_ACT.store(t1, Relaxed);
            DIRTY.store(true, Relaxed);
        }
        0x3E => {
            let h = args.regs[0] as u16 as u64;
            if let Some(f) = st.hf.iter_mut().find(|x| x.used && x.h == h) {
                let ff = *f;
                f.used = false;
                if ff.n > 0 {
                    push(
                        K_FILE,
                        ff.t_open,
                        [ff.n, ff.bytes, ff.ticks, ff.sect, ff.emt, t1 - ff.t_open, ff.minreq, ff.maxreq],
                        &ff.name[..ff.nlen as usize],
                    );
                }
            }
            LAST_ACT.store(t1, Relaxed);
            DIRTY.store(true, Relaxed);
        }
        0x3C | 0x40 | 0x41 | 0x42 | 0x4E | 0x4F | 0x36 => {
            LAST_ACT.store(t1, Relaxed);
            DIRTY.store(true, Relaxed);
        }
        _ => {}
    }
}

// ── report ─────────────────────────────────────────────────────────────
struct W;
impl Write for W {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let st = st();
        for &b in s.as_bytes() {
            if b == b'\n' {
                if st.olen + 2 <= st.out.len() {
                    st.out[st.olen] = b'\r';
                    st.out[st.olen + 1] = b'\n';
                    st.olen += 2;
                }
            } else if st.olen < st.out.len() {
                st.out[st.olen] = b;
                st.olen += 1;
            }
        }
        Ok(())
    }
}
fn ld(c: &AtomicU64) -> u64 {
    c.load(Relaxed)
}

fn format_block(first: bool) {
    let mut w = W;
    let t = now();
    let f = frq();
    let span = t.wrapping_sub(ld(&BLOCK_START));
    let bn = BLOCK_NO.fetch_add(1, Relaxed) + 1;
    let _ = writeln!(w, "==== FS08 block {} : t={} ms since boot, span {} ms ====", bn, ms(t), ms(span));
    if first {
        let m = |i: usize| M1[i].load(Relaxed);
        let _ = writeln!(w, "CNTFRQ {} Hz", f);
        let _ = writeln!(
            w,
            "emmc M1 SLOTISR_VER=0x{:08x} CAPABILITIES=0x{:08x} (base {} MHz) CONTROL0=0x{:08x} (DWIDTH4={} HS={}) CONTROL1=0x{:08x} div={}",
            m(0),
            m(1),
            (m(1) >> 8) & 0xFF,
            m(2),
            (m(2) >> 1) & 1,
            (m(2) >> 2) & 1,
            m(3),
            ((m(3) >> 8) & 0xFF) | (((m(3) >> 6) & 3) << 8)
        );
        let _ = writeln!(w, "emmc M1 mailbox EMMC(1)={} Hz EMMC2(12)={} Hz", m(4), m(5));
    }
    let _ = writeln!(w, "previous report write {} ms (not in any number here)", ms(ld(&LAST_FLUSH_T)));
    let names = ["gemdos", "vdi", "aes", "bios", "xbios"];
    let _ = write!(w, "svc");
    for i in 0..5 {
        let _ = write!(w, "  {} n={} {}ms max={}ms", names[i], ld(&SVC_N[i]), ms(ld(&SVC_T[i])), ms(ld(&SVC_MAX[i])));
    }
    let _ = writeln!(w);
    let _ = writeln!(
        w,
        "fread n={} req={}KB got={}KB time={}ms max={}ms  => {} KB/s inside Fread",
        ld(&FR_N),
        ld(&FR_REQ) / 1024,
        ld(&FR_RET) / 1024,
        ms(ld(&FR_T)),
        ms(ld(&FR_MAX)),
        if ld(&FR_T) > 0 { ld(&FR_RET) * f / ld(&FR_T) / 1024 } else { 0 }
    );
    let _ = writeln!(
        w,
        "fread sizes <=512:{} <=2K:{} <=4K:{} <=16K:{} <=64K:{} <=1M:{} >1M:{}",
        ld(&FR_HIST[0]),
        ld(&FR_HIST[1]),
        ld(&FR_HIST[2]),
        ld(&FR_HIST[3]),
        ld(&FR_HIST[4]),
        ld(&FR_HIST[5]),
        ld(&FR_HIST[6])
    );
    let _ = writeln!(
        w,
        "fat chain-walk from file start: {} fat_entry  per-cluster advance: {}  fat_entry total {} (cache misses inside {})  cluster {} B",
        ld(&WALK_SKIP),
        ld(&WALK_ADV),
        ld(&FAT_CALLS),
        ld(&FAT_MISS),
        ld(&LAST_CLSIZB)
    );
    let (h, m) = crate::bdos::fs::cache::stats();
    let _ = writeln!(
        w,
        "cache hit={} miss={}",
        (h as u64).wrapping_sub(ld(&CACHE_BASE_H)),
        (m as u64).wrapping_sub(ld(&CACHE_BASE_M))
    );
    let _ = writeln!(
        w,
        "blk read calls={} sectors={} | write calls={} sectors={}",
        ld(&BLK_RC),
        ld(&BLK_RS),
        ld(&BLK_WC),
        ld(&BLK_WS)
    );
    let rn = ld(&EM_RN).max(1);
    let _ = writeln!(
        w,
        "emmc read_one n={} total={}ms avg={}us min={}us max={}us errors={}",
        ld(&EM_RN),
        ms(ld(&EM_RT)),
        us(ld(&EM_RT)) / rn,
        us(ld(&EM_RMIN)),
        us(ld(&EM_RMAX)),
        ld(&EM_ERR)
    );
    let _ = writeln!(
        w,
        "emmc read phases avg us: dat_inhibit={} cmd17={} read_rdy={} pio={} data_done={}",
        us(ld(&EM_PH[0])) / rn,
        us(ld(&EM_PH[1])) / rn,
        us(ld(&EM_PH[2])) / rn,
        us(ld(&EM_PH[3])) / rn,
        us(ld(&EM_PH[4])) / rn
    );
    let _ = writeln!(
        w,
        "emmc write_one n={} total={}ms max={}us",
        ld(&EM_WN),
        ms(ld(&EM_WT)),
        us(ld(&EM_WMAX))
    );
    let _ = writeln!(w, "pexec n={} total={}ms", ld(&PX_N), ms(ld(&PX_T)));
    let _ = writeln!(
        w,
        "pump_input n={} own time={}ms max={}us | longest gap {}ms at t={}ms | gaps>100ms {} >500ms {} >1s {} sum {}ms",
        ld(&PUMP_N),
        ms(ld(&PUMP_T)),
        us(ld(&PUMP_MAX)),
        ms(ld(&GAP_MAX)),
        ms(ld(&GAP_MAX_AT)),
        ld(&GAP_100),
        ld(&GAP_500),
        ld(&GAP_1000),
        ms(ld(&GAP_SUM))
    );
    let _ = writeln!(w, "irq timer={} uart={} other={}", ld(&IRQ_TIMER), ld(&IRQ_UART), ld(&IRQ_OTHER));
    let _ = write!(w, "gemdos ops (op:n/ms/maxms):");
    for op in 0..256 {
        let n = ld(&GD_N[op]);
        if n > 0 {
            let _ = write!(w, " {:02x}:{}/{}/{}", op, n, ms(ld(&GD_T[op])), ms(ld(&GD_MAX[op])));
        }
    }
    let _ = writeln!(w);
    let s = st();
    let nev = s.nev;
    let _ = writeln!(w, "events ({} , lost {}):", nev, ld(&EV_LOST));
    for i in 0..nev {
        let e = s.ev[i];
        let name = core::str::from_utf8(&e.name[..e.nlen as usize]).unwrap_or("?");
        match e.kind {
            K_FILE => {
                let _ = writeln!(
                    w,
                    " t={}ms FILE {} freads={} bytes={} req={}..{} t_fread={}ms sectors={} emmc={}ms open..close={}ms",
                    ms(e.t),
                    name,
                    e.v[0],
                    e.v[1],
                    e.v[6],
                    e.v[7],
                    ms(e.v[2]),
                    e.v[3],
                    ms(e.v[4]),
                    ms(e.v[5])
                );
            }
            K_PEXEC => {
                let _ = writeln!(
                    w,
                    " t={}ms PEXEC mode={} {} handler={}ms sectors={} emmc={}ms",
                    ms(e.t),
                    e.v[0],
                    name,
                    ms(e.v[1]),
                    e.v[2],
                    ms(e.v[3])
                );
            }
            K_GAP => {
                let _ = writeln!(
                    w,
                    " t={}ms GAP no pump_input for {}ms: svc={}ms gemdos={}ms emmc={}ms freads={} pexec={} sectors={} => user/other={}ms",
                    ms(e.t),
                    ms(e.v[0]),
                    ms(e.v[1]),
                    ms(e.v[2]),
                    ms(e.v[3]),
                    e.v[4],
                    e.v[5],
                    e.v[6],
                    ms(e.v[0].saturating_sub(e.v[1]))
                );
            }
            _ => {}
        }
    }
    let _ = writeln!(w, "==== end block {} ====", bn);
    let _ = writeln!(w);
}

fn reset_block() {
    for a in SVC_N.iter().chain(SVC_T.iter()).chain(SVC_MAX.iter()) {
        a.store(0, Relaxed);
    }
    for a in GD_N.iter().chain(GD_T.iter()).chain(GD_MAX.iter()) {
        a.store(0, Relaxed);
    }
    for a in FR_HIST.iter().chain(EM_PH.iter()) {
        a.store(0, Relaxed);
    }
    for a in [
        &FR_N, &FR_REQ, &FR_RET, &FR_T, &FR_MAX, &WALK_SKIP, &WALK_ADV, &FAT_CALLS, &FAT_MISS, &BLK_RC, &BLK_RS,
        &BLK_WC, &BLK_WS, &EM_RN, &EM_RT, &EM_RMAX, &EM_RMIN, &EM_WN, &EM_WT, &EM_WMAX, &EM_ERR, &PX_N, &PX_T,
        &PUMP_N, &PUMP_T, &PUMP_MAX, &GAP_MAX, &GAP_MAX_AT, &GAP_100, &GAP_500, &GAP_1000, &GAP_SUM, &IRQ_TIMER,
        &IRQ_UART, &IRQ_OTHER, &EV_LOST,
    ] {
        a.store(0, Relaxed);
    }
    st().nev = 0;
    let (h, m) = crate::bdos::fs::cache::stats();
    CACHE_BASE_H.store(h as u64, Relaxed);
    CACHE_BASE_M.store(m as u64, Relaxed);
    BLOCK_START.store(now(), Relaxed);
}

static PATH: &[u8] = b"C:\\FS08.TXT\0";

fn kargs(regs: [u64; 8], func: u64) -> TrapArgs {
    TrapArgs { regs, func, svc_imm: 1, space: crate::uaccess::Space::Kernel }
}

fn write_out() {
    use crate::bdos::fs::file;
    let s = st();
    let mut h = file::fopen(&kargs([PATH.as_ptr() as u64, 1, 0, 0, 0, 0, 0, 0], 0x3D));
    if h < 0 {
        h = file::fcreate(&kargs([PATH.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0], 0x3C));
        if h < 0 {
            return;
        }
    } else {
        let _ = file::fseek(&kargs([0, h as u64, 2, 0, 0, 0, 0, 0], 0x42));
    }
    let _ = file::fwrite(&kargs([h as u64, s.olen as u64, s.out.as_ptr() as u64, 0, 0, 0, 0, 0], 0x40));
    let _ = file::fclose(&kargs([h as u64, 0, 0, 0, 0, 0, 0, 0], 0x3E));
}

static FIRST_DONE: AtomicBool = AtomicBool::new(false);

static IN_GEMDOS: AtomicBool = AtomicBool::new(false);
/// Set around every GEMDOS dispatch (the flush must never nest into the FS).
pub fn in_gemdos(v: bool) {
    IN_GEMDOS.store(v, Relaxed);
}

fn ready_to_flush() -> bool {
    if off() || !DIRTY.load(Relaxed) || IN_GEMDOS.load(Relaxed) {
        return false;
    }
    now().wrapping_sub(LAST_ACT.load(Relaxed)) >= frq() * 3 / 2
}

fn do_flush() {
    let t = now();
    FLUSHING.store(true, Relaxed);
    st().olen = 0;
    let first = !FIRST_DONE.swap(true, Relaxed);
    format_block(first);
    // serial copy (UART only, never the framebuffer console)
    let f = SERIAL_OUT.load(Relaxed);
    if f != 0 {
        let fp: fn(&[u8]) = unsafe { core::mem::transmute::<usize, fn(&[u8])>(f) };
        let s = st();
        fp(&s.out[..s.olen]);
    }
    write_out();
    let t2 = now();
    LAST_FLUSH_T.store(t2.wrapping_sub(t), Relaxed);
    reset_block();
    DIRTY.store(false, Relaxed);
    // the flush itself is not an input gap
    LAST_PUMP.store(now(), Relaxed);
    FLUSHING.store(false, Relaxed);
}

/// Called from the kernel's trap tail after every dispatched call
/// (kernel exception stack).
pub fn maybe_flush(is_gem: bool) {
    if !is_gem || !ready_to_flush() {
        return;
    }
    do_flush();
}

/// Private 64 KiB stack for a flush started from inside the AES wait loop
/// (pump_input runs on the 16 KiB AES fiber stack, which must not carry
/// the FAT write path).
#[repr(C, align(16))]
struct FlushStack([u8; 64 * 1024]);
struct StackCell(UnsafeCell<FlushStack>);
unsafe impl Sync for StackCell {}
static FLUSH_STACK: StackCell = StackCell(UnsafeCell::new(FlushStack([0; 64 * 1024])));

extern "C" fn flush_tramp() {
    do_flush();
}

/// Called at the START of `pump_input` (IRQs still masked).
pub fn maybe_flush_from_pump() {
    if !ready_to_flush() {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let top = (FLUSH_STACK.0.get() as usize) + 64 * 1024;
        core::arch::asm!(
            "mov x20, sp",
            "mov sp, {stk}",
            "blr {f}",
            "mov sp, x20",
            stk = in(reg) top,
            f = in(reg) flush_tramp as extern "C" fn() as usize,
            out("x20") _,
            clobber_abi("C"),
        );
    }
}
