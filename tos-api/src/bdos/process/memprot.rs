//! Memory protection per address space — FreeMiNT's `PROT_P/G/S/PR/I` on
//! AArch64 (ARCH03 stage 3b, plan `docs/analysis/arch03-implementation-plan.md`
//! §3.2–§3.4, §3.15, invariants I7–I10, decisions E5, E10, E15, E16).
//!
//! ## Model (FreeMiNT `sys/arch/mprot030.c`)
//!
//! Every allocated pool block carries a **protection mode** — FreeMiNT keeps
//! it in `global_mode_table` per page (`mprot030.c:605-615`), NewTOS in a
//! record per block ([`record`]): `Mxalloc` takes it from the mode word or
//! the caller's `PRGFLAGS` (`sys/dosmem.c:209-221`), `Malloc` from the
//! `PRGFLAGS` (nibble 0), a program's TPA and environment from the program's
//! own `PRGFLAGS` (`sys/memory.c:1403-1407,1511-1513`, `sys/k_exec.c:398`),
//! the kernel's own blocks are `PROT_S` (`sys/memory.c:150-159`).
//!
//! What a process may do with a page follows from the mode and whether it is
//! an **owner** (`mark_region`, `mprot030.c:617-693`; table §3.3 of the
//! plan):
//!
//! | mode | owner | everybody else |
//! |---|---|---|
//! | `PROT_P` | read/write/execute | invalid |
//! | `PROT_G` | read/write/execute | read/write/execute |
//! | `PROT_S` | read/write/execute | invalid (EL1 reaches it through the kernel alias) |
//! | `PROT_PR` | read/write/execute | read/execute |
//! | `PROT_I` | invalid | invalid |
//!
//! Free pool memory is invalid for everybody (I8, FreeMiNT `free_region` →
//! `mark_region(PROT_I)`, `sys/memory.c:932`).
//!
//! **Owners** are the block's EmuTOS tag `m_own` and its further owners
//! ([`crate::bdos::mem::region`]). A process slot counts as owner when one of
//! them is a process of its **memory space** — itself, or, for a `Pexec(4)`
//! child, the space it shares ([`super::Process::space_pd`], FreeMiNT
//! `FORK_SHAREVM`, plan E6) — so a thread and its caller see each other's
//! blocks. In addition:
//!
//! * **E5, not strict (default):** a child started with `Pexec` 0/6 keeps
//!   the rights to the blocks its parent's space owned when it started
//!   (FreeMiNT forks the region list and, unless `MPF_STRICT`, does not
//!   revoke it on exec: `sys/k_exec.c:799-806`). The boot argument
//!   `memprot=strict` switches this off.
//! * **E10:** while a user routine of another process runs in the current
//!   one (a `G_USERDEF` callout of a foreign tree), the current space may
//!   reach the owner's blocks ([`grant_temp`]) — FreeMiNT's `prot_temp`
//!   (`mprot030.c:775-805`).
//! * `Ptermres`: what only the resident program owned and was `PROT_P`
//!   becomes `PROT_G` (`sys/k_exit.c:320-336`).
//!
//! ## Switch
//!
//! The boot argument `memprot=off` (FreeMiNT `MEM_PROT=NO`,
//! `sys/bootmenu.c:207-219`) leaves everything as in stage 3a: the pools are
//! mapped once for everybody, block sizes keep the 16-byte grain, no record
//! is kept. With protection on (the default) the pool grain is one page
//! (E16, FreeMiNT `QUANTUM`, `sys/mint/mem.h:152`), so no page has two
//! owners (I10).
//!
//! ## Tables
//!
//! Changes are collected as **dirty ranges** ([`touch`]) and slots that need
//! a full rebuild; [`sync`] brings the tables in line — the shared L3 tables
//! get the non-owner rights, a slot whose rights differ in a 2-MiB region gets
//! a private copy of that region's L3 ([`mmu::make_private`]), a slot whose
//! rights no longer differ is pointed back at the shared table
//! ([`mmu::drop_private`]). [`sync`] runs at the end of every mutating BDOS
//! operation, before every root is installed ([`mmu::install_ttbr0`]) and at
//! the end of every trap, and ends with the TLB maintenance the change needs
//! ([`mmu::flush_tlb`]).
//!
//! A private copy comes from the page-table arena. When the arena is empty
//! the slot keeps the shared, non-owner rights in that region — it can lose
//! access, never gain any — and that is counted ([`table_shortfalls`],
//! [`table_shortfalls_of`]) and reported once per process
//! ([`take_shortfall_report`], printed by the kernel at the end of the trap;
//! ARCH03 3c).
//!
//! ## Concurrency
//!
//! Everything here runs under the BDOS giant lock (single CPU, trap / boot
//! context), like the pools and the tables.

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use tos_types::{BasePage, Long, ProtMode};

use super::mmu::{self, PageFlags, REGION_SPAN};
use super::{MAX_PROCESSES, PROCESS_TABLE};
use crate::bdos::mem::init::{is_initialised, pmd, pmdalt, stram_range, ttram_range};

// ─── Switch ──────────────────────────────────────────────────────────

static ENABLED: AtomicBool = AtomicBool::new(false);
static STRICT: AtomicBool = AtomicBool::new(false);

/// Is memory protection on? (`false` until [`configure`], and with the boot
/// argument `memprot=off`.)
#[inline]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Is the parent-to-child rule strict (`memprot=strict`, E5)?
#[inline]
pub fn strict() -> bool {
    STRICT.load(Ordering::Relaxed)
}

/// Set the switch from the boot arguments. Must run before the BDOS pools
/// are seeded and mapped (the pool grain and the pool mapping depend on it).
///
/// # Safety
///
/// Boot CPU, before any pool allocation.
pub unsafe fn configure(enable: bool, strict_mode: bool) {
    ENABLED.store(enable, Ordering::Relaxed);
    STRICT.store(strict_mode, Ordering::Relaxed);
}

/// The allocation grain of the BDOS pools as an `align_mask`: one page with
/// protection on (E16), else the stage-3a 16 bytes.
#[inline]
pub fn pool_align_mask() -> Long {
    if enabled() { mmu::PAGE_SIZE as Long - 1 } else { 15 }
}

// ─── Block records ───────────────────────────────────────────────────

/// One record per allocated block: at most one per MD.
const MAX_RECORDS: usize =
    crate::bdos::mem::mdpool::MD_BLOCK_COUNT * crate::bdos::mem::mdpool::MDS_PER_BLOCK + 2;

#[derive(Copy, Clone)]
struct Rec {
    /// Block start (`m_start`); `0` = unused.
    loc: u64,
    mode: ProtMode,
    /// Slots that inherited access at their start (E5, not strict).
    inherit: u32,
}

const NO_REC: Rec = Rec { loc: 0, mode: ProtMode::P, inherit: 0 };

struct Records(UnsafeCell<[Rec; MAX_RECORDS]>);
// SAFETY: BDOS giant lock (single CPU).
unsafe impl Sync for Records {}
static RECS: Records = Records(UnsafeCell::new([NO_REC; MAX_RECORDS]));

/// # Safety
///
/// BDOS giant lock held; no other borrow of the records alive.
unsafe fn recs() -> &'static mut [Rec; MAX_RECORDS] {
    // SAFETY: caller contract.
    unsafe { &mut *RECS.0.get() }
}

fn rec_of(r: &mut [Rec; MAX_RECORDS], loc: u64) -> Option<&mut Rec> {
    if loc == 0 {
        return None;
    }
    r.iter_mut().find(|e| e.loc == loc)
}

/// Record (or re-record) the mode of the block that starts at `loc`. A stale
/// record of a freed block at the same address is replaced. `false` only if
/// the table is full, which cannot happen while every record names a live
/// block (one per MD).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn record(loc: u64, mode: ProtMode) -> bool {
    // SAFETY: lock held.
    let r = unsafe { recs() };
    if let Some(e) = rec_of(r, loc) {
        *e = Rec { loc, mode, inherit: 0 };
        return true;
    }
    match r.iter_mut().find(|e| e.loc == 0) {
        Some(e) => {
            *e = Rec { loc, mode, inherit: 0 };
            true
        }
        None => false,
    }
}

/// The protection mode of the block that starts at `loc` (`PROT_P` for a
/// block without a record).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn mode_of(loc: u64) -> ProtMode {
    // SAFETY: lock held.
    rec_of(unsafe { recs() }, loc).map_or(ProtMode::P, |e| e.mode)
}

/// The block that starts at `old` now starts at `new` (the aligned TPA
/// allocation gives its front slack back).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn rekey(old: u64, new: u64) {
    if !enabled() || old == new {
        return;
    }
    // SAFETY: lock held.
    let r = unsafe { recs() };
    if let Some(e) = rec_of(r, new) {
        *e = NO_REC;
    }
    if let Some(e) = rec_of(r, old) {
        e.loc = new;
    }
}

/// A block was just allocated at `[loc, loc + len)` with `mode`: record it
/// and enter its rights. Nothing with protection off.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn on_alloc(loc: u64, len: u64, mode: ProtMode) {
    if !enabled() {
        return;
    }
    // SAFETY: lock held.
    unsafe {
        record(loc, mode);
        touch(loc, loc + len);
        sync();
    }
}

/// The block that started at `loc` and spanned `len` bytes went back to the
/// pool: forget it, and make its pages invalid for everybody (I8) before
/// anybody can be handed them again.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn on_freed(loc: u64, len: u64) {
    if !enabled() {
        return;
    }
    // SAFETY: lock held.
    unsafe {
        if let Some(e) = rec_of(recs(), loc) {
            *e = NO_REC;
        }
        touch(loc, loc + len);
        sync();
    }
}

/// Change the mode of the block that starts at `loc` (`mark_region`).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn set_mode(loc: u64, mode: ProtMode) {
    if !enabled() {
        return;
    }
    // SAFETY: lock held.
    unsafe {
        match rec_of(recs(), loc) {
            Some(e) => e.mode = mode,
            None => {
                record(loc, mode);
            }
        }
        touch_block(loc);
        sync();
    }
}

/// The owners of the block at `loc` changed (attach, detach, hand-over,
/// `set_owner`): enter the new rights.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn on_owner_change(loc: u64) {
    if !enabled() {
        return;
    }
    // SAFETY: lock held.
    unsafe {
        touch_block(loc);
        sync();
    }
}

/// `Ptermres`: the block at `loc` now belongs to the resident holder alone;
/// if it was private it becomes global, so every process can call into and
/// read the resident program (FreeMiNT `sys_ptermres`, `sys/k_exit.c:320-336`:
/// `links == 1 && PROT_P → mark_region(PROT_G)`).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn on_resident(loc: u64) {
    if !enabled() {
        return;
    }
    // SAFETY: lock held.
    unsafe {
        if mode_of(loc) == ProtMode::P {
            set_mode(loc, ProtMode::G);
        } else {
            on_owner_change(loc);
        }
    }
}

// ─── Dirty tracking ──────────────────────────────────────────────────

const MAX_DIRTY: usize = 8;

struct Dirty(UnsafeCell<([(u64, u64); MAX_DIRTY], usize)>);
// SAFETY: BDOS giant lock.
unsafe impl Sync for Dirty {}
static DIRTY: Dirty = Dirty(UnsafeCell::new(([(0, 0); MAX_DIRTY], 0)));

/// Slots whose tables need a full rebuild at the next [`sync`].
static REBUILD: AtomicU32 = AtomicU32::new(0);

/// [`sync`] is running (it must not re-enter itself).
static SYNCING: AtomicBool = AtomicBool::new(false);

/// Times a private table could not be had (arena exhausted), all slots.
static NO_TABLE: AtomicUsize = AtomicUsize::new(0);

/// Per slot: times a private table could not be had for it since its process
/// started ([`table_shortfalls_of`]).
static NO_TABLE_SLOT: [AtomicUsize; MAX_PROCESSES] = [const { AtomicUsize::new(0) }; MAX_PROCESSES];

/// Slots with a shortfall the kernel has not reported yet
/// ([`take_shortfall_report`]).
static SHORT_PENDING: AtomicU32 = AtomicU32::new(0);

/// Slots whose process has been reported already — once per process; cleared
/// when the slot is released ([`on_slot_released`]).
static SHORT_REPORTED: AtomicU32 = AtomicU32::new(0);

/// Note that the rights of `[lo, hi)` may have changed.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn touch(lo: u64, hi: u64) {
    if !enabled() || hi <= lo {
        return;
    }
    let lo = lo & !(mmu::PAGE_SIZE - 1);
    let hi = (hi + mmu::PAGE_SIZE - 1) & !(mmu::PAGE_SIZE - 1);
    // SAFETY: lock held.
    let (ranges, n) = unsafe { &mut *DIRTY.0.get() };
    for r in ranges[..*n].iter_mut() {
        if lo <= r.1 && hi >= r.0 {
            r.0 = r.0.min(lo);
            r.1 = r.1.max(hi);
            return;
        }
    }
    if *n < MAX_DIRTY {
        ranges[*n] = (lo, hi);
        *n += 1;
    } else {
        // Too many: one bounding range.
        let r = &mut ranges[MAX_DIRTY - 1];
        r.0 = r.0.min(lo);
        r.1 = r.1.max(hi);
    }
}

/// [`touch`] the whole block that starts at (or contains) `loc`.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn touch_block(loc: u64) {
    // SAFETY: lock held.
    if let Some((s, e)) = unsafe { block_containing(loc) } {
        // SAFETY: lock held.
        unsafe { touch(s, e) };
    }
}

/// Rebuild `slot`'s tables completely at the next [`sync`] (a new root, a
/// changed memory space, a granted or revoked temporary right).
pub fn mark_rebuild(slot: usize) {
    if enabled() && slot < MAX_PROCESSES {
        REBUILD.fetch_or(1 << slot, Ordering::Relaxed);
    }
}

/// Bring the page tables in line with the recorded rights (see the module
/// docs) and do the TLB maintenance the changes need. Cheap when nothing is
/// pending.
///
/// # Safety
///
/// BDOS giant lock held; EL1 (on the target).
pub unsafe fn sync() {
    if !enabled() || !is_initialised() {
        return;
    }
    // SAFETY: lock held; read of the pending count only.
    if unsafe { (*DIRTY.0.get()).1 } == 0 && REBUILD.load(Ordering::Relaxed) == 0 {
        return; // nothing pending: no table write, no barrier (the trap fast path)
    }
    if SYNCING.swap(true, Ordering::Acquire) {
        return;
    }
    // SAFETY: lock held.
    let (ranges, n) = unsafe { &mut *DIRTY.0.get() };
    let rebuild = REBUILD.swap(0, Ordering::Relaxed);
    if rebuild != 0 {
        for (s, e) in pools() {
            // SAFETY: lock held.
            unsafe { refresh(s, e, rebuild) };
        }
    }
    while *n > 0 {
        *n -= 1;
        let (lo, hi) = ranges[*n];
        // SAFETY: lock held.
        unsafe { refresh(lo, hi, 0) };
    }
    // SAFETY: EL1, barriers + TLB maintenance only.
    unsafe { mmu::flush_tlb() };
    SYNCING.store(false, Ordering::Release);
}

// ─── Rights ──────────────────────────────────────────────────────────

/// E10 grants: `TEMP[s]` = slots whose blocks slot `s` may reach for now.
static TEMP: [AtomicU32; MAX_PROCESSES] = [const { AtomicU32::new(0) }; MAX_PROCESSES];

/// Is `slot` a process with a root (its tables built)?
fn live(slot: usize) -> bool {
    PROCESS_TABLE[slot].l0_pa.load(Ordering::Acquire) != 0
}

/// The memory space of `slot`: its own PD, or the PD of the space it shares
/// as a `Pexec(4)` child.
fn space_root(slot: usize) -> u64 {
    let sp = PROCESS_TABLE[slot].space_pd.load(Ordering::Acquire);
    if sp.is_null() { super::pd_of(slot) as u64 } else { sp as u64 }
}

/// For every live slot, the set of slots in its memory space.
fn group_masks() -> [u32; MAX_PROCESSES] {
    let mut roots = [0u64; MAX_PROCESSES];
    let mut alive = 0u32;
    for (s, root) in roots.iter_mut().enumerate() {
        if live(s) {
            *root = space_root(s);
            alive |= 1 << s;
        }
    }
    let mut g = [0u32; MAX_PROCESSES];
    for s in 0..MAX_PROCESSES {
        if alive & (1 << s) == 0 || roots[s] == 0 {
            continue;
        }
        for t in 0..MAX_PROCESSES {
            if alive & (1 << t) != 0 && roots[t] == roots[s] {
                g[s] |= 1 << t;
            }
        }
    }
    g
}

/// The live slot (one with a root) whose PD pointer is `pd`. Unlike
/// [`super::slot_for`] it does not look at the slot's state: a root exists from the
/// moment the loader built it (some kernel launch paths — the self-tests —
/// run a slot they never claimed), and its rights must be there before its
/// first instruction.
fn live_slot_of(pd: *mut c_void) -> Option<usize> {
    if pd.is_null() {
        return None;
    }
    let kva = super::kalias(pd as u64) as *mut BasePage;
    (0..MAX_PROCESSES).find(|&s| live(s) && PROCESS_TABLE[s].basepage_kva.load(Ordering::Acquire) == kva)
}

/// The slots behind the owner cookies of a block (`m_own` + further owners).
fn owner_slots(own: *mut c_void, further: &[*mut c_void]) -> u32 {
    let mut m = 0u32;
    for &o in core::iter::once(&own).chain(further.iter()) {
        if let Some(s) = live_slot_of(o) {
            m |= 1 << s;
        }
    }
    m
}

/// What an owner of a block in `mode` may do with its pages (table §3.3).
fn owner_flags(mode: ProtMode) -> Option<PageFlags> {
    match mode {
        ProtMode::I => None,
        _ => Some(PageFlags::UserRwExec),
    }
}

/// What everybody else may do (table §3.3, FreeMiNT `other_dt/other_s/
/// other_wp`, `mprot030.c:400-402`).
fn other_flags(mode: ProtMode) -> Option<PageFlags> {
    match mode {
        ProtMode::G => Some(PageFlags::UserRwExec),
        ProtMode::Pr => Some(PageFlags::UserRoExec),
        ProtMode::P | ProtMode::S | ProtMode::I => None,
    }
}

/// Does an owner see this mode differently from everybody else?
fn owner_differs(mode: ProtMode) -> bool {
    matches!(mode, ProtMode::P | ProtMode::S | ProtMode::Pr)
}

/// The pool ranges, as `[start, end)`.
fn pools() -> impl Iterator<Item = (u64, u64)> {
    [stram_range(), ttram_range()]
        .into_iter()
        .filter(|&(s, e)| e > s)
        .map(|(s, e)| (s as u64, e as u64))
}

fn in_pool(va: u64) -> bool {
    pools().any(|(s, e)| va >= s && va < e)
}

/// One allocated block overlapping the region being refreshed.
#[derive(Copy, Clone)]
struct Blk {
    mode: ProtMode,
    owners: u32,
    inherit: u32,
}

const NO_BLK: Blk = Blk { mode: ProtMode::P, owners: 0, inherit: 0 };

/// Page → block index + 1 in the region (`0` = free pool page,
/// [`NOT_POOL`] = not pool memory — left alone).
const NOT_POOL: u16 = u16::MAX;

struct Scratch(UnsafeCell<([Blk; MAX_RECORDS], [u16; 512])>);
// SAFETY: BDOS giant lock.
unsafe impl Sync for Scratch {}
static SCRATCH: Scratch = Scratch(UnsafeCell::new(([NO_BLK; MAX_RECORDS], [0; 512])));

/// Does `slot` hold owner rights on `b`?
fn has_rights(slot: usize, b: &Blk, groups: &[u32; MAX_PROCESSES]) -> bool {
    b.owners & groups[slot] != 0
        || b.inherit & (1 << slot) != 0
        || b.owners & TEMP[slot].load(Ordering::Relaxed) != 0
}

/// The leaf descriptor of pool page `va` for `slot` (`None` = the shared,
/// non-owner pattern).
fn page_desc(va: u64, bi: u16, blks: &[Blk], slot: Option<(usize, &[u32; MAX_PROCESSES])>) -> u64 {
    if bi == 0 {
        return 0; // free pool memory: nobody's (I8)
    }
    let b = &blks[(bi - 1) as usize];
    let flags = match slot {
        Some((s, groups)) if has_rights(s, b, groups) => owner_flags(b.mode),
        _ => other_flags(b.mode),
    };
    mmu::leaf_desc(va, flags)
}

/// Refresh the rights of the pool pages in `[lo, hi)` for the shared tables
/// and every live slot; slots in `full` get their whole 2-MiB regions
/// recomputed (a rebuild).
///
/// # Safety
///
/// BDOS giant lock held.
unsafe fn refresh(lo: u64, hi: u64, full: u32) {
    let groups = group_masks();
    // SAFETY: lock held; exclusive scratch.
    let (blks, pageb) = unsafe { &mut *SCRATCH.0.get() };
    let mut region = lo & !(REGION_SPAN - 1);
    while region < hi {
        let rend = region + REGION_SPAN;
        // SAFETY: lock held.
        let Some(shared) = (unsafe { mmu::shared_l3(region) }) else {
            region = rend;
            continue;
        };
        // The region's blocks, and which page belongs to which.
        let mut nb = 0usize;
        for (i, p) in pageb.iter_mut().enumerate() {
            *p = if in_pool(region + (i as u64) * mmu::PAGE_SIZE) { 0 } else { NOT_POOL };
        }
        for mp in [pmd(), pmdalt()] {
            // SAFETY: lock held; read-only list walk.
            let mut m = unsafe { (*mp).mp_mal };
            while !m.is_null() && nb < blks.len() {
                // SAFETY: list invariant.
                let (start, len, own, next) =
                    unsafe { ((*m).m_start as u32 as u64, (*m).m_length as u32 as u64, (*m).m_own, (*m).m_link) };
                let end = start + len;
                if start < rend && end > region && len > 0 {
                    // SAFETY: lock held.
                    let further = unsafe { crate::bdos::mem::region::further_owners(start as Long) };
                    // SAFETY: lock held.
                    let (mode, inherit) = match rec_of(unsafe { recs() }, start) {
                        Some(r) => (r.mode, r.inherit),
                        None => (ProtMode::P, 0),
                    };
                    blks[nb] = Blk { mode, owners: owner_slots(own, &further), inherit };
                    nb += 1;
                    let a = start.max(region);
                    let z = end.min(rend);
                    let mut va = a & !(mmu::PAGE_SIZE - 1);
                    while va < z {
                        let i = ((va - region) / mmu::PAGE_SIZE) as usize;
                        if pageb[i] != NOT_POOL {
                            pageb[i] = nb as u16;
                        }
                        va += mmu::PAGE_SIZE;
                    }
                }
                m = next;
            }
        }
        let blks = &blks[..nb];
        // The pages of this region inside the range.
        let i_lo = ((lo.max(region) - region) / mmu::PAGE_SIZE) as usize;
        let i_hi = ((hi.min(rend) - region).div_ceil(mmu::PAGE_SIZE)) as usize;
        // Shared tables: the non-owner pattern.
        for (i, &bi) in pageb.iter().enumerate().take(i_hi).skip(i_lo) {
            if bi == NOT_POOL {
                continue;
            }
            let va = region + (i as u64) * mmu::PAGE_SIZE;
            // SAFETY: lock held; `shared` is a live table.
            unsafe { mmu::set_leaf(shared, i, page_desc(va, bi, blks, None)) };
        }
        // Every live slot.
        for s in 0..MAX_PROCESSES {
            if !live(s) {
                continue;
            }
            let needs = blks.iter().any(|b| owner_differs(b.mode) && has_rights(s, b, &groups));
            if !needs {
                // SAFETY: lock held.
                unsafe { mmu::drop_private(s, region) };
                continue;
            }
            // SAFETY: lock held.
            let had = unsafe { mmu::slot_private_l3(s, region) }.is_some();
            // SAFETY: lock held.
            let p = match unsafe { mmu::make_private(s, region) } {
                Ok(p) => p,
                Err(_) => {
                    // No table left: the slot keeps the shared (non-owner)
                    // rights here — it can lose access, never gain it. Not
                    // silent: counted, and reported once per process.
                    note_shortfall(s);
                    continue;
                }
            };
            let (a, z) = if !had || full & (1 << s) != 0 { (0, 512) } else { (i_lo, i_hi) };
            for (i, &bi) in pageb.iter().enumerate().take(z).skip(a) {
                if bi == NOT_POOL {
                    continue;
                }
                let va = region + (i as u64) * mmu::PAGE_SIZE;
                // SAFETY: lock held; `p` is the slot's private table.
                unsafe { mmu::set_leaf(p, i, page_desc(va, bi, blks, Some((s, &groups)))) };
            }
        }
        region = rend;
    }
}

/// Times a slot could not get a private table (the arena was exhausted),
/// over all processes since boot.
pub fn table_shortfalls() -> usize {
    NO_TABLE.load(Ordering::Relaxed)
}

/// Times `slot`'s current process could not get a private table.
pub fn table_shortfalls_of(slot: usize) -> usize {
    NO_TABLE_SLOT.get(slot).map_or(0, |c| c.load(Ordering::Relaxed))
}

/// Count a shortfall for `slot` and queue its report if its process has not
/// been reported yet.
fn note_shortfall(slot: usize) {
    NO_TABLE.fetch_add(1, Ordering::Relaxed);
    NO_TABLE_SLOT[slot].fetch_add(1, Ordering::Relaxed);
    if SHORT_REPORTED.load(Ordering::Relaxed) & (1 << slot) == 0 {
        SHORT_PENDING.fetch_or(1 << slot, Ordering::Relaxed);
    }
}

/// A page-table shortfall to report on the serial line ([`take_shortfall_report`]).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ShortfallReport {
    /// The process slot that could not get a private table.
    pub slot: usize,
    /// Its basepage (PD) address.
    pub pd: u64,
    /// Shortfalls of this process so far.
    pub of_process: usize,
    /// Shortfalls of all processes since boot ([`table_shortfalls`]).
    pub total: usize,
}

/// The next process whose page tables could not get a private table and
/// has not been reported yet — the kernel prints one line per process
/// (`kernel/src/arch/aarch64/exceptions.rs`, end of every trap). Marks it
/// reported: the same process is not reported again.
pub fn take_shortfall_report() -> Option<ShortfallReport> {
    let pending = SHORT_PENDING.load(Ordering::Relaxed);
    if pending == 0 {
        return None;
    }
    let slot = pending.trailing_zeros() as usize;
    SHORT_PENDING.fetch_and(!(1 << slot), Ordering::Relaxed);
    SHORT_REPORTED.fetch_or(1 << slot, Ordering::Relaxed);
    Some(ShortfallReport {
        slot,
        pd: super::pd_of(slot) as u64,
        of_process: table_shortfalls_of(slot),
        total: table_shortfalls(),
    })
}

// ─── Process events ──────────────────────────────────────────────────

/// A root was (re)built for `slot` ([`super::loader`]): enter its rights.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn on_root_built(slot: usize) {
    if !enabled() {
        return;
    }
    mark_rebuild(slot);
    // SAFETY: lock held.
    unsafe { sync() };
}

/// `slot` starts running, launched by the process `parent` (`null`: by the
/// kernel) — `Pexec` 0/4/6. A `Pexec(4)` child shares its caller's space
/// (its rights follow from [`super::Process::space_pd`]); any other child
/// of a process inherits the rights to the blocks its parent's space owns
/// now, unless the mode is strict (E5, FreeMiNT `sys/k_exec.c:799-806`).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn on_launch(slot: usize, parent: *mut BasePage) {
    if !enabled() || slot >= MAX_PROCESSES {
        return;
    }
    let thread = !PROCESS_TABLE[slot].space_pd.load(Ordering::Acquire).is_null();
    if !thread && !strict() {
        if let Some(ps) = live_slot_of(parent.cast()) {
            let groups = group_masks();
            let pg = groups[ps] | (1 << ps);
            for mp in [pmd(), pmdalt()] {
                // SAFETY: lock held; read-only list walk.
                let mut m = unsafe { (*mp).mp_mal };
                while !m.is_null() {
                    // SAFETY: list invariant.
                    let (start, own, next) = unsafe { ((*m).m_start as u32 as u64, (*m).m_own, (*m).m_link) };
                    // SAFETY: lock held.
                    let further = unsafe { crate::bdos::mem::region::further_owners(start as Long) };
                    if owner_slots(own, &further) & pg != 0 {
                        // SAFETY: lock held.
                        let r = unsafe { recs() };
                        if rec_of(r, start).is_none() {
                            // SAFETY: lock held.
                            unsafe { record(start, ProtMode::P) };
                        }
                        if let Some(e) = rec_of(r, start) {
                            e.inherit |= 1 << slot;
                        }
                    }
                    m = next;
                }
            }
        }
    }
    mark_rebuild(slot);
    // SAFETY: lock held.
    unsafe { sync() };
}

/// `slot` is released: its private tables go back to the arena, its
/// inherited and temporary rights are forgotten.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn on_slot_released(slot: usize) {
    if !enabled() || slot >= MAX_PROCESSES {
        return;
    }
    // SAFETY: lock held.
    for e in unsafe { recs() }.iter_mut() {
        e.inherit &= !(1 << slot);
    }
    TEMP[slot].store(0, Ordering::Relaxed);
    for t in &TEMP {
        t.fetch_and(!(1 << slot), Ordering::Relaxed);
    }
    // The next process in this slot is reported on its own.
    NO_TABLE_SLOT[slot].store(0, Ordering::Relaxed);
    SHORT_PENDING.fetch_and(!(1 << slot), Ordering::Relaxed);
    SHORT_REPORTED.fetch_and(!(1 << slot), Ordering::Relaxed);
    REBUILD.fetch_and(!(1 << slot), Ordering::Relaxed);
    // SAFETY: lock held.
    unsafe {
        mmu::release_private_tables(slot);
        // The released root may still be the installed one until the next
        // switch: drop what the TLB holds of it.
        mmu::flush_tlb_always();
    }
}

/// E10: let `slot` reach the blocks of `owner`'s memory space while a user
/// routine of `owner` runs in `slot` (a `G_USERDEF` callout of a foreign
/// tree) — FreeMiNT `prot_temp(loc, len, -1)` (`mprot030.c:775-805`) for
/// the owner's regions. Revoked by [`revoke_temp`].
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn grant_temp(slot: usize, owner: usize) {
    if !enabled() || slot >= MAX_PROCESSES || owner >= MAX_PROCESSES {
        return;
    }
    let g = group_masks()[owner] | (1 << owner);
    TEMP[slot].fetch_or(g, Ordering::Relaxed);
    mark_rebuild(slot);
    // SAFETY: lock held.
    unsafe { sync() };
}

/// End every temporary right of `slot` ([`grant_temp`]).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn revoke_temp(slot: usize) {
    if !enabled() || slot >= MAX_PROCESSES {
        return;
    }
    if TEMP[slot].swap(0, Ordering::Relaxed) != 0 {
        mark_rebuild(slot);
        // SAFETY: lock held.
        unsafe { sync() };
    }
}

// ─── Lookups ─────────────────────────────────────────────────────────

/// The allocated block `[start, end)` that contains `addr`, on either pool.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn block_containing(addr: u64) -> Option<(u64, u64)> {
    if !is_initialised() {
        return None;
    }
    for mp in [pmd(), pmdalt()] {
        // SAFETY: lock held; read-only list walk.
        let mut m = unsafe { (*mp).mp_mal };
        while !m.is_null() {
            // SAFETY: list invariant.
            let (start, len, next) =
                unsafe { ((*m).m_start as u32 as u64, (*m).m_length as u32 as u64, (*m).m_link) };
            if addr >= start && addr < start + len {
                return Some((start, start + len));
            }
            m = next;
        }
    }
    None
}

/// What a faulting address was, for the crash report — FreeMiNT
/// `report_buserr` (`sys/arch/mprot.x:71-124`): the mode of the block it
/// lies in ("private", "super", "readable"; a global block is a hardware
/// error), "free" pool memory, or memory protection does not control
/// ("hardware"). `None` with protection off (FreeMiNT reports nothing then,
/// `mprot.x:96-97`).
pub fn region_kind(far: u64) -> Option<crate::bios::crash::RegionKind> {
    use crate::bios::crash::RegionKind;
    if !enabled() {
        return None;
    }
    if !in_pool(far) {
        return Some(RegionKind::Hardware);
    }
    // SAFETY: the crash path runs in the trap of the faulting program, under
    // the BDOS lock; read-only walks.
    unsafe {
        match block_containing(far) {
            None => Some(RegionKind::Free),
            Some((start, _)) => Some(match mode_of(start) {
                ProtMode::P => RegionKind::Private,
                ProtMode::G => RegionKind::Hardware,
                ProtMode::S => RegionKind::Super,
                ProtMode::Pr => RegionKind::Readable,
                ProtMode::I => RegionKind::Free,
            }),
        }
    }
}
