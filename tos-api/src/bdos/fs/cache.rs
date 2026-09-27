//! Sector cache (Phase 09 D2). Mirrors EmuTOS `bdos/fsbuf.c`.
//!
//! A small write-back cache of 512-byte sectors sitting between the FAT /
//! directory / file layers and the block device. Reads populate it;
//! repeated accesses to the same FAT or directory sector (the common case
//! in `fat_entry`, the directory scan and `Dfree`) hit the cache instead
//! of re-issuing an SD read. Writes mark slots dirty for write-back; the
//! write side flushes via [`flush`].
//!
//! Single block device, single-CPU, IRQ-off trap path — no locking.
//!
//! # Reporting a hard device error (HW03)
//!
//! EmuTOS reports a failed `Rwabs` out of the BDOS with a `longjmp`:
//! `longjmp_rwabs` (`bdos/fs.h:60`) stores the BIOS code in the globals
//! `rwerr`/`errcode`/`errdrv` (`bdos/fsglob.c:40-47`) and jumps to the
//! `setjmp(errbuf)` at the top of `osif`, which "handle[s] as hard error on
//! disk" and returns the code "to caller as if it were the function's
//! return value" (`bdos/bdosmain.c:448-492`, `docs/api/gemdos/_dispatcher.md`
//! §8).
//!
//! There is no `longjmp` here, so the same contract is built from two
//! pieces:
//!
//! * [`flush`] returns the code — `Result` is `#[must_use]`, so a call site
//!   that drops it no longer compiles silently. All nine call sites used to
//!   discard a `bool` (audit finding 5).
//! * the `bool`-typed interior (`read`, `write`, and everything in
//!   `fat.rs`/`dirops.rs` above them) records the code in [`LAST_ERROR`] —
//!   this module's `rwerr` — and the GEMDOS entry point returns it with
//!   [`take_error`]. That is what turns a `Fwrite` short count caused by a
//!   *device error* back into `EWRITF`, instead of letting it pass for
//!   "disk full" (`docs/api/gemdos/file/40-fwrite.md` §"Error conditions").

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering};

use tos_types::error::{EUNDEV, E_OK};
use tos_types::Long;

use crate::bios::block::BlockDev;
use crate::host;

const SLOTS: usize = 16;
const SECTOR: usize = 512;

/// How often the cache re-attempts the write-back of one and the same
/// dirty slot before it stops trying on its own.
///
/// A retry is legitimate — the Pi measurement of 2026-09-25 had twelve of
/// twenty-three sector writes fail and land on a later attempt
/// (`docs/api/hw/emmc-audit.md` §3.2). An *unbounded* retry is not: before
/// HW03 a slot whose write-back failed kept its dirty flag and was tried
/// again by every later `flush()`, for ever, without anyone learning of it.
///
/// The number is a policy choice, not a specification value, and is named
/// here as one. EmuTOS bounds the same thing in two places and lower:
/// `RWABS_RETRIES = 1` at the BIOS layer ("on real machine might want to
/// increase this", `bios/blkdev.h:34`) and "retry at most once" in its SD
/// driver (`bios/sd.c:167-169`). Three gives a transient card busy two more
/// chances than EmuTOS' one and still terminates.
const WRITEBACK_ATTEMPTS: u8 = 3;

struct Slot {
    lba: u64,
    valid: bool,
    dirty: bool,
    /// Failed write-back attempts for the data currently in this slot.
    /// Reset to 0 by a successful write-back and by a fresh [`write`] of
    /// the same sector (new data is a new request).
    wb_fails: u8,
    /// TOS error code of this slot's last failed write-back, so a later
    /// access can report *that* failure rather than a guess.
    wb_err: Long,
    data: [u8; SECTOR],
}

struct Cache {
    slots: UnsafeCell<[Slot; SLOTS]>,
    /// Round-robin eviction pointer.
    victim: AtomicUsize,
}
// SAFETY: accessed only from the single-CPU, IRQ-off trap path.
unsafe impl Sync for Cache {}

static CACHE: Cache = Cache {
    slots: UnsafeCell::new(
        [const {
            Slot {
                lba: 0,
                valid: false,
                dirty: false,
                wb_fails: 0,
                wb_err: E_OK,
                data: [0u8; SECTOR],
            }
        }; SLOTS],
    ),
    victim: AtomicUsize::new(0),
};

static HITS: AtomicU32 = AtomicU32::new(0);
static MISSES: AtomicU32 = AtomicU32::new(0);

/// The hard error of the most recent failed block access — EmuTOS `rwerr` /
/// `errcode` (`bdos/fsglob.c:40-41`). `E_OK` when there is none pending.
static LAST_ERROR: AtomicI32 = AtomicI32::new(E_OK);

// ── Write-back bookkeeping (HW03 point 4: a retry must be visible) ────
/// Write-backs that failed, counting every attempt.
static WB_FAILURES: AtomicU32 = AtomicU32::new(0);
/// Write-backs that were a **repeat** attempt at a slot that had already
/// failed at least once — the retries that used to be silent.
static WB_RETRIES: AtomicU32 = AtomicU32::new(0);
/// Slots that used up [`WRITEBACK_ATTEMPTS`] and are no longer retried
/// automatically. Their data is still in the cache and still dirty.
static WB_EXHAUSTED: AtomicU32 = AtomicU32::new(0);

fn block() -> Option<&'static dyn BlockDev> {
    host::host().and_then(|h| h.block())
}

fn slots() -> &'static mut [Slot; SLOTS] {
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut *CACHE.slots.get() }
}

/// Record a hard device error for the running GEMDOS call — EmuTOS
/// `errcode = rwerr` in `longjmp_rwabs` (`bdos/fs.h:66`).
fn set_error(code: Long) {
    LAST_ERROR.store(code, Ordering::Relaxed);
}

/// Forget any pending hard error. Called at the top of a GEMDOS call, the
/// place EmuTOS establishes a fresh `setjmp(errbuf)` context
/// (`bdos/bdosmain.c:448`).
pub fn clear_error() {
    LAST_ERROR.store(E_OK, Ordering::Relaxed);
}

/// Take the pending hard device error, if any, and clear it. The GEMDOS
/// entry points return it as their own result — `osif`'s "return the error
/// to caller as if it were the function's return value"
/// (`docs/api/gemdos/_dispatcher.md` §8).
pub fn take_error() -> Option<Long> {
    let e = LAST_ERROR.swap(E_OK, Ordering::Relaxed);
    if e == E_OK {
        None
    } else {
        Some(e)
    }
}

/// Write one dirty slot back to the device.
///
/// On success the slot is clean. **On failure the slot keeps its data, its
/// `valid` flag and its dirty flag** — losing it would throw away the only
/// up-to-date copy of the sector (audit finding 4: before HW03 the
/// eviction path cleared `dirty` unconditionally and then overwrote the
/// slot, so a FAT sector could disappear with no trace anywhere).
///
/// This is a deliberate, named deviation from EmuTOS: its `flush()` sets
/// `b_bufdrv = -1` *before* the write ("invalidate in case of error",
/// `bdos/fsbuf.c:93`) and its `osif` error handler then drops every buffer
/// of the drive (`mark_bcbs_invalid`, `bdos/bdosmain.c:490`). EmuTOS does
/// that because a `longjmp` cannot carry the buffer back — the buffer is
/// unreachable from the point the jump lands. We do not jump, so we are not
/// forced to discard, and the ticket's integrity requirement is exactly
/// that no sector disappears because its write-back failed.
fn write_back(s: &mut Slot) -> Result<(), Long> {
    if s.wb_fails > 0 {
        WB_RETRIES.fetch_add(1, Ordering::Relaxed);
    }
    let r = match block() {
        Some(b) => b.write_blocks(s.lba, &s.data),
        // No block device behind a mounted volume: the same code `Rwabs`
        // gives an unknown device (`bios/mod.rs`, EmuTOS `blkdev.c:440-441`).
        None => Err(EUNDEV),
    };
    match r {
        Ok(()) => {
            s.dirty = false;
            s.wb_fails = 0;
            s.wb_err = E_OK;
            Ok(())
        }
        Err(code) => {
            WB_FAILURES.fetch_add(1, Ordering::Relaxed);
            s.wb_fails = s.wb_fails.saturating_add(1);
            s.wb_err = code;
            if s.wb_fails == WRITEBACK_ATTEMPTS {
                WB_EXHAUSTED.fetch_add(1, Ordering::Relaxed);
            }
            set_error(code);
            Err(code)
        }
    }
}

/// May this slot's write-back still be attempted automatically?
///
/// `false` once the slot has used up [`WRITEBACK_ATTEMPTS`]. The sector
/// stays in the cache and stays dirty; what stops is the driver traffic —
/// every later `flush()` reports the error straight away instead of
/// re-issuing a write that has already failed three times.
fn retryable(s: &Slot) -> bool {
    s.wb_fails < WRITEBACK_ATTEMPTS
}

/// Pick a victim slot (round robin), writing it back first if dirty.
///
/// Returns the failure code when the victim is dirty and cannot be written
/// back: the slot is then **not** handed out, so the caller's access fails
/// rather than silently overwriting a sector that never reached the card.
/// The round-robin pointer has already moved on, so the next access picks a
/// different slot and one bad sector does not block the whole cache.
fn evict(slots: &mut [Slot; SLOTS]) -> Result<usize, Long> {
    let v = CACHE.victim.fetch_add(1, Ordering::Relaxed) % SLOTS;
    if slots[v].valid && slots[v].dirty {
        if !retryable(&slots[v]) {
            // Out of attempts: do not touch the device again, but do not
            // hand the slot out either — its sector is still only here.
            let code = slots[v].wb_err;
            set_error(code);
            return Err(code);
        }
        write_back(&mut slots[v])?;
    }
    Ok(v)
}

/// Read sector `lba` into `out`, through the cache.
///
/// `false` means the access failed; the TOS code is in [`take_error`].
pub fn read(lba: u64, out: &mut [u8; SECTOR]) -> bool {
    let slots = slots();
    for s in slots.iter() {
        if s.valid && s.lba == lba {
            out.copy_from_slice(&s.data);
            HITS.fetch_add(1, Ordering::Relaxed);
            return true;
        }
    }
    MISSES.fetch_add(1, Ordering::Relaxed);

    let v = match evict(slots) {
        Ok(v) => v,
        // The victim's dirty sector could not be written back. Giving the
        // slot away here is exactly the loss of audit finding 4.
        Err(_) => return false,
    };
    match block() {
        Some(b) => {
            if let Err(code) = b.read_blocks(lba, &mut slots[v].data) {
                set_error(code);
                slots[v].valid = false;
                return false;
            }
        }
        None => {
            set_error(EUNDEV);
            slots[v].valid = false;
            return false;
        }
    }
    slots[v].lba = lba;
    slots[v].valid = true;
    slots[v].dirty = false;
    slots[v].wb_fails = 0;
    slots[v].wb_err = E_OK;
    out.copy_from_slice(&slots[v].data);
    true
}

/// Write sector `lba` from `data` into the cache (write-back: the slot is
/// marked dirty and flushed later).
///
/// `false` means no slot could be made available because another sector's
/// write-back failed; the TOS code is in [`take_error`].
pub fn write(lba: u64, data: &[u8; SECTOR]) -> bool {
    let slots = slots();
    for s in slots.iter_mut() {
        if s.valid && s.lba == lba {
            s.data.copy_from_slice(data);
            s.dirty = true;
            // New content for this sector: the old content's failed
            // attempts do not count against it.
            s.wb_fails = 0;
            s.wb_err = E_OK;
            return true;
        }
    }
    let v = match evict(slots) {
        Ok(v) => v,
        Err(_) => return false,
    };
    slots[v].data.copy_from_slice(data);
    slots[v].lba = lba;
    slots[v].valid = true;
    slots[v].dirty = true;
    slots[v].wb_fails = 0;
    slots[v].wb_err = E_OK;
    true
}

/// Write back all dirty sectors to the block device.
///
/// Returns the TOS error code of the **first** failure, after having tried
/// every other dirty slot: one bad sector must not keep the rest of the
/// volume's metadata in memory. Sectors that fail stay in the cache, dirty
/// (see [`write_back`]).
pub fn flush() -> Result<(), Long> {
    let slots = slots();
    let mut first_err = None;
    for s in slots.iter_mut() {
        if !(s.valid && s.dirty) {
            continue;
        }
        if !retryable(s) {
            // Already given up on automatically; still an error for the
            // caller, because this sector is not on the card.
            let code = s.wb_err;
            set_error(code);
            first_err.get_or_insert(code);
            continue;
        }
        if let Err(code) = write_back(s) {
            first_err.get_or_insert(code);
        }
    }
    match first_err {
        Some(code) => Err(code),
        None => Ok(()),
    }
}

/// Cache statistics: `(hits, misses)`.
pub fn stats() -> (u32, u32) {
    (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed))
}

/// Write-back statistics: `(failures, retries, exhausted, dirty_slots)`.
///
/// * `failures` — write-back attempts that the device rejected, every
///   attempt counted.
/// * `retries` — of those attempts, the ones that were a repeat at a slot
///   that had already failed. This is the number that used to be invisible.
/// * `exhausted` — slots that used up [`WRITEBACK_ATTEMPTS`]; their sector
///   is still in the cache and still dirty, but no longer written back on
///   its own.
/// * `dirty_slots` — dirty slots right now, i.e. sectors the card has not
///   got yet.
pub fn writeback_stats() -> (u32, u32, u32, u32) {
    let dirty = slots().iter().filter(|s| s.valid && s.dirty).count() as u32;
    (
        WB_FAILURES.load(Ordering::Relaxed),
        WB_RETRIES.load(Ordering::Relaxed),
        WB_EXHAUSTED.load(Ordering::Relaxed),
        dirty,
    )
}
