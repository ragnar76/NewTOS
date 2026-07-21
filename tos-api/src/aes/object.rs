//! AES Object Manager (`objc_*`) — Batch 1: the OBJECT tree model plus
//! `objc_draw` (42), `objc_find` (43), `objc_offset` (44).
//!
//! Spec: [`docs/api/aes/object-form.md`]; rendering extracted from EmuTOS
//! `aes/gemoblib.c` (`ob_draw`/`just_draw`).
//!
//! **Native OBJECT layout:** classic TOS packs `OBJECT` into 24 bytes
//! with a 32-bit `ob_spec`. NewTOS is native AArch64, so `ob_spec` is a
//! **64-bit** value (a pointer for text/string types, a packed box word
//! for box types) and the struct is 32 bytes. Programs build trees with
//! this layout; `addr_in[0]` points at the array.
//!
//! **Batch-1 scope (documented limits, not stubs):** types `G_BOX`,
//! `G_IBOX`, `G_STRING`, `G_BUTTON`, `G_BOXCHAR`; black-on-white look
//! (the full `gr_crack` colour word, 3D effects, icons/images, and the
//! `TEDINFO` text types `G_TEXT`/`G_BOXTEXT` land in later batches). The
//! per-object clip rectangle is honoured at screen granularity for now.

use core::sync::atomic::{AtomicI16, Ordering};

use super::Aespb;
use crate::bdos::process::{current_abi, Abi};
use crate::host::host;
use crate::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect, WriteMode};

// ── OBJECT model ────────────────────────────────────────────────────

/// A GEM object tree node (native NewTOS layout, 32 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Object {
    /// next sibling, or `-1` (`NIL`).
    pub ob_next: i16,
    /// first child, or `-1`.
    pub ob_head: i16,
    /// last child, or `-1`.
    pub ob_tail: i16,
    /// `G_*` graphic type.
    pub ob_type: u16,
    /// `*` flag bits.
    pub ob_flags: u16,
    /// `*` state bits.
    pub ob_state: u16,
    /// type-specific payload: a native pointer (string/TEDINFO) or, for
    /// box types, `(thickness << 16) | colour-word`.
    pub ob_spec: u64,
    /// x relative to the parent.
    pub ob_x: i16,
    /// y relative to the parent.
    pub ob_y: i16,
    /// width in pixels.
    pub ob_width: i16,
    /// height in pixels.
    pub ob_height: i16,
}

// ── ObjTree: ABI-parametrized in-place OBJECT accessor (ADR-020 stage 4) ──
//
// An OBJECT tree is read/written at the **caller's** ABI width: an LP64 caller
// (the desktop, the boot selftests, any kernel-built tree) has 32-byte records
// with a u64 `ob_spec` @16 and coordinates @24..30; an ILP32 caller (a
// source-ported GEM app such as QED) has 24-byte records with a u32 `ob_spec`
// @12 and coordinates @16..22 (confirmed against `Object` above + the ILP32
// relocation in `resource.rs`: `OBJECT_SIZE_ILP32=24`, `OB_TYPE_OFF=6`,
// `OB_SPEC_OFF_ILP32=12`). The link/type WORDs (next/head/tail/type/flags/state)
// share offsets 0..10 in both ABIs.
//
// The ABI is not threaded through call sites; it is read from the current
// execution context (`current_abi`) once when an accessor is built. During an
// ILP32 app's `svc #2` that is `Ilp32`; for an in-kernel (EL1) caller — the
// desktop, whose `current()` is null — it is `Lp64`, so the offsets equal the
// native `Object` struct and every read/write is byte-identical to before.

/// Field offset table for one ABI: the stride and the offsets that differ
/// between LP64 (32-byte) and ILP32 (24-byte) OBJECT records.
#[derive(Clone, Copy)]
pub(crate) struct ObjTree {
    /// VA of the OBJECT array's first record.
    base: u64,
    /// Caller ABI (pointer / record width).
    abi: Abi,
}

impl ObjTree {
    /// Build an accessor over the tree at `base`, reading fields at the
    /// current execution context's ABI width.
    ///
    /// # Invariant
    /// `base` addresses a valid OBJECT array; every index passed to a getter/
    /// setter is in range (the GEM caller's contract, exactly as the previous
    /// `*const Object` + `tree.add(i)` code assumed).
    #[inline]
    pub(crate) fn new(base: *const Object) -> Self {
        Self { base: base as u64, abi: current_abi() }
    }

    /// Build an accessor over a **kernel-built** tree at `base`, forcing the
    /// native LP64 (32-byte) record layout regardless of the current execution
    /// context's ABI.
    ///
    /// Kernel-built trees — the `form_alert`/`form_error` alert array assembled
    /// on the kernel stack, and the fileselector's GEM.RSC tree — are always
    /// native `Object`s (32-byte, `u64 ob_spec`) even while an ILP32 app such as
    /// QED is the current process. Reading `current_abi()` there would pick the
    /// app's 24-byte width and misread the kernel array, so these callers build
    /// the accessor with an explicit LP64 width instead of [`Self::new`].
    ///
    /// # Invariant
    /// As [`Self::new`]: `base` addresses a valid OBJECT array and every index
    /// passed to a getter/setter is in range.
    #[inline]
    pub(crate) fn lp64(base: *const Object) -> Self {
        Self { base: base as u64, abi: Abi::Lp64 }
    }

    /// The tree's ABI — the width at which its `ob_spec`-pointed sub-structs
    /// (TEDINFO/ICONBLK/BITBLK) are also packed. For an app tree ([`Self::new`])
    /// this is `current_abi()`; for a kernel-built tree ([`Self::lp64`]) it is
    /// forced `Lp64` even under an ILP32 app, so its LP64 sub-structs are read at
    /// their native offsets. Used to drive the ABI-aware sub-struct readers.
    #[inline]
    pub(crate) fn abi(self) -> Abi {
        self.abi
    }

    #[inline]
    fn stride(self) -> u64 {
        match self.abi {
            Abi::Lp64 => 32,
            Abi::Ilp32 => 24,
        }
    }
    #[inline]
    fn spec_off(self) -> u64 {
        match self.abi {
            Abi::Lp64 => 16,
            Abi::Ilp32 => 12,
        }
    }
    #[inline]
    fn coord_off(self) -> u64 {
        match self.abi {
            Abi::Lp64 => 24,
            Abi::Ilp32 => 16,
        }
    }
    #[inline]
    fn ptr(self, i: i16, off: u64) -> *mut u8 {
        (self.base + i as u64 * self.stride() + off) as *mut u8
    }
    #[inline]
    fn rd_i16(self, i: i16, off: u64) -> i16 {
        // SAFETY: ObjTree invariant — `base` is a valid OBJECT array and `i`
        // is in range; a WORD field is 2 readable bytes (unaligned-safe).
        unsafe { (self.ptr(i, off) as *const i16).read_unaligned() }
    }
    #[inline]
    fn wr_i16(self, i: i16, off: u64, v: i16) {
        // SAFETY: as `rd_i16`; the record is writable (GEM contract).
        unsafe { (self.ptr(i, off) as *mut i16).write_unaligned(v) }
    }
    #[inline]
    fn rd_u16(self, i: i16, off: u64) -> u16 {
        // SAFETY: as `rd_i16`.
        unsafe { (self.ptr(i, off) as *const u16).read_unaligned() }
    }
    #[inline]
    fn wr_u16(self, i: i16, off: u64, v: u16) {
        // SAFETY: as `wr_i16`.
        unsafe { (self.ptr(i, off) as *mut u16).write_unaligned(v) }
    }

    #[inline]
    pub(crate) fn next(self, i: i16) -> i16 {
        self.rd_i16(i, 0)
    }
    #[inline]
    pub(crate) fn head(self, i: i16) -> i16 {
        self.rd_i16(i, 2)
    }
    #[inline]
    pub(crate) fn tail(self, i: i16) -> i16 {
        self.rd_i16(i, 4)
    }
    #[inline]
    pub(crate) fn ob_type(self, i: i16) -> u16 {
        self.rd_u16(i, 6)
    }
    #[inline]
    pub(crate) fn flags(self, i: i16) -> u16 {
        self.rd_u16(i, 8)
    }
    #[inline]
    pub(crate) fn state(self, i: i16) -> u16 {
        self.rd_u16(i, 10)
    }
    #[inline]
    pub(crate) fn x(self, i: i16) -> i16 {
        self.rd_i16(i, self.coord_off())
    }
    #[inline]
    pub(crate) fn y(self, i: i16) -> i16 {
        self.rd_i16(i, self.coord_off() + 2)
    }
    #[inline]
    pub(crate) fn width(self, i: i16) -> i16 {
        self.rd_i16(i, self.coord_off() + 4)
    }
    #[inline]
    pub(crate) fn height(self, i: i16) -> i16 {
        self.rd_i16(i, self.coord_off() + 6)
    }

    #[inline]
    pub(crate) fn set_next(self, i: i16, v: i16) {
        self.wr_i16(i, 0, v)
    }
    #[inline]
    pub(crate) fn set_head(self, i: i16, v: i16) {
        self.wr_i16(i, 2, v)
    }
    #[inline]
    pub(crate) fn set_tail(self, i: i16, v: i16) {
        self.wr_i16(i, 4, v)
    }
    #[inline]
    pub(crate) fn set_type(self, i: i16, v: u16) {
        self.wr_u16(i, 6, v)
    }
    #[inline]
    pub(crate) fn set_flags(self, i: i16, v: u16) {
        self.wr_u16(i, 8, v)
    }
    #[inline]
    pub(crate) fn set_state(self, i: i16, v: u16) {
        self.wr_u16(i, 10, v)
    }
    #[inline]
    pub(crate) fn set_x(self, i: i16, v: i16) {
        self.wr_i16(i, self.coord_off(), v)
    }
    #[inline]
    pub(crate) fn set_y(self, i: i16, v: i16) {
        self.wr_i16(i, self.coord_off() + 2, v)
    }
    #[inline]
    pub(crate) fn set_width(self, i: i16, v: i16) {
        self.wr_i16(i, self.coord_off() + 4, v)
    }
    #[inline]
    pub(crate) fn set_height(self, i: i16, v: i16) {
        self.wr_i16(i, self.coord_off() + 6, v)
    }

    /// `ob_spec` read at the caller's width: a u64 for LP64, a u32
    /// zero-extended for ILP32. For a string/box object this is a user VA or a
    /// packed colour word; for a sub-struct type it is the base VA of the
    /// TEDINFO/ICONBLK/BITBLK, read at the tree's ABI width by the sub-struct
    /// readers ([`TedInfo::read_abi`] etc.).
    #[inline]
    pub(crate) fn spec(self, i: i16) -> u64 {
        let p = self.ptr(i, self.spec_off());
        // SAFETY: ObjTree invariant; ob_spec is 8 (LP64) / 4 (ILP32) bytes.
        unsafe {
            match self.abi {
                Abi::Lp64 => (p as *const u64).read_unaligned(),
                Abi::Ilp32 => (p as *const u32).read_unaligned() as u64,
            }
        }
    }

    /// The all-ones "no `ob_spec`" sentinel at the caller's width (EmuTOS
    /// `spec == -1L`, a 32-bit LONG): `u64::MAX` for LP64, `0xFFFF_FFFF` for
    /// ILP32 (matching the zero-extended read in [`Self::spec`]). Objects with
    /// this value are neither dereferenced nor drawn.
    #[inline]
    pub(crate) fn spec_none(self) -> u64 {
        match self.abi {
            Abi::Lp64 => u64::MAX,
            Abi::Ilp32 => 0xFFFF_FFFF,
        }
    }
}

// Graphic types (`obdefs.h`).
const G_BOX: u16 = 20;
const G_TEXT: u16 = 21;
const G_BOXTEXT: u16 = 22;
const G_IMAGE: u16 = 23;
const G_IBOX: u16 = 25;
const G_FTEXT: u16 = 29;
const G_FBOXTEXT: u16 = 30;
const G_BUTTON: u16 = 26;
const G_BOXCHAR: u16 = 27;
const G_STRING: u16 = 28;
const G_ICON: u16 = 31;
const G_TITLE: u16 = 32;

/// `BITBLK` — a raw monochrome raster for a `G_IMAGE` object
/// (`obdefs.h`). `bi_pdata` is a native pointer to big-endian 16-bit
/// raster words.
#[repr(C)]
pub struct BitBlk {
    /// pointer to the mono raster data (16-bit words).
    pub bi_pdata: u64,
    /// width of one row in bytes.
    pub bi_wb: i16,
    /// height in rows.
    pub bi_hl: i16,
    /// source x within the raster.
    pub bi_x: i16,
    /// source y within the raster.
    pub bi_y: i16,
    /// foreground colour (PEL) of set bits.
    pub bi_color: i16,
}

/// `TEDINFO` — editable/formatted text for the `G_TEXT`/`G_BOXTEXT`
/// object types (`obdefs.h`). Pointers are native 64-bit.
#[repr(C)]
pub struct TedInfo {
    /// pointer to the text string.
    pub te_ptext: u64,
    /// pointer to the template string.
    pub te_ptmplt: u64,
    /// pointer to the validation string.
    pub te_pvalid: u64,
    /// font id.
    pub te_font: i16,
    /// reserved.
    pub te_junk1: i16,
    /// justification (0 = left, 1 = right, 2 = centre).
    pub te_just: i16,
    /// colour word (`gr_crack` encoding).
    pub te_color: i16,
    /// reserved.
    pub te_junk2: i16,
    /// border thickness.
    pub te_thickness: i16,
    /// text length.
    pub te_txtlen: i16,
    /// template length.
    pub te_tmplen: i16,
}

/// `ICONBLK` — a monochrome icon (mask + data + optional char + text)
/// for a `G_ICON` object (`obdefs.h`). Pointers are native 64-bit.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct IconBlk {
    /// pointer to the mask raster (16-bit words).
    pub ib_pmask: u64,
    /// pointer to the data raster (16-bit words).
    pub ib_pdata: u64,
    /// pointer to the icon's text string.
    pub ib_ptext: u64,
    /// colour/char word: `[15:12]` fg, `[11:8]` bg, `[7:0]` character.
    pub ib_char: i16,
    /// character x/y.
    pub ib_xchar: i16,
    /// character y.
    pub ib_ychar: i16,
    /// icon image rect x/y/w/h (relative to the object).
    pub ib_xicon: i16,
    /// icon y.
    pub ib_yicon: i16,
    /// icon width.
    pub ib_wicon: i16,
    /// icon height.
    pub ib_hicon: i16,
    /// text rect x/y/w/h (relative to the object).
    pub ib_xtext: i16,
    /// text y.
    pub ib_ytext: i16,
    /// text width.
    pub ib_wtext: i16,
    /// text height.
    pub ib_htext: i16,
}

// ── ABI-aware sub-struct readers (ADR-020 stage B) ──────────────────
//
// The `ob_spec` of a G_TEXT/G_IMAGE/G_ICON object points at a TEDINFO /
// BITBLK / ICONBLK sub-struct laid out at the **tree's** ABI width: an LP64
// tree (the desktop, kernel-built alerts, boot selftests) packs 8-byte pointer
// fields, shifting the trailing WORDs; an ILP32 tree (a source-ported GEM app
// such as QED) packs 4-byte pointer fields, so every WORD sits 4 bytes lower
// per pointer. The native `TedInfo`/`BitBlk`/`IconBlk` structs above are the
// LP64 layout, so a plain `&*(spec as *const TedInfo)` misreads an ILP32 blob.
//
// `*::read_abi(base, abi)` materialises the native struct by reading each field
// at its ABI-correct offset with `read_unaligned`; pointer fields are 8 bytes
// LP64 / 4 bytes zero-extended ILP32 (a user VA reachable via the active TTBR0,
// exactly like the OBJECT tree and the string/bitmap data they point at). For
// an LP64 base the offsets equal the native struct, so the materialised value
// is byte-identical to the old direct dereference — LP64 rendering is unchanged.
//
// The `abi` argument is the **tree's** [`ObjTree::abi`], not `current_abi()`:
// a kernel-built (`ObjTree::lp64`) tree points at LP64 sub-structs even while an
// ILP32 app is current, so its readers must use LP64 offsets.

/// Pointer-field width for `abi`: 8 bytes LP64, 4 bytes ILP32. The leading
/// pointer fields of the three sub-structs stride by this, shifting every WORD.
#[inline]
fn abi_ptr_size(abi: Abi) -> u64 {
    match abi {
        Abi::Lp64 => 8,
        Abi::Ilp32 => 4,
    }
}

/// Read a sub-struct pointer field at `off` from `base` at the given ABI width,
/// zero-extending an ILP32 (4-byte) pointer to a native VA.
///
/// # Safety
/// `base + off` addresses a readable pointer field of the sub-struct (the GEM
/// caller's contract for a valid `ob_spec` sub-struct), sized 8 (LP64) / 4
/// (ILP32) bytes.
#[inline]
unsafe fn read_abi_ptr(base: u64, off: u64, abi: Abi) -> u64 {
    let p = (base + off) as *const u8;
    // SAFETY: caller's contract — `base + off` is a readable pointer field.
    unsafe {
        match abi {
            Abi::Lp64 => (p as *const u64).read_unaligned(),
            Abi::Ilp32 => (p as *const u32).read_unaligned() as u64,
        }
    }
}

/// Read a sub-struct WORD field at `off` from `base`.
///
/// # Safety
/// `base + off` addresses a readable 2-byte WORD field of the sub-struct.
#[inline]
unsafe fn read_abi_word(base: u64, off: u64) -> i16 {
    // SAFETY: caller's contract — `base + off` is a readable 2-byte WORD field.
    unsafe { ((base + off) as *const i16).read_unaligned() }
}

impl BitBlk {
    /// Materialise the BITBLK at `base` at the tree's ABI width. Layout: one
    /// pointer (`bi_pdata`) @0, then five WORDs at `abi_ptr_size` (@8 LP64 /
    /// @4 ILP32). `mt_gem.h:2023-2031`.
    ///
    /// # Safety
    /// `base` is the valid `ob_spec` of a G_IMAGE object — a BITBLK packed at
    /// `abi` width.
    #[inline]
    unsafe fn read_abi(base: u64, abi: Abi) -> BitBlk {
        let w = abi_ptr_size(abi); // first WORD offset
        // SAFETY: `base` is a valid BITBLK packed at `abi` width (caller's contract).
        unsafe {
            BitBlk {
                bi_pdata: read_abi_ptr(base, 0, abi),
                bi_wb: read_abi_word(base, w),
                bi_hl: read_abi_word(base, w + 2),
                bi_x: read_abi_word(base, w + 4),
                bi_y: read_abi_word(base, w + 6),
                bi_color: read_abi_word(base, w + 8),
            }
        }
    }
}

impl TedInfo {
    /// Materialise the TEDINFO at `base` at the tree's ABI width. Layout: three
    /// pointers (`te_ptext`/`te_ptmplt`/`te_pvalid`), then eight WORDs at
    /// `3 * abi_ptr_size` (@24 LP64 / @12 ILP32). `mt_gem.h:1988-2001`.
    ///
    /// # Safety
    /// `base` is the valid `ob_spec` of a *TEXT object — a TEDINFO packed at
    /// `abi` width.
    #[inline]
    unsafe fn read_abi(base: u64, abi: Abi) -> TedInfo {
        let ps = abi_ptr_size(abi);
        let w = 3 * ps; // first WORD offset
        // SAFETY: `base` is a valid TEDINFO packed at `abi` width (caller's contract).
        unsafe {
            TedInfo {
                te_ptext: read_abi_ptr(base, 0, abi),
                te_ptmplt: read_abi_ptr(base, ps, abi),
                te_pvalid: read_abi_ptr(base, 2 * ps, abi),
                te_font: read_abi_word(base, w),
                te_junk1: read_abi_word(base, w + 2),
                te_just: read_abi_word(base, w + 4),
                te_color: read_abi_word(base, w + 6),
                te_junk2: read_abi_word(base, w + 8),
                te_thickness: read_abi_word(base, w + 10),
                te_txtlen: read_abi_word(base, w + 12),
                te_tmplen: read_abi_word(base, w + 14),
            }
        }
    }
}

impl IconBlk {
    /// Materialise the ICONBLK at `base` at the tree's ABI width. Layout: three
    /// pointers (`ib_pmask`/`ib_pdata`/`ib_ptext`), then eleven WORDs at
    /// `3 * abi_ptr_size` (@24 LP64 / @12 ILP32). `mt_gem.h:2004-2020`.
    ///
    /// # Safety
    /// `base` is the valid `ob_spec` of a G_ICON object — an ICONBLK packed at
    /// `abi` width.
    #[inline]
    unsafe fn read_abi(base: u64, abi: Abi) -> IconBlk {
        let ps = abi_ptr_size(abi);
        let w = 3 * ps; // first WORD offset
        // SAFETY: `base` is a valid ICONBLK packed at `abi` width (caller's contract).
        unsafe {
            IconBlk {
                ib_pmask: read_abi_ptr(base, 0, abi),
                ib_pdata: read_abi_ptr(base, ps, abi),
                ib_ptext: read_abi_ptr(base, 2 * ps, abi),
                ib_char: read_abi_word(base, w),
                ib_xchar: read_abi_word(base, w + 2),
                ib_ychar: read_abi_word(base, w + 4),
                ib_xicon: read_abi_word(base, w + 6),
                ib_yicon: read_abi_word(base, w + 8),
                ib_wicon: read_abi_word(base, w + 10),
                ib_hicon: read_abi_word(base, w + 12),
                ib_xtext: read_abi_word(base, w + 14),
                ib_ytext: read_abi_word(base, w + 16),
                ib_wtext: read_abi_word(base, w + 18),
                ib_htext: read_abi_word(base, w + 20),
            }
        }
    }
}

// Flag / state bits.
const LASTOB: u16 = 0x0020;
const HIDETREE: u16 = 0x0080;
const SELECTED: u16 = 0x0001;
const DEFAULT: u16 = 0x0002; // ob_flags: default (heavy-frame) object
const EXIT: u16 = 0x0004; // ob_flags: EXIT button (thickens the border by 1)
const CROSSED: u16 = 0x0002; // ob_state: draw a diagonal cross over the object
const CHECKED: u16 = 0x0004;
const DISABLED: u16 = 0x0008;
const OUTLINED: u16 = 0x0010;
const SHADOWED: u16 = 0x0020; // ob_state: solid drop shadow to the lower-right

/// 50 % dither used to grey-out DISABLED objects (EmuTOS `IP_4PATT`).
const STIPPLE_50: [u16; 2] = [0xAAAA, 0x5555];

// 3D-object flags (`ob_flags`, EmuTOS `CONF_WITH_3D_OBJECTS`, obdefs.h:92-97).
const FL3DOBJ: u16 = 0x0200; // "indicator OR activator" bit: set on IND/ACT, CLEAR on BAK
const FL3DIND: u16 = 0x0200; // indicator (recessed): grown + beveled
const FL3DBAK: u16 = 0x0400; // background panel: flat, NOT grown, NOT beveled
const FL3DACT: u16 = 0x0600; // activator / button (raised): grown + beveled
const FL3DMASK: u16 = 0x0600;

const NIL: i16 = -1;

// VDI default palette pens.
const WHITE: Pel = 0;
const BLACK: Pel = 1;
const GRAY_LT: Pel = 8; // EmuTOS LWHITE — the 3D object face
const GRAY_DK: Pel = 9; // EmuTOS LBLACK — the 3D shadow edge

// Text justification (`te_just`).
const TE_LEFT: i16 = 0;
const TE_RIGHT: i16 = 1;
const TE_CNTR: i16 = 2;

// ── dispatch entry points ───────────────────────────────────────────

/// `objc_draw` (42) — draw the subtree rooted at `OB_DRAWOB`, `OB_DEPTH`
/// levels deep, clipped to `OB_XCLIP..OB_HCLIP`. `addr_in[0]` = tree.
pub fn objc_draw(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let drawob = pb.intin(0);
    let depth = pb.intin(1);

    let Some(h) = host() else {
        pb.set_ret(0);
        return;
    };
    let Some(backend) = h.raster() else {
        pb.set_ret(0);
        return;
    };
    if tree.is_null() || drawob < 0 {
        pb.set_ret(0);
        return;
    }

    // Clip to OB_XCLIP/YCLIP/WCLIP/HCLIP (int_in[2..5]); the backend
    // intersects with the screen edges. EmuTOS `objc_draw` sets exactly
    // this VDI clip before drawing, so a partial (damaged-rectangle)
    // redraw does not spill outside the caller's region.
    let (cx, cy, cw, ch) = (pb.intin(2), pb.intin(3), pb.intin(4), pb.intin(5));
    // SAFETY: the GEM caller guarantees a valid tree array.
    unsafe { render_tree(tree, drawob, depth, cx, cy, cw, ch, backend) };
    pb.set_ret(1);
}

/// Render object tree `tree` starting at object `start` for `depth` levels,
/// clipped to `(cx, cy, cw, ch)`, straight through `backend` — the body of
/// [`objc_draw`] **without** the AES trap, reading the tree at the **current**
/// ABI width ([`ObjTree::new`]).
///
/// In-kernel callers that must draw an object tree while already inside the
/// AES dispatch use this instead of a re-entrant `svc #2`. For a **kernel-built**
/// tree (the desktop's `g_screen`, the AES background fallback root) that is
/// always native `Object`s even while an ILP32 app is current, use
/// [`render_tree_lp64`] instead, which forces the 32-byte record layout.
///
/// # Safety
/// `tree` must be a valid `OBJECT` array with `start` in range.
#[allow(clippy::too_many_arguments)]
pub unsafe fn render_tree(
    tree: *const Object,
    start: i16,
    depth: i16,
    cx: i16,
    cy: i16,
    cw: i16,
    ch: i16,
    backend: &dyn RasterBackend,
) {
    if tree.is_null() || start < 0 {
        return;
    }
    render_tree_t(ObjTree::new(tree), start, depth, cx, cy, cw, ch, backend);
}

/// As [`render_tree`], but reads `tree` at the native LP64 (32-byte) record
/// layout regardless of the current execution context's ABI ([`ObjTree::lp64`]).
///
/// This is the non-trapping `w_drawdesk`/`objc_draw` primitive for the AES
/// screen-background composite ([`crate::aes::window`]): both the desktop's
/// registered `g_screen` tree (`WF_NEWDESK`) and the AES-internal fallback root
/// are kernel-built native `Object`s, so they must be read at LP64 width even
/// when an ILP32 app is the current process (exactly like the kernel-built
/// alert/fileselector trees use [`ObjTree::lp64`]).
///
/// # Safety
/// `tree` must be a valid native (LP64) `OBJECT` array with `start` in range.
#[allow(clippy::too_many_arguments)]
pub unsafe fn render_tree_lp64(
    tree: *const Object,
    start: i16,
    depth: i16,
    cx: i16,
    cy: i16,
    cw: i16,
    ch: i16,
    backend: &dyn RasterBackend,
) {
    if tree.is_null() || start < 0 {
        return;
    }
    render_tree_t(ObjTree::lp64(tree), start, depth, cx, cy, cw, ch, backend);
}

/// Shared body of [`render_tree`]/[`render_tree_lp64`]: draw the subtree rooted
/// at `start`, `depth` levels deep, clipped to `(cx, cy, cw, ch)`, through the
/// caller-chosen [`ObjTree`] accessor (which fixes the ABI width).
#[allow(clippy::too_many_arguments)]
fn render_tree_t(
    t: ObjTree,
    start: i16,
    depth: i16,
    cx: i16,
    cy: i16,
    cw: i16,
    ch: i16,
    backend: &dyn RasterBackend,
) {
    let clip = ClipRect { xmn: cx, ymn: cy, xmx: cx + cw - 1, ymx: cy + ch - 1 };
    // Absolute origin of `start` = sum of its ancestors' x/y (like objc_offset).
    let (ox, oy) = abs_offset(t, start);
    draw_tree(t, start, depth, ox - t.x(start), oy - t.y(start), clip, backend);
}

/// `objc_offset` (44) — the object's absolute screen x/y (sum of parent
/// offsets). `int_in[0]` = object; `OB_XOFF`/`OB_YOFF` = `int_out[1..2]`.
pub fn objc_offset(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    let (x, y) = abs_offset(ObjTree::new(tree), obj);
    pb.set_intout(1, x);
    pb.set_intout(2, y);
    pb.set_ret(1);
}

/// `objc_add` (40) — link object `child` (`int_in[1]`) as the last child of
/// `parent` (`int_in[0]`). `addr_in[0]` = tree. EmuTOS `ob_add`.
pub fn objc_add(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let parent = pb.intin(0);
    let child = pb.intin(1);
    if tree.is_null() || parent < 0 || child < 0 {
        pb.set_ret(0);
        return;
    }
    ob_add(ObjTree::new(tree), parent, child);
    pb.set_ret(1);
}

/// `objc_delete` (41) — unlink object `int_in[0]` from its parent's child
/// list (the ROOT cannot be deleted). `addr_in[0]` = tree. EmuTOS `ob_delete`.
pub fn objc_delete(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    let ok = ob_delete(ObjTree::new(tree), obj);
    pb.set_ret(ok);
}

/// `objc_order` (45) — move object `int_in[0]` to sibling position
/// `int_in[1]` (0 = front/head, -1 = back/tail). `addr_in[0]` = tree.
/// EmuTOS `ob_order`.
pub fn objc_order(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    let newpos = pb.intin(1);
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    let ok = ob_order(ObjTree::new(tree), obj, newpos);
    pb.set_ret(if ok { 1 } else { 0 });
}

/// Link `child` as the last child of `parent` (EmuTOS `ob_add`).
fn ob_add(t: ObjTree, parent: i16, child: i16) {
    if parent == NIL || child == NIL {
        return;
    }
    t.set_next(child, parent); // last sibling → parent
    let lastkid = t.tail(parent);
    if lastkid == NIL {
        t.set_head(parent, child); // first child
    } else {
        t.set_next(lastkid, child); // append to the end
    }
    t.set_tail(parent, child);
}

/// Unlink `obj` from its parent's child list (EmuTOS `ob_delete`). Returns 1
/// on success, 0 for the ROOT or a not-found object.
fn ob_delete(t: ObjTree, obj: i16) -> i16 {
    if obj == 0 {
        return 0; // can't delete the ROOT
    }
    let mut nextsib = t.next(obj);
    let parent = parent_of(t, obj);
    if parent == NIL {
        return 0;
    }
    if t.head(parent) == obj {
        if t.tail(parent) == obj {
            nextsib = NIL; // the only child — clear head + tail
            t.set_tail(parent, NIL);
        }
        t.set_head(parent, nextsib);
    } else {
        let prev = get_prev(t, parent, obj);
        if prev == NIL {
            return 0; // not in this chain
        }
        t.set_next(prev, nextsib);
        if t.tail(parent) == obj {
            t.set_tail(parent, prev);
        }
    }
    1
}

/// Move `mov_obj` to sibling position `new_pos` (0 = head, NIL = tail) among
/// its siblings (EmuTOS `ob_order`). Defensive over the reference: a parent
/// left empty after the delete, or a `new_pos` past the end, is handled
/// without dereferencing NIL (which the reference would).
///
fn ob_order(t: ObjTree, mov_obj: i16, new_pos: i16) -> bool {
    if mov_obj == 0 {
        return false; // the ROOT has no siblings
    }
    let parent = parent_of(t, mov_obj);
    if parent == NIL {
        return false;
    }
    ob_delete(t, mov_obj);
    let head = t.head(parent);
    if head == NIL {
        // parent is now empty → mov_obj becomes its sole child
        t.set_next(mov_obj, parent);
        t.set_head(parent, mov_obj);
        t.set_tail(parent, mov_obj);
        return true;
    }
    let mut chg_obj = head;
    if new_pos == 0 {
        t.set_next(mov_obj, chg_obj);
        t.set_head(parent, mov_obj);
    } else {
        if new_pos == NIL {
            chg_obj = t.tail(parent);
        } else {
            let mut ii = 1;
            while ii < new_pos {
                let n = t.next(chg_obj);
                if n == parent || n == NIL {
                    break; // clamp to the end of the sibling list
                }
                chg_obj = n;
                ii += 1;
            }
        }
        let cn = t.next(chg_obj);
        t.set_next(mov_obj, cn);
        t.set_next(chg_obj, mov_obj);
    }
    if t.next(mov_obj) == parent {
        t.set_tail(parent, mov_obj);
    }
    true
}

/// Previous sibling of `obj` under `parent`, or `NIL` (EmuTOS `get_prev`).
fn get_prev(t: ObjTree, parent: i16, obj: i16) -> i16 {
    let mut pobj = t.head(parent);
    if pobj == obj {
        return NIL;
    }
    loop {
        let nobj = t.next(pobj);
        if nobj == obj {
            return pobj;
        }
        if nobj == parent || nobj == NIL {
            return NIL;
        }
        pobj = nobj;
    }
}

/// `objc_change` (47) — set `obj`'s state to `OB_NEWSTATE` (`int_in[6]`)
/// and, if `OB_REDRAW` (`int_in[7]`) is set, redraw that object.
/// `addr_in[0]` = tree.
pub fn objc_change(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    let newstate = pb.intin(6) as u16;
    let redraw = pb.intin(7) != 0;
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    change(tree, obj, newstate, redraw);
    pb.set_ret(1);
}

// ── objc_sysvar (3-D look) ──────────────────────────────────────────────
//
// The 3-D look system variables (EmuTOS `ob_sysvar`), with EmuTOS' defaults
// (`gemoblib.c`): activators move their text on press (`acttxtmove = 1`) but
// keep their colour; indicators change colour when selected
// (`indcolchange = 1`) but do not move text. `just_draw` honours these
// (`movetext`/`changecol`); a SET restyles the next draw.
static IND_TXTMOVE: AtomicI16 = AtomicI16::new(0);
static IND_COLCHANGE: AtomicI16 = AtomicI16::new(1);
static ACT_TXTMOVE: AtomicI16 = AtomicI16::new(1);
static ACT_COLCHANGE: AtomicI16 = AtomicI16::new(0);
static IND_BUTCOL: AtomicI16 = AtomicI16::new(GRAY_LT as i16);
static ACT_BUTCOL: AtomicI16 = AtomicI16::new(GRAY_LT as i16);
static BACKGR_COL: AtomicI16 = AtomicI16::new(GRAY_LT as i16);

// objc_sysvar `which` selectors (obdefs.h).
const LK3DIND: i16 = 1;
const LK3DACT: i16 = 2;
const INDBUTCOL: i16 = 3;
const ACTBUTCOL: i16 = 4;
const BACKGRCOL: i16 = 5;
const AD3DVALUE: i16 = 6;
const ADJ3DSTD: i16 = 2; // NewTOS's bevel is 2px (EmuTOS ADJ3DSTD)
const ADJ3DOUT: i16 = 3; // extra extent of an OUTLINED 3D object (EmuTOS ADJ3DOUT)

/// `objc_sysvar` (48) — get (`int_in[0]` = 0) or set (1) a 3-D look variable
/// `int_in[1]` from values `int_in[2..3]`. GET returns the pair in
/// `int_out[1..2]`; `int_out[0]` = 1 (ok) / 0 (bad which or out-of-range
/// colour). EmuTOS `ob_sysvar`.
pub fn objc_sysvar(pb: &Aespb) {
    let (mode, which, in1, in2) = (pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3));
    let ncol = host()
        .and_then(|h| h.raster())
        .map(|r| r.num_colors() as i16)
        .unwrap_or(16);
    let ok = if mode != 0 {
        // SET (-1 in either value leaves that field unchanged).
        match which {
            LK3DIND => {
                if in1 != -1 {
                    IND_TXTMOVE.store(in1, Ordering::Relaxed);
                }
                if in2 != -1 {
                    IND_COLCHANGE.store(in2, Ordering::Relaxed);
                }
                true
            }
            LK3DACT => {
                if in1 != -1 {
                    ACT_TXTMOVE.store(in1, Ordering::Relaxed);
                }
                if in2 != -1 {
                    ACT_COLCHANGE.store(in2, Ordering::Relaxed);
                }
                true
            }
            INDBUTCOL if in1 < ncol => {
                IND_BUTCOL.store(in1, Ordering::Relaxed);
                true
            }
            ACTBUTCOL if in1 < ncol => {
                ACT_BUTCOL.store(in1, Ordering::Relaxed);
                true
            }
            BACKGRCOL if in1 < ncol => {
                BACKGR_COL.store(in1, Ordering::Relaxed);
                true
            }
            _ => false, // unknown which, or colour >= ws_ncolors
        }
    } else {
        // GET
        let (o1, o2, valid) = match which {
            LK3DIND => (IND_TXTMOVE.load(Ordering::Relaxed), IND_COLCHANGE.load(Ordering::Relaxed), true),
            LK3DACT => (ACT_TXTMOVE.load(Ordering::Relaxed), ACT_COLCHANGE.load(Ordering::Relaxed), true),
            INDBUTCOL => (IND_BUTCOL.load(Ordering::Relaxed), 0, true),
            ACTBUTCOL => (ACT_BUTCOL.load(Ordering::Relaxed), 0, true),
            BACKGRCOL => (BACKGR_COL.load(Ordering::Relaxed), 0, true),
            AD3DVALUE => (ADJ3DSTD, ADJ3DSTD, true),
            _ => (0, 0, false),
        };
        pb.set_intout(1, o1);
        pb.set_intout(2, o2);
        valid
    };
    pb.set_ret(if ok { 1 } else { 0 });
}

/// Set `obj`'s state and optionally redraw just that object. Shared by
/// `objc_change` and the Form Manager (to show a pressed button). Reads the
/// tree at the **current** ABI width (an app-provided tree); a kernel-built
/// tree uses [`change_t`] with an [`ObjTree::lp64`] accessor.
pub(crate) fn change(tree: *mut Object, obj: i16, newstate: u16, redraw: bool) {
    change_t(ObjTree::new(tree), obj, newstate, redraw);
}

/// Set `obj`'s state and optionally redraw it, driving the tree through the
/// caller-supplied [`ObjTree`] accessor (whose ABI width the caller chose:
/// `new` for an app tree, `lp64` for a kernel-built one).
pub(crate) fn change_t(t: ObjTree, obj: i16, newstate: u16, redraw: bool) {
    // EmuTOS `ob_change` (gemoblib.c:1263): a `spec == -1` sentinel object
    // is left entirely untouched — no state change, no redraw — so the
    // redraw path never dereferences `-1` as a pointer.
    if t.spec(obj) == t.spec_none() {
        return;
    }
    t.set_state(obj, newstate);
    if !redraw {
        return;
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        let clip = ClipRect::full(backend.width(), backend.height());
        let (ox, oy) = abs_offset(t, obj);
        // depth 0 redraws only this object.
        draw_tree(t, obj, 0, ox - t.x(obj), oy - t.y(obj), clip, backend);
    }
}

// objc_edit kinds (`obdefs.h`).
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;
const ED_END: i16 = 3;

/// `objc_edit` (46) — edit the EDITABLE `TEDINFO` text field `obj`.
/// `int_in[1]` = character, `int_in[2]` = cursor index, `int_in[3]` =
/// kind (EDSTART/EDINIT/EDCHAR/EDEND). The new cursor index is returned
/// in `OB_ODX` (`int_out[1]`).
///
/// Batch-6 scope: plain in-place editing of `te_ptext` (insert printable,
/// backspace; cursor shown as a caret). The template (`te_ptmplt`) and
/// validation (`te_pvalid`) of formatted fields land in a later batch.
pub fn objc_edit(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj <= 0 {
        // EmuTOS `ob_edit` (gemobed.c:342): `obj <= 0` — including the ROOT
        // (0) — is a no-op that returns TRUE without touching the field. The
        // ROOT of a dialog is a box whose `ob_spec` is a colour/thickness
        // word, not a `TEDINFO` pointer, so `edit_field` would deref a wild
        // pointer. The objc_edit slot contract is "always TRUE".
        pb.set_intout(1, pb.intin(2));
        pb.set_ret(1);
        return;
    }
    let idx = edit_field(tree, obj, pb.intin(1), pb.intin(2), pb.intin(3));
    pb.set_intout(1, idx);
    pb.set_ret(1);
}

/// Edit the EDITABLE `TEDINFO` field `obj` in place and return the new
/// cursor index. Shared by `objc_edit` (the trap) and the Form Manager's
/// modal loop. `kind` = EDINIT (cursor to end) / EDCHAR (insert/delete at
/// the cursor) / EDEND (finish, no caret). Redraws the field + caret.
///
/// Faithful to EmuTOS `ob_edit` (`gemobed.c`): the cursor index `idx` is a
/// **raw-string** index into `te_ptext` (data only, no template literals);
/// the on-screen caret column is `find_pos(te_ptmplt, idx)`, and each typed
/// character is validated against the `te_pvalid` code for its position.
/// For a plain field (no template / no validation — e.g. the desktop's
/// rename box) `find_pos` is the identity and validation accepts anything,
/// so this reduces to plain in-place editing.
pub(crate) fn edit_field(tree: *const Object, obj: i16, in_char: i16, idx: i16, kind: i16) -> i16 {
    edit_field_t(ObjTree::new(tree), obj, in_char, idx, kind)
}

/// Edit the EDITABLE `TEDINFO` field `obj` through the caller-supplied
/// [`ObjTree`] accessor (`new` for an app tree, `lp64` for a kernel-built
/// tree such as the fileselector's). Body of [`edit_field`].
pub(crate) fn edit_field_t(t: ObjTree, obj: i16, in_char: i16, mut idx: i16, kind: i16) -> i16 {
    let spec = t.spec(obj);
    if spec == 0 {
        return idx;
    }
    // The editable field's ob_spec points at a TEDINFO packed at the tree's ABI
    // width; read/write `te_ptext`/`te_ptmplt`/`te_pvalid` at that width so an
    // ILP32 app's edit fields (cursor/insert/backspace) work like LP64's.
    // SAFETY: `spec` is the valid TEDINFO base of an EDITABLE field (GEM contract).
    let ted = unsafe { TedInfo::read_abi(spec, t.abi()) };
    let ted = &ted;
    let txtlen = ted.te_txtlen.max(1);
    let text = ted.te_ptext as *mut u8;
    let tmpl = read_cstr(ted.te_ptmplt);

    match kind {
        ED_INIT => idx = cstr_len_p(text) as i16,
        ED_CHAR => edit_char(ted, text, txtlen, &mut idx, in_char),
        _ => {} // EDSTART / EDEND
    }

    // Redraw the field, then the caret (unless we are ending the edit).
    if let Some(backend) = host().and_then(|h| h.raster()) {
        let clip = ClipRect::full(backend.width(), backend.height());
        let (ox, oy) = abs_offset(t, obj);
        // depth 0 redraws only this field.
        draw_tree(t, obj, 0, ox - t.x(obj), oy - t.y(obj), clip, backend);
        if kind != ED_END {
            // Caret at the template-relative column (EmuTOS `find_pos`): a
            // 1-px black bar the height of the text.
            let col = find_pos(tmpl, idx);
            let cy = oy + (t.height(obj) - crate::vdi::font::CELL_H) / 2;
            backend.fill_rect(ox + col * crate::vdi::font::CELL_W, cy, 1, crate::vdi::font::CELL_H, BLACK, clip);
        }
    }
    idx
}

// Arrow scancodes in the key word's high byte (EmuTOS `scancode.h`).
const SC_LEFT: u8 = 0x4B;
const SC_RIGHT: u8 = 0x4D;

/// Apply one keystroke to the raw text buffer at the cursor (EmuTOS
/// `ob_edit` EDCHAR). Handles backspace / delete / escape / left-right
/// arrows and validated printable insertion.
fn edit_char(ted: &TedInfo, text: *mut u8, txtlen: i16, idx: &mut i16, ch: i16) {
    let pvalid = read_cstr(ted.te_pvalid);
    let c = (ch & 0xff) as u8;
    let scan = ((ch >> 8) & 0xff) as u8;
    let len = cstr_len_p(text) as i16;

    // Cursor movement (scancode in the high byte).
    match scan {
        SC_LEFT => {
            if *idx > 0 {
                *idx -= 1;
            }
            return;
        }
        SC_RIGHT => {
            if *idx < len {
                *idx += 1;
            }
            return;
        }
        _ => {}
    }

    match c {
        0x08 => {
            // BACKSPACE: delete the char before the cursor.
            if *idx > 0 {
                *idx -= 1;
                delete_at(text, *idx, len);
            }
        }
        0x7F => {
            // DELETE: delete the char at the cursor.
            if *idx < len {
                delete_at(text, *idx, len);
            }
        }
        0x1B => {
            // ESCAPE: clear the whole field.
            *idx = 0;
            // SAFETY: `text` is a valid buffer of at least one byte.
            unsafe { *text = 0 };
        }
        0x20..=0xFF => {
            // Printable: validate against this position's `te_pvalid` code,
            // then insert (subject to the text length).
            let mut ic = c;
            if check(&mut ic, valid_code(pvalid, *idx)) && len < txtlen - 1 {
                ins_char(text, *idx, ic, len);
                *idx += 1;
            }
        }
        _ => {}
    }
}

/// Delete the char at `pos` from the NUL-terminated buffer of length `len`
/// (shift the tail down, EmuTOS `ob_delit`/`strcpy`).
fn delete_at(text: *mut u8, pos: i16, len: i16) {
    // SAFETY: shifting within the NUL-terminated buffer (pos < len).
    unsafe {
        let mut i = pos;
        while i < len {
            *text.add(i as usize) = *text.add((i + 1) as usize);
            i += 1;
        }
    }
}

/// Insert `chr` at `pos` in the NUL-terminated buffer of length `len`
/// (EmuTOS `ins_char`; caller has already checked `len < tot_len - 1`).
fn ins_char(text: *mut u8, pos: i16, chr: u8, len: i16) {
    // SAFETY: room exists; shift the tail up (incl. the NUL) then store.
    unsafe {
        let mut i = len;
        while i >= pos {
            *text.add((i + 1) as usize) = *text.add(i as usize);
            i -= 1;
        }
        *text.add(pos as usize) = chr;
    }
}

/// The `te_pvalid` validation code for raw position `idx` (EmuTOS expands
/// the string to `te_tmplen` by repeating the last code). An empty
/// validation string yields code 0 → "accept anything" (plain field).
fn valid_code(pvalid: &[u8], idx: i16) -> u8 {
    if pvalid.is_empty() {
        return 0;
    }
    let i = (idx as usize).min(pvalid.len() - 1);
    pvalid[i]
}

/// Map a raw-string index to its template column (EmuTOS `find_pos`): the
/// index of the `idx`-th `_` in `tmpl`, advanced to the next `_`. Degrades
/// to the identity when the template has no `_` left (plain field).
fn find_pos(tmpl: &[u8], mut pos: i16) -> i16 {
    let mut i = 0i16;
    while pos > 0 {
        if i as usize >= tmpl.len() {
            // Template exhausted: remaining positions map 1:1 past its end.
            return i + pos;
        }
        if tmpl[i as usize] == b'_' {
            pos -= 1;
        }
        i += 1;
    }
    // Skip to the first `_` at or after `i`.
    while (i as usize) < tmpl.len() && tmpl[i as usize] != b'_' {
        i += 1;
    }
    i
}

// TEDINFO validation character sets (EmuTOS `gemobed.c`; `..` denotes an
// inclusive range, parsed by `instr`). The derived sets drop the leading
// members of their parent (`A` = `N` without `0..9`, etc.).
const VALIDATE_N: &[u8] = b"0..9A..Z \x80\x8e\x8f\x90\x92\x99\x9a\x9e\xa5\xb5\xb6\xb7\xb8\xc2..\xdc";
const VALIDATE_LN: &[u8] = b"0..9a..zA..Z \x80..\xff"; // EmuTOS VALIDATE_n
const VALIDATE_F: &[u8] = b":?*a..zA..Z0..9_\x80..\xff";
const VALIDATE_P: &[u8] = b".?*a..zA..Z0..9_\\:\x80..\xff";

/// ASCII upcase (EmuTOS `toupper`, Roman range only).
fn upcase(c: u8) -> u8 {
    if c.is_ascii_lowercase() {
        c - 32
    } else {
        c
    }
}

/// Test whether `chr` is in the range/enum set `set` (EmuTOS `instr`):
/// each `x..z` is an inclusive range, every other byte a literal member.
fn instr(chr: u8, set: &[u8]) -> bool {
    let mut i = 0;
    while i < set.len() {
        let lo = set[i];
        i += 1;
        // A trailing `..z` after `lo` makes it the range `lo..=z`.
        let hi = if i + 1 < set.len() && set[i] == b'.' && set[i + 1] == b'.' {
            i += 2;
            let h = set.get(i).copied().unwrap_or(lo);
            i += 1;
            h
        } else {
            lo
        };
        if chr >= lo && chr <= hi {
            return true;
        }
    }
    false
}

/// Validate/upshift a character against a `te_pvalid` code (EmuTOS
/// `check`). Code 0 (no validation) and `X`/`x` accept anything; `x`
/// upshifts. Returns whether the character is allowed, upshifting
/// `*in_char` in place when the code requests it.
fn check(in_char: &mut u8, valchar: u8) -> bool {
    let (set, up): (&[u8], bool) = match valchar {
        0 => return true, // plain field: accept anything
        b'9' => (b"0..9", false),
        b'A' => (&VALIDATE_N[4..], true),
        b'N' => (VALIDATE_N, true),
        b'a' => (&VALIDATE_LN[4..], false),
        b'n' => (VALIDATE_LN, false),
        b'F' => (VALIDATE_F, true),
        b'f' => (&VALIDATE_F[3..], true),
        b'P' => (VALIDATE_P, true),
        b'p' => (&VALIDATE_P[3..], true),
        b'X' => return true,
        b'x' => {
            *in_char = upcase(*in_char);
            return true;
        }
        _ => return false,
    };
    if instr(*in_char, set) {
        if up {
            *in_char = upcase(*in_char);
        }
        true
    } else {
        false
    }
}

/// Length of a NUL-terminated string at `p` (cap 256).
fn cstr_len_p(p: *const u8) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: GEM contract — valid NUL-terminated buffer.
    unsafe {
        let mut n = 0;
        while n < 256 && *p.add(n) != 0 {
            n += 1;
        }
        n
    }
}

/// `objc_find` (43) — hit-test: index of the deepest **visible** object
/// (`!HIDETREE`, inside its rect) under `(OB_MX, OB_MY)` within `OB_DEPTH`
/// levels of `OB_DRAWOB`, or `-1`. DISABLED objects **are** returned (the
/// disabled filter is in `form_do`, not here). `addr_in[0]` = tree.
pub fn objc_find(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let startob = pb.intin(0);
    let depth = pb.intin(1);
    let mx = pb.intin(2);
    let my = pb.intin(3);
    if tree.is_null() || startob < 0 {
        pb.set_ret(NIL);
        return;
    }
    let t = ObjTree::new(tree);
    let (ox, oy) = abs_offset(t, startob);
    let found = find_hit(t, startob, depth, ox - t.x(startob), oy - t.y(startob), mx, my);
    pb.set_ret(found);
}

/// Hit-test entry for internal callers (the Form Manager's modal loop):
/// deepest visible object under `(mx, my)` within `depth` levels of
/// `root`, or `NIL`. Mirrors `objc_find` — DISABLED objects are returned;
/// `form_do` applies the disabled filter on the result.
///
/// # Safety
/// `tree` is a valid OBJECT array.
pub(crate) unsafe fn hit_test(
    tree: *const Object,
    root: i16,
    depth: i16,
    mx: i16,
    my: i16,
) -> i16 {
    hit_test_t(ObjTree::new(tree), root, depth, mx, my)
}

/// Hit-test through the caller-supplied [`ObjTree`] accessor (`new` for an app
/// tree, `lp64` for a kernel-built one such as an alert or the fileselector).
/// Body of [`hit_test`].
pub(crate) fn hit_test_t(t: ObjTree, root: i16, depth: i16, mx: i16, my: i16) -> i16 {
    let (ox, oy) = abs_offset(t, root);
    find_hit(t, root, depth, ox - t.x(root), oy - t.y(root), mx, my)
}

/// Absolute screen rectangle `(x, y, w, h)` of `obj`, reading an app tree at
/// the current ABI width. A kernel-built tree uses [`abs_rect_t`].
pub(crate) fn abs_rect(tree: *const Object, obj: i16) -> (i16, i16, i16, i16) {
    abs_rect_t(ObjTree::new(tree), obj)
}

/// Absolute screen rectangle `(x, y, w, h)` of `obj` through the caller-supplied
/// [`ObjTree`] accessor. Body of [`abs_rect`].
pub(crate) fn abs_rect_t(t: ObjTree, obj: i16) -> (i16, i16, i16, i16) {
    let (x, y) = abs_offset(t, obj);
    (x, y, t.width(obj), t.height(obj))
}

/// First child of `parent` whose absolute rectangle contains `(mx, my)`,
/// scanning only direct children; `NIL` if none. Used for menu hit-tests.
pub(crate) fn child_at(tree: *const Object, parent: i16, mx: i16, my: i16) -> i16 {
    let t = ObjTree::new(tree);
    let mut c = t.head(parent);
    while c != NIL && c != parent {
        let (x, y, w, h) = abs_rect(tree, c);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return c;
        }
        c = t.next(c);
    }
    NIL
}

/// Set or clear one `ob_state` bit on `obj` and redraw it. Thin wrapper
/// the Menu Manager shares for title/item highlighting.
pub(crate) fn set_state_bit(tree: *mut Object, obj: i16, bit: u16, set: bool) {
    set_state_bit_t(ObjTree::new(tree), obj, bit, set);
}

/// Set or clear one `ob_state` bit on `obj` and redraw it, through the
/// caller-supplied [`ObjTree`] accessor (`lp64` for a kernel-built tree such as
/// the fileselector). Body of [`set_state_bit`].
pub(crate) fn set_state_bit_t(t: ObjTree, obj: i16, bit: u16, set: bool) {
    let st = t.state(obj);
    let new = if set { st | bit } else { st & !bit };
    change_t(t, obj, new, true);
}

/// Draw the subtree rooted at `obj` (any object), `depth` levels deep, at
/// its absolute screen position. Used by the Menu Manager to draw the bar.
pub(crate) fn draw_obj(tree: *const Object, obj: i16, depth: i16, backend: &dyn RasterBackend) {
    draw_obj_t(ObjTree::new(tree), obj, depth, backend);
}

/// Draw the subtree rooted at `obj` through the caller-supplied [`ObjTree`]
/// accessor (`lp64` for a kernel-built tree such as the fileselector). Body of
/// [`draw_obj`].
pub(crate) fn draw_obj_t(t: ObjTree, obj: i16, depth: i16, backend: &dyn RasterBackend) {
    let clip = ClipRect::full(backend.width(), backend.height());
    let (ox, oy) = abs_offset(t, obj);
    draw_tree(t, obj, depth, ox - t.x(obj), oy - t.y(obj), clip, backend);
}

/// Draw a whole tree (used by the Form Manager after building an alert and by
/// the fileselector). The caller supplies the [`ObjTree`] accessor and thereby
/// chooses the ABI width: kernel-built trees pass an [`ObjTree::lp64`] accessor
/// so a 32-byte record is read even while an ILP32 app is current.
pub(crate) fn draw_root(t: ObjTree, depth: i16, backend: &dyn RasterBackend) {
    let clip = ClipRect::full(backend.width(), backend.height());
    // Root is index 0; its ob_x/ob_y are absolute (no parent), so the parent
    // offset is 0.
    draw_tree(t, 0, depth, 0, 0, clip, backend);
}

// ── tree helpers ────────────────────────────────────────────────────

/// Absolute screen origin of `obj` = sum of x/y from the root down. Walks
/// parents by scanning sibling chains (EmuTOS `ob_offset`).
fn abs_offset(t: ObjTree, obj: i16) -> (i16, i16) {
    let mut x = 0i16;
    let mut y = 0i16;
    let mut cur = obj;
    while cur != NIL {
        x += t.x(cur);
        y += t.y(cur);
        cur = parent_of(t, cur);
    }
    (x, y)
}

/// `obj`'s parent index, or `NIL` for the root (EmuTOS `get_par`). Exposed
/// for the Form Manager's radio-button sibling sweep.
///
/// # Safety
/// `tree` is a valid OBJECT array (GEM contract).
pub(crate) unsafe fn parent(tree: *const Object, obj: i16) -> i16 {
    parent_of(ObjTree::new(tree), obj)
}

/// Index of `obj`'s parent, or `NIL` for the root. The parent is the node
/// whose child chain reaches `obj` (TOS finds it via `ob_next` wrapping
/// back to the parent at the end of the sibling list).
fn parent_of(t: ObjTree, obj: i16) -> i16 {
    // Follow ob_next from obj until it points to a node that lists obj as
    // a child range end — TOS sets the last sibling's ob_next to the
    // parent index. Walk ob_next until we find a node whose ob_tail is `last`.
    let mut last = obj;
    loop {
        let n = t.next(last);
        if n == NIL {
            return NIL; // root
        }
        // If n's tail is `last`, then n is the parent.
        if t.tail(n) == last {
            return n;
        }
        last = n;
    }
}

/// Recursively draw `obj` and up to `depth` levels of descendants. `(px,
/// py)` is the absolute origin of `obj`'s parent.
fn draw_tree(
    t: ObjTree,
    obj: i16,
    depth: i16,
    px: i16,
    py: i16,
    clip: ClipRect,
    backend: &dyn RasterBackend,
) {
    if t.flags(obj) & HIDETREE != 0 {
        return;
    }
    let x = px + t.x(obj);
    let y = py + t.y(obj);
    // Skip drawing an object whose `ob_spec` is the `-1` "none" sentinel
    // (EmuTOS `just_draw`, gemoblib.c:491) so it is never dereferenced as a
    // pointer — but still recurse into its children: EmuTOS `everyobj` stops
    // the tree walk only on HIDETREE, not on `spec == -1`.
    if t.spec(obj) != t.spec_none() {
        just_draw(t, obj, x, y, clip, backend);
    }

    if depth > 0 {
        let mut child = t.head(obj);
        while child != NIL {
            draw_tree(t, child, depth - 1, x, y, clip, backend);
            if child == t.tail(obj) {
                break;
            }
            child = t.next(child);
        }
    }
}

/// Render a single object by type at absolute `(x, y)` (Batch-1 types).
// The 8 AES "inside patterns" IP_HOLLOW(0)..IP_SOLID(7) as 4-word dithers
// (EmuTOS `vdi/vdi_fill.c` DITHER; IP_4PATT = 50 %).
const IP_PATTERNS: [[u16; 4]; 8] = [
    [0x0000, 0x0000, 0x0000, 0x0000], // IP_HOLLOW
    [0x0000, 0x4444, 0x0000, 0x1111],
    [0x0000, 0x5555, 0x0000, 0x5555],
    [0x8888, 0x5555, 0x2222, 0x5555],
    [0xAAAA, 0x5555, 0xAAAA, 0x5555], // IP_4PATT (50 %)
    [0xAAAA, 0xDDDD, 0xAAAA, 0x7777],
    [0xEEEE, 0xFFFF, 0xBBBB, 0xFFFF],
    [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF], // IP_SOLID
];
const IP_HOLLOW: u16 = 0;
const IP_SOLID: u16 = 7;

/// Decode a GEM object colour word (EmuTOS `gr_crack`): returns
/// `(border, text, pattern, inside, replace_mode)`.
fn gr_crack(color: u16) -> (Pel, Pel, u16, Pel, bool) {
    (
        ((color >> 12) & 0x0f) as Pel, // border colour
        ((color >> 8) & 0x0f) as Pel,  // text colour
        (color >> 4) & 0x07,           // inside pattern
        (color & 0x0f) as Pel,         // inside colour
        color & 0x80 != 0,             // 1 = REPLACE, 0 = TRANSPARENT
    )
}

/// Complementary colour for a SELECTED object (EmuTOS `xor_color`, reduced to
/// the palette pairs NewTOS actually uses: white↔black, lwhite↔lblack, …).
fn xor_color(c: Pel) -> Pel {
    match c {
        WHITE => BLACK,
        BLACK => WHITE,
        _ => c ^ 1,
    }
}

/// Fill `(x,y,w,h)` with the AES inside pattern `ipat` in colour `icol`
/// (EmuTOS `gr_rect`, `gemgraf.c:547-559`): HOLLOW draws nothing, SOLID a flat
/// fill, the rest an `icol`-on-white dither.
///
/// The dither is drawn in **`WriteMode::Replace`** — EmuTOS `gr_rect` always
/// fills with `MD_REPLACE` (`bb_fill(MD_REPLACE, …)`, `gemgraf.c:558`), so a
/// pattern's *clear* bits are painted the background colour (white), not left
/// transparent. This is what makes a patterned box (the desktop `DROOT`
/// background, `IP_4PATT` green-on-white) an opaque fill that erases whatever it
/// covers, rather than letting the underlying pixels show through the gaps.
fn gr_rect(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, icol: Pel, ipat: u16, clip: ClipRect) {
    if w <= 0 || h <= 0 {
        return;
    }
    match ipat {
        IP_HOLLOW => {}
        IP_SOLID => backend.fill_rect(x, y, w, h, icol, clip),
        p => {
            let pat = PatternFill {
                fg: icol,
                bg: WHITE,
                pattern: &IP_PATTERNS[(p & 7) as usize],
                mode: WriteMode::Replace,
            };
            backend.fill_pattern(x, y, w, h, pat, clip);
        }
    }
}

/// Maximum formatted-string length (EmuTOS `obdefs.h` `MAX_LEN`).
const MAX_LEN: usize = 81;

/// Merge a raw text string into a template (EmuTOS `ob_format`): each `_`
/// in the template consumes the next character of `raw` (blanked back to
/// `_` once `raw` is exhausted); every other template character is copied
/// verbatim. A leading `@` in `raw` means "empty". `just == TE_RIGHT`
/// fills from the right end. The formatted result (always template length,
/// capped at `out.len()`) is written into `out`; the length is returned.
fn ob_format(just: i16, raw: &[u8], tmpl: &[u8], out: &mut [u8]) -> usize {
    // A leading '@' marks an empty raw string (EmuTOS clears it to "").
    let raw: &[u8] = if raw.first() == Some(&b'@') { &[] } else { raw };
    let ptlen = tmpl.len().min(out.len());
    let prlen = raw.len();

    if just == TE_RIGHT {
        // Walk template and raw from their right ends.
        let mut r = prlen; // one past the next raw char to consume
        for k in 0..ptlen {
            let ti = ptlen - 1 - k;
            out[ti] = if tmpl[ti] != b'_' {
                tmpl[ti]
            } else if r > 0 {
                r -= 1;
                raw[r]
            } else {
                b'_'
            };
        }
    } else {
        let mut r = 0usize; // next raw char to consume
        for i in 0..ptlen {
            out[i] = if tmpl[i] != b'_' {
                tmpl[i]
            } else if r < prlen {
                let c = raw[r];
                r += 1;
                c
            } else {
                b'_'
            };
        }
    }
    ptlen
}

fn just_draw(t: ObjTree, obj: i16, x: i16, y: i16, clip: ClipRect, backend: &dyn RasterBackend) {
    // Snapshot the object's fields once at the caller's ABI width; a sub-struct
    // (`ob_spec`-pointed TEDINFO/ICONBLK/BITBLK) is read through its ABI-aware
    // reader (`*::read_abi`) at the same width in the type arms below.
    let ob_state = t.state(obj);
    let ob_flags = t.flags(obj);
    let ob_type = t.ob_type(obj);
    let ob_spec = t.spec(obj);
    let ob_width = t.width(obj);
    let ob_height = t.height(obj);
    let selected = ob_state & SELECTED != 0;
    let disabled = ob_state & DISABLED != 0;
    let d3 = ob_flags & FL3DMASK;
    // D7: the grow and the add_3d_effect bevel key on FL3DOBJ (0x0200), the
    // "indicator OR activator" bit — set on FL3DIND/FL3DACT but CLEAR on
    // FL3DBAK (0x0400). EmuTOS `just_draw` grows only `if (flags & FL3DOBJ)`
    // (gemoblib.c:504) and bevels only `if (flags & FL3DOBJ)` (gemoblib.c:950).
    // An FL3DBAK background panel is therefore neither grown nor beveled; its
    // 3D optic comes from `draw_3d_outline` when OUTLINED (gemoblib.c:875-877).
    let d3obj = ob_flags & FL3DOBJ != 0;

    // EmuTOS `just_draw` grows an FL3DOBJ object by ADJ3DSTD (2px) on every side,
    // so its bevel sits in the margin *outside* the nominal rectangle instead of
    // eating the object's own edge (which shrank every 3D button by 4px and let
    // the bevel overlap the border/text). Grow here, then everything below —
    // fill, border, text, bevel, DISABLED dither — draws against the grown
    // rectangle. FL3DBAK is not an FL3DOBJ, so it keeps its nominal rect.
    let (x, y, w, h) = if d3obj {
        (x - ADJ3DSTD, y - ADJ3DSTD, ob_width + 2 * ADJ3DSTD, ob_height + 2 * ADJ3DSTD)
    } else {
        (x, y, ob_width, ob_height)
    };

    // Black-on-white look. SELECTED inverts fill/text. A 3D object keeps a
    // grey face (the pressed state shows via the bevel, drawn after).
    let (fill, ink) = if d3 != 0 {
        (GRAY_LT, BLACK)
    } else if selected {
        (BLACK, WHITE)
    } else {
        (WHITE, BLACK)
    };

    // 3D dynamics on SELECTED (EmuTOS `just_draw` movetext/changecol): an
    // activator nudges its text on press (acttxtmove), an indicator changes
    // colour (indcolchange). A non-3D object keeps the classic inversion
    // (changecol = true, no move).
    let (movetext, changecol) = match d3 {
        FL3DACT => (
            ACT_TXTMOVE.load(Ordering::Relaxed) != 0,
            ACT_COLCHANGE.load(Ordering::Relaxed) != 0,
        ),
        FL3DIND => (
            IND_TXTMOVE.load(Ordering::Relaxed) != 0,
            IND_COLCHANGE.load(Ordering::Relaxed) != 0,
        ),
        _ => (false, true),
    };

    // Border colour + signed thickness captured from the type body for the
    // SHADOWED decoration below (EmuTOS `just_draw` reuses `bcol`/`th` from the
    // box/text body in the trailing `if (state)` block, gemoblib.c:894-901).
    let mut deco_bcol: Pel = BLACK;
    let mut deco_th: i16 = 0;

    match ob_type {
        G_BOX | G_IBOX | G_BUTTON | G_BOXCHAR => {
            // Crack the colour word: box types pack it in `ob_spec`'s low 16
            // bits (EmuTOS `gr_crack`); a G_BUTTON's `ob_spec` is a text
            // pointer, so it takes the standard black-border / white-interior
            // button colours. Border thickness is in `ob_spec` bits 16..23 for
            // the box types; a button uses 1px.
            let (bcol, tcol, mut ipat, mut icol, _mode) = if ob_type == G_BUTTON {
                (BLACK, BLACK, IP_HOLLOW, WHITE, false)
            } else {
                gr_crack(ob_spec as u16)
            };
            // D4: a G_BUTTON's border is derived from its role and drawn
            // OUTSIDE its rect (negative th), EmuTOS `ob_sst`
            // (gemobjop.c:59-65): base -1, one thinner for EXIT, one thinner
            // for DEFAULT — so a default exit button is -3 and reads as the
            // heavy "this is Enter" outline. The border is drawn whenever
            // th != 0 *regardless of 3D* (gemoblib.c:663-667); the 3D bevel
            // (add_3d_effect, gemoblib.c:950-951) is layered on top afterwards,
            // so a 3D button keeps its black outline AND gains a bevel. Box
            // types keep their spec-byte thickness. D7: an FL3DBAK panel is NOT
            // an FL3DOBJ, so it takes its spec-byte th too (EmuTOS `ob_sst`
            // gives every G_BOX `th = spec byte 1`, gemobjop.c:54-58, and
            // `just_draw` never zeroes a box th for 3D) — only FL3DIND/FL3DACT
            // *boxes* keep the bevel-instead-of-border path (th=0).
            let th = if ob_type == G_BUTTON {
                let mut bt: i16 = -1;
                if ob_flags & EXIT != 0 {
                    bt -= 1;
                }
                if ob_flags & DEFAULT != 0 {
                    bt -= 1;
                }
                bt
            } else if d3obj {
                0 // FL3DIND/FL3DACT box types use a bevel (drawn after), not a border
            } else {
                ((ob_spec >> 16) as u8 as i8) as i16
            };
            // 3D colour override (EmuTOS): a hollow-white box/button takes a
            // solid 3D face colour so the bevel reads against it.
            if ob_type != G_IBOX && ipat == IP_HOLLOW && icol == WHITE && d3 != 0 {
                ipat = IP_SOLID;
                icol = match d3 {
                    FL3DACT => ACT_BUTCOL.load(Ordering::Relaxed) as Pel,
                    FL3DIND => IND_BUTCOL.load(Ordering::Relaxed) as Pel,
                    _ => BACKGR_COL.load(Ordering::Relaxed) as Pel, // FL3DBAK
                };
            }
            // SELECTED, but only when the object changes colour (EmuTOS
            // `changecol`): a hollow box goes solid black, else xor the
            // interior. A 3D activator (actcolchange = 0) keeps its face —
            // the bevel alone shows the press.
            if selected && changecol {
                if ipat == IP_HOLLOW {
                    ipat = IP_SOLID;
                    icol = BLACK;
                } else {
                    icol = xor_color(icol);
                }
            }
            // D-089 signed thickness: `th > 0` draws the border INSIDE the rect
            // (the interior fill is inset by `th`); `th < 0` draws it OUTSIDE
            // (the fill keeps the full rect, the border is an add-on around it);
            // `th == 0` = no border. `inset` is EmuTOS `tmpth = (th<0)?0:th`.
            let inset = th.max(0);
            if th != 0 {
                draw_signed_border(backend, x, y, w, h, th, bcol, clip);
            }
            if ob_type != G_IBOX {
                gr_rect(backend, x + inset, y + inset, w - 2 * inset, h - 2 * inset, icol, ipat, clip);
            }
            deco_bcol = bcol;
            deco_th = th;
            // Text colour, xor'd when selected *and* the object changes colour.
            let ink = if selected && changecol { xor_color(tcol) } else { tcol };
            // `movetext` (EmuTOS): an activator's label sits 1px up-left while
            // raised and drops to the base position when pressed.
            let (tx, ty) = if movetext && !selected { (x - 1, y - 1) } else { (x, y) };
            if ob_type == G_BOXCHAR {
                let ch = ((ob_spec >> 24) & 0xFF) as u8;
                if ch != 0 {
                    draw_text(backend, tx, ty, w, h, &[ch], TE_CNTR, ink, clip);
                }
            } else if ob_type == G_BUTTON {
                let s = read_cstr(ob_spec);
                draw_text(backend, tx, ty, w, h, s, TE_CNTR, ink, clip);
            }
        }
        G_STRING => {
            // Menu items / plain strings. SELECTED inverts (menu
            // highlight); CHECKED draws a check glyph over the leading blank.
            if selected {
                backend.fill_rect(x, y, w, h, BLACK, clip);
            }
            if ob_state & CHECKED != 0 {
                // EmuTOS draws the CHECKED mark at `t.g_x+2` — 2 *pixels*, not
                // 2 cells — over the string's leading blank (gemoblib.c:903-908).
                // Vertically centred to align with `draw_text`'s baseline
                // (identical to EmuTOS's `t.g_y` when the cell height equals the
                // font height, which is the menu-item case).
                backend.draw_glyph(
                    x + 2,
                    y + (h - crate::vdi::font::CELL_H) / 2,
                    crate::vdi::font::glyph(0x08), // ST-font check mark
                    ink,
                    WriteMode::Transparent,
                    clip,
                );
            }
            let s = read_cstr(ob_spec);
            // EmuTOS draws a G_STRING at `t.g_x` with no code indent
            // (gemoblib.c:817, `tmpx = t.g_x`). A menu item's check column comes
            // from the leading blanks built into its resource string (GEM uses
            // two leading spaces), not a code-added indent — so plain dialog
            // labels sit flush at `ob_x` and menu items reserve their column via
            // the string itself.
            draw_text(backend, x, y, w, h, s, TE_LEFT, ink, clip);
        }
        G_TITLE => {
            // EmuTOS draws a G_TITLE through the shared string block at `t.g_x`
            // with no indent (gemoblib.c:801-846) and no box; the menu-dropped
            // highlight is a whole-rect XOR (gemoblib.c:932-943,
            // `xor_is_ok(G_TITLE) == TRUE`). NewTOS toggles a title's SELECTED
            // state by *redrawing* it (`object::change`, not an XOR toggle), so
            // the un-highlight redraw must clear the prior BLACK highlight —
            // hence the state-driven fill here (BLACK highlight when dropped,
            // WHITE clear otherwise) is the b/w equivalent of EmuTOS's XOR and
            // is load-bearing for `close_box` / `menu_tnormal` in `aes/menu.rs`
            // (removing the un-selected fill would leave black-on-black). The
            // 1-cell text indent, by contrast, was pure divergence and is
            // dropped so titles sit at `ob_x` like EmuTOS.
            backend.fill_rect(x, y, w, h, fill, clip);
            let s = read_cstr(ob_spec);
            draw_text(backend, x, y, w, h, s, TE_LEFT, ink, clip);
        }
        G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => {
            // The TEDINFO interior is read at the tree's ABI width, so an ILP32
            // app's text/box draws too; for an LP64 tree the reader reproduces
            // the native offsets byte-for-byte (rendering unchanged).
            if ob_spec != 0 {
                // SAFETY: ob_spec of a *TEXT object points at a TEDINFO packed
                // at the tree's ABI width.
                let ted = unsafe { TedInfo::read_abi(ob_spec, t.abi()) };
                let ted = &ted;
                // Crack the TEDINFO colour word (EmuTOS `gr_crack`): border,
                // text, inside pattern, inside colour.
                let (bcol, tcol, mut ipat, mut icol, _mode) = gr_crack(ted.te_color as u16);
                let mut is_box = ob_type == G_BOXTEXT || ob_type == G_FBOXTEXT;
                let mut th = if d3 != 0 { 0 } else { ted.te_thickness };
                // 3D background (EmuTOS): a hollow-white text background takes
                // the solid 3D background colour; a plain TEXT/FTEXT becomes a
                // BOXTEXT/FBOXTEXT with zero border so the fill is drawn.
                if d3 != 0 && ipat == IP_HOLLOW && icol == WHITE {
                    ipat = IP_SOLID;
                    icol = BACKGR_COL.load(Ordering::Relaxed) as Pel;
                    is_box = true;
                    th = 0;
                }
                // SELECTED (EmuTOS `changecol`): xor the interior + text.
                if selected {
                    if ipat == IP_HOLLOW {
                        ipat = IP_SOLID;
                        icol = BLACK;
                    } else {
                        icol = xor_color(icol);
                    }
                }
                // D-089 signed thickness (see the box arm above): positive =
                // inside border (fill/text inset by `inset`), negative = outside
                // border (fill/text keep the full rect).
                let inset = th.max(0);
                if is_box {
                    if th != 0 {
                        draw_signed_border(backend, x, y, w, h, th, bcol, clip);
                    }
                    gr_rect(backend, x + inset, y + inset, w - 2 * inset, h - 2 * inset, icol, ipat, clip);
                }
                deco_bcol = bcol;
                deco_th = th;
                // Format the string: the F-variants merge `te_ptext` into the
                // `te_ptmplt` template (EmuTOS `ob_format`); the plain variants
                // draw `te_ptext` directly.
                let raw = read_cstr(ted.te_ptext);
                let mut fmtbuf = [0u8; MAX_LEN];
                let s: &[u8] = if ob_type == G_FTEXT || ob_type == G_FBOXTEXT {
                    let tmpl = read_cstr(ted.te_ptmplt);
                    let n = ob_format(ted.te_just, raw, tmpl, &mut fmtbuf);
                    &fmtbuf[..n]
                } else {
                    raw
                };
                // Text colour from te_color, xor'd when SELECTED (EmuTOS).
                let ink = if selected { xor_color(tcol) } else { tcol };
                // Text sits inside the box, shrunk by a positive border only.
                draw_text(backend, x + inset, y + inset, w - 2 * inset, h - 2 * inset, s, ted.te_just, ink, clip);
            }
        }
        G_IMAGE => {
            // The BITBLK interior is read at the tree's ABI width (see the *TEXT
            // arm); an LP64 tree reads it byte-identically to before.
            if ob_spec != 0 {
                // SAFETY: ob_spec of a G_IMAGE points at a BITBLK packed at the
                // tree's ABI width.
                let bi = unsafe { BitBlk::read_abi(ob_spec, t.abi()) };
                let bi = &bi;
                // SELECTED (EmuTOS just_draw, gemoblib.c:766-770): XOR the image
                // background (the blit is transparent, so the inverted ground
                // shows through the clear bits) and invert the image ink.
                let col = if selected {
                    let pat = PatternFill {
                        fg: BLACK,
                        bg: WHITE,
                        pattern: &IP_PATTERNS[IP_SOLID as usize],
                        mode: WriteMode::Xor,
                    };
                    backend.fill_pattern(x, y, w, h, pat, clip);
                    xor_color(bi.bi_color as Pel)
                } else {
                    bi.bi_color as Pel
                };
                blit_image(backend, bi, x, y, col, clip);
            }
        }
        G_ICON => {
            // The ICONBLK interior is read at the tree's ABI width (see the
            // *TEXT arm); an LP64 tree reads it byte-identically to before.
            if ob_spec != 0 {
                // SAFETY: ob_spec of a G_ICON points at an ICONBLK packed at
                // the tree's ABI width.
                let ib = unsafe { IconBlk::read_abi(ob_spec, t.abi()) };
                draw_icon(backend, &ib, x, y, selected, clip);
            }
        }
        // Remaining types (TEDINFO text, colour icons, userdef) land in
        // later batches.
        _ => {}
    }

    // ── ob_state decorations ────────────────────────────────────────────
    // EmuTOS `just_draw` draws these after the type body, in the order
    // OUTLINED → SHADOWED → CHECKED → CROSSED → DISABLED → SELECTED, then the
    // 3D bevel last (gemoblib.c:867-952). CHECKED is drawn inline in the
    // G_STRING arm (the only checked object type NewTOS renders); the trailing
    // SELECTED XOR is folded into each type's inline colour inversion (EmuTOS's
    // `changecol` path) for the black/white palette.

    // D3: EmuTOS draws NO extra frame for DEFAULT — `just_draw` never reads
    // the DEFAULT flag; a default button's heavier look comes entirely from
    // its thicker negative `th` (D4, gemobjop.c:63-64). The previously-invented
    // double frame 3–4 px out is removed. OUTLINED is a real EmuTOS decoration
    // (gemoblib.c:869-887): an OUTLINED FL3DBAK background gets the nested 3D
    // outline (`draw_3d_outline`, gemoblib.c:875-877), every other type gets the
    // classic GEM double frame — a black line 3 px out plus a 2 px white ring —
    // so the object's own border becomes the inner line (gemoblib.c:882-885).
    if ob_state & OUTLINED != 0 {
        if d3 == FL3DBAK {
            // D7 (N3): flat-panel 3D outline, not the flat double frame.
            draw_3d_outline(backend, x, y, w, h, clip);
        } else {
            draw_border(backend, x - 3, y - 3, w + 6, h + 6, 1, BLACK, clip);
            draw_border(backend, x - 2, y - 2, w + 4, h + 4, 2, WHITE, clip);
        }
    }

    // SHADOWED: a solid drop shadow to the lower-right, sized by |th| and drawn
    // in the border colour (EmuTOS gemoblib.c:889-901). A th==0 object casts no
    // shadow (the shadow size derives from the border thickness). For a positive
    // (inside) border the shadow hugs the inset content rect; for a negative
    // (outside) border it hugs the full rect.
    if ob_state & SHADOWED != 0 {
        let sth = deco_th.abs();
        if sth != 0 {
            let (sx, sy, sw, sh) = if deco_th > 0 {
                (x + deco_th, y + deco_th, w - 2 * deco_th, h - 2 * deco_th)
            } else {
                (x, y, w, h)
            };
            backend.fill_rect(sx, sy + sh + sth, sw + sth, 2 * sth, deco_bcol, clip);
            backend.fill_rect(sx + sw + sth, sy, 2 * sth, sh + 3 * sth, deco_bcol, clip);
        }
    }

    // CROSSED: two WHITE diagonals across the object (EmuTOS gemoblib.c:910-915,
    // transparent) — visible over a filled/selected box.
    if ob_state & CROSSED != 0 {
        backend.draw_line(x, y, x + w - 1, y + h - 1, WHITE, WriteMode::Transparent, clip, 0xFFFF);
        backend.draw_line(x, y + h - 1, x + w - 1, y, WHITE, WriteMode::Transparent, clip, 0xFFFF);
    }

    // DISABLED: grey the object out with a 50 % transparent dither (EmuTOS
    // `bb_fill(IP_4PATT)`), over the 3D-background colour for a 3D panel.
    if disabled {
        let fg = if d3 == FL3DBAK { GRAY_LT } else { WHITE };
        let pat = PatternFill { fg, bg: WHITE, pattern: &STIPPLE_50, mode: WriteMode::Transparent };
        backend.fill_pattern(x, y, w, h, pat, clip);
    }

    // 3D bevel LAST (EmuTOS gemoblib.c:950-951 calls add_3d_effect after the
    // whole state block, gated on `flags & FL3DOBJ`): a raised activator (unless
    // pressed), a recessed indicator. D7: FL3DBAK is NOT an FL3DOBJ, so it gets
    // NO button bevel — its 3D optic is the draw_3d_outline recess above.
    match d3 {
        FL3DACT => add_3d_effect(backend, x, y, w, h, !selected, clip),
        FL3DIND => add_3d_effect(backend, x, y, w, h, false, clip),
        _ => {} // FL3DBAK (flat panel) and non-3D: no bevel
    }
}

/// Draw a 2-pixel bevel around a rectangle for the 3D look: `raised` gives
/// a white top/left highlight and a dark-grey bottom/right shadow;
/// `!raised` (recessed) swaps them. EmuTOS `add_3d_effect`.
fn add_3d_effect(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, raised: bool, clip: ClipRect) {
    if w < 2 || h < 2 {
        return;
    }
    let (tl, br) = if raised { (WHITE, GRAY_DK) } else { (GRAY_DK, WHITE) };
    for i in 0..2 {
        // top + left = highlight; bottom + right = shadow
        backend.fill_rect(x + i, y + i, w - 2 * i, 1, tl, clip);
        backend.fill_rect(x + i, y + i, 1, h - 2 * i, tl, clip);
        backend.fill_rect(x + i, y + h - 1 - i, w - 2 * i, 1, br, clip);
        backend.fill_rect(x + w - 1 - i, y + i, 1, h - 2 * i, br, clip);
    }
}

/// Draw the EmuTOS 3D outline for an OUTLINED FL3DBAK background panel
/// (`draw_3d_outline`, gemoblib.c:153-214). It is a 3-pixel nested frame sitting
/// 1..3 px *outside* the rect (offsets +3, +2, +1). The bottom+right edges are
/// all grey (EmuTOS LBLACK = [`GRAY_DK`]); the top+left edges are grey on the
/// outermost line (offset +3) and white on the two inner lines (+2, +1) — a
/// raised-panel optic distinct from both the button bevel ([`add_3d_effect`])
/// and the classic OUTLINED black+white double frame. All draws are opaque
/// ([`WriteMode::Replace`], EmuTOS `MD_REPLACE`) with a solid line mask, and the
/// top-left group is drawn after the bottom-right group so shared corner pixels
/// take the later colour exactly as EmuTOS's `v_pline` order does.
fn draw_3d_outline(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, clip: ClipRect) {
    if w < 1 || h < 1 {
        return;
    }
    let mode = WriteMode::Replace;
    let solid = 0xFFFF;
    // Bottom-left -> bottom-right -> top-right: 3 grey nested L-shapes.
    for d in [3i16, 2, 1] {
        let bl_x = x - d;
        let br_x = x + w - 1 + d;
        let bot_y = y + h - 1 + d;
        let top_y = y - d;
        backend.draw_line(bl_x, bot_y, br_x, bot_y, GRAY_DK, mode, clip, solid); // bottom
        backend.draw_line(br_x, bot_y, br_x, top_y, GRAY_DK, mode, clip, solid); // right
    }
    // Bottom-left -> top-left -> top-right: outermost grey, inner two white.
    for d in [3i16, 2, 1] {
        let side_x = x - d;
        let bot_y = y + h - 1 + d;
        let top_y = y - d;
        let tr_x = x + w - 1 + d;
        let col = if d == 3 { GRAY_DK } else { WHITE };
        backend.draw_line(side_x, bot_y, side_x, top_y, col, mode, clip, solid); // left
        backend.draw_line(side_x, top_y, tr_x, top_y, col, mode, clip, solid); // top
    }
}

/// Draw a `t`-pixel border frame around `(x, y, w, h)` as four strips.
fn draw_border(
    backend: &dyn RasterBackend,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    t: i16,
    col: Pel,
    clip: ClipRect,
) {
    let t = t.max(1);
    backend.fill_rect(x, y, w, t, col, clip); // top
    backend.fill_rect(x, y + h - t, w, t, col, clip); // bottom
    backend.fill_rect(x, y, t, h, col, clip); // left
    backend.fill_rect(x + w - t, y, t, h, col, clip); // right
}

/// Draw a box border honouring the D-089 signed-thickness convention: a
/// positive `th` draws the frame INSIDE `(x, y, w, h)` (EmuTOS `gr_box` with
/// `th > 0` shrinks the nested outlines inward), a negative `th` draws it
/// OUTSIDE (the frame sits in the |th|-pixel margin around the rect, leaving the
/// rect itself for the fill — matching `gr_box`'s outward-growing outlines once
/// the full-rect fill overpaints the shared edge). `th == 0` draws nothing.
///
/// D5: a negative `th` draws **`|th|+1`** pixels, **edge-inclusive** — EmuTOS
/// `gr_box` (gemgraf.c:813-832) does `if (th < 0) th--;` then draws one nested
/// outline per step up to and including `gr_inside(0)`, i.e. the innermost ring
/// lands on the rect's own edge. So `th = -1` is a 2 px frame spanning `x-1..x`,
/// not a single `x-1` ring. Because the border is drawn before the interior
/// fill (see the box/button arm), a *solid* box's fill overpaints that inner
/// (edge) ring, leaving the visible border unchanged; only HOLLOW interiors
/// (buttons, hollow boxes) show the extra edge pixel.
#[allow(clippy::too_many_arguments)]
fn draw_signed_border(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, th: i16, col: Pel, clip: ClipRect) {
    match th.cmp(&0) {
        core::cmp::Ordering::Greater => draw_border(backend, x, y, w, h, th, col, clip),
        core::cmp::Ordering::Less => {
            let t = -th;
            draw_border(backend, x - t, y - t, w + 2 * t, h + 2 * t, t + 1, col, clip);
        }
        core::cmp::Ordering::Equal => {}
    }
}

/// Draw `text` justified inside `(x, y, w, h)`, vertically centred, one
/// 8×16 glyph per byte.
fn draw_text(
    backend: &dyn RasterBackend,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    text: &[u8],
    just: i16,
    fg: Pel,
    clip: ClipRect,
) {
    let len = text.len() as i16;
    let tw = len * crate::vdi::font::CELL_W;
    let tx = match just {
        TE_CNTR => x + (w - tw) / 2,
        TE_RIGHT => x + w - tw,
        _ => x, // TE_LEFT
    };
    let ty = y + (h - crate::vdi::font::CELL_H) / 2;
    // Confine the glyphs to the object's text box `(x, y, w, h)` by
    // intersecting the caller's clip with it. EmuTOS `gr_gicon` clips the
    // icon label to `ib_wtext`; without this a string wider than its box
    // (e.g. a 12-char 8.3 name in a narrower icon cell) bleeds past the box
    // into neighbouring objects instead of truncating at the box edge.
    let box_clip = ClipRect {
        xmn: clip.xmn.max(x),
        ymn: clip.ymn.max(y),
        xmx: clip.xmx.min(x + w - 1),
        ymx: clip.ymx.min(y + h - 1),
    };
    for (i, &c) in text.iter().enumerate() {
        let gx = tx + i as i16 * crate::vdi::font::CELL_W;
        backend.draw_glyph(gx, ty, crate::vdi::font::glyph(c as u16), fg, WriteMode::Transparent, box_clip);
    }
}

/// Read a NUL-terminated C string from a native pointer in `ob_spec`
/// (trusted GEM pointer, like the other AES managers). Capped at 256.
fn read_cstr(spec: u64) -> &'static [u8] {
    if spec == 0 {
        return &[];
    }
    let p = spec as *const u8;
    // SAFETY: GEM contract — `ob_spec` of a string/button is a valid
    // pointer to a NUL-terminated string in the caller's memory.
    unsafe {
        let mut n = 0usize;
        while n < 256 && *p.add(n) != 0 {
            n += 1;
        }
        core::slice::from_raw_parts(p, n)
    }
}

/// Blit a `G_IMAGE` BITBLK at `(x, y)`: a transparent mono blit in colour
/// `col` (set bits drawn, clear bits leave the background). `col` is normally
/// `bi_color`, or its complement for a SELECTED image (EmuTOS `just_draw`).
fn blit_image(backend: &dyn RasterBackend, bi: &BitBlk, x: i16, y: i16, col: Pel, clip: ClipRect) {
    let stride = (bi.bi_wb / 2).max(1) as u16; // bytes → 16-bit words
    blit_plane(
        backend,
        bi.bi_pdata,
        stride,
        bi.bi_x,
        bi.bi_y,
        Rect { x, y, w: bi.bi_wb * 8, h: bi.bi_hl },
        col,
        clip,
    );
}

/// Draw a `G_ICON` ICONBLK at object origin `(ox, oy)`: the mask plane in
/// the background colour, the data plane in the foreground colour, then
/// the character and text. SELECTED swaps fg/bg (EmuTOS `gr_gicon`).
fn draw_icon(
    backend: &dyn RasterBackend,
    ib: &IconBlk,
    ox: i16,
    oy: i16,
    selected: bool,
    clip: ClipRect,
) {
    let chword = ib.ib_char;
    let mut fg = ((chword >> 12) & 0x0f) as Pel;
    let mut bg = ((chword >> 8) & 0x0f) as Pel;
    if selected {
        core::mem::swap(&mut fg, &mut bg);
    }
    let stride = (ib.ib_wicon / 16).max(1) as u16; // icon width in words
    let rect = Rect {
        x: ox + ib.ib_xicon,
        y: oy + ib.ib_yicon,
        w: ib.ib_wicon,
        h: ib.ib_hicon,
    };
    blit_plane(backend, ib.ib_pmask, stride, 0, 0, rect, bg, clip);
    blit_plane(backend, ib.ib_pdata, stride, 0, 0, rect, fg, clip);

    let c = (chword & 0xff) as u8;
    if c != 0 {
        backend.draw_glyph(
            ox + ib.ib_xchar,
            oy + ib.ib_ychar,
            crate::vdi::font::glyph(c as u16),
            fg,
            WriteMode::Transparent,
            clip,
        );
    }
    if ib.ib_ptext != 0 {
        let s = read_cstr(ib.ib_ptext);
        if !s.is_empty() {
            // Label background (EmuTOS `gr_gicon`): a solid `bg`-colour
            // rectangle behind the label so it reads against a patterned
            // desktop, and a SELECTED icon's label shows the swapped
            // (inverted) background. Then the label text in `fg`.
            gr_rect(
                backend,
                ox + ib.ib_xtext,
                oy + ib.ib_ytext,
                ib.ib_wtext,
                ib.ib_htext,
                bg,
                IP_SOLID,
                clip,
            );
            draw_text(
                backend,
                ox + ib.ib_xtext,
                oy + ib.ib_ytext,
                ib.ib_wtext,
                ib.ib_htext,
                s,
                TE_CNTR,
                fg,
                clip,
            );
        }
    }
}

/// Transparent mono blit of one bit-plane (`addr` → big-endian 16-bit
/// words, `stride` words per row) into `dst`, set bits drawn in `fg`.
fn blit_plane(
    backend: &dyn RasterBackend,
    addr: u64,
    stride: u16,
    src_x: i16,
    src_y: i16,
    dst: Rect,
    fg: Pel,
    clip: ClipRect,
) {
    if addr == 0 || dst.w <= 0 || dst.h <= 0 {
        return;
    }
    let rows = src_y.max(0) as usize + dst.h as usize;
    let total = stride as usize * rows;
    // SAFETY: GEM contract — `addr` points at >= `total` raster words.
    let bits = unsafe { core::slice::from_raw_parts(addr as *const u16, total) };
    backend.blit_mono(
        dst,
        MonoSrc {
            bits,
            stride_words: stride,
            x: src_x,
            y: src_y,
        },
        fg,
        WHITE,
        // Transparent (former `opaque = false`): set bits in `fg`, clear
        // bits leave the background — the desktop icon/image blit path.
        WriteMode::Transparent,
        clip,
    );
}

/// Recursive hit-test for `objc_find`. Returns the deepest matching child
/// index, else `obj` if `(mx, my)` is inside it, else `NIL`.
fn find_hit(
    t: ObjTree,
    obj: i16,
    depth: i16,
    px: i16,
    py: i16,
    mx: i16,
    my: i16,
) -> i16 {
    let ob_flags = t.flags(obj);
    if ob_flags & HIDETREE != 0 {
        return NIL;
    }
    let ob_state = t.state(obj);
    let x = px + t.x(obj);
    let y = py + t.y(obj);
    // Hit-test the same extent `just_draw` renders. D7: EmuTOS grows the hit
    // rect only for an FL3DOBJ object (`ob_actxywh`, gemoblib.c:322-335 keys on
    // `flags & FL3DOBJ`), which is set on FL3DIND/FL3DACT but CLEAR on FL3DBAK —
    // so an FL3DBAK panel is hit-tested at its un-grown rect, in lockstep with
    // the draw path (which no longer grows it either). An FL3DOBJ object is drawn
    // ADJ3DSTD px larger on every side, plus ADJ3DOUT when OUTLINED, so a click
    // on the visible bevel/outline edge must count as inside. SHADOWED objects
    // are the documented exception: EmuTOS `ob_find` (gemoblib.c:1063-1072)
    // hit-tests them at their un-grown relative rect. Children keep the nominal
    // origin `(x, y)`, exactly as `draw_tree` passes it — draw and hit stay
    // consistent at every level.
    let (hx, hy, hw, hh) = if ob_flags & FL3DOBJ != 0 && ob_state & SHADOWED == 0 {
        let mut adj = ADJ3DSTD;
        if ob_state & OUTLINED != 0 {
            adj += ADJ3DOUT;
        }
        (x - adj, y - adj, t.width(obj) + 2 * adj, t.height(obj) + 2 * adj)
    } else {
        (x, y, t.width(obj), t.height(obj))
    };
    let inside = mx >= hx && mx < hx + hw && my >= hy && my < hy + hh;
    if !inside {
        return NIL;
    }

    if depth > 0 {
        // Search children last-to-first (topmost sibling wins).
        let mut found = NIL;
        let mut child = t.head(obj);
        while child != NIL {
            let hit = find_hit(t, child, depth - 1, x, y, mx, my);
            if hit != NIL {
                found = hit; // keep the last (topmost) match
            }
            if child == t.tail(obj) {
                break;
            }
            child = t.next(child);
        }
        if found != NIL {
            return found;
        }
    }

    // Hit on `obj` itself. Per the spec, `objc_find`/`find_hit` does **not**
    // skip DISABLED objects — it checks only `inside()` and `!HIDETREE`
    // (`gemoblib.c:1080`). A DISABLED object under the point is still
    // returned; ignoring a click on a disabled object is the Form Manager's
    // job (`form_do`/`fm_do` filters DISABLED itself, `gemfmlib.c:224`). See
    // `docs/api/aes/object-form.md` ("objc_find returns DISABLED objects").
    obj
}

// Keep LASTOB referenced (used by tree-building callers / future batches).
const _: u16 = LASTOB;
