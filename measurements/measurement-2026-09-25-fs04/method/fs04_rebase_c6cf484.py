#!/usr/bin/env python3
"""FS04 probe, Fassung 3: rebase onto the rewritten EMMC driver (c6cf484).

Applied **to `fs04_probe_patch.py` itself** (in place), turning the Fassung-2
script into the Fassung-3 script:

    python3 method/fs04_rebase_c6cf484.py method/fs04_probe_patch.py

What it changes, and why:

  1. Section 5 (the EMMC instrumentation) is re-anchored on the new driver.
     The waits are now time-budgeted per `Phase` and every failure goes
     through `recover`, so the old anchors no longer exist. Nothing here
     reproduces a function body: each wait keeps its own body and only gets a
     thin timing wrapper in front of it (`*_inner`), so a later change inside
     a wait cannot be silently reverted by this patch.

  2. The scope error in the probe's own accounting is fixed. The old probe
     recorded a phase only AFTER it had succeeded, because the `add(...)` sat
     behind the `?` / early return -- exactly the expensive case, a wait that
     ran out of budget, was missing from the totals. That is why
     `dev.write_one n=11` stood against `dev.w.wait_dat n=23` in
     `raw/pi04-FS04.TXT` and 0.8 s in the device could not be reconciled with
     1.6 s in `Fwrite`. Every wrapper now records on EVERY exit path, and two
     `scope.check.*` lines print the identity instead of claiming it.

  3. The driver's own counters (`emmc::error_stats()`) go into the report:
     `timeouts`, `timeouts_by_phase` and the error bits CTO/CCRC/CEND/CBAD/
     DTO/DCRC/DEND, each with the delta for this block. They answer the
     audit's M2 directly. `emmc.spin` is dropped -- since the timeout became a
     duration its iteration count is no longer a limit and carried no
     information; a NOTE line in each block says so.

Everything else is untouched: same block format, same `C:\\FS04.TXT`, same
append behaviour, same ordering guarantee (snapshot, format, serial, then
file I/O outside the measured window).
"""
import sys

p = sys.argv[1]
s = open(p).read()


def sub1(old, new):
    global s
    assert s.count(old) == 1, (s.count(old), old[:80])
    s = s.replace(old, new, 1)


# ── part 1: section 5, re-anchored ─────────────────────────────────────────

NEW5 = r'''# ── 5. EMMC driver: the five-phase split, re-anchored on the new driver ───
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

'''

start = s.index("# ── 5. EMMC driver: per-command timing in five phases ─")
end = s.index("# ── 5b. framebuffer cache maintenance")
s = s[:start] + NEW5 + s[end:]


# ── part 2: the report gets the driver's counters and the scope check ──────

sub1("""use tos_api::fs04probe as fp;

use super::dir::{self, Op, SelEntry};
use crate::kprintln;""",
"""use tos_api::fs04probe as fp;

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
};""")

sub1("""    freq: u64,
    wall: u64,
    prev_write: u64,
    block: u32,
}""",
"""    freq: u64,
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
}""")

sub1("""    freq: 0,
    wall: 0,
    prev_write: 0,
    block: 0,
}));""",
"""    freq: 0,
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
}""")

sub1("""    s.freq = fp::freq();
    s.wall = wall;
    s.prev_write = PREV_WRITE.load(Ordering::Relaxed);
    s.block = BLOCK.fetch_add(1, Ordering::Relaxed) + 1;
}""",
"""    // The driver's own bookkeeping goes into the snapshot too, BEFORE the
    // file write -- which is itself sector traffic and could add to it.
    s.emmc = emmc::error_stats();
    s.emmc_prev = *prev_stats();
    *prev_stats() = s.emmc;
    s.freq = fp::freq();
    s.wall = wall;
    s.prev_write = PREV_WRITE.load(Ordering::Relaxed);
    s.block = BLOCK.fetch_add(1, Ordering::Relaxed) + 1;
}""")

sub1("""    let _ = write!(t, "--- GEMDOS calls (opcode: n, total us, max us) ---\\r\\n");""",
"""    // Does the device total equal the sum of its phases?  Printed rather
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
            "scope.check.r      n(read_one)={} n(sectors)={}  total={}us phases={}us diff={}us\\r\\n",
            s.cnt[fp::slot::DEV_READ],
            s.tck[fp::slot::BLK_READ],
            snap_us(s, s.tck[fp::slot::DEV_READ]),
            snap_us(s, rsum),
            snap_us(s, s.tck[fp::slot::DEV_READ].saturating_sub(rsum))
        );
        let _ = write!(
            t,
            "scope.check.w      n(write_one)={} n(sectors)={}  total={}us phases={}us diff={}us\\r\\n",
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
        "emmc.errors        timeouts={}(+{}) io={}(+{}) recoveries={}(+{}) nonrecov={}(+{}) other={}(+{})\\r\\n",
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
    let _ = write!(t, "\\r\\n");
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
    let _ = write!(t, "\\r\\n");
    let _ = write!(
        t,
        "emmc.last          status=0x{:08x} phase={}\\r\\n",
        e.last_status,
        emmc::PHASE_NAMES[e.last_phase as usize % emmc::Phase::COUNT]
    );
    let _ = write!(
        t,
        "NOTE  `emmc.spin` is gone from this block.  Since the timeout became a\\r\\n\\
         \\x20     duration (c6cf484) its iteration count is no longer a limit and\\r\\n\\
         \\x20     carried no information; a wait that ran out is now reported as a\\r\\n\\
         \\x20     timeout, by phase, on the three emmc.* lines above.\\r\\n"
    );
    let _ = write!(t, "--- GEMDOS calls (opcode: n, total us, max us) ---\\r\\n");""")

sub1("""    let _ = write!(t, "==== end start-up note ====\\r\\n\\r\\n");""",
"""    {
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
        let _ = write!(t, "\\r\\n");
        *prev_stats() = e;
    }
    let _ = write!(t, "==== end start-up note ====\\r\\n\\r\\n");""")

open(p, 'w').write(s)
import ast
ast.parse(s)
print("fs04_probe_patch.py rebased onto c6cf484")
