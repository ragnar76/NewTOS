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
    /// Tree origin: `true` for a **kernel-built** tree ([`Self::lp64`] — the
    /// desktop's `g_screen`, kernel-built alerts/fileselector), `false` for an
    /// **app-provided** tree ([`Self::new`], reached through the AES trap). This
    /// is the privilege discriminator for a `G_USERDEF`'s `ub_code` callback:
    /// a kernel-built tree's callback is an EL1 function at this code's privilege
    /// (same-privilege call); an app tree's callback lives in the EL0 app and
    /// must not be up-called from the EL1 AES handler. It is **not** derivable
    /// from `abi`: a native LP64 app runs at EL0 yet has `abi == Lp64`, so the
    /// origin must be tracked explicitly (Memory `aes-object-abi-is-tree-abi`).
    kernel_built: bool,
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
        Self { base: base as u64, abi: current_abi(), kernel_built: false }
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
        Self { base: base as u64, abi: Abi::Lp64, kernel_built: true }
    }

    /// Build an accessor over an **app-provided** tree at `base`, reading fields
    /// at an explicitly supplied `abi` width (captured earlier, **not** from the
    /// current execution context) and with `kernel_built = false`.
    ///
    /// This is for a tree an EL0 GEM app hands the AES to draw **later**, when
    /// the registering app may no longer be the current process — specifically
    /// the desktop-background tree an app registers via
    /// `wind_set(DESKWH, WF_NEWDESK, tree)` ([`crate::aes::window`]). Its ABI is
    /// captured at registration (the app was current then) and replayed here;
    /// and because the tree lives in EL0 app memory, a `G_USERDEF`'s `ub_code`
    /// must be treated as an EL0 callback (`kernel_built = false` → deferred by
    /// [`ob_user`], never called at EL1). [`Self::new`] would instead read
    /// `current_abi()` at build time (wrong when the app is not current), and
    /// [`Self::lp64`] would wrongly mark the app tree kernel-built (the
    /// WF_NEWDESK-background crash: TeraDesk's `ub_bckbox` called at EL1 → PXN
    /// instruction abort).
    ///
    /// # Invariant
    /// As [`Self::new`]: `base` addresses a valid OBJECT array (at `abi` width)
    /// and every index passed to a getter/setter is in range.
    #[inline]
    pub(crate) fn app_at(base: *const Object, abi: Abi) -> Self {
        Self { base: base as u64, abi, kernel_built: false }
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

    /// `true` iff this is a **kernel-built** tree ([`Self::lp64`]) — the
    /// privilege discriminator for a `G_USERDEF` callback (see the field doc).
    #[inline]
    pub(crate) fn kernel_built(self) -> bool {
        self.kernel_built
    }

    /// The tree's OBJECT-array base VA (EmuTOS `OBJECT *tree`), used to fill a
    /// `PARMBLK`'s `pb_tree` for a `G_USERDEF` callback.
    #[inline]
    pub(crate) fn tree_ptr(self) -> u64 {
        self.base
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

    /// The raw `ob_spec` field read at the caller's width: a u64 for LP64, a u32
    /// zero-extended for ILP32. This is the value *before* INDIRECT resolution —
    /// for an `INDIRECT` object it is a pointer to the cell holding the real
    /// spec, not the spec itself. All consumers use [`Self::spec`], which
    /// resolves INDIRECT; this raw reader exists only for that resolution.
    #[inline]
    fn raw_spec(self, i: i16) -> u64 {
        let p = self.ptr(i, self.spec_off());
        // SAFETY: ObjTree invariant; ob_spec is 8 (LP64) / 4 (ILP32) bytes.
        unsafe {
            match self.abi {
                Abi::Lp64 => (p as *const u64).read_unaligned(),
                Abi::Ilp32 => (p as *const u32).read_unaligned() as u64,
            }
        }
    }

    /// `ob_spec` resolved through the `INDIRECT` flag (EmuTOS `ob_sst`,
    /// gemobjop.c:36-37; `ob_getsp`, gemobed.c:60-62). Normally the raw field:
    /// a user VA or a packed colour word; for a sub-struct type the base VA of
    /// the TEDINFO/ICONBLK/BITBLK/USERBLK read at the tree's ABI width by the
    /// sub-struct readers ([`TedInfo::read_abi`] etc.).
    ///
    /// When `ob_flags & INDIRECT` is set the raw `ob_spec` is instead a pointer
    /// to a cell holding the real spec (`*pspec = *(LONG *)ob_spec` in EmuTOS);
    /// that cell is read here at the **tree's ABI width** — u64 for an LP64 tree,
    /// u32 zero-extended for an ILP32 app tree, exactly like a direct `ob_spec`
    /// read (Memory `aes-object-abi-is-tree-abi`: the indirect read follows the
    /// tree's ABI, not blanket LP64). Resolving centrally here means every
    /// consumer (`just_draw`, `change_t`, `edit_field_t`) draws/edits the true
    /// spec instead of dereferencing the pointer cell as a colour word / wild
    /// TEDINFO/ICONBLK.
    #[inline]
    pub(crate) fn spec(self, i: i16) -> u64 {
        let raw = self.raw_spec(i);
        if self.flags(i) & INDIRECT == 0 {
            return raw;
        }
        // INDIRECT: `raw` is the VA of a cell holding the real spec at tree width.
        // SAFETY: GEM contract — an INDIRECT object's ob_spec points at a
        // readable spec cell, 8 (LP64) / 4 (ILP32) bytes.
        unsafe {
            match self.abi {
                Abi::Lp64 => (raw as *const u64).read_unaligned(),
                Abi::Ilp32 => (raw as *const u32).read_unaligned() as u64,
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

    /// EmuTOS `ob_sst` border-thickness word `th` (gemobjop.c:41-70), used by
    /// `ob_center` (`form_center`) to size a SHADOWED root's drop-shadow extent.
    /// Reproduces the per-type `switch` for **every** object type, not just the
    /// box family:
    /// - `G_TITLE` → `1`
    /// - `G_TEXT`/`G_BOXTEXT`/`G_FTEXT`/`G_FBOXTEXT` → the TEDINFO `te_thickness`
    /// - `G_BOX`/`G_BOXCHAR`/`G_IBOX` → the signed thickness byte (bits 16..23 of
    ///   the INDIRECT-resolved colour LONG, EmuTOS `*(((char *)pspec)+1)`)
    /// - `G_BUTTON` → `-1`, one thinner for `EXIT`, one thinner for `DEFAULT`
    /// - any other type → `0`
    ///
    /// The trailing `if (th > 128) th -= 256` byte fixup is preserved.
    pub(crate) fn sst_th(self, obj: i16) -> i16 {
        let mut th: i16 = match self.ob_type(obj) & 0x00ff {
            G_TITLE => 1,
            G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => {
                let spec = self.spec(obj);
                if spec == 0 {
                    0
                } else {
                    // SAFETY: the ob_spec of a *TEXT object is a TEDINFO packed
                    // at the tree's ABI width (INDIRECT already resolved).
                    unsafe { TedInfo::read_abi(spec, self.abi).te_thickness }
                }
            }
            G_BOX | G_BOXCHAR | G_IBOX => ((self.spec(obj) >> 16) as u8 as i8) as i16,
            G_BUTTON => {
                let flags = self.flags(obj);
                let mut bt: i16 = -1;
                if flags & EXIT != 0 {
                    bt -= 1;
                }
                if flags & DEFAULT != 0 {
                    bt -= 1;
                }
                bt
            }
            _ => 0,
        };
        if th > 128 {
            th -= 256;
        }
        th
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
const G_CICON: u16 = 33; // colour icon: a CICONBLK that begins with a mono ICONBLK
const G_USERDEF: u16 = 24; // application-drawn object (USERBLK + ub_code callback)

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

/// Write a pointer field at `off` in `base` at the given ABI width (the inverse
/// of [`read_abi_ptr`]): an 8-byte store for LP64, the low 4 bytes for ILP32.
///
/// # Safety
/// `base + off` addresses a writable pointer field of the sub-struct, sized
/// 8 (LP64) / 4 (ILP32) bytes.
#[inline]
unsafe fn write_abi_ptr(base: u64, off: u64, abi: Abi, v: u64) {
    let p = (base + off) as *mut u8;
    // SAFETY: caller's contract — `base + off` is a writable pointer field.
    unsafe {
        match abi {
            Abi::Lp64 => (p as *mut u64).write_unaligned(v),
            Abi::Ilp32 => (p as *mut u32).write_unaligned(v as u32),
        }
    }
}

/// Write a WORD field at `off` in `base` (the inverse of [`read_abi_word`]).
///
/// # Safety
/// `base + off` addresses a writable 2-byte WORD field of the sub-struct.
#[inline]
unsafe fn write_abi_word(base: u64, off: u64, v: i16) {
    // SAFETY: caller's contract — `base + off` is a writable 2-byte WORD field.
    unsafe { ((base + off) as *mut i16).write_unaligned(v) }
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

/// `USERBLK` (`obdefs.h`) — the descriptor a `G_USERDEF` object's `ob_spec`
/// points at: a draw/change callback plus one opaque parameter LONG. Both
/// fields are read at the tree's ABI width (pointer-sized), so a kernel-built
/// LP64 USERBLK holds a native 64-bit `ub_code`.
#[derive(Clone, Copy)]
pub struct UserBlk {
    /// `WORD (*ub_code)(PARMBLK *)` — the user draw/change routine (native VA).
    pub ub_code: u64,
    /// `ub_parm` — an opaque parameter passed straight through in `pb_parm`.
    pub ub_parm: u64,
}

impl UserBlk {
    /// Materialise the USERBLK at `base` at the tree's ABI width: two
    /// pointer-sized fields (`ub_code` @0, `ub_parm` @`abi_ptr_size`).
    ///
    /// # Safety
    /// `base` is the valid `ob_spec` of a `G_USERDEF` object — a USERBLK packed
    /// at `abi` width.
    #[inline]
    unsafe fn read_abi(base: u64, abi: Abi) -> UserBlk {
        let ps = abi_ptr_size(abi);
        // SAFETY: `base` is a valid USERBLK packed at `abi` width (caller's contract).
        unsafe {
            UserBlk {
                ub_code: read_abi_ptr(base, 0, abi),
                ub_parm: read_abi_ptr(base, ps, abi),
            }
        }
    }
}

/// `PARMBLK` (`obdefs.h`) — the block passed by pointer to a `G_USERDEF`'s
/// `ub_code` callback. Native NewTOS layout (`#[repr(C)]`), built on the kernel
/// stack in [`ob_user`] and handed to a **kernel-built** (same-privilege, EL1)
/// callback. Field order matches EmuTOS: tree, object, prev/curr state, the
/// object rect (`pb_x..pb_h`), the current clip rect (`pb_xc..pb_hc`), and the
/// USERBLK's `ub_parm`.
#[repr(C)]
pub struct ParmBlk {
    /// `pb_tree` — the OBJECT-array base VA.
    pub pb_tree: u64,
    /// `pb_obj` — index of the userdef object.
    pub pb_obj: i16,
    /// `pb_prevstate` — the object's state before the change.
    pub pb_prevstate: i16,
    /// `pb_currstate` — the object's state after the change.
    pub pb_currstate: i16,
    /// object rect x (absolute).
    pub pb_x: i16,
    /// object rect y (absolute).
    pub pb_y: i16,
    /// object rect width.
    pub pb_w: i16,
    /// object rect height.
    pub pb_h: i16,
    /// clip rect x.
    pub pb_xc: i16,
    /// clip rect y.
    pub pb_yc: i16,
    /// clip rect width.
    pub pb_wc: i16,
    /// clip rect height.
    pub pb_hc: i16,
    /// `pb_parm` — the USERBLK's opaque parameter.
    pub pb_parm: u64,
}

impl ParmBlk {
    /// Write this PARMBLK into `dst` (a kernel VA aliasing the EL0 scratch
    /// region) at the tree's `abi` width, matching the app's C `PARMBLK`
    /// (`mt_gem.h`): `OBJECT *pb_tree; short pb_obj..pb_hc; long pb_parm;`.
    /// The pointer fields (`pb_tree`, `pb_parm`) are ABI-width (8 LP64 / 4
    /// ILP32); the eleven WORDs are ABI-neutral. `pb_parm` follows the WORD
    /// block at the next pointer-aligned offset — the natural C-struct
    /// padding (LP64: `pb_parm` @32 in a 40-byte struct; ILP32: @28 in a
    /// 32-byte struct). Mirror of [`UserBlk::read_abi`] / the `ObjTree`
    /// field-offset machinery (AES-as-thread design §4).
    ///
    /// # Safety
    /// `dst .. dst + 40` is a writable region (the reserved PARMBLK scratch
    /// aliased through the kernel high-half); `abi` is the tree's ABI.
    unsafe fn write_abi(&self, dst: u64, abi: Abi) {
        let ps = abi_ptr_size(abi); // 8 (LP64) / 4 (ILP32)
        let w = ps; // first WORD offset (pb_obj), just past the pointer
        // pb_parm follows the 11 WORDs (22 bytes), pointer-aligned.
        let parm_off = (w + 22).next_multiple_of(ps);
        // SAFETY: forwarded to the field writers' contract — `dst` backs the
        // widest (LP64, 40-byte) layout, so every field offset is in range.
        unsafe {
            write_abi_ptr(dst, 0, abi, self.pb_tree);
            write_abi_word(dst, w, self.pb_obj);
            write_abi_word(dst, w + 2, self.pb_prevstate);
            write_abi_word(dst, w + 4, self.pb_currstate);
            write_abi_word(dst, w + 6, self.pb_x);
            write_abi_word(dst, w + 8, self.pb_y);
            write_abi_word(dst, w + 10, self.pb_w);
            write_abi_word(dst, w + 12, self.pb_h);
            write_abi_word(dst, w + 14, self.pb_xc);
            write_abi_word(dst, w + 16, self.pb_yc);
            write_abi_word(dst, w + 18, self.pb_wc);
            write_abi_word(dst, w + 20, self.pb_hc);
            write_abi_ptr(dst, parm_off, abi, self.pb_parm);
        }
    }
}

// Flag / state bits.
const LASTOB: u16 = 0x0020;
const HIDETREE: u16 = 0x0080;
const INDIRECT: u16 = 0x0100; // ob_flags: ob_spec points at a cell holding the real spec
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

// AES font ids (`TEDINFO.te_font`, EmuTOS `obdefs.h:62-63`).
const FONT_IBM: i16 = 3;
const FONT_SMALL: i16 = 5;

/// The AES's two desktop font sizes. EmuTOS keeps both open at once
/// (`gsx_start`, `gemgraf.c:405-410`) and every text-drawing site names one:
/// `gsx_tblt(IBM, …)` / `gr_gtext(…, SMALL, …)`. The cell metrics are
/// EmuTOS's `gl_wchar`/`gl_hchar` and `gl_wschar`/`gl_hschar`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AesFont {
    /// Normal desktop font (`IBM` = 3): `gl_wchar` × `gl_hchar` = 8×16.
    Ibm,
    /// Small desktop font (`SMALL` = 5): `gl_wschar` × `gl_hschar` = 6×6.
    /// Icon labels and icon overlay characters are drawn in this size, as
    /// is any `TEDINFO` whose `te_font` says so.
    Small,
}

impl AesFont {
    /// Map a `TEDINFO.te_font` code.
    ///
    /// EmuTOS's `ch_width`/`ch_height` (`gemgraf.c:456-477`) return 0 for any
    /// code that is neither `IBM` nor `SMALL`, and `gsx_tcalc` immediately
    /// divides by that height (`gemgraf.c:494`) — so EmuTOS has no defined
    /// third font here, it has a division by zero. Everything that is not
    /// `SMALL` is therefore read as `IBM` ([`FONT_IBM`]), the code every GEM
    /// resource editor writes for the normal font. Deliberately **not**
    /// asserted: an application resource may legally carry a GDOS font id we
    /// have no face for, and answering that with a kernel panic would be worse
    /// than EmuTOS's own undefined behaviour.
    #[inline]
    fn from_te_font(te_font: i16) -> AesFont {
        match te_font {
            FONT_SMALL => AesFont::Small,
            FONT_IBM => AesFont::Ibm,
            _ => AesFont::Ibm,
        }
    }

    /// `ch_width` (`gemgraf.c:456-465`) — the cell advance in pixels.
    #[inline]
    fn cell_w(self) -> i16 {
        match self {
            AesFont::Ibm => crate::vdi::font::CELL_W,
            AesFont::Small => crate::vdi::font::SMALL_CELL_W,
        }
    }

    /// `ch_height` (`gemgraf.c:468-477`) — the cell height in pixels.
    #[inline]
    fn cell_h(self) -> i16 {
        match self {
            AesFont::Ibm => crate::vdi::font::CELL_H,
            AesFont::Small => crate::vdi::font::SMALL_CELL_H,
        }
    }
}

// ── dispatch entry points ───────────────────────────────────────────

/// Map an AES `int_in` clip quadruple (`OB_XCLIP..OB_HCLIP`, `int_in[2..5]`:
/// `cx`/`cy`/`cw`/`ch`) to a VDI clip rectangle, honouring the EmuTOS
/// **zero-clip convention** (`gsx_sclip`, gemgraf.c:88-104): a zero-sized
/// rectangle (`cw <= 0 || ch <= 0`) means *no clip* — EmuTOS calls
/// `vs_clip(FALSE)`, i.e. clipping is turned OFF and everything is drawn
/// unclipped — so this returns `None` and the caller substitutes the full
/// screen. A non-empty rectangle becomes `(cx, cy)..(cx+cw-1, cy+ch-1)`,
/// exactly EmuTOS's `ptsin` computation.
///
/// This is the fix for AUD-P3-013: the previous inline `ClipRect { xmx: cx+cw-1
/// }` turned `cw == 0` into an empty `xmx < xmn` rectangle that drew *nothing*,
/// inverting the documented "w/h == 0 = draw everything" convention. Shared by
/// `objc_draw` (through `render_tree_t`) and `objc_change` so both build the AES
/// clip identically (D-052 documents `int_in[2..5]` as the clip).
fn clip_from_intin(cx: i16, cy: i16, cw: i16, ch: i16) -> Option<ClipRect> {
    if cw <= 0 || ch <= 0 {
        None
    } else {
        Some(ClipRect { xmn: cx, ymn: cy, xmx: cx + cw - 1, ymx: cy + ch - 1 })
    }
}

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

/// As [`render_tree_lp64`], but for an **app-provided** background tree an EL0
/// GEM app registered via `wind_set(DESKWH, WF_NEWDESK, tree)`: read the tree at
/// the `abi` captured at registration and treat it as **not** kernel-built
/// ([`ObjTree::app_at`]), so a `G_USERDEF`'s `ub_code` is an EL0 callback that
/// [`ob_user`] defers instead of calling at EL1. This is the WF_NEWDESK-
/// background counterpart to the ILP32-app `objc_draw`-via-SVC path
/// ([`ObjTree::new`]): the same privilege guard, but with the ABI recorded at
/// registration because the composite runs when the app may not be current.
///
/// # Safety
/// `tree` must be a valid `OBJECT` array (at `abi` width) with `start` in range.
#[allow(clippy::too_many_arguments)]
pub unsafe fn render_tree_app(
    tree: *const Object,
    start: i16,
    depth: i16,
    cx: i16,
    cy: i16,
    cw: i16,
    ch: i16,
    abi: Abi,
    backend: &dyn RasterBackend,
) {
    if tree.is_null() || start < 0 {
        return;
    }
    render_tree_t(ObjTree::app_at(tree, abi), start, depth, cx, cy, cw, ch, backend);
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
    // AUD-P3-013 zero-clip convention: `cw`/`ch == 0` means "no clip" = the full
    // screen, not an empty rectangle (which drew nothing). For a real damaged
    // rectangle this is byte-identical to the previous inline build.
    let clip = clip_from_intin(cx, cy, cw, ch)
        .unwrap_or_else(|| ClipRect::full(backend.width(), backend.height()));
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
    // OB_XCLIP..OB_HCLIP (int_in[2..5]): EmuTOS sets exactly this VDI clip
    // before `ob_change` (gemsuper.c:228-230, `gsx_sclip((GRECT*)&OB_XCLIP)`),
    // so a partial (damaged-rectangle) redraw stays inside the caller's region
    // instead of spilling full-screen. Zero w/h = no clip = full screen, per the
    // `gsx_sclip` convention (AUD-P3-003; D-052 documents int_in[2..5] as clip).
    let clip = clip_from_intin(pb.intin(2), pb.intin(3), pb.intin(4), pb.intin(5));
    change_clip_t(ObjTree::new(tree), obj, newstate, redraw, clip);
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
/// `new` for an app tree, `lp64` for a kernel-built one). The redraw covers the
/// full screen — the behaviour every internal caller (menu/form highlight,
/// `graf_watchbox`) wants; `objc_change` uses [`change_clip_t`] directly to
/// honour its `int_in[2..5]` clip rectangle instead.
pub(crate) fn change_t(t: ObjTree, obj: i16, newstate: u16, redraw: bool) {
    change_clip_t(t, obj, newstate, redraw, None);
}

/// Body of [`change_t`]: set `obj`'s state and, if `redraw`, redraw just that
/// object within `clip_override` (`None` = the full screen, the internal
/// callers' behaviour; `objc_change` passes the AES `int_in[2..5]` clip).
fn change_clip_t(t: ObjTree, obj: i16, newstate: u16, redraw: bool, clip_override: Option<ClipRect>) {
    let spec = t.spec(obj);
    // EmuTOS `ob_change` early-out (gemoblib.c:1263): `if ((curr_state ==
    // new_state) || (spec == -1L)) return;`. An unchanged state or the `-1`
    // sentinel leaves the object entirely untouched — no state write, no redraw:
    // the redraw path never dereferences `-1`, and re-asserting a state the
    // object already has never flickers. AUD-P3-003 adds the previously-missing
    // `curr_state == new_state` half (the `spec == -1` half was already present).
    let curr_state = t.state(obj);
    if curr_state == newstate || spec == t.spec_none() {
        return;
    }
    t.set_state(obj, newstate);
    if !redraw {
        return;
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        // Zero-clip (`None`) = the full screen; otherwise the caller's rectangle.
        let clip = clip_override.unwrap_or_else(|| ClipRect::full(backend.width(), backend.height()));
        let (ox, oy) = abs_offset(t, obj);
        // AUD-P3-002: EmuTOS `ob_change` redraws a G_USERDEF by calling its user
        // routine with (prev=curr_state, curr=new_state) and suppresses the
        // generic object redraw (`redraw = FALSE`, gemoblib.c:1282-1284). The
        // dispatch is on the 0x00ff-masked type (EmuTOS `ob_sst`, gemobjop.c:40 —
        // AUD-P3-007). The callback is only invoked for a kernel-built tree (see
        // `ob_user`); an app tree's EL0 callback is deferred. A null `ob_spec`
        // (no USERBLK) is left to the generic redraw below so `ob_user` never
        // dereferences null.
        if t.ob_type(obj) & 0x00ff == G_USERDEF && spec != 0 {
            ob_user(t, obj, ox, oy, t.width(obj), t.height(obj), spec, clip, curr_state, newstate);
            return;
        }
        // depth 0 redraws only this object.
        draw_tree(t, obj, 0, ox - t.x(obj), oy - t.y(obj), clip, backend);
    }
}

// objc_edit kinds (`obdefs.h`).
const ED_START: i16 = 0;
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;
const ED_END: i16 = 3;

/// `objc_edit` (46) — edit the EDITABLE `TEDINFO` text field `obj`.
/// `int_in[1]` = character, `int_in[2]` = cursor index, `int_in[3]` =
/// kind (EDSTART/EDINIT/EDCHAR/EDEND). The new cursor index is returned
/// in `OB_ODX` (`int_out[1]`).
///
/// Handles `EDINIT` (cursor to end), `EDCHAR` (validated insert / backspace /
/// delete / arrows, with over-type at the length limit and template
/// skip-ahead for formatted fields) and `EDEND`; `EDSTART` is a no-op.
pub fn objc_edit(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let obj = pb.intin(0);
    let kind = pb.intin(3);
    if kind == ED_START || tree.is_null() || obj <= 0 {
        // EmuTOS `ob_edit` (gemobed.c:342-343): `if ((kind == EDSTART) ||
        // (obj <= 0)) return TRUE;` — a pure no-op with no draw and no
        // TEDINFO access, returning TRUE with the passed-in index unchanged
        // (AUD-P3-008). EDSTART begins an edit sequence without touching the
        // field; `obj <= 0` (incl. the ROOT, 0) has no TEDINFO (its `ob_spec`
        // is a colour/thickness word), so editing it would deref a wild
        // pointer. `int_out[1]` = the caller's `int_in[2]`.
        pb.set_intout(1, pb.intin(2));
        pb.set_ret(1);
        return;
    }
    let idx = edit_field(tree, obj, pb.intin(1), pb.intin(2), kind);
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
        ED_CHAR => edit_char(ted, tmpl, text, txtlen, &mut idx, in_char),
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
fn edit_char(ted: &TedInfo, tmpl: &[u8], text: *mut u8, txtlen: i16, idx: &mut i16, ch: i16) {
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
            // EmuTOS `ob_edit` EDCHAR `default` branch (gemobed.c:405-441):
            // validated printable insertion, over-typing the last position at
            // the length limit and, on a validation failure in a formatted
            // field, skipping the cursor ahead to the next data slot.
            //
            // `cur_pos` is the template column of the cursor (EmuTOS `start` =
            // `find_pos`), needed only by the skip-ahead scan.
            let mut cur_pos = find_pos(tmpl, *idx);
            let mut tmp_back = false;
            // Full field (`idx > te_txtlen-2`): back the cursor up one so the
            // insert over-types the last character rather than being dropped
            // (gemobed.c:407-413, AUD-P3-009).
            if *idx > txtlen - 2 {
                cur_pos -= 1;
                tmp_back = true;
                *idx -= 1;
            }
            let mut ic = c;
            if check(&mut ic, valid_code(pvalid, *idx)) {
                // `ins_char` truncates at `te_txtlen` (gemobed.c:420), so a
                // full field drops its tail instead of overflowing.
                ins_char(text, *idx, ic, txtlen);
                *idx += 1;
            } else {
                // Validation failed: try to skip ahead to the next template
                // data slot whose separator is the typed char, blanking the
                // skipped positions (gemobed.c:424-438, AUD-P3-010). For a
                // plain field (no validation) `check` always succeeds, so this
                // branch runs for formatted (`te_ptmplt`) fields only.
                if tmp_back {
                    *idx += 1;
                    cur_pos += 1;
                }
                let pos = scan_to_end(tmpl, cur_pos, *idx, c);
                if pos < txtlen - 2 {
                    fill_blanks(text, *idx, pos);
                    *idx = pos;
                }
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

/// Insert `chr` at `pos` in the NUL-terminated buffer, truncating at
/// `tot_len` (the TEDINFO `te_txtlen`, buffer size including the NUL).
/// Faithful to EmuTOS `ins_char` (gemobed.c:117): shifts the tail up and
/// drops the overflow when the field is full, so the caller need not guard
/// the length.
fn ins_char(text: *mut u8, pos: i16, chr: u8, tot_len: i16) {
    let len = cstr_len_p(text) as i16;
    // SAFETY: `pos <= len <= tot_len-1`; every write index is `<= tot_len-1`,
    // so all accesses stay inside the caller's `te_txtlen`-byte buffer.
    unsafe {
        let mut ii = len;
        while ii > pos {
            *text.add(ii as usize) = *text.add((ii - 1) as usize);
            ii -= 1;
        }
        *text.add(pos as usize) = chr;
        if len + 1 < tot_len {
            *text.add((len + 1) as usize) = 0;
        } else {
            *text.add((tot_len - 1) as usize) = 0;
        }
    }
}

/// Scan the template `tmpl` forward from column `start`, counting `_` data
/// slots into `idx`, until the typed separator `chr` is found (or the
/// template ends), returning the raw-string index of that data slot. Faithful
/// to EmuTOS `scan_to_end` (gemobed.c:141): lets a typed template separator
/// jump the cursor to the following data position.
fn scan_to_end(tmpl: &[u8], start: i16, mut idx: i16, chr: u8) -> i16 {
    let mut i = start.max(0) as usize;
    while i < tmpl.len() && tmpl[i] != chr {
        if tmpl[i] == b'_' {
            idx += 1;
        }
        i += 1;
    }
    idx
}

/// Blank out raw positions `[from, to)` and terminate at `to` (EmuTOS
/// `memset(&rawstr[idx], ' ', pos-idx); rawstr[pos] = '\0'`, gemobed.c:434-435).
fn fill_blanks(text: *mut u8, from: i16, to: i16) {
    // SAFETY: caller guarantees `from <= to < te_txtlen-2`, so writes stay in
    // the buffer.
    unsafe {
        let mut i = from;
        while i < to {
            *text.add(i as usize) = b' ';
            i += 1;
        }
        *text.add(to as usize) = 0;
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

/// Draw the subtree rooted at `obj`, `depth` levels deep, confined to `clip`.
/// Like [`draw_obj`] but with a caller-supplied clip rectangle: the Menu Manager
/// passes a clip equal to the drop-down box's grown save/restore area so nothing
/// is painted outside the saved region — the invariant *saved-area ⊇ drawn-area*
/// that makes the on-close restore exact even for a box with a `th<0` outward
/// border or a child positioned past the box edge. EmuTOS's decoration-aware
/// menu path does the same: `popup_blit` sets `gsx_sclip(&t)` over exactly the
/// grown save rectangle before drawing (`gemmnext.c:154`).
pub(crate) fn draw_obj_clip(
    tree: *const Object,
    obj: i16,
    depth: i16,
    clip: ClipRect,
    backend: &dyn RasterBackend,
) {
    let t = ObjTree::new(tree);
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
/// (EmuTOS `gr_rect`, `gemgraf.c:547-559`): SOLID a flat `icol` fill, the rest
/// an `icol`-on-white dither, and — crucially — HOLLOW an opaque **white** fill,
/// NOT a no-op.
///
/// All patterns (HOLLOW included) are drawn in **`WriteMode::Replace`** — EmuTOS
/// `gr_rect` always fills with `MD_REPLACE` (`bb_fill(MD_REPLACE, FIS_HOLLOW,
/// pattern=0x0000, …)`, `gemgraf.c:547-559`). For a hollow box that reaches
/// `vr_recfl`/the REPLACE fill (`vdi_line.c:528-548`) with the all-zero HOLLOW
/// pattern, `pattern & mask` is 0 for *every* pixel, so every pixel is written
/// `bgcol = palette[0] = WHITE` (`vdi_line.c:461`). A hollow G_BOX interior is
/// therefore filled OPAQUE WHITE, not left transparent — the fill colour is the
/// background (WHITE), INDEPENDENT of `icol`. A dither's *clear* bits are painted
/// the same white background. This is what makes a patterned box (the desktop
/// `DROOT` background, `IP_4PATT` green-on-white) or a hollow-white box (a
/// TeraDesk dropdown, `ob_spec=0x00ff1100` → IP_HOLLOW/WHITE) an opaque fill that
/// erases whatever it covers, rather than letting the underlying pixels show
/// through.
fn gr_rect(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, icol: Pel, ipat: u16, clip: ClipRect) {
    if w <= 0 || h <= 0 {
        return;
    }
    match ipat {
        // HOLLOW = REPLACE-mode fill with the all-zero pattern: every pixel is
        // painted the background colour WHITE (bgcol = palette[0]), NOT skipped.
        // The fill colour is the background WHITE, independent of `icol`.
        IP_HOLLOW => backend.fill_rect(x, y, w, h, WHITE, clip),
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

/// EmuTOS `ob_user` (`gemoblib.c:433-448`): load the `USERBLK` from `spec`,
/// fill a [`ParmBlk`] (tree, object, prev/curr state, the object rect, the clip
/// rect, `ub_parm`) and invoke the user draw/change callback `ub_code(PARMBLK*)`,
/// returning its WORD result as the object's new state.
///
/// **Privilege split (AUD-P3-002).** The callback is only *invoked* when the
/// tree is **kernel-built** ([`ObjTree::kernel_built`]) — then `ub_code` is an
/// EL1 function at this AES code's privilege, so a direct same-privilege call is
/// sound. For an **app-provided** tree the callback lives in the EL0 app;
/// calling it from the EL1 AES trap handler is a cross-privilege upcall (EL1→EL0,
/// re-entrant during the app's `svc`) for which NewTOS has no mechanism yet. In
/// that case the PARMBLK is fully prepared but the call is **deferred** and the
/// state returned unchanged — the upcall infrastructure is an open design
/// decision handed to the lead (same class as the vex-vector upcall).
#[allow(clippy::too_many_arguments)]
fn ob_user(
    t: ObjTree,
    obj: i16,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    spec: u64,
    clip: ClipRect,
    prev_state: u16,
    new_state: u16,
) -> u16 {
    // SAFETY: `spec` is the ob_spec of a G_USERDEF object — a USERBLK packed at
    // the tree's ABI width (GEM contract).
    let ub = unsafe { UserBlk::read_abi(spec, t.abi()) };
    let pb = ParmBlk {
        pb_tree: t.tree_ptr(),
        pb_obj: obj,
        pb_prevstate: prev_state as i16,
        pb_currstate: new_state as i16,
        pb_x: x,
        pb_y: y,
        pb_w: w,
        pb_h: h,
        pb_xc: clip.xmn,
        pb_yc: clip.ymn,
        pb_wc: clip.xmx - clip.xmn + 1,
        pb_hc: clip.ymx - clip.ymn + 1,
        pb_parm: ub.ub_parm,
    };
    if ub.ub_code == 0 {
        // No USERBLK draw routine — nothing to call; keep the state unchanged.
        new_state
    } else if t.kernel_built() {
        // Same-privilege EL1 callback: `ub_code` is a native
        // `extern "C" fn(*const ParmBlk) -> WORD`. Go through a data pointer
        // (not an integer) so no int-to-fn transmute lint fires.
        let code = ub.ub_code as *const ();
        // SAFETY: kernel-built USERBLK — `ub_code` is a valid EL1 function
        // pointer of this exact signature (the kernel/desktop that built the
        // tree is responsible for that contract).
        let f: extern "C" fn(*const ParmBlk) -> i16 = unsafe { core::mem::transmute(code) };
        f(&pb) as u16
    } else {
        // App-provided tree: `ub_code` lives in EL0 app memory. Synchronous
        // `G_USERDEF` callout via the AES fiber (AES-as-thread design §3.4/§5
        // step 2). Write the PARMBLK into the process's reserved EL0 scratch —
        // through the kernel high-half alias `basepage_kva + PARMBLK_SCRATCH_VA`,
        // at the tree's ABI width — then stall the fiber: the kernel runs
        // `ub_code(parmblk)` at EL0 (no PXN) and resumes us with the WORD result
        // (EmuTOS `state = ob_user(...)`, gemoblib.c:793). No live process slot
        // (e.g. an EL1 test caller reaching an app tree) falls back to the old
        // "keep the state" behaviour rather than dereferencing a null basepage.
        use crate::bdos::process::loader::{PARMBLK_SCRATCH_VA, USER_VA_BASE};
        use crate::bdos::process::{current, slot_for, PROCESS_TABLE};
        match slot_for(current()) {
            Some(slot) => {
                let bp = PROCESS_TABLE[slot]
                    .basepage_kva
                    .load(Ordering::Acquire) as u64;
                if bp == 0 {
                    new_state
                } else {
                    // SAFETY: `bp + PARMBLK_SCRATCH_VA` is the kernel high-half
                    // alias of the reserved scratch in the process's page 0,
                    // RW-mapped, ≥ 40 bytes; single-CPU under the trap.
                    unsafe { pb.write_abi(bp + PARMBLK_SCRATCH_VA, t.abi()); }
                    let parmblk_ptr = USER_VA_BASE + PARMBLK_SCRATCH_VA;
                    crate::aes::thread::block_for_userdef(
                        slot,
                        ub.ub_code,
                        parmblk_ptr,
                    ) as u16
                }
            }
            None => new_state,
        }
    }
}

fn just_draw(t: ObjTree, obj: i16, x: i16, y: i16, clip: ClipRect, backend: &dyn RasterBackend) {
    // Snapshot the object's fields once at the caller's ABI width; a sub-struct
    // (`ob_spec`-pointed TEDINFO/ICONBLK/BITBLK) is read through its ABI-aware
    // reader (`*::read_abi`) at the same width in the type arms below.
    // `ob_state` is `mut`: a G_USERDEF's callback returns the object's new state
    // (EmuTOS `state = ob_user(...)`, gemoblib.c:793), which then drives the
    // trailing OUTLINED/SHADOWED/CROSSED/DISABLED decorations. Every other type
    // leaves it unchanged, so their rendering is byte-identical to before.
    let mut ob_state = t.state(obj);
    let ob_flags = t.flags(obj);
    // AUD-P3-007: dispatch on the **base** type only. EmuTOS `ob_sst`
    // (gemobjop.c:40) masks `*ptype = objptr->ob_type & 0x00ff` before every
    // type switch — the high byte carries extended-type/flags (e.g. the
    // submenu-slot the Menu Manager packs into a G_STRING item's `ob_type`), so
    // an unmasked compare would drop any such object into the `_ => {}` arm and
    // leave it undrawn. The masked value also drives the G_USERDEF check below.
    let ob_type = t.ob_type(obj) & 0x00ff;
    let ob_spec = t.spec(obj);
    let ob_width = t.width(obj);
    let ob_height = t.height(obj);
    let selected = ob_state & SELECTED != 0;
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
            if ob_type == G_BOXCHAR {
                // A G_BOXCHAR is a TEDINFO-family draw: EmuTOS copies `t`,
                // insets it by `tmpth`, applies the 3D `movetext` nudge and
                // calls `gr_gtext(TE_CNTR, IBM, …)` with a one-character string
                // (gemoblib.c:719-757 — `edblk.te_font = IBM` is forced at :729).
                let ch = ((ob_spec >> 24) & 0xFF) as u8;
                if ch != 0 {
                    let (cx, cy) = ted_movetext(movetext, selected, x + inset, y + inset);
                    gr_gtext(
                        backend,
                        TE_CNTR,
                        AesFont::Ibm,
                        &[ch],
                        cx,
                        cy,
                        w - 2 * inset,
                        h - 2 * inset,
                        ink,
                        clip,
                    );
                }
            } else if ob_type == G_BUTTON {
                // A G_BUTTON's label is drawn by the shared string block
                // (gemoblib.c:801-846), **not** by `gr_gtext`: no clamping to
                // the box, no whole-character truncation, plain `n/2` centring
                // on the *uninset* rect, and its own `movetext` rule
                // (`tmpx--; tmpy--` while raised, gemoblib.c:833-836).
                let s = read_cstr(ob_spec);
                let len = s.len() as i16;
                let mut tmpx = x + (w - len * crate::vdi::font::CELL_W) / 2;
                let mut tmpy = y + (h - crate::vdi::font::CELL_H) / 2;
                if movetext && !selected {
                    tmpx -= 1;
                    tmpy -= 1;
                }
                gsx_tblt(backend, AesFont::Ibm, tmpx, tmpy, s, ink, clip);
            }
        }
        G_STRING => {
            // Menu items / plain strings. SELECTED inverts (menu
            // highlight); CHECKED draws a check glyph over the leading blank.
            if selected {
                backend.fill_rect(x, y, w, h, BLACK, clip);
            }
            if ob_state & CHECKED != 0 {
                // EmuTOS draws the CHECKED mark at `t.g_x+2, t.g_y` — 2
                // *pixels* to the right, no vertical centring — over the
                // string's leading blank, in the IBM font
                // (`gsx_tblt(IBM, t.g_x+2, t.g_y, 1)`, gemoblib.c:903-908).
                gsx_tblt(backend, AesFont::Ibm, x + 2, y, &[0x08], ink, clip);
            }
            let s = read_cstr(ob_spec);
            // EmuTOS draws a G_STRING through the shared string block at
            // `tmpx = t.g_x`, `tmpy = t.g_y + ((t.g_h-gl_hchar)/2)` in the IBM
            // font, with no clamping and no truncation (gemoblib.c:817-845) —
            // `gr_gtext`'s clamped/rounded path is *not* used for strings. A
            // menu item's check column comes from the leading blanks built into
            // its resource string (GEM uses two leading spaces), not a
            // code-added indent.
            let tmpy = y + (h - crate::vdi::font::CELL_H) / 2;
            gsx_tblt(backend, AesFont::Ibm, x, tmpy, s, ink, clip);
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
            // Same string block as G_STRING: IBM font, `t.g_x`, vertically
            // centred on `gl_hchar` (gemoblib.c:817-845).
            let tmpy = y + (h - crate::vdi::font::CELL_H) / 2;
            gsx_tblt(backend, AesFont::Ibm, x, tmpy, s, ink, clip);
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
                // EmuTOS: `rc_copy(&t,&c); gr_inside(&c, tmpth);` then the 3D
                // `movetext` nudge, then `gr_gtext(te_just, te_font, te_ptext,
                // &c)` (gemoblib.c:735-757) — so the text sits inside a
                // *positive* border and is drawn in the **TEDINFO's own font**,
                // which a resource may set to SMALL.
                let (cx, cy) = ted_movetext(movetext, selected, x + inset, y + inset);
                gr_gtext(
                    backend,
                    ted.te_just,
                    AesFont::from_te_font(ted.te_font),
                    s,
                    cx,
                    cy,
                    w - 2 * inset,
                    h - 2 * inset,
                    ink,
                    clip,
                );
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
        G_ICON | G_CICON => {
            // The ICONBLK interior is read at the tree's ABI width (see the
            // *TEXT arm); an LP64 tree reads it byte-identically to before.
            //
            // AUD-P3-006: a G_CICON's `ob_spec` is a CICONBLK, which *begins*
            // with a mono ICONBLK (`obdefs.h` `CICONBLK { ICONBLK monoblk; CICON
            // *mainlist; }`), so its head is read exactly like a G_ICON and drawn
            // via the same mono path (EmuTOS `just_draw`, gemoblib.c:780-791,
            // shares the ICONBLK body). NewTOS draws the MONO version — the
            // minimum EmuTOS itself renders; the colour `mainlist` chain
            // (CONF_WITH_COLOUR_ICONS) is a documented later addition, not a stub.
            if ob_spec != 0 {
                // SAFETY: ob_spec of a G_ICON/G_CICON points at an ICONBLK (the
                // CICONBLK head for G_CICON) packed at the tree's ABI width.
                let ib = unsafe { IconBlk::read_abi(ob_spec, t.abi()) };
                draw_icon(backend, &ib, x, y, selected, clip);
            }
        }
        // AUD-P3-002: an application-drawn object. Fill a PARMBLK and call its
        // `ub_code` (EmuTOS `just_draw` G_USERDEF, gemoblib.c:792-794:
        // `state = ob_user(tree, obj, &t, spec, state, state)`). The returned
        // state feeds the trailing decorations. `ob_user` only *invokes* the
        // callback for a kernel-built (same-privilege EL1) tree; an app tree's
        // EL0 callback is deferred (see `ob_user`). A null `ob_spec` (no USERBLK)
        // is left to the `_` arm so `ob_user` never dereferences null.
        G_USERDEF if ob_spec != 0 => {
            ob_state = ob_user(t, obj, x, y, w, h, ob_spec, clip, ob_state, ob_state);
        }
        // Remaining sub-types land in later batches.
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
    if ob_state & DISABLED != 0 {
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

/// The 3D text nudge EmuTOS applies to the TEDINFO family — `G_TEXT`,
/// `G_BOXTEXT`, `G_FTEXT`, `G_FBOXTEXT`, `G_BOXCHAR` (`gemoblib.c:744-752`):
///
/// ```text
/// if (movetext) {
///     if (state & SELECTED) c.g_x++;   /* pressed: right */
///     else                  c.g_y--;   /* raised:  up    */
/// }
/// ```
///
/// Note this is **not** the `G_BUTTON` rule — the string block moves a raised
/// button's label one pixel up *and* left (`gemoblib.c:833-836`) — so the two
/// are kept apart, as in EmuTOS.
#[inline]
fn ted_movetext(movetext: bool, selected: bool, x: i16, y: i16) -> (i16, i16) {
    if !movetext {
        (x, y)
    } else if selected {
        (x + 1, y)
    } else {
        (x, y - 1)
    }
}

/// `gsx_tblt` (EmuTOS `gemgraf.c:500-527`) — blit `text` in `fnt` with the
/// **cell top** at `(x, y)`.
///
/// EmuTOS hands the VDI a *baseline* of `y + gl_hptschar` (IBM) or
/// `y + gl_hsptschar` (SMALL) and never touches `vst_alignment`, so the VDI
/// default (left / baseline) applies and the glyph's first raster row lands
/// at `baseline - top` = `y`. Drawing cell-top-anchored is therefore the same
/// pixels, without a baseline round trip.
fn gsx_tblt(
    backend: &dyn RasterBackend,
    fnt: AesFont,
    x: i16,
    y: i16,
    text: &[u8],
    fg: Pel,
    clip: ClipRect,
) {
    let cw = fnt.cell_w();
    for (i, &c) in text.iter().enumerate() {
        let gx = x + i as i16 * cw;
        match fnt {
            AesFont::Ibm => {
                backend.draw_glyph(gx, y, crate::vdi::font::glyph(c as u16), fg, WriteMode::Transparent, clip)
            }
            AesFont::Small => backend.draw_glyph_sized(
                gx,
                y,
                crate::vdi::font::glyph_small(c as u16),
                crate::vdi::font::SMALL_CELL_W,
                crate::vdi::font::SMALL_CELL_H,
                fg,
                WriteMode::Transparent,
                clip,
            ),
        }
    }
}

/// `gr_gtext` (EmuTOS `gemgraf.c:595-605`) — justify `text` inside the box
/// `(x, y, w, h)` and blit it in `fnt`. Inlines its two helpers:
///
/// * `gsx_tcalc` (`gemgraf.c:480-497`) clamps the text extent to the box
///   (`textw = min(w, len*cw)`, `texth = min(h, ch)`) and then drops **whole
///   characters** that do not fit (`numchs = min(numchs, textw/cw)`). A box
///   shorter than one character cell (`texth / ch == 0`) yields `numchs = 0`
///   — EmuTOS draws *nothing* rather than a vertically cropped line.
/// * `gr_just` (`gemgraf.c:567-589`) centres vertically and justifies
///   horizontally, in both axes only when there is slack and rounding the
///   half-margin **up** (`(n + 1) / 2`).
///
/// This is the path every `G_TEXT`/`G_BOXTEXT`/`G_FTEXT`/`G_FBOXTEXT`/
/// `G_BOXCHAR` object and every icon label takes. `G_STRING`/`G_TITLE`/
/// `G_BUTTON` do **not** come through here — EmuTOS justifies those inline in
/// `just_draw` with a different (unclamped, untruncated, `n/2`) formula, which
/// the call sites reproduce.
#[allow(clippy::too_many_arguments)]
fn gr_gtext(
    backend: &dyn RasterBackend,
    just: i16,
    fnt: AesFont,
    text: &[u8],
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    fg: Pel,
    clip: ClipRect,
) {
    let cw = fnt.cell_w();
    let ch = fnt.cell_h();

    // gsx_tcalc — `saturating_mul` only guards the (unreachable, `read_cstr`
    // caps at 256 chars) i16 overflow EmuTOS would wrap on; the clamp below
    // makes any saturated value indistinguishable from the exact one.
    let mut numchs = text.len() as i16;
    let textw = w.min(numchs.saturating_mul(cw));
    let texth = h.min(ch);
    numchs = if texth / ch != 0 { numchs.min(textw / cw) } else { 0 };
    if numchs <= 0 {
        return;
    }

    // gr_just
    let mut ty = y;
    let slack_h = h - texth;
    if slack_h > 0 {
        ty += (slack_h + 1) / 2;
    }
    let mut tx = x;
    let slack_w = w - textw;
    if slack_w > 0 {
        match just {
            TE_CNTR => tx += (slack_w + 1) / 2,
            TE_RIGHT => tx += slack_w,
            TE_LEFT => {}
            // EmuTOS's `if (TE_CNTR) … else if (TE_RIGHT) …` chain leaves any
            // other `te_just` value at the box origin, like TE_LEFT.
            _ => {}
        }
    }

    gsx_tblt(backend, fnt, tx, ty, &text[..numchs as usize], fg, clip);
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

    // The overlay character (a drive letter, a trash "T", …): EmuTOS draws it
    // in the **SMALL** font at `ib_xicon + ib_xchar` / `ib_yicon + ib_ychar`,
    // i.e. `ib_xchar`/`ib_ychar` are relative to the icon *bitmap*, not to the
    // object (`gsx_tblt(SMALL, ib->ib_xicon+ib->ib_xchar,
    // ib->ib_yicon+ib->ib_ychar, 1)`, gemgraf.c:800-803 — `ib_xicon`/`ib_yicon`
    // were already made absolute by `just_draw`, gemoblib.c:784-789).
    let c = (chword & 0xff) as u8;
    if c != 0 {
        gsx_tblt(
            backend,
            AesFont::Small,
            ox + ib.ib_xicon + ib.ib_xchar,
            oy + ib.ib_yicon + ib.ib_ychar,
            &[c],
            fg,
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
            // The label: `gr_gtext(TE_CNTR, SMALL, ib->ib_ptext, pt)` with
            // `pt` = the `ib_xtext..ib_htext` rectangle (gemgraf.c:806). The
            // SMALL font is what makes a resource's label box fit — GEM icon
            // resources size it for 6×6 (EmuTOS's own `icon.rsc` and
            // TeraDesk's `icons.rsc` both use `ib_wtext = 12*6 = 72`,
            // `ib_htext = 6+2 = 8`), so drawing it in the 8×16 font
            // overflows the box in both axes.
            gr_gtext(
                backend,
                TE_CNTR,
                AesFont::Small,
                s,
                ox + ib.ib_xtext,
                oy + ib.ib_ytext,
                ib.ib_wtext,
                ib.ib_htext,
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
