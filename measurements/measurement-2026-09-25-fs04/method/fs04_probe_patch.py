#!/usr/bin/env python3
"""FS04-TEMP measurement scaffolding.

Applies, to a SCRATCH copy of the tree (never the shared tree):

  1. tos-api/src/fs04probe.rs           -- counters + CNTPCT_EL0 tick source
  2. tos-api/src/lib.rs                 -- `pub mod fs04probe;`
  3. tos-api/src/gemdos/mod.rs          -- per-opcode count/ticks at dispatch
  4. tos-api/src/bdos/fs/cache.rs       -- sector-cache hit/miss/writeback
  5. kernel/src/drivers/bcm2711/emmc.rs -- per-COMMAND timing, split into the
                                           five phases of a PIO single block
  6. kernel/src/desk/fs04drv.rs         -- the driver + the report printer
  7. kernel/src/desk/mod.rs             -- run the driver after "desktop up"

Usage: fs04_probe_patch.py <scratch-tree>
"""
import sys, os, re

ws = sys.argv[1]
mode = sys.argv[2] if len(sys.argv) > 2 else 'driver'   # 'driver' | 'gesture'

def rd(p):
    return open(os.path.join(ws, p)).read()
def wr(p, s):
    open(os.path.join(ws, p), 'w').write(s)
def sub1(p, old, new):
    s = rd(p)
    assert s.count(old) == 1, (p, s.count(old), old[:70])
    wr(p, s.replace(old, new, 1))

# ── 1. the probe module ────────────────────────────────────────────────────
wr('tos-api/src/fs04probe.rs', r'''
//! FS04-TEMP — measurement probe (audit scaffolding; NOT product code).
//!
//! Counters live in `.bss`, are updated with relaxed atomics on the
//! single-CPU trap path, and are printed ONCE after the measured operation
//! finishes. Nothing is written to the serial port while the operation runs,
//! because at 115200 baud a single character costs ~87 us and would dominate
//! the very distribution being measured.

use core::sync::atomic::{AtomicU64, Ordering};

/// Free-running physical counter, `CNTPCT_EL0` (54 MHz on the Pi 4 and on
/// QEMU `-M raspi4b`).
#[inline(always)]
pub fn now() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let v: u64;
        // SAFETY: read-only system-register read, permitted at EL1.
        unsafe {
            core::arch::asm!("mrs {0}, cntpct_el0", out(reg) v,
                             options(nomem, nostack, preserves_flags));
        }
        v
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        0
    }
}

/// `CNTFRQ_EL0` — ticks per second.
#[inline(always)]
pub fn freq() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let v: u64;
        // SAFETY: read-only system-register read, permitted at EL1.
        unsafe {
            core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) v,
                             options(nomem, nostack, preserves_flags));
        }
        v
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        1
    }
}

pub const NSLOT: usize = 40;

pub static CNT: [AtomicU64; NSLOT] = [const { AtomicU64::new(0) }; NSLOT];
pub static TCK: [AtomicU64; NSLOT] = [const { AtomicU64::new(0) }; NSLOT];
pub static MAXT: [AtomicU64; NSLOT] = [const { AtomicU64::new(0) }; NSLOT];
pub static MINT: [AtomicU64; NSLOT] = [const { AtomicU64::new(u64::MAX) }; NSLOT];

/// Per-GEMDOS-opcode call count / ticks / worst single call.
pub static GCNT: [AtomicU64; 256] = [const { AtomicU64::new(0) }; 256];
pub static GTCK: [AtomicU64; 256] = [const { AtomicU64::new(0) }; 256];
pub static GMAX: [AtomicU64; 256] = [const { AtomicU64::new(0) }; 256];

pub const NAMES: [&str; NSLOT] = [
    "dev.read_one",      // 0
    "dev.write_one",     // 1
    "dev.r.wait_dat",    // 2
    "dev.r.cmd17",       // 3
    "dev.r.read_rdy",    // 4
    "dev.r.pio",         // 5
    "dev.r.data_done",   // 6
    "dev.w.wait_dat",    // 7
    "dev.w.cmd24",       // 8
    "dev.w.write_rdy",   // 9
    "dev.w.pio",         // 10
    "dev.w.data_done",   // 11
    "cache.read.hit",    // 12
    "cache.read.miss",   // 13
    "cache.write.hit",   // 14
    "cache.write.miss",  // 15
    "cache.evict.wb",    // 16
    "cache.flush.wb",    // 17
    "cache.flush.call",  // 18
    "gemdos.dispatch",   // 19
    "desk.fun_op",       // 20
    "desk.count_pass",   // 21
    "desk.copy_pass",    // 22
    "desk.buf_alloc",    // 23
    "dofcopy.Fopen",     // 24
    "dofcopy.outname",   // 25
    "dofcopy.Fcreate",   // 26
    "dofcopy.Fread",     // 27
    "dofcopy.Fwrite",    // 28
    "dofcopy.Fdatime",   // 29
    "dofcopy.Fclose",    // 30
    "dofcopy.total",     // 31
    "blk.read_blocks",   // 32  CNT=calls  TCK=sectors
    "blk.write_blocks",  // 33  CNT=calls  TCK=sectors
    "desk.fun_drag",     // 34
    "desk.rebuild_win",  // 35
    "desk.redraw_desk",  // 36
    "fb.clean",          // 37  CNT=calls  TCK=bytes  MAX=largest span
    "fb.clean_lines",    // 38  CNT=calls  TCK=64-byte cache lines
    "emmc.spin",         // 39  CNT=waits  TCK=poll iterations  MAX=worst wait
];

pub mod slot {
    pub const DEV_READ: usize = 0;
    pub const DEV_WRITE: usize = 1;
    pub const DEV_R_WAITDAT: usize = 2;
    pub const DEV_R_CMD: usize = 3;
    pub const DEV_R_RDY: usize = 4;
    pub const DEV_R_PIO: usize = 5;
    pub const DEV_R_DONE: usize = 6;
    pub const DEV_W_WAITDAT: usize = 7;
    pub const DEV_W_CMD: usize = 8;
    pub const DEV_W_RDY: usize = 9;
    pub const DEV_W_PIO: usize = 10;
    pub const DEV_W_DONE: usize = 11;
    pub const C_R_HIT: usize = 12;
    pub const C_R_MISS: usize = 13;
    pub const C_W_HIT: usize = 14;
    pub const C_W_MISS: usize = 15;
    pub const C_EVICT_WB: usize = 16;
    pub const C_FLUSH_WB: usize = 17;
    pub const C_FLUSH_CALL: usize = 18;
    pub const GEMDOS: usize = 19;
    pub const FUN_OP: usize = 20;
    pub const COUNT_PASS: usize = 21;
    pub const COPY_PASS: usize = 22;
    pub const BUF_ALLOC: usize = 23;
    pub const DC_OPEN: usize = 24;
    pub const DC_OUTNAME: usize = 25;
    pub const DC_CREATE: usize = 26;
    pub const DC_READ: usize = 27;
    pub const DC_WRITE: usize = 28;
    pub const DC_SETDT: usize = 29;
    pub const DC_CLOSE: usize = 30;
    pub const DC_TOTAL: usize = 31;
    pub const BLK_READ: usize = 32;
    pub const BLK_WRITE: usize = 33;
    pub const DRAG_TOTAL: usize = 34;
    pub const REBUILD: usize = 35;
    pub const REDRAW: usize = 36;
    pub const FB_CLEAN: usize = 37;
    pub const FB_CLEAN_LINES: usize = 38;
    pub const EMMC_SPIN: usize = 39;
}

/// Record one occurrence of `s` lasting `dt` ticks.
#[inline(always)]
pub fn add(s: usize, dt: u64) {
    CNT[s].fetch_add(1, Ordering::Relaxed);
    TCK[s].fetch_add(dt, Ordering::Relaxed);
    MAXT[s].fetch_max(dt, Ordering::Relaxed);
    MINT[s].fetch_min(dt, Ordering::Relaxed);
}

/// Record one occurrence of `s` with no duration.
#[inline(always)]
pub fn bump(s: usize) {
    CNT[s].fetch_add(1, Ordering::Relaxed);
}

/// Record one occurrence of `s` carrying an amount (sectors, bytes).
#[inline(always)]
pub fn amount(s: usize, n: u64) {
    CNT[s].fetch_add(1, Ordering::Relaxed);
    TCK[s].fetch_add(n, Ordering::Relaxed);
    MAXT[s].fetch_max(n, Ordering::Relaxed);
    MINT[s].fetch_min(n, Ordering::Relaxed);
}

/// Record one GEMDOS call.
#[inline(always)]
pub fn gadd(op: usize, dt: u64) {
    let op = op & 0xFF;
    GCNT[op].fetch_add(1, Ordering::Relaxed);
    GTCK[op].fetch_add(dt, Ordering::Relaxed);
    GMAX[op].fetch_max(dt, Ordering::Relaxed);
}

pub fn reset() {
    for i in 0..NSLOT {
        CNT[i].store(0, Ordering::Relaxed);
        TCK[i].store(0, Ordering::Relaxed);
        MAXT[i].store(0, Ordering::Relaxed);
        MINT[i].store(u64::MAX, Ordering::Relaxed);
    }
    for i in 0..256 {
        GCNT[i].store(0, Ordering::Relaxed);
        GTCK[i].store(0, Ordering::Relaxed);
        GMAX[i].store(0, Ordering::Relaxed);
    }
}
''')

# ── 2. register the module ─────────────────────────────────────────────────
sub1('tos-api/src/lib.rs', 'pub mod gemdos;', 'pub mod fs04probe; // FS04-TEMP\npub mod gemdos;')

# ── 3. GEMDOS dispatcher ───────────────────────────────────────────────────
sub1('tos-api/src/gemdos/mod.rs',
     """    let t0 = trace::call_begin();
    let ret = dispatch_inner(args);
    trace::call_end(args, t0, ret);
    ret""",
     """    let t0 = trace::call_begin();
    let p0 = crate::fs04probe::now(); // FS04-TEMP
    let ret = dispatch_inner(args);
    // FS04-TEMP: per-opcode call count + ticks, and the dispatcher total.
    {
        let dt = crate::fs04probe::now().wrapping_sub(p0);
        crate::fs04probe::gadd(args.func_id() as usize, dt);
        crate::fs04probe::add(crate::fs04probe::slot::GEMDOS, dt);
    }
    trace::call_end(args, t0, ret);
    ret""")

# ── 4. sector cache ────────────────────────────────────────────────────────
c = 'tos-api/src/bdos/fs/cache.rs'
sub1(c, """    if slots[v].valid && slots[v].dirty {
        if let Some(b) = block() {
            b.write_blocks(slots[v].lba, &slots[v].data);
        }
        slots[v].dirty = false;
    }""",
     """    if slots[v].valid && slots[v].dirty {
        crate::fs04probe::bump(crate::fs04probe::slot::C_EVICT_WB); // FS04-TEMP
        if let Some(b) = block() {
            b.write_blocks(slots[v].lba, &slots[v].data);
        }
        slots[v].dirty = false;
    }""")
sub1(c, """            HITS.fetch_add(1, Ordering::Relaxed);
            return true;""",
     """            HITS.fetch_add(1, Ordering::Relaxed);
            crate::fs04probe::bump(crate::fs04probe::slot::C_R_HIT); // FS04-TEMP
            return true;""")
sub1(c, """    MISSES.fetch_add(1, Ordering::Relaxed);
""",
     """    MISSES.fetch_add(1, Ordering::Relaxed);
    crate::fs04probe::bump(crate::fs04probe::slot::C_R_MISS); // FS04-TEMP
""")
sub1(c, """        if s.valid && s.lba == lba {
            s.data.copy_from_slice(data);
            s.dirty = true;
            return true;
        }
    }
    let v = evict(slots);""",
     """        if s.valid && s.lba == lba {
            s.data.copy_from_slice(data);
            s.dirty = true;
            crate::fs04probe::bump(crate::fs04probe::slot::C_W_HIT); // FS04-TEMP
            return true;
        }
    }
    crate::fs04probe::bump(crate::fs04probe::slot::C_W_MISS); // FS04-TEMP
    let v = evict(slots);""")
sub1(c, """pub fn flush() -> bool {
    let slots = slots();
    let mut ok = true;
    for s in slots.iter_mut() {
        if s.valid && s.dirty {""",
     """pub fn flush() -> bool {
    crate::fs04probe::bump(crate::fs04probe::slot::C_FLUSH_CALL); // FS04-TEMP
    let slots = slots();
    let mut ok = true;
    for s in slots.iter_mut() {
        if s.valid && s.dirty {
            crate::fs04probe::bump(crate::fs04probe::slot::C_FLUSH_WB); // FS04-TEMP""")

# ── 5. EMMC driver: the five-phase split, re-anchored on the new driver ───
#
# c6cf484 rewrote the driver: the waits are time-budgeted per `Phase`, every
# failure goes through `recover`, and the error bits are kept and counted.
# Two consequences for the probe:
#
#  * The instrumentation is re-anchored on the NEW functions.  Nothing here
#    reproduces a function body -- each wait keeps its own body and only gets
#    a thin timing wrapper in front of it (`*_inner`), so a later change
#    inside a wait cannot be silently reverted by this patch.
#  * The old probe recorded a phase only AFTER it had succeeded, because the
#    `add(...)` sat behind the `?` / early return.  Exactly the expensive case
#    -- a wait that ran out of budget -- was therefore missing from the
#    totals, which is why `dev.write_one n=11` stood against
#    `dev.w.wait_dat n=23` in raw/pi04-FS04.TXT and 0.8 s in the device could
#    not be reconciled with 1.6 s in `Fwrite`.  Every wrapper below records on
#    EVERY exit path.
e = 'kernel/src/drivers/bcm2711/emmc.rs'

sub1(e, """impl Emmc {
    /// Poll until `(rd(off) & mask) == 0` within `phase`'s time budget.""",
     """// ── FS04-TEMP: measurement scaffolding ────────────────────────────────
//
// Which sector operation is in flight.  `Phase::DatInhibit` and the command
// handshake are shared by reads and writes; the report keeps them on separate
// lines (`dev.r.wait_dat` / `dev.w.wait_dat`, `dev.r.cmd17` / `dev.w.cmd24`)
// because what `DatInhibit` waits out before a WRITE is the previous write's
// programming busy (audit 2.4) and before a READ it is not.
const FS04_DIR_NONE: u8 = 0;
const FS04_DIR_READ: u8 = 1;
const FS04_DIR_WRITE: u8 = 2;
static FS04_DIR: core::sync::atomic::AtomicU8 =
    core::sync::atomic::AtomicU8::new(FS04_DIR_NONE);

/// FS04-TEMP: `(direction, phase)` -> the report slot the old block used.
/// `CmdInhibit`/`CmdDone` deliberately map to nothing: they happen inside
/// `command()`, which is timed as a whole, and booking them here as well
/// would count the same microseconds twice.
fn fs04_phase_slot(phase: Phase) -> Option<usize> {
    use tos_api::fs04probe::slot as sl;
    match (FS04_DIR.load(Ordering::Relaxed), phase) {
        (FS04_DIR_READ, Phase::DatInhibit) => Some(sl::DEV_R_WAITDAT),
        (FS04_DIR_READ, Phase::ReadRdy) => Some(sl::DEV_R_RDY),
        (FS04_DIR_READ, Phase::ReadDone) => Some(sl::DEV_R_DONE),
        (FS04_DIR_WRITE, Phase::DatInhibit) => Some(sl::DEV_W_WAITDAT),
        (FS04_DIR_WRITE, Phase::WriteRdy) => Some(sl::DEV_W_RDY),
        (FS04_DIR_WRITE, Phase::WriteDone) => Some(sl::DEV_W_DONE),
        _ => None,
    }
}
// ── end FS04-TEMP ─────────────────────────────────────────────────────

impl Emmc {
    /// FS04-TEMP: time the wait on **every** exit path and book it to the
    /// read or the write line.  The body is untouched below.
    fn wait_clear(off: usize, mask: u32, phase: Phase) -> bool {
        let p = tos_api::fs04probe::now();
        let r = Self::wait_clear_inner(off, mask, phase);
        if let Some(sl) = fs04_phase_slot(phase) {
            tos_api::fs04probe::add(sl, tos_api::fs04probe::now().wrapping_sub(p));
        }
        r
    }

    /// Poll until `(rd(off) & mask) == 0` within `phase`'s time budget.""")

sub1(e, "    fn wait_clear(off: usize, mask: u32, phase: Phase) -> bool {\n        let mut budget",
     "    fn wait_clear_inner(off: usize, mask: u32, phase: Phase) -> bool {\n        let mut budget")

sub1(e, "    /// Wait for `flag` in INTERRUPT (clearing it), or an error / timeout.",
     """    /// FS04-TEMP: time the wait on **every** exit path (success, error bit
    /// and expired budget alike) and book it to the read or write line.
    fn wait_interrupt(flag: u32, phase: Phase) -> Result<(), BlockError> {
        let p = tos_api::fs04probe::now();
        let r = Self::wait_interrupt_inner(flag, phase);
        if let Some(sl) = fs04_phase_slot(phase) {
            tos_api::fs04probe::add(sl, tos_api::fs04probe::now().wrapping_sub(p));
        }
        r
    }

    /// Wait for `flag` in INTERRUPT (clearing it), or an error / timeout.""")

sub1(e, "    fn wait_interrupt(flag: u32, phase: Phase) -> Result<(), BlockError> {\n        let mut budget",
     "    fn wait_interrupt_inner(flag: u32, phase: Phase) -> Result<(), BlockError> {\n        let mut budget")

sub1(e, "    /// Issue a command and return `RESP0`.",
     """    /// FS04-TEMP: the whole command handshake -- Command Inhibit, the
    /// register writes and Command Complete -- as one figure, which is what
    /// `dev.r.cmd17` / `dev.w.cmd24` meant in the earlier blocks.  Recorded
    /// on both exits.  Outside a sector operation (identification, recovery)
    /// the direction is NONE and nothing is recorded, as before.
    fn command(&self, cmdtm: u32, arg: u32) -> Result<u32, BlockError> {
        let p = tos_api::fs04probe::now();
        let r = self.command_inner(cmdtm, arg);
        let dt = tos_api::fs04probe::now().wrapping_sub(p);
        match FS04_DIR.load(Ordering::Relaxed) {
            FS04_DIR_READ => tos_api::fs04probe::add(tos_api::fs04probe::slot::DEV_R_CMD, dt),
            FS04_DIR_WRITE => tos_api::fs04probe::add(tos_api::fs04probe::slot::DEV_W_CMD, dt),
            _ => {}
        }
        r
    }

    /// Issue a command and return `RESP0`.""")

sub1(e, "    fn command(&self, cmdtm: u32, arg: u32) -> Result<u32, BlockError> {\n        if !Self::wait_clear(STATUS, SR_CMD_INHIBIT, Phase::CmdInhibit) {",
     "    fn command_inner(&self, cmdtm: u32, arg: u32) -> Result<u32, BlockError> {\n        if !Self::wait_clear(STATUS, SR_CMD_INHIBIT, Phase::CmdInhibit) {")

sub1(e, "    /// Read one 512-byte block at `lba` into `buf` (PIO).",
     """    /// FS04-TEMP: the whole sector read, recorded on **both** exits -- a
    /// sector that failed cost time too, and leaving it out was the scope
    /// error that made `dev.read_one n` disagree with `blk.read_blocks
    /// calls` in raw/pi04-FS04.TXT.
    fn read_one(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        let p = tos_api::fs04probe::now();
        FS04_DIR.store(FS04_DIR_READ, Ordering::Relaxed);
        let r = self.read_one_inner(lba, buf);
        FS04_DIR.store(FS04_DIR_NONE, Ordering::Relaxed);
        tos_api::fs04probe::add(
            tos_api::fs04probe::slot::DEV_READ,
            tos_api::fs04probe::now().wrapping_sub(p),
        );
        r
    }

    /// Read one 512-byte block at `lba` into `buf` (PIO).""")

sub1(e, "    fn read_one(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {\n        if !self.ready.load(Ordering::Acquire) {",
     "    fn read_one_inner(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {\n        if !self.ready.load(Ordering::Acquire) {")

sub1(e, "    /// Write one 512-byte block at `lba` from `buf` (PIO).",
     """    /// FS04-TEMP: the whole sector write, recorded on **both** exits -- see
    /// `read_one`.  This is the counter whose scope did not line up with the
    /// per-phase counters before.
    fn write_one(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {
        let p = tos_api::fs04probe::now();
        FS04_DIR.store(FS04_DIR_WRITE, Ordering::Relaxed);
        let r = self.write_one_inner(lba, buf);
        FS04_DIR.store(FS04_DIR_NONE, Ordering::Relaxed);
        tos_api::fs04probe::add(
            tos_api::fs04probe::slot::DEV_WRITE,
            tos_api::fs04probe::now().wrapping_sub(p),
        );
        r
    }

    /// Write one 512-byte block at `lba` from `buf` (PIO).""")

sub1(e, "    fn write_one(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {\n        if !self.ready.load(Ordering::Acquire) {",
     "    fn write_one_inner(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {\n        if !self.ready.load(Ordering::Acquire) {")

# the two PIO loops -- bracketed where they stand, not reproduced
sub1(e, """        for word in buf.chunks_exact_mut(4) {
            let w = rd(DATA);
            word.copy_from_slice(&w.to_le_bytes());
        }""",
     """        let p_pio = tos_api::fs04probe::now(); // FS04-TEMP
        for word in buf.chunks_exact_mut(4) {
            let w = rd(DATA);
            word.copy_from_slice(&w.to_le_bytes());
        }
        tos_api::fs04probe::add(
            tos_api::fs04probe::slot::DEV_R_PIO,
            tos_api::fs04probe::now().wrapping_sub(p_pio),
        ); // FS04-TEMP""")

sub1(e, """        for word in buf.chunks_exact(4) {
            wr(DATA, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }""",
     """        let p_pio = tos_api::fs04probe::now(); // FS04-TEMP
        for word in buf.chunks_exact(4) {
            wr(DATA, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }
        tos_api::fs04probe::add(
            tos_api::fs04probe::slot::DEV_W_PIO,
            tos_api::fs04probe::now().wrapping_sub(p_pio),
        ); // FS04-TEMP""")

# how many sectors per BlockDevice call -- the ticket's sharpest question
sub1(e, """        for (i, sector) in buf.chunks_exact_mut(SECTOR_SIZE).enumerate() {
            self.read_one(lba + i as u64, sector)?;
        }""",
     """        // FS04-TEMP: CNT = calls, TCK = sectors asked for in one call.
        tos_api::fs04probe::amount(
            tos_api::fs04probe::slot::BLK_READ,
            (buf.len() / SECTOR_SIZE) as u64,
        );
        for (i, sector) in buf.chunks_exact_mut(SECTOR_SIZE).enumerate() {
            self.read_one(lba + i as u64, sector)?;
        }""")
sub1(e, """        for (i, sector) in buf.chunks_exact(SECTOR_SIZE).enumerate() {
            self.write_one(lba + i as u64, sector)?;
        }""",
     """        // FS04-TEMP: CNT = calls, TCK = sectors asked for in one call.
        tos_api::fs04probe::amount(
            tos_api::fs04probe::slot::BLK_WRITE,
            (buf.len() / SECTOR_SIZE) as u64,
        );
        for (i, sector) in buf.chunks_exact(SECTOR_SIZE).enumerate() {
            self.write_one(lba + i as u64, sector)?;
        }""")

# ── 5b. framebuffer cache maintenance: how much is cleaned per drop ────────
sub1('kernel/src/vdi_backend.rs',
     """        let start = base + (y0 as usize * stride + x0 as usize) * 4;
        let end = base + (y1 as usize * stride + x1 as usize) * 4 + 4;""",
     """        let start = base + (y0 as usize * stride + x0 as usize) * 4;
        let end = base + (y1 as usize * stride + x1 as usize) * 4 + 4;
        // FS04-TEMP: the span actually maintained, and the number of 64-byte
        // `dc cvac` operations it costs.  QEMU models neither, a Pi 4 pays both.
        {
            use tos_api::fs04probe as fp;
            let span = (end - start) as u64;
            fp::amount(fp::slot::FB_CLEAN, span);
            fp::amount(fp::slot::FB_CLEAN_LINES, span.div_ceil(64));
        }""")

# ── 6. desktop copy loop: per-phase timestamps ─────────────────────────────
d = 'kernel/src/desk/dir.rs'
sub1(d, """    let srcfh = dos_open(src, 0);
    if srcfh < 0 {
        return More::Failed;
    }
    let srcfh = srcfh as i16;""",
     """    use tos_api::fs04probe as fp; // FS04-TEMP
    let p_total = fp::now();
    let p0 = fp::now();
    let srcfh = dos_open(src, 0);
    fp::add(fp::slot::DC_OPEN, fp::now().wrapping_sub(p0));
    if srcfh < 0 {
        return More::Failed;
    }
    let srcfh = srcfh as i16;""")
sub1(d, """    match output_fname(src, dst, ctx) {
        More::Ok => {}""",
     """    let p1 = fp::now();
    let of = output_fname(src, dst, ctx);
    fp::add(fp::slot::DC_OUTNAME, fp::now().wrapping_sub(p1));
    match of {
        More::Ok => {}""")
sub1(d, """    let dstfh = dos_create(dst, attr & !(FA_SUBDIR as u16));
    if dstfh < 0 {""",
     """    let p2 = fp::now();
    let dstfh = dos_create(dst, attr & !(FA_SUBDIR as u16));
    fp::add(fp::slot::DC_CREATE, fp::now().wrapping_sub(p2));
    if dstfh < 0 {""")
sub1(d, """        let n = dos_read(srcfh, buf.len() as i32, buf.as_mut_ptr());
        if n == 0 {
            dos_setdt(dstfh, time, date); // stamp target from source (deskdir.c:428)
            break;
        }
        if n < 0 {
            ok = false;
            break;
        }
        let w = dos_write(dstfh, n, buf.as_ptr());""",
     """        let p3 = fp::now();
        let n = dos_read(srcfh, buf.len() as i32, buf.as_mut_ptr());
        fp::add(fp::slot::DC_READ, fp::now().wrapping_sub(p3));
        if n == 0 {
            let p4 = fp::now();
            dos_setdt(dstfh, time, date); // stamp target from source (deskdir.c:428)
            fp::add(fp::slot::DC_SETDT, fp::now().wrapping_sub(p4));
            break;
        }
        if n < 0 {
            ok = false;
            break;
        }
        let p5 = fp::now();
        let w = dos_write(dstfh, n, buf.as_ptr());
        fp::add(fp::slot::DC_WRITE, fp::now().wrapping_sub(p5));""")
sub1(d, """    dos_close(srcfh);
    dos_close(dstfh);
    if !ok {""",
     """    let p6 = fp::now();
    dos_close(srcfh);
    dos_close(dstfh);
    fp::add(fp::slot::DC_CLOSE, fp::now().wrapping_sub(p6));
    fp::add(fp::slot::DC_TOTAL, fp::now().wrapping_sub(p_total));
    if !ok {""")

# fun_op: the two passes and the buffer allocation
sub1(d, """    let mut count = DirCount::default();
    let mut empty: [u8; 0] = [];
    if !dir_op(Op::Count, icontype, src_dir, entries, None, &mut count, &mut empty) {""",
     """    use tos_api::fs04probe as fp; // FS04-TEMP
    let p_funop = fp::now();
    let mut count = DirCount::default();
    let mut empty: [u8; 0] = [];
    let p_cnt = fp::now();
    let cnt_ok = dir_op(Op::Count, icontype, src_dir, entries, None, &mut count, &mut empty);
    fp::add(fp::slot::COUNT_PASS, fp::now().wrapping_sub(p_cnt));
    if !cnt_ok {""")
sub1(d, """    let mut buf: Vec<u8> = Vec::new();
    if matches!(op, Op::Copy | Op::Move) {
        buf.resize(COPY_BUF, 0);
    }
    let all_ok = dir_op(op, icontype, src_dir, entries, dst_dir, &mut count, &mut buf);
    if !all_ok {
        fun_alert(1, b"Some items could not be processed.");
    }
    true""",
     """    let p_alloc = fp::now();
    let mut buf: Vec<u8> = Vec::new();
    if matches!(op, Op::Copy | Op::Move) {
        buf.resize(COPY_BUF, 0);
    }
    fp::add(fp::slot::BUF_ALLOC, fp::now().wrapping_sub(p_alloc));
    let p_copy = fp::now();
    let all_ok = dir_op(op, icontype, src_dir, entries, dst_dir, &mut count, &mut buf);
    fp::add(fp::slot::COPY_PASS, fp::now().wrapping_sub(p_copy));
    if !all_ok {
        fun_alert(1, b"Some items could not be processed.");
    }
    fp::add(fp::slot::FUN_OP, fp::now().wrapping_sub(p_funop));
    true""")


if mode == 'gesture':
    # The whole drop -- fun_op AND the window rebuild and full redraw that
    # follow it -- is what the user perceives as the freeze.
    sub1(d, """pub fn fun_drag(wh: i16, dest_wh: i16, dobj: i16, mx: i16, my: i16, keystate: u16) -> bool {""",
         """pub fn fun_drag(wh: i16, dest_wh: i16, dobj: i16, mx: i16, my: i16, keystate: u16) -> bool {
    // FS04-TEMP: zero the counters at the drop, time the whole handler.
    super::fs04drv::mark_reset();
    let p_drag = tos_api::fs04probe::now();
    let r = fun_drag_inner(wh, dest_wh, dobj, mx, my, keystate);
    tos_api::fs04probe::add(
        tos_api::fs04probe::slot::DRAG_TOTAL,
        tos_api::fs04probe::now().wrapping_sub(p_drag),
    );
    // Printed only AFTER the handler has returned, so the serial traffic is
    // outside the interval it reports.
    super::fs04drv::mark_report("drop");
    r
}

#[allow(missing_docs)]
fn fun_drag_inner(wh: i16, dest_wh: i16, dobj: i16, mx: i16, my: i16, keystate: u16) -> bool {""")
    sub1(d, """pub fn rebuild_window(handle: i16) {""",
         """pub fn rebuild_window(handle: i16) {
    let p = tos_api::fs04probe::now(); // FS04-TEMP
    rebuild_window_inner(handle);
    tos_api::fs04probe::add(
        tos_api::fs04probe::slot::REBUILD,
        tos_api::fs04probe::now().wrapping_sub(p),
    );
}

fn rebuild_window_inner(handle: i16) {""")
    sub1('kernel/src/desk/redraw.rs', """pub fn redraw_desktop() {""",
         """pub fn redraw_desktop() {
    let p = tos_api::fs04probe::now(); // FS04-TEMP
    redraw_desktop_inner();
    tos_api::fs04probe::add(
        tos_api::fs04probe::slot::REDRAW,
        tos_api::fs04probe::now().wrapping_sub(p),
    );
}

fn redraw_desktop_inner() {""")
    # The confirmation click is the user's time, not the machine's: the clock
    # (and every counter) starts the instant form_do returns with OK.
    sub1(d, """        if confirm {
            super::form_do(ctx.tree, 0);
            // `more = (tree[CDOK].ob_state & SELECTED) ? TRUE : FALSE;` (`:963`)
            ctx.stop = b.tree[CDOK as usize].ob_state & OB_SELECTED == 0;
        }""",
         """        if confirm {
            super::form_do(ctx.tree, 0);
            // `more = (tree[CDOK].ob_state & SELECTED) ? TRUE : FALSE;` (`:963`)
            ctx.stop = b.tree[CDOK as usize].ob_state & OB_SELECTED == 0;
            super::fs04drv::mark_reset(); // FS04-TEMP: the freeze starts here
        }""")

# ── 5c. Fseek, so the report can be APPENDED rather than overwritten ───────
sub1('kernel/src/desk/dosbind.rs', "const FDATIME: u16 = 0x57;",
     "const FDATIME: u16 = 0x57;\nconst FSEEK: u16 = 0x42; // FS04-TEMP")
sub1('kernel/src/desk/dosbind.rs',
     """/// `dos_delete` — `Fdelete(name)` (`gemdos.h:125-128`).""",
     """/// FS04-TEMP: `Fseek(offset, handle, seekmode)` — `seekmode` 2 = from the
/// end, which is how the probe appends its report instead of overwriting it.
pub(super) fn dos_seek(offset: i32, handle: i16, seekmode: u16) -> Long {
    dos_trap(FSEEK, sx(offset as i64), sx(handle as i64), seekmode as u64)
}

/// `dos_delete` — `Fdelete(name)` (`gemdos.h:125-128`).""")

# ── 5d. the active framebuffer's geometry, for the report header ───────────
sub1('kernel/src/vdi_backend.rs',
     """/// The screen VDI backend (zero-sized; state lives in the framebuffer).""",
     """/// FS04-TEMP: the active framebuffer's geometry, so the probe's report can
/// name the screen the numbers were taken on.  On the Pi that is the HDMI mode
/// the VideoCore reported, which is not knowable from the QEMU runs.
pub fn fs04_fb_geometry() -> (u32, u32, u32) {
    let fb = active_fb();
    (fb.width(), fb.height(), fb.stride_px())
}

/// The screen VDI backend (zero-sized; state lives in the framebuffer).""")

# ── 7. the driver and the report sink ──────────────────────────────────────
wr('kernel/src/desk/fs04drv.rs', r'''
//! FS04-TEMP — the measurement driver and the report sink (audit scaffolding;
//! NOT product code).
//!
//! The report goes **to a file on the card**, `C:\FS04.TXT`, because the Pi
//! under test has no serial capture — the only output the user sees is the
//! screen, and a `kprintln` there is invisible.  The same text also goes to
//! the serial console, so the QEMU runs stay comparable with
//! `measurement-2026-09-25-fs04/raw/`.
//!
//! ## The ordering that keeps the instrument out of its own numbers
//!
//! Writing a file is GEMDOS + FAT + sector cache + EMMC traffic — exactly the
//! traffic being measured.  [`report`] therefore runs in four strictly ordered
//! steps and the file I/O is the **last** of them:
//!
//! 1. [`take_snapshot`] copies every counter out of the atomics into [`SNAP`].
//!    From here on nothing that happens can change what is reported.
//! 2. [`format_block`] renders the whole block into [`TEXT`] — still no I/O.
//! 3. [`emit_serial`] prints [`TEXT`] (free, and identical to the file).
//! 4. [`write_card`] opens, seeks to the end, writes and closes.
//!
//! `report` itself is only ever called **after** the measured interval has
//! been closed: the `fun_drag` wrapper records `DRAG_TOTAL` first and calls
//! `report` afterwards, and the driver's `one_copy` reads its end timestamp
//! before calling it.  The cost of step 4 is measured and reported in the
//! **next** block's header (`previous report write`), so it is never silently
//! folded into a measurement — it just is not in this block's numbers.
//!
//! Blocks are appended (`Fseek(0, h, 2)`), never overwritten, so repeating the
//! gesture leaves several blocks in one file.

use core::cell::UnsafeCell;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use tos_api::fs04probe as fp;

use super::dir::{self, Op, SelEntry};
use crate::drivers::bcm2711::emmc;
use crate::kprintln;

/// FS04-TEMP: an all-zero `ErrorStats`, for the statics below (`Default` is
/// not `const`).
const NO_ERRORS: emmc::ErrorStats = emmc::ErrorStats {
    last_status: 0,
    last_phase: emmc::Phase::CmdInhibit,
    bits: [0; 7],
    other: 0,
    io_errors: 0,
    timeouts: 0,
    timeouts_by_phase: [0; emmc::Phase::COUNT],
    recoveries: 0,
    nonrecoverable: 0,
};

/// Where the report lands: the root of the boot partition, plain 8.3 text, so
/// the card can be read on any host with no tooling.
const REPORT_PATH: &str = "C:\\FS04.TXT";

/// Cost of the previous [`write_card`], in counter ticks.
static PREV_WRITE: AtomicU64 = AtomicU64::new(0);
/// Block counter, so repeated gestures are distinguishable in the file.
static BLOCK: AtomicU32 = AtomicU32::new(0);
/// Wall-clock origin, set by [`mark_reset`].
static T0: AtomicU64 = AtomicU64::new(0);

// ── the two scratch buffers ────────────────────────────────────────────────

struct Shared<T>(UnsafeCell<T>);
// SAFETY: the desktop is single-CPU and `report` is never re-entered (the
// file I/O in step 4 cannot reach the desktop's copy path).
unsafe impl<T> Sync for Shared<T> {}

const TEXT_CAP: usize = 16 * 1024;
struct TextBuf {
    data: [u8; TEXT_CAP],
    len: usize,
}
static TEXT: Shared<TextBuf> = Shared(UnsafeCell::new(TextBuf {
    data: [0; TEXT_CAP],
    len: 0,
}));

fn text() -> &'static mut TextBuf {
    // SAFETY: single-CPU, non-reentrant (see `Shared`).
    unsafe { &mut *TEXT.0.get() }
}

impl core::fmt::Write for TextBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let n = b.len().min(TEXT_CAP - self.len);
        self.data[self.len..self.len + n].copy_from_slice(&b[..n]);
        self.len += n;
        Ok(())
    }
}

/// Number of distinct GEMDOS opcodes kept in a snapshot.
const NG: usize = 64;

struct Snap {
    cnt: [u64; fp::NSLOT],
    tck: [u64; fp::NSLOT],
    mx: [u64; fp::NSLOT],
    mn: [u64; fp::NSLOT],
    /// `(opcode, calls, ticks, worst)`.
    g: [(u16, u64, u64, u64); NG],
    gn: usize,
    freq: u64,
    wall: u64,
    prev_write: u64,
    block: u32,
    /// The driver's own error bookkeeping (`emmc::error_stats()`), and what
    /// it read at the end of the previous block, so this block can show the
    /// **delta** as well as the running total. Cumulative since boot: the
    /// driver has no reset, and inventing one in the probe would hide a
    /// failure that happened before the gesture.
    emmc: emmc::ErrorStats,
    emmc_prev: emmc::ErrorStats,
}

static SNAP: Shared<Snap> = Shared(UnsafeCell::new(Snap {
    cnt: [0; fp::NSLOT],
    tck: [0; fp::NSLOT],
    mx: [0; fp::NSLOT],
    mn: [0; fp::NSLOT],
    g: [(0, 0, 0, 0); NG],
    gn: 0,
    freq: 0,
    wall: 0,
    prev_write: 0,
    block: 0,
    emmc: NO_ERRORS,
    emmc_prev: NO_ERRORS,
}));

/// FS04-TEMP: the driver's counters as of the end of the previous block.
static PREV_STATS: Shared<emmc::ErrorStats> = Shared(UnsafeCell::new(NO_ERRORS));

fn prev_stats() -> &'static mut emmc::ErrorStats {
    // SAFETY: single-CPU, non-reentrant (see `Shared`).
    unsafe { &mut *PREV_STATS.0.get() }
}

fn snap() -> &'static mut Snap {
    // SAFETY: single-CPU, non-reentrant (see `Shared`).
    unsafe { &mut *SNAP.0.get() }
}

// ── step 1: every counter out of the atomics, before any I/O ───────────────

fn take_snapshot(wall: u64) {
    let s = snap();
    for i in 0..fp::NSLOT {
        s.cnt[i] = fp::CNT[i].load(Ordering::Relaxed);
        s.tck[i] = fp::TCK[i].load(Ordering::Relaxed);
        s.mx[i] = fp::MAXT[i].load(Ordering::Relaxed);
        s.mn[i] = fp::MINT[i].load(Ordering::Relaxed);
    }
    s.gn = 0;
    for op in 0..256usize {
        let c = fp::GCNT[op].load(Ordering::Relaxed);
        if c == 0 || s.gn >= NG {
            continue;
        }
        s.g[s.gn] = (
            op as u16,
            c,
            fp::GTCK[op].load(Ordering::Relaxed),
            fp::GMAX[op].load(Ordering::Relaxed),
        );
        s.gn += 1;
    }
    // The driver's own bookkeeping goes into the snapshot too, BEFORE the
    // file write -- which is itself sector traffic and could add to it.
    s.emmc = emmc::error_stats();
    s.emmc_prev = *prev_stats();
    *prev_stats() = s.emmc;
    s.freq = fp::freq();
    s.wall = wall;
    s.prev_write = PREV_WRITE.load(Ordering::Relaxed);
    s.block = BLOCK.fetch_add(1, Ordering::Relaxed) + 1;
}

fn tick_us(t: u64) -> u64 {
    let f = fp::freq();
    if f == 0 {
        0
    } else {
        t * 1_000_000 / f
    }
}

fn snap_us(s: &Snap, t: u64) -> u64 {
    if s.freq == 0 {
        0
    } else {
        t * 1_000_000 / s.freq
    }
}

// ── step 2: render the block; still no I/O ─────────────────────────────────

fn format_block(tag: &str) {
    let s = snap();
    let t = text();
    t.len = 0;
    let (fw, fh, stride) = crate::vdi_backend::fs04_fb_geometry();
    let (dw, dh) = super::global::with(|g| (g.g_desk.w, g.g_desk.h)).unwrap_or((0, 0));
    let _ = write!(
        t,
        "==== FS04 block {} : {} ====\r\n",
        s.block, tag
    );
    let _ = write!(
        t,
        "screen {}x{} stride {}px   desk {}x{}   CNTFRQ {} Hz\r\n",
        fw, fh, stride, dw, dh, s.freq
    );
    let _ = write!(
        t,
        "wall {} us ({} ticks)\r\n",
        snap_us(s, s.wall),
        s.wall
    );
    let _ = write!(
        t,
        "previous report write {} us  (this block's own write is in no number below)\r\n",
        snap_us(s, s.prev_write)
    );
    let _ = write!(
        t,
        "NOTE  `wall` is the time from the OK click in the confirmation box to the\r\n\
         \x20     end of the whole drop -- that is the freeze.  desk.fun_op,\r\n\
         \x20     desk.copy_pass and desk.fun_drag START BEFORE that box and so also\r\n\
         \x20     contain the seconds it stood waiting for the click; they are not it.\r\n"
    );
    for i in 0..fp::NSLOT {
        let c = s.cnt[i];
        if c == 0 {
            continue;
        }
        if i == fp::slot::BLK_READ
            || i == fp::slot::BLK_WRITE
            || i == fp::slot::FB_CLEAN
            || i == fp::slot::FB_CLEAN_LINES
            || i == fp::slot::EMMC_SPIN
        {
            let _ = write!(
                t,
                "{:<18} calls={:<7} amount={:<9} max/call={} min/call={}\r\n",
                fp::NAMES[i], c, s.tck[i], s.mx[i], s.mn[i]
            );
        } else if s.tck[i] == 0 && s.mx[i] == 0 {
            // A pure occurrence counter -- no duration was ever recorded for it.
            let _ = write!(t, "{:<18} n={}\r\n", fp::NAMES[i], c);
        } else {
            let _ = write!(
                t,
                "{:<18} n={:<7} total={:<9}us avg={:<7}us max={:<7}us min={}us\r\n",
                fp::NAMES[i],
                c,
                snap_us(s, s.tck[i]),
                snap_us(s, s.tck[i] / c),
                snap_us(s, s.mx[i]),
                snap_us(s, s.mn[i])
            );
        }
    }
    // Does the device total equal the sum of its phases?  Printed rather
    // than claimed: every wait is now timed on every exit path, so the two
    // must agree up to the register writes between them.  A visible gap
    // means a phase is escaping the accounting again.
    let phase_sum = |a: [usize; 5]| -> u64 { a.iter().map(|&i| s.tck[i]).sum() };
    let rsum = phase_sum([
        fp::slot::DEV_R_WAITDAT,
        fp::slot::DEV_R_CMD,
        fp::slot::DEV_R_RDY,
        fp::slot::DEV_R_PIO,
        fp::slot::DEV_R_DONE,
    ]);
    let wsum = phase_sum([
        fp::slot::DEV_W_WAITDAT,
        fp::slot::DEV_W_CMD,
        fp::slot::DEV_W_RDY,
        fp::slot::DEV_W_PIO,
        fp::slot::DEV_W_DONE,
    ]);
    if s.cnt[fp::slot::DEV_READ] != 0 || s.cnt[fp::slot::DEV_WRITE] != 0 {
        let _ = write!(
            t,
            "scope.check.r      n(read_one)={} n(sectors)={}  total={}us phases={}us diff={}us\r\n",
            s.cnt[fp::slot::DEV_READ],
            s.tck[fp::slot::BLK_READ],
            snap_us(s, s.tck[fp::slot::DEV_READ]),
            snap_us(s, rsum),
            snap_us(s, s.tck[fp::slot::DEV_READ].saturating_sub(rsum))
        );
        let _ = write!(
            t,
            "scope.check.w      n(write_one)={} n(sectors)={}  total={}us phases={}us diff={}us\r\n",
            s.cnt[fp::slot::DEV_WRITE],
            s.tck[fp::slot::BLK_WRITE],
            snap_us(s, s.tck[fp::slot::DEV_WRITE]),
            snap_us(s, wsum),
            snap_us(s, s.tck[fp::slot::DEV_WRITE].saturating_sub(wsum))
        );
    }
    // The driver's OWN counters (c6cf484). `emmc.spin` is gone from this
    // block: since the timeout became a duration, its iteration count is no
    // longer a limit and carried no information. What replaces it is here --
    // a timeout is now reported as one, by phase, and so is every error bit.
    // Totals are cumulative since boot, `(+n)` is this block's share.
    let e = &s.emmc;
    let ep = &s.emmc_prev;
    let _ = write!(
        t,
        "emmc.errors        timeouts={}(+{}) io={}(+{}) recoveries={}(+{}) nonrecov={}(+{}) other={}(+{})\r\n",
        e.timeouts,
        e.timeouts.saturating_sub(ep.timeouts),
        e.io_errors,
        e.io_errors.saturating_sub(ep.io_errors),
        e.recoveries,
        e.recoveries.saturating_sub(ep.recoveries),
        e.nonrecoverable,
        e.nonrecoverable.saturating_sub(ep.nonrecoverable),
        e.other,
        e.other.saturating_sub(ep.other)
    );
    let _ = write!(t, "emmc.timeouts_by_phase");
    for i in 0..emmc::Phase::COUNT {
        let _ = write!(
            t,
            " {}={}(+{})",
            emmc::PHASE_NAMES[i],
            e.timeouts_by_phase[i],
            e.timeouts_by_phase[i].saturating_sub(ep.timeouts_by_phase[i])
        );
    }
    let _ = write!(t, "\r\n");
    let _ = write!(t, "emmc.error_bits   ");
    for i in 0..emmc::ERROR_BIT_NAMES.len() {
        let _ = write!(
            t,
            " {}={}(+{})",
            emmc::ERROR_BIT_NAMES[i],
            e.bits[i],
            e.bits[i].saturating_sub(ep.bits[i])
        );
    }
    let _ = write!(t, "\r\n");
    let _ = write!(
        t,
        "emmc.last          status=0x{:08x} phase={}\r\n",
        e.last_status,
        emmc::PHASE_NAMES[e.last_phase as usize % emmc::Phase::COUNT]
    );
    let _ = write!(
        t,
        "NOTE  `emmc.spin` is gone from this block.  Since the timeout became a\r\n\
         \x20     duration (c6cf484) its iteration count is no longer a limit and\r\n\
         \x20     carried no information; a wait that ran out is now reported as a\r\n\
         \x20     timeout, by phase, on the three emmc.* lines above.\r\n"
    );
    let _ = write!(t, "--- GEMDOS calls (opcode: n, total us, max us) ---\r\n");
    for k in 0..s.gn {
        let (op, c, tk, mx) = s.g[k];
        let _ = write!(
            t,
            "  0x{:02x}  n={:<6} total={:<9}us max={}us\r\n",
            op,
            c,
            snap_us(s, tk),
            snap_us(s, mx)
        );
    }
    let _ = write!(t, "==== end block {} ====\r\n\r\n", s.block);
}

// ── step 3: the same text on the serial console ────────────────────────────

fn emit_serial() {
    let t = text();
    let Ok(s) = core::str::from_utf8(&t.data[..t.len]) else {
        kprintln!("[fs04] report buffer is not UTF-8");
        return;
    };
    for line in s.split('\n') {
        let line = line.trim_end_matches('\r');
        if !line.is_empty() {
            kprintln!("[fs04] {}", line);
        }
    }
}

// ── step 4: and only now, the card ─────────────────────────────────────────

/// Append [`TEXT`] to [`REPORT_PATH`]. Returns the cost in counter ticks.
fn write_card() -> u64 {
    let t0 = fp::now();
    let t = text();
    if t.len == 0 {
        return 0;
    }
    let mut h = super::dosbind::dos_open(REPORT_PATH, 2);
    if h < 0 {
        h = super::dosbind::dos_create(REPORT_PATH, 0);
        if h < 0 {
            kprintln!("[fs04] cannot create {} (err {})", REPORT_PATH, h);
            return fp::now().wrapping_sub(t0);
        }
    } else {
        // Append: put the file pointer at the end, so repeated gestures stack
        // up instead of overwriting each other.
        super::dosbind::dos_seek(0, h as i16, 2);
    }
    let hh = h as i16;
    let n = super::dosbind::dos_write(hh, t.len as i32, t.data.as_ptr());
    super::dosbind::dos_close(hh);
    if n != t.len as tos_types::Long {
        kprintln!("[fs04] short write to {}: {} of {}", REPORT_PATH, n, t.len);
    }
    fp::now().wrapping_sub(t0)
}

// ── the public entry points ────────────────────────────────────────────────

/// Zero every counter and start the wall clock.
pub fn mark_reset() {
    fp::reset();
    T0.store(fp::now(), Ordering::Relaxed);
}

/// Report what has accumulated since [`mark_reset`].
pub fn mark_report(tag: &str) {
    let t0 = T0.load(Ordering::Relaxed);
    report(tag, fp::now().wrapping_sub(t0));
}

fn report(tag: &str, wall: u64) {
    take_snapshot(wall); // 1 — nothing after this can change the numbers
    format_block(tag); // 2 — still no I/O
    emit_serial(); // 3 — free
    let dt = write_card(); // 4 — GEMDOS + FAT + EMMC, deliberately last
    PREV_WRITE.store(dt, Ordering::Relaxed);
    kprintln!(
        "[fs04] block written to {} in {} us",
        REPORT_PATH,
        tick_us(dt)
    );
}

/// One short block at start-up: it names the machine the later blocks were
/// taken on (screen geometry above all, which the QEMU runs cannot know), and
/// it proves — in the very binary that is handed over — that the file path
/// works before any measurement depends on it.
pub fn boot_note() {
    let t = text();
    t.len = 0;
    let (fw, fh, stride) = crate::vdi_backend::fs04_fb_geometry();
    let (dw, dh) = super::global::with(|g| (g.g_desk.w, g.g_desk.h)).unwrap_or((0, 0));
    let bits = super::dosbind::dos_sdrv(super::dosbind::dos_gdrv());
    let _ = write!(t, "==== FS04 probe: kernel started ====\r\n");
    let _ = write!(
        t,
        "screen {}x{} stride {}px   desk {}x{}   CNTFRQ {} Hz\r\n",
        fw,
        fh,
        stride,
        dw,
        dh,
        fp::freq()
    );
    let _ = write!(t, "drives:");
    for i in 0..16u32 {
        if bits & (1 << i) != 0 {
            let _ = write!(t, " {}:", (b'A' + i as u8) as char);
        }
    }
    let _ = write!(t, "\r\n");
    let _ = write!(
        t,
        "Copy a file from one drive to another now; one block per copy is\r\n\
         appended below.  Repeat as often as you like.\r\n"
    );
    {
        let e = emmc::error_stats();
        let _ = write!(
            t,
            "emmc after identification: {} error(s), {} timeout(s), {} recovery(ies)",
            e.io_errors, e.timeouts, e.recoveries
        );
        for i in 0..emmc::ERROR_BIT_NAMES.len() {
            if e.bits[i] != 0 {
                let _ = write!(t, " {}={}", emmc::ERROR_BIT_NAMES[i], e.bits[i]);
            }
        }
        let _ = write!(t, "\r\n");
        *prev_stats() = e;
    }
    let _ = write!(t, "==== end start-up note ====\r\n\r\n");
    emit_serial();
    let dt = write_card();
    PREV_WRITE.store(dt, Ordering::Relaxed);
    kprintln!(
        "[fs04] start-up note written to {} in {} us",
        REPORT_PATH,
        tick_us(dt)
    );
}

// ── the non-interactive driver (only wired in `driver` mode) ───────────────

fn entry_for(path: &str) -> Option<SelEntry> {
    let info = dir::file_exists(path)?;
    let mut name = [0u8; 14];
    let b = info.name.as_bytes();
    let n = b.len().min(13);
    name[..n].copy_from_slice(&b[..n]);
    Some(SelEntry {
        name,
        is_dir: false,
        size: info.size,
        time: info.time,
        date: info.date,
    })
}

fn one_copy(tag: &str, src_dir: &str, file: &str, dst_dir: &str) {
    let full = {
        let mut s = alloc::string::String::from(src_dir);
        s.push_str(file);
        s
    };
    let Some(e) = entry_for(&full) else {
        kprintln!("[fs04] {}: source {} not found", tag, full);
        return;
    };
    kprintln!("[fs04] {}: {} ({} bytes) -> {}", tag, full, e.size, dst_dir);
    fp::reset();
    let t0 = fp::now();
    let ran = dir::fun_op(Op::Copy, None, src_dir, &[e], Some(dst_dir));
    let t1 = fp::now();
    kprintln!("[fs04] {}: fun_op returned {}", tag, ran);
    report(tag, t1.wrapping_sub(t0));
}

/// The whole measurement, run once after the desktop is up.
pub fn run() {
    kprintln!("[fs04] driver start; CNTFRQ={} Hz", fp::freq());
    // The copy-confirmation dialog (`g_ccopypref`) would block a headless run
    // on `form_do`; the user's click is not part of the measured work.
    let old = super::global::with(|g| {
        let o = g.g_ccopypref;
        g.g_ccopypref = false;
        o
    })
    .unwrap_or(true);

    // A: cross-partition, destination empty and never touched before -- the
    //    exact case in the ticket.
    one_copy("A-cold-C-to-D", "C:\\", "README.TXT", "D:\\");
    // B: the same copy again, same partition pair, now warm.
    one_copy("B-warm-C-to-D", "C:\\", "HELLO.TXT", "D:\\");
    // C: same-partition copy for comparison (no boundary crossed).
    one_copy("C-same-part-C-to-C", "C:\\", "CONFIG.TXT", "C:\\SUB\\");
    // D: another cold partition, to separate first-touch cost from copy cost.
    one_copy("D-cold-C-to-E", "C:\\", "README.TXT", "E:\\");
    // E: a 1 MiB file -- 32 full 32-KiB buffers.  The question this answers is
    //    whether a 32-KiB Fread survives as one transfer down to the device,
    //    or decomposes into 64 single-sector accesses.
    one_copy("E-big-1MiB-C-to-D", "C:\\", "BIG1M.BIN", "D:\\");
    // F: what a full FAT scan costs (Dfree -> count_free_clusters).  Measured
    //    to see whether it is in the copy path at all, and what it would cost
    //    if it were.
    {
        fp::reset();
        let t0 = fp::now();
        let (tot, avail) = super::dosbind::dos_space(4); // 4 = D:
        let t1 = fp::now();
        kprintln!("[fs04] F-dfree-D: total={} avail={}", tot, avail);
        report("F-dfree-D", t1.wrapping_sub(t0));
    }

    super::global::with(|g| g.g_ccopypref = old);
    kprintln!("[fs04] driver done");
}
''')

# ── 8. wire the driver in ──────────────────────────────────────────────────
m = 'kernel/src/desk/mod.rs'
sub1(m, 'pub mod ftype;', 'pub mod fs04drv; // FS04-TEMP\npub mod ftype;')
# The start-up note runs in BOTH modes: it records the screen geometry the
# numbers were taken on, and it exercises the report file in the very binary
# that is handed over, before any measurement depends on it.
drv = '    fs04drv::run(); // FS04-TEMP\n' if mode == 'driver' else ''
sub1(m, """    // Hand off to the interaction loop. It returns when a handler set `done`""",
     """    fs04drv::boot_note(); // FS04-TEMP
""" + drv + """
    // Hand off to the interaction loop. It returns when a handler set `done`""")

print("fs04 probe patch applied to", ws, "mode:", mode)
