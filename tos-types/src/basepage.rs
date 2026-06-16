//! TOS process descriptor / **BasePage** (`PD`).
//!
//! The 256-byte block at the start of every process's TPA. Programs
//! index into it by absolute byte offsets, so the layout is **part
//! of the binary contract** — not an implementation detail. Any
//! source-port that compiles `<tos.h>` against this header sees the
//! same field offsets as a Compendium-aware program would on Atari
//! TOS / EmuTOS, and the compile-time asserts at the bottom of this
//! file fail loudly if the layout ever drifts.
//!
//! Spec: [`docs/api/structures/basepage.md`](../../docs/api/structures/basepage.md)
//! (Phase 00 D6 extraction from EmuTOS @ `1a7484ab`,
//! `include/bdosdefs.h:65-95`).
//!
//! ## ABI: pointer-shaped fields are `Long` (i32)
//!
//! On m68k each pointer is 4 bytes and matches `LONG`; the Atari
//! basepage layout treats every "pointer or LONG" field as 4 bytes
//! at a 4-byte offset. Source-port programs read these fields
//! directly with `(int32_t *)basepage + N` arithmetic, so we keep
//! them as `Long`. Real 64-bit kernel pointers (e.g. into the
//! page-table data structures) live elsewhere — never in the
//! basepage.
//!
//! Phase 06's `Pexec` populates the basepage at process load time
//! with the four segment bases (`p_lowtpa`, `p_tbase`, `p_dbase`,
//! `p_bbase`) and the corresponding lengths; everything else is
//! either inherited from the parent (env, current dir) or written
//! by GEMDOS calls (`Fsetdta` updates `p_xdta`, `Dsetdrv` updates
//! `p_curdrv`, …).

use crate::Long;

// ─── Process flags (`p_flags`) ──────────────────────────────────────

/// `PF_FASTLOAD` — skip the BSS clear at process load. Set in the
/// program header by linkers that emit a pre-zeroed BSS image.
pub const PF_FASTLOAD: u32 = 0x0001;

/// `PF_TTRAMLOAD` — program may be loaded into TT-RAM (alt-RAM on
/// Atari, "any RAM page" on AArch64). Old DMA-bound code clears
/// this so it stays in ST-RAM.
pub const PF_TTRAMLOAD: u32 = 0x0002;

/// `PF_TTRAMMEM` — `Malloc()` from this process may use TT-RAM.
/// When clear, `Malloc` is forced to `MX_STRAM` mode so DMA-bound
/// allocations stay in DMA-safe RAM.
pub const PF_TTRAMMEM: u32 = 0x0004;

/// `PF_STANDARD` — the typical "modern program" flag combination.
pub const PF_STANDARD: u32 = PF_FASTLOAD | PF_TTRAMLOAD | PF_TTRAMMEM;

// ─── Layout constants ──────────────────────────────────────────────

/// Number of standard handles in `p_uft`. Fixed by ABI.
pub const NUMSTD: usize = 6;

/// Number of per-drive current-directory slots in `p_curdir`. Equal
/// to `BLKDEVNUM` in EmuTOS (the maximum drive count). The
/// Compendium / NewTOS-supported value is 16.
pub const NUMCURDIR: usize = 16;

/// Pad bytes between `p_curdir` and `p_3fill` so the 32-byte slot
/// at offset 0x40 is fully consumed. Always `32 - NUMCURDIR`.
pub const P_2FILL_LEN: usize = 32 - NUMCURDIR;

/// Command-line buffer size (`p_cmdlin`). Fixed by ABI.
pub const PDCLSIZE: usize = 0x80;

// ─── BasePage layout ───────────────────────────────────────────────

/// TOS process descriptor (a.k.a. BasePage). 256 bytes, exactly the
/// Compendium "Basepage" / `<tos.h>` `BASEPAGE` layout.
///
/// Fields fall into four groups:
///   * **Memory layout** (`p_lowtpa`..`p_blen`) — set by the loader;
///     describe the TPA (Transient Program Area) extents and the
///     three program segments inside it.
///   * **Process state** (`p_xdta`, `p_parent`, `p_flags`, `p_env`,
///     `p_uft`, `p_lddrv`, `p_curdrv`) — runtime state consulted by
///     GEMDOS / inherited from the parent.
///   * **Reserved** (`p_1fill`, `p_2fill`, `p_3fill`) — kept zero by
///     EmuTOS; preserved for binary compatibility with programs that
///     poke into the gaps.
///   * **Saved registers + cmdline** (`p_dreg`, `p_areg`,
///     `p_cmdlin`) — `Pexec` save area for the parent's `D0` /
///     `A3..A7`, and the command-line tail handed to the child.
///
/// On AArch64 the saved-register area is a vestigial m68k construct;
/// Phase 06 will use those slots to stash the parent's banked
/// AArch64 state when the child runs (or simply leave them zero
/// for native source-ports that don't expect them populated).
#[repr(C)]
#[derive(Debug)]
pub struct BasePage {
    /// 0x00 — TPA start address. Always equal to `&self`.
    pub p_lowtpa: Long,
    /// 0x04 — TPA end + 1 (one past the highest valid byte).
    pub p_hitpa: Long,
    /// 0x08 — text segment base (`p_lowtpa + sizeof(BasePage)`).
    pub p_tbase: Long,
    /// 0x0C — text segment length in bytes.
    pub p_tlen: Long,
    /// 0x10 — data segment base.
    pub p_dbase: Long,
    /// 0x14 — data segment length in bytes.
    pub p_dlen: Long,
    /// 0x18 — BSS segment base.
    pub p_bbase: Long,
    /// 0x1C — BSS segment length in bytes.
    pub p_blen: Long,

    /// 0x20 — current DTA pointer (set by `Fsetdta`; defaults to
    /// `&p_cmdlin[0]` per Atari convention).
    pub p_xdta: Long,
    /// 0x24 — parent's BasePage pointer; `0` for the boot process.
    pub p_parent: Long,
    /// 0x28 — `PF_*` flags (see [`PF_FASTLOAD`] etc.).
    pub p_flags: u32,
    /// 0x2C — environment-string-chain pointer (NUL-terminated
    /// `KEY=value` records, double-NUL at end).
    pub p_env: Long,

    /// 0x30 — standard-handle redirection table (six handles
    /// stdin/stdout/stderr/stdaux/stdprn/reserved).
    pub p_uft: [i8; NUMSTD],
    /// 0x36 — drive number the program was loaded from.
    pub p_lddrv: i8,
    /// 0x37 — current default drive (read by `Dgetdrv`, written by
    /// `Dsetdrv`).
    pub p_curdrv: u8,

    /// 0x38..0x40 — reserved. Two `LONG` slots; EmuTOS keeps zero.
    pub p_1fill: [Long; 2],

    /// 0x40 — per-drive current-directory index into `dirtbl[]`.
    /// Sixteen slots, one per drive A:..P:.
    pub p_curdir: [u8; NUMCURDIR],
    /// 0x50 — pad bytes that fill the 32-byte slot at 0x40 to its
    /// nominal end at 0x60. Always zero.
    pub p_2fill: [u8; P_2FILL_LEN],

    /// 0x60..0x68 — reserved. Two `LONG` slots; EmuTOS keeps zero.
    pub p_3fill: [Long; 2],

    /// 0x68 — `Pexec` save slot for the parent's `D0` register
    /// (m68k). On AArch64 this is one of the first available scratch
    /// slots for Phase 06's parent-state save / restore.
    pub p_dreg: [Long; 1],
    /// 0x6C..0x80 — `Pexec` save slot for the parent's `A3..A7`
    /// (m68k). Five `LONG` cells.
    pub p_areg: [Long; 5],

    /// 0x80..0x100 — command-line tail handed to the child by the
    /// shell. Format:
    ///
    /// ```text
    ///   p_cmdlin[0]  = length byte (0..125 valid)
    ///   p_cmdlin[1..] = ASCII command tail, terminated by `\r` (0x0D)
    /// ```
    ///
    /// **The trailing `\r` is *not* counted in the length byte.**
    /// `Pexec` truncates anything beyond 125 bytes. Doubles as the
    /// initial DTA buffer (see `p_xdta`) until the program calls
    /// `Fsetdta`.
    pub p_cmdlin: [u8; PDCLSIZE],
}

// ─── Compile-time layout asserts ────────────────────────────────────
//
// One assert per documented field offset, plus the total size and
// alignment. If any of these fire, the basepage has drifted from
// the Compendium / `<tos.h>` layout — which would silently break
// every source-port that pokes into the basepage by absolute offset.

const _: () = {
    // Total size and alignment.
    assert!(core::mem::size_of::<BasePage>() == 256);
    assert!(core::mem::align_of::<BasePage>() == 4);

    // Memory layout group (0x00..0x20).
    assert!(core::mem::offset_of!(BasePage, p_lowtpa) == 0x00);
    assert!(core::mem::offset_of!(BasePage, p_hitpa)  == 0x04);
    assert!(core::mem::offset_of!(BasePage, p_tbase)  == 0x08);
    assert!(core::mem::offset_of!(BasePage, p_tlen)   == 0x0C);
    assert!(core::mem::offset_of!(BasePage, p_dbase)  == 0x10);
    assert!(core::mem::offset_of!(BasePage, p_dlen)   == 0x14);
    assert!(core::mem::offset_of!(BasePage, p_bbase)  == 0x18);
    assert!(core::mem::offset_of!(BasePage, p_blen)   == 0x1C);

    // Process state group (0x20..0x38).
    assert!(core::mem::offset_of!(BasePage, p_xdta)   == 0x20);
    assert!(core::mem::offset_of!(BasePage, p_parent) == 0x24);
    assert!(core::mem::offset_of!(BasePage, p_flags)  == 0x28);
    assert!(core::mem::offset_of!(BasePage, p_env)    == 0x2C);
    assert!(core::mem::offset_of!(BasePage, p_uft)    == 0x30);
    assert!(core::mem::offset_of!(BasePage, p_lddrv)  == 0x36);
    assert!(core::mem::offset_of!(BasePage, p_curdrv) == 0x37);

    // Reserved + cwd group (0x38..0x68).
    assert!(core::mem::offset_of!(BasePage, p_1fill)  == 0x38);
    assert!(core::mem::offset_of!(BasePage, p_curdir) == 0x40);
    assert!(core::mem::offset_of!(BasePage, p_2fill)  == 0x50);
    assert!(core::mem::offset_of!(BasePage, p_3fill)  == 0x60);

    // Saved register + cmdline group (0x68..0x100).
    assert!(core::mem::offset_of!(BasePage, p_dreg)   == 0x68);
    assert!(core::mem::offset_of!(BasePage, p_areg)   == 0x6C);
    assert!(core::mem::offset_of!(BasePage, p_cmdlin) == 0x80);
};

// SAFETY: `BasePage` contains only `Long` (i32), `u8`, `i8`, `u32`
// values — no raw pointers, no interior mutability. It is `Send`
// and `Sync` by the same logic as `MD`/`MPB`: BDOS-side accesses
// are serialised by the giant lock; the kernel never holds two
// concurrent `&mut BasePage`.
//
// We declare the impls explicitly rather than auto-derive because
// the field-by-field auto-derivation would consider the trait the
// same way it does for ordinary types — but BasePage will eventually
// gain raw pointers (for example a kernel-side back-pointer to the
// process's per-process page-table root), and the explicit impl
// keeps that addition cheap.
unsafe impl Send for BasePage {}
unsafe impl Sync for BasePage {}

impl BasePage {
    /// Construct an all-zero basepage. The layout-critical fields
    /// (`p_lowtpa`, `p_hitpa`, segment bases/lengths, `p_cmdlin`
    /// length byte) must be filled in by the caller (typically the
    /// `Pexec` loader) before the child process starts.
    pub const fn zeroed() -> Self {
        Self {
            p_lowtpa: 0,
            p_hitpa:  0,
            p_tbase:  0,
            p_tlen:   0,
            p_dbase:  0,
            p_dlen:   0,
            p_bbase:  0,
            p_blen:   0,
            p_xdta:   0,
            p_parent: 0,
            p_flags:  0,
            p_env:    0,
            p_uft:    [0; NUMSTD],
            p_lddrv:  0,
            p_curdrv: 0,
            p_1fill:  [0; 2],
            p_curdir: [0; NUMCURDIR],
            p_2fill:  [0; P_2FILL_LEN],
            p_3fill:  [0; 2],
            p_dreg:   [0; 1],
            p_areg:   [0; 5],
            p_cmdlin: [0; PDCLSIZE],
        }
    }
}
