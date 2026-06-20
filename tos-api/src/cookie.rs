//! NewTOS cookie jar — a kernel-owned system-feature registry.
//!
//! Spec / data model: [`docs/api/bios/_cookie-jar.md`]; access ABI:
//! [`docs/api/gemdos/154-ssystem.md`] (FreeMiNT `Ssystem`). Decision:
//! ADR-012 (`docs/MEMORY.md`).
//!
//! The jar is kernel-owned: a fixed-capacity static array (the no-`alloc`
//! analogue of FreeMiNT's fixed `dflt_jar[]`; a growable heap jar was the
//! ADR-012 sketch but `tos-api` is `#![no_std]` without `alloc`, and a
//! fixed array is closer to FreeMiNT's own model). It lives in kernel
//! memory; user programs never get a pointer to it — they read values
//! through the `Ssystem` cookie subfunctions. Single-CPU, single-threaded
//! access (boot seeding + the SVC trap path), so it uses the same
//! `UnsafeCell` discipline as [`crate::aes`]'s process table.

use core::cell::UnsafeCell;

/// Maximum number of cookies. FreeMiNT seeds ~10–15; 32 leaves headroom
/// for `Setcookie` without an allocator.
const JAR_CAP: usize = 32;

// NewTOS cookie tags (4-char big-endian ASCII, as on TOS).
/// `NTOS` — NewTOS identity; value = version (lets ported programs detect
/// the environment). This is the NewTOS-specific cookie.
pub const NTOS: u32 = u32::from_be_bytes(*b"NTOS");
/// `_MCH` — machine type.
pub const MCH: u32 = u32::from_be_bytes(*b"_MCH");
/// `_IDT` — date/time display format.
pub const IDT: u32 = u32::from_be_bytes(*b"_IDT");
/// `_AKP` — keyboard / language (low word = language id).
pub const AKP: u32 = u32::from_be_bytes(*b"_AKP");

#[derive(Clone, Copy)]
struct Slot {
    tag: u32,
    value: u32,
}

struct Jar {
    slots: [Slot; JAR_CAP],
    count: usize,
}

struct JarCell(UnsafeCell<Jar>);
// SAFETY: the jar is touched only from the single boot CPU — at boot
// (`seed_defaults`) and from the synchronous SVC handler (`Ssystem`),
// never concurrently.
unsafe impl Sync for JarCell {}

static JAR: JarCell = JarCell(UnsafeCell::new(Jar {
    slots: [Slot { tag: 0, value: 0 }; JAR_CAP],
    count: 0,
}));

#[inline]
#[allow(clippy::mut_from_ref)]
unsafe fn jar() -> &'static mut Jar {
    // SAFETY: single-CPU, non-reentrant access (see `JarCell`).
    unsafe { &mut *JAR.0.get() }
}

/// A cookie tag is valid iff **every byte is non-zero** (FreeMiNT
/// `set_cookie`): this keeps the slot-number space (high word 0) free for
/// `Getcookie` enumeration.
pub fn valid_tag(tag: u32) -> bool {
    tag & 0xFF00_0000 != 0
        && tag & 0x00FF_0000 != 0
        && tag & 0x0000_FF00 != 0
        && tag & 0x0000_00FF != 0
}

/// Add a cookie, or update its value if the tag already exists. Returns
/// `false` if the tag is invalid or the jar is full (new tag, no room).
pub fn set(tag: u32, value: u32) -> bool {
    if !valid_tag(tag) {
        return false;
    }
    // SAFETY: single-CPU access.
    let j = unsafe { jar() };
    for i in 0..j.count {
        if j.slots[i].tag == tag {
            j.slots[i].value = value;
            return true;
        }
    }
    if j.count >= JAR_CAP {
        return false;
    }
    j.slots[j.count] = Slot { tag, value };
    j.count += 1;
    true
}

/// Look up a cookie value by tag.
pub fn get(tag: u32) -> Option<u32> {
    // SAFETY: single-CPU access; read-only use of the exclusive ref.
    let j = unsafe { jar() };
    (0..j.count).find(|&i| j.slots[i].tag == tag).map(|i| j.slots[i].value)
}

/// Number of cookies present — the value returned for the `tag == 0`
/// (NULL-slot) query.
pub fn count() -> u32 {
    // SAFETY: single-CPU access.
    unsafe { jar() }.count as u32
}

/// Tag of the 1-based `n`-th cookie (slot enumeration). `None` if out of
/// range.
pub fn slot_tag(n: u32) -> Option<u32> {
    if n == 0 {
        return None;
    }
    // SAFETY: single-CPU access.
    let j = unsafe { jar() };
    let idx = (n - 1) as usize;
    (idx < j.count).then(|| j.slots[idx].tag)
}

/// Remove a cookie by tag, compacting the jar. Returns `true` if it was
/// present.
pub fn del(tag: u32) -> bool {
    // SAFETY: single-CPU access.
    let j = unsafe { jar() };
    match (0..j.count).find(|&i| j.slots[i].tag == tag) {
        Some(i) => {
            for k in i..j.count - 1 {
                j.slots[k] = j.slots[k + 1];
            }
            j.count -= 1;
            true
        }
        None => false,
    }
}

/// Seed the NewTOS default cookie set at boot (replaces EmuTOS'
/// `fill_cookie_jar`). Only the tags that are meaningful for NewTOS are
/// present; the Atari-hardware cookies (`_VDO`/`_SND`/`_FDC`/`_FPU`) are
/// deliberately omitted. Values are provisional bring-up policy.
pub fn seed_defaults(version: u32) {
    set(NTOS, version); // NewTOS identity + version
    set(MCH, 0x0005_0000); // machine id: a NewTOS-specific marker (non-Atari)
    set(IDT, 0x0000_112F); // date/time: '/' sep, DD-MM-YY, 24-hour
    set(AKP, 0x0000_0000); // keyboard language: US
}
