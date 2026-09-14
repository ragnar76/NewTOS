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
///
/// On success this also pulls every loaded process's `$5A0` mirror back in
/// step (see [`publish`]).
pub fn set(tag: u32, value: u32) -> bool {
    if !valid_tag(tag) {
        return false;
    }
    // The exclusive borrow of the jar ends with this block: `publish` reads
    // the jar again (through `count`/`slot_tag`/`get`), so it must not run
    // while a `&mut Jar` is still live.
    let changed = {
        // SAFETY: single-CPU access.
        let j = unsafe { jar() };
        match (0..j.count).find(|&i| j.slots[i].tag == tag) {
            Some(i) => {
                j.slots[i].value = value;
                true
            }
            None if j.count < JAR_CAP => {
                j.slots[j.count] = Slot { tag, value };
                j.count += 1;
                true
            }
            None => false, // jar full
        }
    };
    if changed {
        publish();
    }
    changed
}

/// Push the jar's current content into the `$5A0` mirror of every process
/// that owns a TPA.
///
/// The jar is kernel-owned (ADR-012), but a 1:1-ported TOS program does not
/// reach it that way: it dereferences `_p_cookies`@`$5A0` and scans the array
/// (TeraDesk `library/utility/cookie.c:28,56`). NewTOS materialises that array
/// per process, inside the process's own low-memory area
/// (`bdos::process::loader::write_cookie_jar`). Without this call the mirror
/// would be a snapshot of load time and a cookie set afterwards would never
/// reach a running program — which is exactly the gap
/// `docs/api/bios/_cookie-jar.md` §OFFEN records.
///
/// Called from **inside** [`set`] / [`del`] rather than from their callers, so
/// the invariant "whoever changes the kernel jar pulls the mirrors along" is
/// structural: a future third mutator gets it for free. At boot
/// ([`seed_defaults`]) no process owns a TPA yet, so it is a no-op there.
#[inline]
fn publish() {
    crate::bdos::process::loader::refresh_cookie_mirrors();
}

/// Look up a cookie value by tag.
pub fn get(tag: u32) -> Option<u32> {
    // SAFETY: single-CPU access; read-only use of the exclusive ref.
    let j = unsafe { jar() };
    (0..j.count).find(|&i| j.slots[i].tag == tag).map(|i| j.slots[i].value)
}

/// Number of cookies **present** (occupied slots). Not the value returned
/// for the `tag == 0` capacity query — see [`capacity`].
pub fn count() -> u32 {
    // SAFETY: single-CPU access.
    unsafe { jar() }.count as u32
}

/// Total slot **capacity** of the jar — what FreeMiNT stores in the
/// terminator slot (`sys/cookie.c:322-326`) and returns for
/// `S_GETCOOKIE(tag == 0)`. This is the fixed jar size, preserved verbatim
/// on every `set_cookie` append; it is **not** the occupied count
/// ([`count`]).
pub fn capacity() -> u32 {
    JAR_CAP as u32
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
///
/// On success this also pulls every loaded process's `$5A0` mirror back in
/// step (see [`publish`]), so the removed tag stops being visible there too.
pub fn del(tag: u32) -> bool {
    // Borrow scoped exactly as in `set`, and for the same reason.
    let removed = {
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
    };
    if removed {
        publish();
    }
    removed
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
