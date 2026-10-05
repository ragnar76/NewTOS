//! BDOS memory descriptor (`MD`) and memory partition block (`MPB`).
//!
//! Spec: [`docs/api/structures/md.md`](../../../docs/api/structures/md.md).
//! Source of truth: EmuTOS `include/bdosdefs.h:14-29` and the
//! allocator in `bdos/mem.c`.
//!
//! ## ABI divergence on AArch64
//!
//! The Atari ABI defines `MD` as 16 bytes (4 × `LONG`/pointer at
//! 4 B each on m68k) and `MPB` as 12 bytes (3 × pointer). On
//! AArch64 the natural pointer width is 8 B, so we have to make
//! one explicit choice for each "pointer or `LONG`" field. NewTOS
//! keeps the **size-and-offset** fields (`m_start`, `m_length`)
//! as `LONG` (`i32`) — matching what `Malloc` returns and what
//! TOS programs see — and the **link / owner** fields
//! (`m_link`, `m_own`) as proper 64-bit pointers. The resulting
//! layout is **24 B for `MD` and 24 B for `MPB`** (verified by
//! the `size_of` `const` asserts below).
//!
//! Trade-off of `m_start: Long`:
//!   * `Malloc` / `Mxalloc` can return `(*md).m_start` directly,
//!     no truncation step.
//!   * Physical addresses must fit in `i32` (≤ 2 GiB signed). On
//!     QEMU `aarch64-virt` (DRAM 0x4000_0000..0x5100_0000) and
//!     RPi4 with ≤ 2 GiB the entire memory map fits. RPi4 4 GiB
//!     and 8 GiB SKUs would wrap into negative `i32` for high
//!     TT-RAM addresses; the planned "low-half address window"
//!     (Phase 05/06 hand-off, see
//!     `docs/api/gemdos/memory/48-malloc.md` § "Allocations
//!     beyond ABI width") will keep user-visible pointers in the
//!     32-bit-safe range. Until that lands, RPi4 `>= 2 GiB` is
//!     gated.
//!
//! Raw external access by source-port programs goes via the
//! address window the same way — user code never sees a raw
//! `MD` and never has to care about its size.

use crate::Long;

/// TOS memory descriptor. Describes one allocated or free region.
///
/// Field **order** matches the Atari ABI; field **sizes and
/// offsets** are AArch64-natural (8 B pointers, 4 B `LONG`s):
///
/// | Field      | offset | size | equivalent m68k offset |
/// |------------|-------:|-----:|-----------------------:|
/// | `m_link`   |   0    |  8   | 0                      |
/// | `m_start`  |   8    |  4   | 4                      |
/// | `m_length` |  12    |  4   | 8                      |
/// | `m_own`    |  16    |  8   | 12                     |
/// | total      | 24     |      | 16                     |
///
/// `m_link` walks the BDOS free / allocated lists; `m_start` is the
/// address `Malloc`/`Mxalloc` returns to user code; `m_length` is
/// the region size in bytes; `m_own` points to the owning
/// `BASEPAGE` (set on alloc, consulted by `Pterm` cleanup in
/// Phase 06 — until then a sentinel address, see
/// `tos_api::bdos::mem::traps::current_owner`).
#[repr(C)]
#[derive(Debug)]
pub struct MD {
    /// Link to the next MD in this list (free list or alloc list).
    /// `null` terminates the list.
    pub m_link: *mut MD,
    /// Start address of the described region.
    pub m_start: Long,
    /// Region length in bytes.
    pub m_length: Long,
    /// Owning BASEPAGE pointer, or `null` for free MDs.
    /// Until Phase 06 introduces real PDs, the kernel uses a
    /// sentinel value here; see `docs/api/structures/md.md`.
    pub m_own: *mut core::ffi::c_void,
}

const _: () = assert!(
    core::mem::size_of::<MD>() == 2 * core::mem::size_of::<usize>() + 8,
    "MD layout drift: m_link/m_own pointer size + 8 bytes for m_start/m_length",
);
const _: () = assert!(core::mem::align_of::<MD>() <= 16);

// `MD` is only ever accessed under the BDOS lock from a single
// trap-handling thread; the raw pointers in `m_link` / `m_own` make
// the autoderived `Send`/`Sync` impls absent. We assert them
// explicitly because the BDOS layer is the sole owner.
//
// SAFETY: All access to MDs is serialised by the BDOS giant lock
// (Phase 04 D6).
unsafe impl Send for MD {}
// SAFETY: see above.
unsafe impl Sync for MD {}

/// TOS memory partition block. Each pool (ST-RAM, TT-RAM) has one.
///
/// `mp_mfl` is the free-list head, kept address-ordered to make
/// coalescing in `freeit` an O(1) neighbour check.
/// `mp_mal` is the allocated-list head; `Mfree` walks it to locate
/// the MD it must release. `mp_rover` is unused in EmuTOS but
/// preserved for ABI compatibility.
#[repr(C)]
#[derive(Debug)]
pub struct MPB {
    /// Memory free list (address-ordered).
    pub mp_mfl: *mut MD,
    /// Memory allocated list.
    pub mp_mal: *mut MD,
    /// Roving pointer (Atari legacy field; unused by EmuTOS).
    pub mp_rover: *mut MD,
}

const _: () = assert!(
    core::mem::size_of::<MPB>() == 3 * core::mem::size_of::<usize>(),
    "MPB layout drift: three pointers",
);

// SAFETY: see `MD` impls above; the same single-locked-thread
// invariant applies.
unsafe impl Send for MPB {}
// SAFETY: see above.
unsafe impl Sync for MPB {}

impl MPB {
    /// Construct an empty MPB. Callers seed it with one or more MDs
    /// during pool init (`tos-api::bdos::mem::init`).
    pub const fn empty() -> Self {
        Self {
            mp_mfl: core::ptr::null_mut(),
            mp_mal: core::ptr::null_mut(),
            mp_rover: core::ptr::null_mut(),
        }
    }
}

/// `Mxalloc` mode — see `docs/api/gemdos/memory/44-mxalloc.md`.
pub const MX_STRAM: i16 = 0;
/// `Mxalloc` mode — TT-RAM only.
pub const MX_TTRAM: i16 = 1;
/// `Mxalloc` mode — ST-RAM preferred, fall back to TT-RAM.
pub const MX_PREFSTRAM: i16 = 2;
/// `Mxalloc` mode — TT-RAM preferred, fall back to ST-RAM.
pub const MX_PREFTTRAM: i16 = 3;
/// Mask of supported `Mxalloc` mode bits. EmuTOS clears the rest.
pub const MX_MODEMASK: i16 = 0x03;

// ─── Memory-protection modes (FreeMiNT model) ───────────────────────
//
// NewTOS adopts FreeMiNT's protection model rather than inventing one, so
// FreeMiNT's full per-process protection can later be built on top of these
// same names and values instead of replacing them (user decision 2026-09-05,
// ADR-023). Values are verbatim from `references/freemint/sys/mint/mem.h`.
//
// FreeMiNT carries the mode in two encodings and NewTOS keeps both:
//
//   * `F_PROT_*` — the *external* encoding: the protection nibble of a
//     program's `PRGFLAGS` (`sys/mint/mem.h:120-126`). EmuTOS ignores these
//     bits — they are defined here so the protection work has the ABI fixed.
//   * `PROT_*` / [`ProtMode`] — the *internal* encoding: the small integer
//     FreeMiNT passes to `alloc_region()` / `mark_region()`
//     (`sys/mint/mem.h:130-136`). This is what NewTOS's kernel-side publisher
//     (`tos-api::bdos::process::usermem::expose`) takes today.
//
// **The `Mxalloc` mode word uses the same nibble shifted by one** — FreeMiNT
// `mem.h:118`: "when users call Mxalloc, they add 0x10 to what you see here".
// In a mode word the nibble `0x00` means "the caller's PRGFLAGS mode",
// `0x10` = `PROT_P`, `0x20` = `PROT_G`, `0x30` = `PROT_S`, `0x40` =
// `PROT_PR`, `0x50` = `PROT_I` (`sys/dosmem.c:209-221`: `protmode = (mode &
// F_PROTMODE) >> F_PROTSHIFT; if (protmode == 0) protmode = (memflags &
// F_PROTMODE) >> F_PROTSHIFT; else --protmode;`). So `F_PROT_G` (`0x10`) in a
// PRGFLAGS word is `PROT_G`, but `0x10` in an `Mxalloc` mode is `PROT_P`.
// [`prot_from_prgflags`] and [`prot_from_mxalloc`] are the two decoders
// (ARCH03 plan Frage 8). Since ARCH03 stage 3b `Mxalloc` decodes the nibble
// with [`prot_from_mxalloc`] and the per-process rights enforce it
// (`tos-api::bdos::process::memprot`); only the pool selection is still
// `mode & MX_MODEMASK`.

/// Mask of the protection-mode bits in an `Mxalloc` mode word / `PRGFLAGS`
/// (`F_PROTMODE`, `references/freemint/sys/mint/mem.h:120`).
pub const F_PROTMODE: i16 = 0xf0;
/// Private — no read or write for anyone but the owner
/// (`F_PROT_P`, `sys/mint/mem.h:121`). FreeMiNT's default.
pub const F_PROT_P: i16 = 0x00;
/// Global — any access is OK (`F_PROT_G`, `sys/mint/mem.h:122`).
pub const F_PROT_G: i16 = 0x10;
/// Supervisor — any supervisor access is OK (`F_PROT_S`, `sys/mint/mem.h:123`).
pub const F_PROT_S: i16 = 0x20;
/// Read-only — any read is OK, no write (`F_PROT_PR`, `sys/mint/mem.h:124`).
pub const F_PROT_PR: i16 = 0x30;
/// Invalid page (`F_PROT_I`, `sys/mint/mem.h:125`).
pub const F_PROT_I: i16 = 0x40;
/// Bit position of the protection nibble (`F_PROTSHIFT`, `sys/mint/mem.h:127`).
pub const F_PROTSHIFT: u32 = 4;
/// `Mxalloc` mode bit "don't auto-free this memory" (`F_KEEP`,
/// `sys/mint/mem.h:104-105`): when the block's last owner terminates, the
/// block is not freed but passes to the system, and any process may then
/// `Mfree` it (FreeMiNT `sys/dosmem.c:131-135,161-165,365-398`,
/// `sys/proc_help.c:242-258`). Not part of the protection mode.
pub const F_KEEP: i16 = 0x4000;

/// Internal protection value — private (`PROT_P`, `sys/mint/mem.h:130`).
pub const PROT_P: u8 = 0;
/// Internal protection value — global (`PROT_G`, `sys/mint/mem.h:131`).
pub const PROT_G: u8 = 1;
/// Internal protection value — supervisor (`PROT_S`, `sys/mint/mem.h:132`).
pub const PROT_S: u8 = 2;
/// Internal protection value — read-only (`PROT_PR`, `sys/mint/mem.h:133`).
pub const PROT_PR: u8 = 3;
/// Internal protection value — invalid page (`PROT_I`, `sys/mint/mem.h:134`).
pub const PROT_I: u8 = 4;
/// Highest defined internal protection value
/// (`PROT_MAX_MODE`, `sys/mint/mem.h:135`).
pub const PROT_MAX_MODE: u8 = 4;
/// Mask of the internal protection value
/// (`PROT_PROTMODE`, `sys/mint/mem.h:136`).
pub const PROT_PROTMODE: u8 = 0xf;

/// Typed form of FreeMiNT's internal `PROT_*` values — the access a process
/// is granted to a region. Discriminants are the `PROT_*` constants above, so
/// `mode as u8` is exactly what FreeMiNT passes to `alloc_region()`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProtMode {
    /// `PROT_P` — no read or write (private to the owner).
    P = 0,
    /// `PROT_G` — any access OK.
    G = 1,
    /// `PROT_S` — any supervisor access OK; nothing for the user.
    S = 2,
    /// `PROT_PR` — any read OK, no write.
    Pr = 3,
    /// `PROT_I` — invalid page.
    I = 4,
}

impl ProtMode {
    /// The internal mode for FreeMiNT's `PROT_*` value `v`, with FreeMiNT's
    /// fallback for anything above [`PROT_MAX_MODE`]: `PROT_P`
    /// (`sys/dosmem.c:217-221`, "invalid protection mode changed to
    /// private").
    pub const fn from_prot(v: u32) -> ProtMode {
        match v {
            1 => ProtMode::G,
            2 => ProtMode::S,
            3 => ProtMode::Pr,
            4 => ProtMode::I,
            _ => ProtMode::P,
        }
    }
}

/// The protection mode a program's `PRGFLAGS` word asks for:
/// `(memflags & F_PROTMODE) >> F_PROTSHIFT` (FreeMiNT `sys/dosmem.c:213`,
/// the `protmode == 0` branch), unshifted — `F_PROT_G` (`0x10`) is
/// `PROT_G` — and anything above `PROT_MAX_MODE` private (`:217-221`).
pub const fn prot_from_prgflags(prgflags: u32) -> ProtMode {
    ProtMode::from_prot((prgflags & F_PROTMODE as u32) >> F_PROTSHIFT)
}

/// The protection mode an `Mxalloc(size, mode)` asks for, FreeMiNT
/// `m_xalloc` (`sys/dosmem.c:209-221`): the nibble `(mode & F_PROTMODE) >>
/// F_PROTSHIFT`; `0` takes the caller's PRGFLAGS mode
/// ([`prot_from_prgflags`]), anything else is **one more** than the
/// `PROT_*` value it means (`--protmode`, `mem.h:118`), and a result above
/// `PROT_MAX_MODE` is private. Bit `0x08` and the `F_KEEP`/`F_ALLOCZERO`
/// bits are not part of the mode.
pub const fn prot_from_mxalloc(mode: i16, prgflags: u32) -> ProtMode {
    let nibble = ((mode as u16 as u32) & F_PROTMODE as u32) >> F_PROTSHIFT;
    if nibble == 0 {
        prot_from_prgflags(prgflags)
    } else {
        ProtMode::from_prot(nibble - 1)
    }
}

#[cfg(test)]
mod prot_tests {
    use super::*;

    #[test]
    fn prgflags_nibble_is_the_prot_value_itself() {
        assert_eq!(prot_from_prgflags(F_PROT_P as u32), ProtMode::P);
        assert_eq!(prot_from_prgflags(F_PROT_G as u32), ProtMode::G);
        assert_eq!(prot_from_prgflags(F_PROT_S as u32), ProtMode::S);
        assert_eq!(prot_from_prgflags(F_PROT_PR as u32), ProtMode::Pr);
        assert_eq!(prot_from_prgflags(F_PROT_I as u32), ProtMode::I);
        // Other PRGFLAGS bits do not matter (PF_STANDARD = 7, TT-RAM bits,
        // the F_MINALT nibble at 28..31).
        assert_eq!(prot_from_prgflags(0x1000_0017), ProtMode::G);
        // Above PROT_MAX_MODE → private (dosmem.c:217-221).
        assert_eq!(prot_from_prgflags(0x50), ProtMode::P);
        assert_eq!(prot_from_prgflags(0xF0), ProtMode::P);
    }

    #[test]
    fn mxalloc_nibble_is_one_more_than_the_prot_value() {
        // mem.h:118 "when users call Mxalloc, they add 0x10".
        assert_eq!(prot_from_mxalloc(0x10, 0), ProtMode::P);
        assert_eq!(prot_from_mxalloc(0x20, 0), ProtMode::G);
        assert_eq!(prot_from_mxalloc(0x30, 0), ProtMode::S);
        assert_eq!(prot_from_mxalloc(0x40, 0), ProtMode::Pr);
        assert_eq!(prot_from_mxalloc(0x50, 0), ProtMode::I);
        // The same bits mean something else in PRGFLAGS: 0x10 there is G.
        assert_ne!(prot_from_mxalloc(0x10, 0), prot_from_prgflags(0x10));
    }

    #[test]
    fn mxalloc_nibble_zero_takes_the_callers_prgflags() {
        assert_eq!(prot_from_mxalloc(0x00, F_PROT_G as u32), ProtMode::G);
        assert_eq!(prot_from_mxalloc(MX_TTRAM, F_PROT_PR as u32), ProtMode::Pr);
        assert_eq!(prot_from_mxalloc(MX_PREFTTRAM, 0x0000_0007), ProtMode::P);
    }

    #[test]
    fn mxalloc_other_bits_and_out_of_range() {
        // LDG's `Mxalloc(size, 3 + 8 + 32)` (ldgutil.c): nibble 0x20 → PROT_G;
        // the TT-RAM-preferred low bits and 0x08 are not the mode.
        assert_eq!(prot_from_mxalloc(3 + 8 + 32, 0), ProtMode::G);
        // F_KEEP (0x4000) does not change the mode.
        assert_eq!(prot_from_mxalloc(0x4000 | 0x20, 0), ProtMode::G);
        // 0x60..0xF0 → PROT_* 5..14 → private.
        assert_eq!(prot_from_mxalloc(0x60, F_PROT_G as u32), ProtMode::P);
        assert_eq!(prot_from_mxalloc(0xF0, F_PROT_G as u32), ProtMode::P);
    }
}
