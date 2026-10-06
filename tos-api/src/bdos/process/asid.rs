//! Address-space identifiers for the per-process roots — ARCH03 stage 4
//! (`docs/analysis/arch03-implementation-plan.md` Frage 15, §3.2 "TLBI und
//! QUIRK-001", Stufe 4; E12).
//!
//! Pure bookkeeping, no system registers: which ASID a slot's root carries
//! in `TTBR0_EL1[63:48]`, and which TLB invalidations a page-table change
//! still owes. The register writes and the `tlbi` instructions live in
//! [`super::mmu`]; this module is host-testable on its own.
//!
//! ## Architecture (ARM ARM DDI 0487 M.d, verified in V0a §2/§3)
//!
//! * With `TCR_EL1.A1 = 0` the ASID of every EL1&0 translation is
//!   `TTBR0_EL1.ASID` (`RSCBSR`, D8-7853/7854). 8 bits with `TCR_EL1.AS = 0`,
//!   16 bits with `AS = 1`, the latter only where
//!   `ID_AA64MMFR0_EL1.ASIDBits = 0b0010` (D24-9917, D24-9247; Cortex-A72:
//!   16 bits, TRM Table 4-35).
//! * A leaf with `nG = 1` applies to the current ASID only (`RQGKGF`,
//!   D8-7854); entries of non-final lookup levels are non-global in any case
//!   (`RCHVGJ`).
//! * Entries are tagged with the ASID they were walked under and stay in the
//!   TLB across a `TTBR0_EL1` write (`IHGKKC`): a switch to another root with
//!   another ASID needs no invalidation, but **reusing** an ASID for other
//!   tables does.
//!
//! ## Allocation (generations)
//!
//! An ASID belongs to a slot's *tenancy*, not to the slot: building a slot's
//! root for a new process **retires** the old ASID ([`Allocator::retire`]),
//! and the next install hands out a fresh one ([`Allocator::assign`]). Numbers
//! are handed out in ascending order and never twice within one
//! **generation**, so a retired number's stale entries can never be matched
//! by somebody else. When the numbers run out a new generation starts: every
//! slot's ASID becomes stale and the caller must drop the whole TLB
//! (`TLBI VMALLE1IS` — not `ASIDE1`, whose wording in `RFKSVM` D8-7868
//! limits it to final-level entries, V0a C6/B7) **before** the first number of
//! the new generation is used, with no stale-generation ASID left in
//! `TTBR0_EL1` while it does so (see [`super::mmu`]). ASID 0 is never handed
//! out: it is the "no process" root's (the empty L0 the kernel installs
//! between processes).

use super::MAX_PROCESSES;

/// ASID 0 — reserved for the kernel's empty no-process root.
pub const NO_ASID: u16 = 0;

/// Per-slot ASID bookkeeping with generations.
#[derive(Debug)]
pub struct Allocator {
    /// Numbers per generation (ASIDs `1..limit` are handed out).
    limit: u32,
    /// Current generation (starts at 1; 0 = "never assigned").
    generation: u32,
    /// Next number to hand out in this generation.
    next: u32,
    /// The slot's ASID and the generation it was handed out in.
    asid: [u16; MAX_PROCESSES],
    gen_of: [u32; MAX_PROCESSES],
    /// Generations started after the first (statistics, tests).
    rollovers: u32,
    /// ASIDs handed out since boot (statistics, tests).
    assigned: u64,
}

impl Allocator {
    /// An allocator with no numbers (disabled) — [`Self::configure`] gives it
    /// its range.
    pub const fn new() -> Self {
        Self {
            limit: 0,
            generation: 1,
            next: 1,
            asid: [NO_ASID; MAX_PROCESSES],
            gen_of: [0; MAX_PROCESSES],
            rollovers: 0,
            assigned: 0,
        }
    }

    /// Use ASIDs `1..limit` (`limit` = 2^ASID width, at most 65 536; at least
    /// 2 so that one number exists). Forgets every assignment.
    pub fn configure(&mut self, limit: u32) {
        *self = Self::new();
        self.limit = limit.clamp(2, 1 << 16);
    }

    /// Numbers per generation as configured (`0` = not configured).
    pub fn limit(&self) -> u32 {
        self.limit
    }

    /// The slot's ASID if it holds one of the current generation — the only
    /// case in which the TLB can hold entries of the slot's present tables.
    pub fn current(&self, slot: usize) -> Option<u16> {
        if slot >= MAX_PROCESSES {
            return None;
        }
        let a = self.asid[slot];
        (a != NO_ASID && self.gen_of[slot] == self.generation).then_some(a)
    }

    /// The ASID to install `slot`'s root with. Returns `(asid, rolled_over)`:
    /// `rolled_over` = this call started a new generation, and the caller must
    /// invalidate the whole TLB before the returned ASID goes live.
    /// `slot >= MAX_PROCESSES` or an unconfigured allocator → `(NO_ASID,
    /// false)`.
    pub fn assign(&mut self, slot: usize) -> (u16, bool) {
        if slot >= MAX_PROCESSES || self.limit < 2 {
            return (NO_ASID, false);
        }
        if let Some(a) = self.current(slot) {
            return (a, false);
        }
        let mut rolled = false;
        if self.next >= self.limit {
            self.generation = self.generation.wrapping_add(1).max(1);
            self.next = 1;
            self.rollovers += 1;
            rolled = true;
        }
        let a = self.next as u16;
        self.next += 1;
        self.asid[slot] = a;
        self.gen_of[slot] = self.generation;
        self.assigned += 1;
        (a, rolled)
    }

    /// The slot's tables are rebuilt for a new tenant (or torn down): its
    /// ASID is never used again in this generation. Returns the retired
    /// number if it was current (the caller may drop its entries, which is
    /// hygiene, not correctness).
    pub fn retire(&mut self, slot: usize) -> Option<u16> {
        let a = self.current(slot);
        if slot < MAX_PROCESSES {
            self.asid[slot] = NO_ASID;
            self.gen_of[slot] = 0;
        }
        a
    }

    /// Current generation (statistics, tests).
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Generations started after the first (statistics, tests).
    pub fn rollovers(&self) -> u32 {
        self.rollovers
    }

    /// ASIDs handed out since [`Self::configure`] (statistics, tests).
    pub fn assigned(&self) -> u64 {
        self.assigned
    }
}

impl Default for Allocator {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Owed invalidations ──────────────────────────────────────────────

/// One TLB invalidation a page-table change owes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Inval {
    /// `TLBI VAAE1IS` — the page at `va` for every ASID (a shared table's
    /// leaf changed; every root that has no private copy of the region walks
    /// it). Any lookup level (`RZSXRQ`, D8-7869).
    AllAsids {
        /// The page's user VA.
        va: u64,
    },
    /// `TLBI VAE1IS` — the page at `va` for one ASID (a slot's private table
    /// changed). Non-global entries of any lookup level that match the ASID
    /// (`RVMSWQ`, D8-7868).
    Asid {
        /// The owner's ASID.
        asid: u16,
        /// The page's user VA.
        va: u64,
    },
}

impl Inval {
    /// The `Xt` operand: ASID in bits [63:48] (0 for `VAAE1`), `VA[55:12]` in
    /// bits [43:0] (`IZVNJG`/`ILHKCP`, D8-7863/7864); TTL [47:44] = 0.
    pub fn operand(self) -> u64 {
        match self {
            Inval::AllAsids { va } => (va >> 12) & VA_FIELD,
            Inval::Asid { asid, va } => ((asid as u64) << 48) | ((va >> 12) & VA_FIELD),
        }
    }
}

/// `Xt[43:0]` = `VA[55:12]`.
const VA_FIELD: u64 = (1 << 44) - 1;

/// Entries a [`Queue`] holds before it gives up and asks for a full flush.
pub const QUEUE_CAP: usize = 64;

/// Invalidations owed since the last flush. Overflow is not an error: the
/// flush then drops the whole TLB (`VMALLE1IS`), which covers every entry
/// that would have been queued.
#[derive(Debug)]
pub struct Queue {
    ops: [Inval; QUEUE_CAP],
    len: usize,
    overflow: bool,
}

impl Queue {
    /// An empty queue.
    pub const fn new() -> Self {
        Self { ops: [Inval::AllAsids { va: 0 }; QUEUE_CAP], len: 0, overflow: false }
    }

    /// Owe `op` (a duplicate is not queued twice).
    pub fn push(&mut self, op: Inval) {
        if self.overflow || self.ops[..self.len].contains(&op) {
            return;
        }
        if self.len == QUEUE_CAP {
            self.overflow = true;
            return;
        }
        self.ops[self.len] = op;
        self.len += 1;
    }

    /// The owed invalidations, and whether the queue overflowed (then only a
    /// full flush is enough). Empties the queue.
    pub fn take(&mut self) -> (Ops, bool) {
        let out = Ops { ops: self.ops, len: self.len };
        let overflow = self.overflow;
        self.len = 0;
        self.overflow = false;
        (out, overflow)
    }

    /// Nothing owed.
    pub fn is_empty(&self) -> bool {
        self.len == 0 && !self.overflow
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

/// A drained [`Queue`].
#[derive(Copy, Clone, Debug)]
pub struct Ops {
    ops: [Inval; QUEUE_CAP],
    len: usize,
}

impl Ops {
    /// The invalidations, in the order they were owed.
    pub fn as_slice(&self) -> &[Inval] {
        &self.ops[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_hands_out_nothing() {
        let mut a = Allocator::new();
        assert_eq!(a.assign(0), (NO_ASID, false));
        assert_eq!(a.current(0), None);
    }

    #[test]
    fn operand_layout() {
        assert_eq!(Inval::Asid { asid: 0x1234, va: 0x0040_5000 }.operand(), 0x1234_0000_0000_0405);
        assert_eq!(Inval::AllAsids { va: 0x3AE0_0000 }.operand(), 0x3AE00);
    }
}
