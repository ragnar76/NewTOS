//! AES Resource Manager (`rsrc_*`, opcodes 110–114) — Batch 1.
//!
//! Spec: [`docs/api/aes/resource-fileselector.md`]. **NewTOS uses a native
//! resource format** (decision 2026-06-21): the on-disk `.RSC` stores our
//! own 64-bit `OBJECT` layout, little-endian, with internal pointers held
//! as **byte offsets from the blob base**. Loading is therefore a pure
//! offset→pointer relocation — no endianness swap and no 16→64-bit
//! expansion (unlike a classic Atari `.RSC`). A **loaded** `.RSC`'s
//! coordinates are already in pixels (the host-side `mkrsc.py` pre-converts,
//! and `rsrc_load` does not re-`ob_fix`), so `rsrc_obfix` only ever converts
//! an **inline** resource an app built itself (see [`rsrc_obfix`]).
//!
//! Native `.RSC` layout (all little-endian). Two header generations coexist;
//! the loader picks by `version` and is backward-compatible:
//!
//! * **v1 (LP64) / v2 (ILP32)** — 28-byte header, no free-string/free-image
//!   tables. GEM.RSC / DESK.RSC and any pre-D-054 resource. R_FRSTR / R_STRING
//!   / R_FRIMG / R_IMAGEDATA report the documented "not found" (there is no
//!   table), so these files keep loading unchanged.
//! * **v3 (LP64) / v4 (ILP32)** — 44-byte header that additionally carries a
//!   **free-string** and a **free-image** pointer table, so `rsrc_gaddr`'s
//!   `R_FRSTR`/`R_STRING`/`R_FRIMG`/`R_IMAGEDATA` selectors resolve (D-054).
//! * **v5 (LP64) / v6 (ILP32)** — 68-byte header that additionally carries the
//!   **TEDINFO / ICONBLK / BITBLK** array bases + counts, so the remaining
//!   `rsrc_gaddr`/`rsrc_saddr` selectors (`R_TEDINFO`/`R_ICONBLK`/`R_BITBLK`
//!   and the `R_OBSPEC`/`R_TEP*`/`R_IBP*`/`R_BIPDATA` sub-pointer aliases)
//!   resolve to real struct/field addresses (AUD-P6-002). The sub-struct data
//!   themselves already existed in v3/v4 blobs; v5/v6 only records where they
//!   are. v1–v4 keep loading (those selectors report "not found").
//!
//! ```text
//!   NrscHeader: magic "NRSC", version, n_trees, n_objects,
//!               off_trindex, off_objects, total_size            (v1/v2: 28 B)
//!               [, off_frstr, n_frstr, off_frimg, n_frimg]      (v3/v4: 44 B)
//!   [u32; n_trees]   tree index  @ off_trindex  (root OBJECT index/tree)
//!   [Object; n_objects]          @ off_objects  (ob_spec = offset for the
//!                                                 string object types, else
//!                                                 a literal colour/char word)
//!   [ptr; n_frstr]   frstr table @ off_frstr    (v3/v4: offset → string data)
//!   [ptr; n_frimg]   frimg table @ off_frimg    (v3/v4: offset → a BITBLK)
//!   string / data bytes          (referenced by the ob_spec / table offsets)
//! ```
//!
//! The frstr/frimg **slot** width matches `ob_spec`: 8 bytes (LP64) or 4 bytes
//! (ILP32), relocated at the calling app's ABI width like the OBJECT array.
//!
//! **Batch-1 scope:** the format, the relocation core
//! (`relocate_and_register`), `rsrc_gaddr`, `rsrc_obfix`, `rsrc_free`.
//! Loading from SD (`bdos::fs::read_file`), TEDINFO/ICONBLK/BITBLK
//! sub-pointer relocation, and `rsrc_saddr` are Batch 2.

use super::object::{BitBlk, IconBlk, ObjTree, Object, TedInfo};
use super::Aespb;
use crate::bdos::process::{usermem, Abi};
use crate::vdi::font::{CELL_H, CELL_W};
use core::cell::UnsafeCell;
use core::mem::size_of;
use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use tos_types::{BasePage, Long, MX_PREFTTRAM};

// Object base types, classified for `ob_spec` relocation exactly like EmuTOS
// `fix_objects` (gemrslib.c:620-635): the BOX types carry a literal colour
// word (never relocated), G_CICON a CICONBLK-table index (special case), the
// TEDINFO/BITBLK/ICONBLK types a sub-struct pointer (relocated together with
// their internal pointers), and every other type a plain blob-offset `ob_spec`
// (the relocation `default:`).
const G_BOX: u16 = 20;
const G_TEXT: u16 = 21;
const G_BOXTEXT: u16 = 22;
const G_IMAGE: u16 = 23;
const G_IBOX: u16 = 25;
const G_BOXCHAR: u16 = 27;
const G_FTEXT: u16 = 29;
const G_FBOXTEXT: u16 = 30;
const G_ICON: u16 = 31;
const G_CICON: u16 = 33;

// rsrc_gaddr / rsrc_saddr R_* selectors (aesdefs.h:578-594; spec
// resource-fileselector.md §0). `get_addr` (gemrslib.c:112-176) maps each to a
// slot address: R_TREE/R_OBJECT and the array types to a record base, the
// R_*P* aliases to a pointer *field within* the indexed record, and
// R_STRING/R_IMAGEDATA to the dereferenced free-string / free-image data.
const R_TREE: i16 = 0;
const R_OBJECT: i16 = 1;
/// `&tedinfo[index]` (== `&tedinfo[index].te_ptext`, its first field).
const R_TEDINFO: i16 = 2;
/// `&iconblk[index]` (== `&iconblk[index].ib_pmask`, its first field).
const R_ICONBLK: i16 = 3;
/// `&bitblk[index]` (== `&bitblk[index].bi_pdata`, its first field).
const R_BITBLK: i16 = 4;
/// Free-string **data** pointer `frstr[index]` (dereferenced).
const R_STRING: i16 = 5;
/// Free-image **data** pointer `frimg[index]` (dereferenced).
const R_IMAGEDATA: i16 = 6;
/// Address of `object[index].ob_spec`.
const R_OBSPEC: i16 = 7;
/// Address of `tedinfo[index].te_ptext` (same slot as `R_TEDINFO`).
const R_TEPTEXT: i16 = 8;
/// Address of `tedinfo[index].te_ptmplt`.
const R_TEPTMPLT: i16 = 9;
/// Address of `tedinfo[index].te_pvalid`.
const R_TEPVALID: i16 = 10;
/// Address of `iconblk[index].ib_pmask` (same slot as `R_ICONBLK`).
const R_IBPMASK: i16 = 11;
/// Address of `iconblk[index].ib_pdata`.
const R_IBPDATA: i16 = 12;
/// Address of `iconblk[index].ib_ptext`.
const R_IBPTEXT: i16 = 13;
/// Address of `bitblk[index].bi_pdata` (same slot as `R_BITBLK`).
const R_BIPDATA: i16 = 14;
/// Address **of** the free-string pointer slot, `&frstr[index]`.
const R_FRSTR: i16 = 15;
/// Address **of** the free-image pointer slot, `&frimg[index]`.
const R_FRIMG: i16 = 16;

/// Byte offset of `ob_spec` within an OBJECT: 16 (LP64 32-byte record, 4 pad
/// bytes before the 8-byte pointer) or [`OB_SPEC_OFF_ILP32`] (12) for the
/// packed ILP32 24-byte record. Used by `R_OBSPEC`.
const OB_SPEC_OFF_LP64: usize = 16;

/// Native sub-struct record strides for an LP64 caller — the kernel `repr(C)`
/// sizes (`size_of`): TEDINFO 40, ICONBLK 48, BITBLK 24. An ILP32 caller links
/// the 4-byte-pointer layout (`mt_gem.h`), so the strides shrink (see below);
/// both match `mkrsc.py`'s `NAT_TED`/`NAT_IB`/`NAT_BB`.
const TEDINFO_SIZE: usize = size_of::<TedInfo>();
const ICONBLK_SIZE: usize = size_of::<IconBlk>();
const BITBLK_SIZE: usize = size_of::<BitBlk>();
/// ILP32 sub-struct record strides (`mt_gem.h`: 4-byte `te_p*`/`ib_p*`/
/// `bi_pdata`): TEDINFO 28, ICONBLK 36, BITBLK 16 (`mkrsc.py` ILP32 layout).
const TEDINFO_SIZE_ILP32: usize = 28;
const ICONBLK_SIZE_ILP32: usize = 36;
const BITBLK_SIZE_ILP32: usize = 16;

/// First NRSC `version` whose header carries the free-string / free-image
/// pointer tables (44-byte header). v1/v2 have the 28-byte header and no
/// tables — the loader treats their frstr/frimg counts as 0 (backward compat).
const NRSC_VER_TABLES: u32 = 3;

/// First NRSC `version` whose header additionally carries the TEDINFO / ICONBLK
/// / BITBLK array bases + counts (68-byte header: v5 LP64, v6 ILP32), so
/// `rsrc_gaddr`/`rsrc_saddr` resolve the R_TEDINFO/R_ICONBLK/R_BITBLK family
/// and their sub-pointer aliases. v1–v4 lack these fields — the loader leaves
/// the three counts at 0 and those selectors report the documented "not found"
/// (backward compatible: v1–v4 keep loading unchanged).
const NRSC_VER_SUBSTRUCTS: u32 = 5;

/// Maximum number of object trees registered from one `.RSC`. A real GEM
/// app has one tree per dialog/menu: QED's `qed.rsc` declares 32
/// (`n_trees=32`), so a 16-cap left `rsrc_gaddr(R_TREE, idx>=16)` returning
/// 0 — the app then dereferenced a null tree base and faulted. Sized to 64
/// for headroom (512 B of pointer table); the header's `n_trees` still
/// bounds the actual count.
const MAX_TREES: usize = 64;

/// Native resource-file header. 28 bytes for v1/v2, 44 bytes for v3/v4, 68
/// bytes for v5/v6. Each generation's extra fields exist only for a high
/// enough `version` (`>= NRSC_VER_TABLES` for the free-string/-image table
/// fields, `>= NRSC_VER_SUBSTRUCTS` for the TEDINFO/ICONBLK/BITBLK fields);
/// for an older file those trailing bytes overlap the tree index / objects and
/// MUST NOT be interpreted (the loader gates on `version`).
#[repr(C)]
pub struct NrscHeader {
    /// `b"NRSC"`.
    pub magic: [u8; 4],
    /// Format version: 1 = LP64/28 B, 2 = ILP32/28 B, 3 = LP64/44 B (+tables),
    /// 4 = ILP32/44 B (+tables), 5 = LP64/68 B (+substruct bases), 6 = ILP32/
    /// 68 B (+substruct bases).
    pub version: u32,
    /// Number of object trees.
    pub n_trees: u32,
    /// Number of objects total.
    pub n_objects: u32,
    /// Offset to the tree index (`[u32; n_trees]` of root object indices).
    pub off_trindex: u32,
    /// Offset to the `Object` array.
    pub off_objects: u32,
    /// Total blob size in bytes.
    pub total_size: u32,
    /// Offset to the free-string pointer table (v3+ only, else undefined).
    pub off_frstr: u32,
    /// Number of free strings (v3+ only, else undefined).
    pub n_frstr: u32,
    /// Offset to the free-image pointer table (v3+ only, else undefined).
    pub off_frimg: u32,
    /// Number of free images (v3+ only, else undefined).
    pub n_frimg: u32,
    /// Offset to the `TedInfo` array (v5/v6 only, else undefined).
    pub off_ted: u32,
    /// Number of `TedInfo` records (v5/v6 only, else undefined).
    pub n_ted: u32,
    /// Offset to the `IconBlk` array (v5/v6 only, else undefined).
    pub off_ib: u32,
    /// Number of `IconBlk` records (v5/v6 only, else undefined).
    pub n_ib: u32,
    /// Offset to the `BitBlk` array (v5/v6 only, else undefined).
    pub off_bb: u32,
    /// Number of `BitBlk` records (v5/v6 only, else undefined).
    pub n_bb: u32,
}

/// The loaded resource (single resource per app — NewTOS is single-app).
///
/// Stored as **target VAs + a record stride** rather than typed `*mut Object`
/// pointers, so an ILP32 caller's 24-byte OBJECT array is indexed at its own
/// stride (ADR-020 stage 4): the kernel `Object` type is the LP64 32-byte
/// layout, but an ILP32 app links a 24-byte OBJECT with a 4-byte `ob_spec`.
/// `rsrc_gaddr` hands back `obj_base + index * obj_size`, so the app always
/// receives records of its own width.
struct Loaded {
    /// Target VA of each tree's root OBJECT (0 = unused slot).
    trees: UnsafeCell<[u64; MAX_TREES]>,
    n_trees: UnsafeCell<usize>,
    /// Target VA of the OBJECT array (0 = nothing loaded).
    obj_base: UnsafeCell<u64>,
    /// Stride of one OBJECT record: 32 (LP64 caller) or 24 (ILP32 caller).
    obj_size: UnsafeCell<usize>,
    n_objects: UnsafeCell<usize>,
    /// Target VA of `frstr[0]` — the app-visible base returned by `R_FRSTR`
    /// (0 = no free-string table, i.e. a v1/v2 resource).
    frstr_target: UnsafeCell<u64>,
    /// Kernel-reachable VA of `frstr[0]` — used to *dereference* a slot for
    /// `R_STRING` (equals `frstr_target` for a direct EL1 caller; the pool
    /// block's kernel alias for the EL0 path).
    frstr_access: UnsafeCell<u64>,
    n_frstr: UnsafeCell<usize>,
    /// Target VA of `frimg[0]` (`R_FRIMG`), 0 = no free-image table.
    frimg_target: UnsafeCell<u64>,
    /// Kernel-reachable VA of `frimg[0]` (`R_IMAGEDATA` dereference).
    frimg_access: UnsafeCell<u64>,
    n_frimg: UnsafeCell<usize>,
    /// Width of one frstr/frimg pointer slot: 8 (LP64) or 4 (ILP32).
    ptr_size: UnsafeCell<usize>,
    /// Kernel-reachable alias of `object[0]` (`obj_base` is the app-visible
    /// target VA). Equals `obj_base` for a direct EL1 caller; the pool block's
    /// kernel alias for the EL0 path. Used by `rsrc_saddr`
    /// (R_OBJECT/R_OBSPEC/R_TREE) to write a slot through a kernel VA.
    obj_access: UnsafeCell<u64>,
    /// Target VA of `tedinfo[0]` (`R_TEDINFO`/`R_TEP*`), 0 = no table (v1–v4).
    ted_target: UnsafeCell<u64>,
    /// Kernel-reachable alias of `tedinfo[0]` (for `rsrc_saddr`).
    ted_access: UnsafeCell<u64>,
    n_ted: UnsafeCell<usize>,
    /// Stride of one TEDINFO record: 40 (LP64) or 28 (ILP32).
    ted_size: UnsafeCell<usize>,
    /// Target VA of `iconblk[0]` (`R_ICONBLK`/`R_IBP*`), 0 = no table.
    ib_target: UnsafeCell<u64>,
    /// Kernel-reachable alias of `iconblk[0]`.
    ib_access: UnsafeCell<u64>,
    n_ib: UnsafeCell<usize>,
    /// Stride of one ICONBLK record: 48 (LP64) or 36 (ILP32).
    ib_size: UnsafeCell<usize>,
    /// Target VA of `bitblk[0]` (`R_BITBLK`/`R_BIPDATA`), 0 = no table.
    bb_target: UnsafeCell<u64>,
    /// Kernel-reachable alias of `bitblk[0]`.
    bb_access: UnsafeCell<u64>,
    n_bb: UnsafeCell<usize>,
    /// Stride of one BITBLK record: 24 (LP64) or 16 (ILP32).
    bb_size: UnsafeCell<usize>,
}
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for Loaded {}

static RSC: Loaded = Loaded {
    trees: UnsafeCell::new([0u64; MAX_TREES]),
    n_trees: UnsafeCell::new(0),
    obj_base: UnsafeCell::new(0),
    obj_size: UnsafeCell::new(0),
    n_objects: UnsafeCell::new(0),
    frstr_target: UnsafeCell::new(0),
    frstr_access: UnsafeCell::new(0),
    n_frstr: UnsafeCell::new(0),
    frimg_target: UnsafeCell::new(0),
    frimg_access: UnsafeCell::new(0),
    n_frimg: UnsafeCell::new(0),
    ptr_size: UnsafeCell::new(0),
    obj_access: UnsafeCell::new(0),
    ted_target: UnsafeCell::new(0),
    ted_access: UnsafeCell::new(0),
    n_ted: UnsafeCell::new(0),
    ted_size: UnsafeCell::new(0),
    ib_target: UnsafeCell::new(0),
    ib_access: UnsafeCell::new(0),
    n_ib: UnsafeCell::new(0),
    ib_size: UnsafeCell::new(0),
    bb_target: UnsafeCell::new(0),
    bb_access: UnsafeCell::new(0),
    n_bb: UnsafeCell::new(0),
    bb_size: UnsafeCell::new(0),
};

/// ILP32 OBJECT record stride (4-byte `ob_spec` @ +12; see `mt_gem.h`
/// `sizeof(OBJECT) == 24`). The LP64 stride is [`OBJECT_SIZE`] (32).
const OBJECT_SIZE_ILP32: usize = 24;
/// Byte offset of `ob_type` within an OBJECT (same in both ABIs).
const OB_TYPE_OFF: usize = 6;
/// Byte offset of `ob_spec` within an ILP32 OBJECT (4-byte field).
const OB_SPEC_OFF_ILP32: usize = 12;

/// Relocate a native `.RSC` blob in place (offsets → absolute pointers)
/// and register its trees. `base` points at the start of the blob (which
/// must remain live until `rsrc_free`). Returns false on a bad header.
///
/// # Safety
/// `base` must point at a valid, writable native `.RSC` blob of at least
/// its declared `total_size` bytes.
pub unsafe fn relocate_and_register(base: *mut u8) -> bool {
    // In-place kernel path: access base == target base, LP64 (desktop / the
    // selftests always build 32-byte native resources).
    // SAFETY: caller guarantees a valid blob.
    unsafe { relocate_and_register_core(base, base as u64, Abi::Lp64) }
}

/// [`relocate_and_register`] with the access/target-base split of
/// [`relocate_blob_core`]: the registered tree/object pointers resolve to
/// `target_base`, while every field is touched through `access_base`. Used
/// by the EL0 `rsrc_load` path where `target_base` is the user VA the blob
/// is mapped at and `access_base` is the pool block's kernel alias.
///
/// # Safety
/// As [`relocate_and_register`], with `access_base` reachable from the
/// kernel and the blob eventually visible at `target_base`.
unsafe fn relocate_and_register_core(access_base: *mut u8, target_base: u64, abi: Abi) -> bool {
    // SAFETY: caller guarantees a valid blob.
    unsafe {
        // Relocate offsets → target VAs at the caller's OBJECT width.
        let ok = match abi {
            Abi::Ilp32 => relocate_blob_core_ilp32(access_base, target_base),
            Abi::Lp64 => relocate_blob_core(access_base, target_base),
        };
        if !ok {
            return false;
        }
        let obj_size = match abi {
            Abi::Ilp32 => OBJECT_SIZE_ILP32,
            Abi::Lp64 => OBJECT_SIZE,
        };
        let h = &*(access_base as *const NrscHeader);
        // Target VA of the object array (what callers dereference).
        let objs_target = target_base + h.off_objects as u64;
        // The tree index is read through the kernel alias. It is `[u32]`
        // (root OBJECT *index*) in both ABIs; the byte VA of a root is
        // `objs_target + index * obj_size`.
        let tri = access_base.add(h.off_trindex as usize) as *const u32;
        let n = (h.n_trees as usize).min(MAX_TREES);
        let trees = &mut *RSC.trees.get();
        for (t, slot) in trees.iter_mut().enumerate().take(n) {
            let root = *tri.add(t) as u64;
            *slot = objs_target + root * obj_size as u64;
        }
        *RSC.n_trees.get() = n;
        *RSC.obj_base.get() = objs_target;
        // Kernel-reachable alias of the object array, for `rsrc_saddr`'s
        // R_OBJECT/R_OBSPEC/R_TREE writes (equals `objs_target` for the in-place
        // EL1 path; the pool block's kernel alias on the EL0 path).
        *RSC.obj_access.get() = access_base as u64 + h.off_objects as u64;
        *RSC.obj_size.get() = obj_size;
        *RSC.n_objects.get() = h.n_objects as usize;

        // Free-string / free-image pointer tables (NRSC >= v3). The slots were
        // already rebased onto `target_base` by the relocation core; here we
        // record the app-visible base (`*_target`, returned by R_FRSTR/R_FRIMG)
        // and the kernel-reachable base (`*_access`, used to dereference a slot
        // for R_STRING/R_IMAGEDATA). An older file leaves the counts at 0, so
        // those selectors report the documented "not found".
        let ptr_size = match abi {
            Abi::Ilp32 => 4usize,
            Abi::Lp64 => 8usize,
        };
        *RSC.ptr_size.get() = ptr_size;
        if h.version >= NRSC_VER_TABLES {
            *RSC.frstr_target.get() = target_base + h.off_frstr as u64;
            *RSC.frstr_access.get() = access_base as u64 + h.off_frstr as u64;
            *RSC.n_frstr.get() = h.n_frstr as usize;
            *RSC.frimg_target.get() = target_base + h.off_frimg as u64;
            *RSC.frimg_access.get() = access_base as u64 + h.off_frimg as u64;
            *RSC.n_frimg.get() = h.n_frimg as usize;
        } else {
            *RSC.frstr_target.get() = 0;
            *RSC.frstr_access.get() = 0;
            *RSC.n_frstr.get() = 0;
            *RSC.frimg_target.get() = 0;
            *RSC.frimg_access.get() = 0;
            *RSC.n_frimg.get() = 0;
        }

        // TEDINFO / ICONBLK / BITBLK array bases (NRSC >= v5), so rsrc_gaddr /
        // rsrc_saddr resolve R_TEDINFO/R_ICONBLK/R_BITBLK and their sub-pointer
        // aliases (R_OBSPEC/R_TEP*/R_IBP*/R_BIPDATA). The three arrays already
        // existed in v3/v4 blobs (the OBJECT walk relocates their internals);
        // v5/v6 only records their offsets + counts in the 68-byte header. The
        // strides are the caller-ABI record sizes (mkrsc.py NAT_TED/NAT_IB/
        // NAT_BB). A v1–v4 file has none of this — the counts stay 0 and those
        // selectors report the documented "not found" (backward compatible).
        let (ted_size, ib_size, bb_size) = match abi {
            Abi::Ilp32 => (TEDINFO_SIZE_ILP32, ICONBLK_SIZE_ILP32, BITBLK_SIZE_ILP32),
            Abi::Lp64 => (TEDINFO_SIZE, ICONBLK_SIZE, BITBLK_SIZE),
        };
        if h.version >= NRSC_VER_SUBSTRUCTS {
            *RSC.ted_target.get() = target_base + h.off_ted as u64;
            *RSC.ted_access.get() = access_base as u64 + h.off_ted as u64;
            *RSC.n_ted.get() = h.n_ted as usize;
            *RSC.ted_size.get() = ted_size;
            *RSC.ib_target.get() = target_base + h.off_ib as u64;
            *RSC.ib_access.get() = access_base as u64 + h.off_ib as u64;
            *RSC.n_ib.get() = h.n_ib as usize;
            *RSC.ib_size.get() = ib_size;
            *RSC.bb_target.get() = target_base + h.off_bb as u64;
            *RSC.bb_access.get() = access_base as u64 + h.off_bb as u64;
            *RSC.n_bb.get() = h.n_bb as usize;
            *RSC.bb_size.get() = bb_size;
        } else {
            *RSC.ted_target.get() = 0;
            *RSC.ted_access.get() = 0;
            *RSC.n_ted.get() = 0;
            *RSC.ted_size.get() = 0;
            *RSC.ib_target.get() = 0;
            *RSC.ib_access.get() = 0;
            *RSC.n_ib.get() = 0;
            *RSC.ib_size.get() = 0;
            *RSC.bb_target.get() = 0;
            *RSC.bb_access.get() = 0;
            *RSC.n_bb.get() = 0;
            *RSC.bb_size.get() = 0;
        }
    }
    true
}

// ─── ILP32 relocation (24-byte OBJECT / 4-byte ob_spec) ─────────────────
//
// Byte-addressed twin of `relocate_blob_core` for an ILP32 caller (ADR-020):
// the OBJECT stride is 24, `ob_type` is a u16 @ +6 and `ob_spec` a u32 @ +12,
// and the sub-structs (TEDINFO/BITBLK/ICONBLK) carry 4-byte pointers. Every
// field is read/written unaligned through raw `u8` pointers, so the kernel's
// LP64 `Object`/`TedInfo`/… structs are never imposed on the ILP32 blob.

/// Read a little-endian u16 at `p` (unaligned-safe).
///
/// # Safety
/// `p` must point at ≥ 2 readable bytes.
#[inline]
unsafe fn rd16(p: *const u8) -> u16 {
    // SAFETY: caller guarantees 2 readable bytes; read_unaligned handles any
    // alignment.
    unsafe { (p as *const u16).read_unaligned() }
}

/// Read a little-endian u32 at `p` (unaligned-safe).
///
/// # Safety
/// `p` must point at ≥ 4 readable bytes.
#[inline]
unsafe fn rd32(p: *const u8) -> u32 {
    // SAFETY: caller guarantees 4 readable bytes.
    unsafe { (p as *const u32).read_unaligned() }
}

/// Relocate a 4-byte offset field at `p` in place (add `tb`, skip 0).
///
/// # Safety
/// `p` must point at ≥ 4 readable/writable bytes.
#[inline]
unsafe fn reloc32(p: *mut u8, tb: u32) {
    // SAFETY: caller guarantees 4 accessible bytes.
    unsafe {
        let v = (p as *const u32).read_unaligned();
        if v != 0 {
            (p as *mut u32).write_unaligned(v.wrapping_add(tb));
        }
    }
}

/// ILP32 twin of [`relocate_blob_core`]: relocate the 24-byte OBJECT array and
/// its 4-byte-pointer sub-structs from blob offsets to `target_base` VAs.
///
/// # Safety
/// `access_base` must point at a valid, writable native `.RSC` blob of at
/// least its declared `total_size` bytes.
unsafe fn relocate_blob_core_ilp32(access_base: *mut u8, target_base: u64) -> bool {
    // SAFETY: caller guarantees a valid blob.
    unsafe {
        let h = &*(access_base as *const NrscHeader);
        if &h.magic != b"NRSC" {
            return false;
        }
        let objs = access_base.add(h.off_objects as usize);
        let tb = target_base as u32;
        let n = h.n_objects as usize;
        for i in 0..n {
            let obj = objs.add(i * OBJECT_SIZE_ILP32);
            let spec = rd32(obj.add(OB_SPEC_OFF_ILP32));
            if spec == 0 {
                continue;
            }
            // Classify by the *base* type only (high byte = extended flags).
            // Relocation by EXCLUSION (EmuTOS `fix_objects`, gemrslib.c:620-635;
            // see the LP64 twin `relocate_blob_core` for the full rationale):
            // BOX types keep their literal word, G_CICON is a CICONBLK-table
            // index (special case), and every other type's `ob_spec` is a blob
            // offset relocated with `+= tb`.
            match rd16(obj.add(OB_TYPE_OFF)) & 0x00ff {
                // Literal colour/thickness word — never a pointer.
                G_BOX | G_IBOX | G_BOXCHAR => {}
                // Colour icon: `ob_spec` indexes the CICONBLK pointer table,
                // NOT a blob offset — excluded from the `+= tb` relocation
                // (P6-012). `mkrsc.py` has no CICON data path (it zeroes
                // G_CICON `ob_spec`, skipped above), so nothing to fix up.
                G_CICON => {}
                G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => {
                    let nspec = spec.wrapping_add(tb);
                    (obj.add(OB_SPEC_OFF_ILP32) as *mut u32).write_unaligned(nspec);
                    if !substruct_seen_ilp32(objs, i, nspec) {
                        // TEDINFO: te_ptext@0, te_ptmplt@4, te_pvalid@8.
                        let ted = access_base.add(spec as usize);
                        reloc32(ted, tb);
                        reloc32(ted.add(4), tb);
                        reloc32(ted.add(8), tb);
                    }
                }
                G_IMAGE => {
                    let nspec = spec.wrapping_add(tb);
                    (obj.add(OB_SPEC_OFF_ILP32) as *mut u32).write_unaligned(nspec);
                    if !substruct_seen_ilp32(objs, i, nspec) {
                        // BITBLK: bi_pdata@0.
                        reloc32(access_base.add(spec as usize), tb);
                    }
                }
                G_ICON => {
                    let nspec = spec.wrapping_add(tb);
                    (obj.add(OB_SPEC_OFF_ILP32) as *mut u32).write_unaligned(nspec);
                    if !substruct_seen_ilp32(objs, i, nspec) {
                        // ICONBLK: ib_pmask@0, ib_pdata@4, ib_ptext@8.
                        let ib = access_base.add(spec as usize);
                        reloc32(ib, tb);
                        reloc32(ib.add(4), tb);
                        reloc32(ib.add(8), tb);
                    }
                }
                // Plain blob-offset `ob_spec` (G_STRING/G_BUTTON/G_TITLE,
                // G_USERDEF, G_PROGDEF, …) → relocate (EmuTOS `fix_long`).
                _ => {
                    (obj.add(OB_SPEC_OFF_ILP32) as *mut u32)
                        .write_unaligned(spec.wrapping_add(tb));
                }
            }
        }
        // Free-string / free-image pointer tables (NRSC >= v3, 4-byte slots).
        if h.version >= NRSC_VER_TABLES {
            let fs = access_base.add(h.off_frstr as usize);
            for i in 0..h.n_frstr as usize {
                reloc32(fs.add(i * 4), tb);
            }
            let fi = access_base.add(h.off_frimg as usize);
            for i in 0..h.n_frimg as usize {
                reloc32(fi.add(i * 4), tb);
            }
        }
    }
    true
}

/// ILP32 twin of [`substruct_seen`]: whether an already-processed object
/// (`objs[0..upto]`, 24-byte stride) shares the sub-struct now at `spec`.
///
/// # Safety
/// `objs` must point at the ILP32 OBJECT array with at least `upto` entries.
unsafe fn substruct_seen_ilp32(objs: *const u8, upto: usize, spec: u32) -> bool {
    for j in 0..upto {
        // SAFETY: j < upto ≤ processed count.
        let o = unsafe { objs.add(j * OBJECT_SIZE_ILP32) };
        // SAFETY: o is a valid 24-byte record.
        let ty = unsafe { rd16(o.add(OB_TYPE_OFF)) } & 0x00ff;
        let sp = unsafe { rd32(o.add(OB_SPEC_OFF_ILP32)) };
        if has_substruct(ty) && sp == spec {
            return true;
        }
    }
    false
}

/// Relocate a native `.RSC` blob in place (offsets → pointers) **without**
/// touching the global tree table — for a second resource (e.g. the AES's
/// own GEM.RSC behind the file selector) that must coexist with the app's.
///
/// # Safety
/// As [`relocate_and_register`].
pub unsafe fn relocate_blob(base: *mut u8) -> bool {
    // In-place relocation: the blob is accessed and dereferenced through
    // the same pointer, so the access base and the target base coincide.
    // SAFETY: caller guarantees a valid blob.
    unsafe { relocate_blob_core(base, base as u64) }
}

/// Core relocation, splitting the pointer used to **access** the blob from
/// the base the fixed-up pointers must **resolve to**.
///
/// * `access_base` — a kernel-reachable pointer at the blob's first byte;
///   every field is read/written through it.
/// * `target_base` — the VA the offsets are rebased onto. For the in-place
///   kernel path this equals `access_base as u64`. For the EL0 path the
///   blob lives in a BDOS-pool block that is *also* mapped into the calling
///   process at a user VA; the kernel writes through the pool's kernel
///   high-half alias (`access_base`) but the stored pointers must be the
///   **user** VA (`target_base`), because `rsrc_gaddr` hands them to EL0.
///
/// A sub-struct (TEDINFO/BITBLK/ICONBLK) is therefore located for its own
/// internal fix-up via `access_base + (ob_spec − target_base)` — i.e. the
/// kernel alias of the same offset — never via the just-written user VA.
///
/// # Safety
/// `access_base` must point at a valid, writable native `.RSC` blob of at
/// least its declared `total_size` bytes.
unsafe fn relocate_blob_core(access_base: *mut u8, target_base: u64) -> bool {
    // SAFETY: caller guarantees a valid blob.
    unsafe {
        let h = &*(access_base as *const NrscHeader);
        if &h.magic != b"NRSC" {
            return false;
        }
        let objs = access_base.add(h.off_objects as usize) as *mut Object;
        let ab = access_base as u64;
        let tb = target_base;
        for i in 0..h.n_objects as usize {
            let o = &mut *objs.add(i);
            if o.ob_spec == 0 {
                continue;
            }
            // Classify by the *base* type only: the high byte carries
            // extended-type flags (EmuTOS `fix_objects`, gemrslib.c:619
            // `obtype = obj->ob_type & 0x00ff`; `mkrsc.py` writes the full
            // 16-bit `typ` but classifies on `typ & 0xFF`). Without the mask
            // an object with the high byte set would miss every arm, its
            // `ob_spec` would keep a raw file offset, and `objc_draw` would
            // dereference a small integer.
            // Relocation by EXCLUSION, matching EmuTOS `fix_objects`
            // (gemrslib.c:620-635): the BOX types keep their literal colour
            // word, G_CICON is a CICONBLK-table index (special case), and every
            // other type's `ob_spec` is a blob offset relocated with `+= tb`.
            // (The earlier by-INCLUSION list silently left G_USERDEF and any
            // other non-listed type unrelocated — AUD-P6-012.)
            match o.ob_type & 0x00ff {
                // Literal colour/thickness word — never a pointer.
                G_BOX | G_IBOX | G_BOXCHAR => {}
                // Colour icon: `ob_spec` indexes the CICONBLK pointer table
                // (EmuTOS `ob_spec = ciconblkptr[ob_spec]`), NOT a blob offset,
                // so it is excluded from the `+= tb` relocation. The full
                // CICONBLK fix-up (`fixup_all_ciconblks`/`transform_all_cicons`)
                // activates once `mkrsc.py` emits a CICON table; today the tool
                // rejects CICON `.rsc` files and zeroes any G_CICON `ob_spec`
                // (drawn empty, skipped above by the `ob_spec == 0` guard), so
                // there is no CICON data path to fix up (AUD-P6-012).
                G_CICON => {}
                G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => {
                    o.ob_spec += tb;
                    // Relocate the shared TEDINFO's internal pointers once.
                    if !substruct_seen(objs, i, o.ob_spec) {
                        let t = &mut *((ab + (o.ob_spec - tb)) as *mut TedInfo);
                        reloc(&mut t.te_ptext, tb);
                        reloc(&mut t.te_ptmplt, tb);
                        reloc(&mut t.te_pvalid, tb);
                    }
                }
                G_IMAGE => {
                    o.ob_spec += tb;
                    if !substruct_seen(objs, i, o.ob_spec) {
                        let bi = &mut *((ab + (o.ob_spec - tb)) as *mut BitBlk);
                        reloc(&mut bi.bi_pdata, tb);
                    }
                }
                G_ICON => {
                    o.ob_spec += tb;
                    if !substruct_seen(objs, i, o.ob_spec) {
                        let ib = &mut *((ab + (o.ob_spec - tb)) as *mut IconBlk);
                        reloc(&mut ib.ib_pmask, tb);
                        reloc(&mut ib.ib_pdata, tb);
                        reloc(&mut ib.ib_ptext, tb);
                    }
                }
                // Plain blob-offset `ob_spec` (G_STRING/G_BUTTON/G_TITLE,
                // G_USERDEF, G_PROGDEF, …) → relocate (EmuTOS `fix_long`).
                _ => o.ob_spec += tb,
            }
        }
        // Free-string / free-image pointer tables (NRSC >= v3, 8-byte slots):
        // each slot is a blob offset (string data for frstr, a BITBLK for
        // frimg) rebased onto `target_base`, leaving 0 as "empty".
        if h.version >= NRSC_VER_TABLES {
            reloc_table_u64(access_base.add(h.off_frstr as usize), h.n_frstr as usize, tb);
            reloc_table_u64(access_base.add(h.off_frimg as usize), h.n_frimg as usize, tb);
        }
    }
    true
}

/// Relocate a table of `count` 8-byte pointer slots at `base` in place: add
/// `tb` to each non-zero slot (LP64 frstr/frimg). Slots may be unaligned.
///
/// # Safety
/// `base` must point at ≥ `count * 8` readable/writable bytes.
unsafe fn reloc_table_u64(base: *mut u8, count: usize, tb: u64) {
    for i in 0..count {
        // SAFETY: i < count; the caller guarantees the slot span.
        let p = unsafe { base.add(i * 8) } as *mut u64;
        // SAFETY: 8 readable/writable bytes at `p`.
        unsafe {
            let v = p.read_unaligned();
            if v != 0 {
                p.write_unaligned(v + tb);
            }
        }
    }
}

/// Pointer to the root object of tree `idx` in a relocated blob at `base`.
///
/// # Safety
/// `base` must point at a relocated native `.RSC` blob with `idx` < n_trees.
pub unsafe fn tree_root(base: *mut u8, idx: usize) -> *mut Object {
    // SAFETY: caller guarantees a relocated blob.
    unsafe {
        let h = &*(base as *const NrscHeader);
        let objs = base.add(h.off_objects as usize) as *mut Object;
        let tri = base.add(h.off_trindex as usize) as *const u32;
        objs.add(*tri.add(idx) as usize)
    }
}

/// Free-string data pointer `frstr[index]` from a **relocated** native blob at
/// `base` — the direct analog of EmuTOS `rs_str()` (`gemrslib.c:854`, which
/// indexes the AES's own resource free-string table). The file selector uses it
/// to fetch its default dialog title `ITEMSLCT` from the loaded GEM.RSC
/// (`gemfslib.c:692`, `rs_str(ITEMSLCT)`).
///
/// `base` is a blob relocated in place via [`relocate_blob`] (LP64), so the
/// free-string table holds 8-byte pointer slots already rebased to valid
/// pointers. Returns a null pointer when the blob predates the free-string
/// table (`version < NRSC_VER_TABLES`, a v1/v2 resource) or `index` is out of
/// range.
///
/// # Safety
/// `base` must point at a relocated native `.RSC` blob.
pub unsafe fn blob_frstr(base: *const u8, index: usize) -> *const u8 {
    // SAFETY: caller guarantees a relocated blob at `base`.
    unsafe {
        let h = &*(base as *const NrscHeader);
        if &h.magic != b"NRSC" || h.version < NRSC_VER_TABLES || index >= h.n_frstr as usize {
            return core::ptr::null();
        }
        // LP64 free-string table: 8-byte pointer slots, possibly unaligned.
        let slot = base.add(h.off_frstr as usize + index * 8) as *const u64;
        slot.read_unaligned() as *const u8
    }
}

/// Add `base` to a non-zero offset field (in-place relocation).
#[inline]
fn reloc(field: &mut u64, base: u64) {
    if *field != 0 {
        *field += base;
    }
}

/// Object base types whose `ob_spec` points at a sub-struct (TEDINFO,
/// BITBLK or ICONBLK) with its own internal pointers to relocate.
#[inline]
fn has_substruct(base_type: u16) -> bool {
    matches!(
        base_type,
        G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT | G_IMAGE | G_ICON
    )
}

/// Whether the sub-struct now at absolute address `spec` was already
/// relocated by an earlier object (`objs[0..upto]` are already processed).
///
/// EmuTOS walks the TEDINFO/BITBLK/ICONBLK arrays once each, so a sub-struct
/// shared by several objects is relocated exactly once
/// (gemrslib.c:655-669,799-805). `NrscHeader` omits those array offsets, so
/// we dedup on the *target address* instead: two objects share a sub-struct
/// iff their (already relocated) `ob_spec` values are equal. A byte offset in
/// the blob maps to exactly one sub-struct type, so equal addresses imply the
/// same struct — no cross-type false match. The per-object `ob_spec` itself is
/// still relocated for every object (both sharers must point at the struct);
/// only the struct's *internal* pointers are deduped.
///
/// # Safety
/// `objs` must point at the relocated OBJECT array with at least `upto`
/// valid entries.
unsafe fn substruct_seen(objs: *mut Object, upto: usize, spec: u64) -> bool {
    for j in 0..upto {
        // SAFETY: j < upto ≤ processed count; distinct from the current
        // object, so no aliasing with the caller's live &mut.
        let o = unsafe { &*objs.add(j) };
        if has_substruct(o.ob_type & 0x00ff) && o.ob_spec == spec {
            return true;
        }
    }
    false
}

/// Capacity of the single resource buffer (one loaded `.RSC` at a time).
const RSC_CAP: usize = 64 * 1024;

struct RscBuf(UnsafeCell<[u8; RSC_CAP]>);
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for RscBuf {}
static RSC_BUF: RscBuf = RscBuf(UnsafeCell::new([0u8; RSC_CAP]));

// ─── Per-process EL0 resource window (folded into `usermem`, ADR-021) ─
//
// A `svc`-dispatched (EL0) `rsrc_load` cannot use the kernel high-half
// RSC_BUF: an EL0 app faults reading a kernel VA. EmuTOS puts the RSC in
// the *application's* memory (`rs_readit` → `dos_alloc_anyram`,
// gemrslib.c:775) and returns app pointers via `rs_gaddr`. NewTOS mirrors
// that: the blob goes into a BDOS-pool block that is mapped into the
// calling process's **EL0 heap window** (`crate::bdos::process::usermem`)
// as a `Kind::Rsc` block — sharing one per-process window + bump cursor
// with GEMDOS `Malloc`/`Mxalloc` — and `rsrc_gaddr` hands back the user VA.

/// Kernel high-half offset (mirror of `process::{arena,loader,mmu}`): a
/// BDOS-pool physical address `pa` is reachable from the kernel at
/// `pa + VA_OFFSET`.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

/// User-page size (one 4-KiB physical frame).
const PAGE_SIZE: u64 = usermem::PAGE_SIZE;

/// Pool `m_start` of the EL0 process's currently-loaded `.RSC` block, for
/// `Mfree` at `rsrc_free`; `0` when nothing is loaded or the load used the
/// kernel-VA path. Holds a [`Long`] (Atari 32-bit pool address) so it feeds
/// `mfree` directly and identifies the `usermem` window entry to drop.
static USER_RSC_RAW: AtomicI32 = AtomicI32::new(0);

/// Directory of the currently-launched program (e.g. `C:\APPS\QED\`), so
/// `rsrc_load` finds the resource **beside the application** (lead directive
/// 2026-07-20). A GEM app typically passes just `qed.rsc` (its `gl_appdir` is
/// empty), so without this the kernel could only look in the fixed
/// `C:\GEMSYS\RSC\` / `C:\` locations. Set by the kernel launcher
/// ([`set_app_dir`]) from the launch path before the program enters EL0.
const APP_DIR_CAP: usize = 64;
struct AppDir {
    buf: UnsafeCell<[u8; APP_DIR_CAP]>,
    len: AtomicUsize,
}
// SAFETY: single boot CPU; set by the launcher, read by rsrc_load, never
// concurrently.
unsafe impl Sync for AppDir {}
static APP_DIR: AppDir = AppDir {
    buf: UnsafeCell::new([0u8; APP_DIR_CAP]),
    len: AtomicUsize::new(0),
};

/// Record the launched program's directory so a later `rsrc_load` looks for
/// its `.RSC` beside it. `path` is the full launch path (`C:\APPS\QED\QED.NAP`);
/// the directory portion (up to and including the last `\`) is kept. Passing a
/// path with no `\` clears the app directory. Called by the kernel launcher.
pub fn set_app_dir(path: &[u8]) {
    let dir_len = path
        .iter()
        .rposition(|&c| c == b'\\')
        .map(|i| i + 1)
        .unwrap_or(0)
        .min(APP_DIR_CAP);
    // SAFETY: single-CPU launcher context; exclusive writer.
    unsafe {
        let buf = &mut *APP_DIR.buf.get();
        buf[..dir_len].copy_from_slice(&path[..dir_len]);
    }
    APP_DIR.len.store(dir_len, Ordering::Release);
}

/// Copy the launched application's directory (as recorded by [`set_app_dir`],
/// e.g. `C:\APPS\QED\`, always ending in a path separator) into `out`,
/// returning the number of bytes written. This is NewTOS's analogue of
/// EmuTOS's `rlr->p_appdir`, used by `shel_find` to search the application
/// directory first (`findfile` step 1, `gemshlib.c:415-423`). An empty result
/// (length 0) means no application directory is set.
pub fn app_dir(out: &mut [u8]) -> usize {
    let len = APP_DIR.len.load(Ordering::Acquire).min(out.len());
    // SAFETY: single-CPU; `len` bytes were written by `set_app_dir`.
    unsafe {
        let buf = &*APP_DIR.buf.get();
        out[..len].copy_from_slice(&buf[..len]);
    }
    len
}

/// `rsrc_load` (110) — load `addr_in[0]` (a `.RSC` filename) from disk.
/// NewTOS searches the launched app's own directory first (lead directive),
/// then `C:\GEMSYS\RSC\<name>`, then `C:\<name>`. Returns 1 on success, 0 on
/// failure.
pub fn rsrc_load(pb: &Aespb) {
    let fname = pb.addrin(0) as *const u8;
    if fname.is_null() {
        pb.set_ret(0);
        return;
    }
    // Copy the bare filename (cap at 14 chars: 8.3 + NUL slack).
    let mut name = [0u8; 16];
    let nlen = {
        let mut i = 0;
        // SAFETY: trusted GEM pointer to a NUL-terminated string.
        unsafe {
            while i < name.len() - 1 {
                let c = *fname.add(i);
                if c == 0 {
                    break;
                }
                name[i] = c;
                i += 1;
            }
        }
        i
    };

    // A `svc`-dispatched (EL0) caller cannot read the kernel high-half
    // RSC_BUF; a direct EL1 caller (the desktop, the selftests) can. Pick
    // the placement by whether a user process is running: EmuTOS puts the
    // RSC in the *application's* memory (`rs_readit` → `dos_alloc_anyram`,
    // gemrslib.c:775) and returns app pointers via `rs_gaddr`. `RUN` is set
    // to the launched process's basepage before its `eret` to EL0
    // (`launch.rs`), so it is non-null exactly for an EL0 app caller.
    let run = crate::bdos::process::current();
    // The blob's OBJECT width must match the caller's ABI: an ILP32 app reads
    // 24-byte OBJECTs, so relocate at that width and hand it 24-byte records.
    let abi = crate::bdos::process::current_abi();

    // App-directory prefix (lead directive): search the launched program's own
    // folder first. Empty (a kernel-side caller / no launcher) → skipped.
    let app_len = APP_DIR.len.load(Ordering::Acquire);
    // SAFETY: single-CPU; `app_len` bytes were written by `set_app_dir`.
    let app_buf = unsafe { &*APP_DIR.buf.get() };
    let app_dir = &app_buf[..app_len];

    // SAFETY: single-CPU AES; exclusive logical owner of the buffer.
    let buf = unsafe { &mut *RSC_BUF.0.get() };
    for prefix in [app_dir, b"C:\\GEMSYS\\RSC\\".as_slice(), b"C:\\".as_slice()] {
        if prefix.is_empty() {
            continue; // no app directory recorded
        }
        let mut path = [0u8; 64];
        let mut n = 0;
        for &c in prefix.iter().chain(name[..nlen].iter()) {
            if n < path.len() {
                path[n] = c;
                n += 1;
            }
        }
        let Ok(path_str) = core::str::from_utf8(&path[..n]) else {
            continue;
        };
        let Some(len) = crate::bdos::fs::read_file(path_str, buf) else {
            continue;
        };
        if len < size_of::<NrscHeader>() {
            continue;
        }
        // SAFETY: `buf` now holds `len` file bytes; both relocation paths
        // re-validate the `NRSC` header before touching anything.
        let ok = if run.is_null() {
            // Kernel-side caller: relocate in place, kernel-VA pointers (LP64).
            unsafe { relocate_and_register(buf.as_mut_ptr()) }
        } else {
            // EL0 app caller: copy into per-process EL0-mapped memory and
            // relocate to user VAs at the caller's OBJECT width.
            unsafe { load_into_user(run, buf, len, abi) }
        };
        if ok {
            pb.set_ret(1);
            return;
        }
    }
    pb.set_ret(0);
}

/// Place a candidate native `.RSC` blob (`src[..len]`) into the calling
/// process's per-process resource window and register its trees with
/// **user** VAs, so an EL0 `rsrc_gaddr` returns dereferenceable pointers.
/// Returns `false` — leaving nothing allocated — on a bad header, no slot,
/// an exhausted window, `ENSMEM`, or a mapping error.
///
/// Protected-memory realisation of EmuTOS `rs_readit` (gemrslib.c:775): the
/// RSC lives in per-application memory (a BDOS-pool block, `Mxalloc`),
/// separate from the app's TPA, reclaimed at `rsrc_free` / the process's
/// `Pterm` (owner tag).
///
/// # Safety
/// Single-CPU trap context (the implicit BDOS giant lock); `run` is the
/// current process basepage; `src[..len]` is a candidate blob.
unsafe fn load_into_user(run: *mut BasePage, src: &[u8; RSC_CAP], len: usize, abi: Abi) -> bool {
    // Cheap non-NRSC reject before any allocation.
    // SAFETY: `len >= size_of::<NrscHeader>()` guaranteed by the caller.
    if unsafe { &(*(src.as_ptr() as *const NrscHeader)).magic } != b"NRSC" {
        return false;
    }
    let Some(slot) = crate::bdos::process::slot_for(run) else {
        return false;
    };

    // Page-round the blob and allocate a page-aligned pool block (over-
    // allocate one page so the rounded-up base fits, exactly as
    // `arena::alloc_tpa` / the GEMDOS heap path). The block is stamped owned
    // by the running process (`current_owner`), so its `Pterm` reclaims it
    // even if the app exits without `rsrc_free`.
    let pages = (len as u64 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    // SAFETY: single-CPU trap context holds the implicit BDOS lock.
    let raw = unsafe {
        crate::bdos::mem::traps::mxalloc((pages + PAGE_SIZE) as Long, MX_PREFTTRAM)
    };
    if raw == 0 {
        return false;
    }
    let base_pa = (raw as u64 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let base_kva = base_pa.wrapping_add(VA_OFFSET) as *mut u8;

    // Map the block into the process's EL0 heap window (as a `Kind::Rsc`
    // block) and take the user VA the blob's pointers will resolve to.
    // SAFETY: `raw` addresses `pages + one page` live pool bytes owned by
    // the calling process.
    let Some(app_va) = (unsafe { usermem::map(slot, raw, pages, usermem::Kind::Rsc) })
    else {
        // SAFETY: `raw` is on the allocated list.
        unsafe { crate::bdos::mem::traps::mfree(raw) };
        return false;
    };

    // Copy the raw file bytes into the block via its kernel alias, then
    // relocate offsets → user VAs (accessed through the alias, resolved to
    // `app_va`).
    // SAFETY: `base_kva` aliases `pages` live RW pool bytes; `src` is valid.
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr(), base_kva, len);
    }
    // SAFETY: `base_kva` holds the copied blob; header re-checked inside.
    if !unsafe { relocate_and_register_core(base_kva, app_va, abi) } {
        // Drop the window entry and free the pool block.
        usermem::free_by_mstart(slot, raw);
        // SAFETY: `raw` is on the allocated list.
        unsafe { crate::bdos::mem::traps::mfree(raw) };
        return false;
    }

    // Order the blob writes (made through the kernel alias) before the app
    // reads them at `app_va`; `usermem::map` already published the PTEs, and
    // the window VAs were previously invalid so no TLBI is needed (QUIRK-001).
    // SAFETY: barrier-only.
    unsafe { usermem::publish() };

    USER_RSC_RAW.store(raw, Ordering::Release);
    true
}

/// `rsrc_free` (111) — drop the loaded resource's tree table.
///
/// Returns 1 only if a resource was actually loaded; 0 when nothing was
/// loaded and on a double free, matching EmuTOS `rs_free` (returns 0 when
/// `ap_rscmem == NULL`, gemrslib.c:685-708; spec verification criterion
/// resource-fileselector.md:439-440). The tree table is reset either way.
pub fn rsrc_free(pb: &Aespb) {
    // Return a per-process EL0 RSC block to the pool if one is tracked, and
    // drop its `usermem` window entry. The window *mapping* is intentionally
    // left in place: the app holds no live pointers after the free, and the
    // bump cursor never re-hands the VA, so the stale mapping is never
    // remapped (no TLBI) nor read. The block's owner tag makes a later
    // `Pterm` a no-op for it (it is now off the allocated list, so
    // `free_all_owned` / a second `Mfree` skip it).
    let raw = USER_RSC_RAW.swap(0, Ordering::AcqRel);
    if raw != 0 {
        if let Some(slot) =
            crate::bdos::process::slot_for(crate::bdos::process::current())
        {
            usermem::free_by_mstart(slot, raw);
        }
        // SAFETY: single-CPU trap context (implicit BDOS lock); `raw` came
        // from `mxalloc` in `load_into_user` and has not been freed.
        unsafe { crate::bdos::mem::traps::mfree(raw) };
    }

    // SAFETY: single-CPU AES. `obj_base` is non-zero iff a resource is loaded
    // (set by `relocate_and_register`, cleared here).
    let was_loaded = unsafe { *RSC.obj_base.get() != 0 };
    // SAFETY: single-CPU AES.
    unsafe {
        *RSC.n_trees.get() = 0;
        *RSC.obj_base.get() = 0;
        *RSC.obj_size.get() = 0;
        *RSC.n_objects.get() = 0;
        *RSC.frstr_target.get() = 0;
        *RSC.frstr_access.get() = 0;
        *RSC.n_frstr.get() = 0;
        *RSC.frimg_target.get() = 0;
        *RSC.frimg_access.get() = 0;
        *RSC.n_frimg.get() = 0;
        *RSC.ptr_size.get() = 0;
        *RSC.obj_access.get() = 0;
        *RSC.ted_target.get() = 0;
        *RSC.ted_access.get() = 0;
        *RSC.n_ted.get() = 0;
        *RSC.ted_size.get() = 0;
        *RSC.ib_target.get() = 0;
        *RSC.ib_access.get() = 0;
        *RSC.n_ib.get() = 0;
        *RSC.ib_size.get() = 0;
        *RSC.bb_target.get() = 0;
        *RSC.bb_access.get() = 0;
        *RSC.n_bb.get() = 0;
        *RSC.bb_size.get() = 0;
    }
    pb.set_ret(if was_loaded { 1 } else { 0 });
}

/// A resolved resource slot: the app-visible address (`target`, returned by
/// [`rsrc_gaddr`]) and the kernel-reachable alias (`access`, written by
/// [`rsrc_saddr`] and dereferenced for `R_STRING`/`R_IMAGEDATA`). For a direct
/// EL1 caller the two coincide; for an EL0-loaded resource they differ by the
/// blob's target↔access delta (the whole blob is one contiguous region mapped
/// at both a user VA and a kernel high-half alias).
#[derive(Clone, Copy)]
struct Slot {
    target: u64,
    access: u64,
}

/// The (target, access) address of member `off` of the `index`-th record of an
/// array based at `target_base`/`access_base` with `count` records of `size`
/// bytes. Returns `None` (EmuTOS would hand back an in-bounds pointer; NewTOS
/// uses the documented safer bounds check — D-054 / AUD-P6-016) for an absent
/// table (`target_base == 0`, i.e. a resource without that section) or an
/// out-of-range index.
fn array_slot(
    target_base: u64,
    access_base: u64,
    count: usize,
    size: usize,
    off: u64,
    index: usize,
) -> Option<Slot> {
    if target_base == 0 || index >= count {
        return None;
    }
    let d = index as u64 * size as u64 + off;
    Some(Slot { target: target_base + d, access: access_base + d })
}

/// The slot designated by `R_STRING`/`R_IMAGEDATA`: EmuTOS `get_addr`
/// dereferences the `index`-th free-string / free-image pointer and returns the
/// **data** pointer it holds (gemrslib.c:159-162). The value read from the
/// (already relocated) slot is a target VA into the same blob, so its kernel
/// alias is `value + (access_base − target_base)` (the blob's target↔access
/// delta). `None` for an absent table, an out-of-range index, or a NULL slot.
///
/// # Safety
/// Single-CPU AES; `access_base`, when non-zero, is a kernel-reachable base of
/// a relocated frstr/frimg table with at least `count` slots of `ptr_size`.
unsafe fn deref_slot(
    target_base: u64,
    access_base: u64,
    count: usize,
    ptr_size: usize,
    index: usize,
) -> Option<Slot> {
    if access_base == 0 || index >= count {
        return None;
    }
    let p = (access_base + (index * ptr_size) as u64) as *const u8;
    // SAFETY: `index < count`; the slot span is live per the caller contract.
    let v = unsafe {
        match ptr_size {
            4 => (p as *const u32).read_unaligned() as u64,
            _ => (p as *const u64).read_unaligned(),
        }
    };
    if v == 0 {
        return None;
    }
    let delta = access_base.wrapping_sub(target_base);
    Some(Slot { target: v, access: v.wrapping_add(delta) })
}

/// Resolve the slot of resource object `index` of `R_*` type `rtype`, faithful
/// to EmuTOS `get_addr` (gemrslib.c:112-176) + the R_* table (aesdefs.h:578-594).
/// Shared by [`rsrc_gaddr`] (which returns `Slot::target`) and [`rsrc_saddr`]
/// (which writes through `Slot::access`). The sub-pointer aliases resolve to a
/// pointer field *within* the indexed record at the caller's ABI width:
/// `ob_spec` @16 (LP64) / @12 (ILP32); the TEDINFO/ICONBLK sub-pointers at
/// `ptr_size` strides (`te_ptmplt`/`ib_pdata` @`ptr_size`, `te_pvalid`/
/// `ib_ptext` @`2*ptr_size`). `None` is EmuTOS's `-1L` (unknown type) or
/// NewTOS's safer not-found (absent table / out-of-range index).
///
/// # Safety
/// Single-CPU AES; every descriptor cell read below was set by
/// `relocate_and_register` (and cleared by `rsrc_free`).
unsafe fn get_addr(rtype: i16, index: usize) -> Option<Slot> {
    // SAFETY: single-CPU AES; the descriptor cells are valid (set at load).
    unsafe {
        let ps = *RSC.ptr_size.get() as u64;
        let obj_t = *RSC.obj_base.get();
        let obj_a = *RSC.obj_access.get();
        let n_obj = *RSC.n_objects.get();
        let obj_sz = *RSC.obj_size.get();
        match rtype {
            R_TREE => {
                if index >= *RSC.n_trees.get() {
                    return None;
                }
                // The tree root lives inside the OBJECT array, so its kernel
                // alias uses the same blob delta as the object array.
                let target = (*RSC.trees.get())[index];
                let delta = obj_a.wrapping_sub(obj_t);
                Some(Slot { target, access: target.wrapping_add(delta) })
            }
            R_OBJECT => array_slot(obj_t, obj_a, n_obj, obj_sz, 0, index),
            R_OBSPEC => {
                let off = if obj_sz == OBJECT_SIZE {
                    OB_SPEC_OFF_LP64 as u64
                } else {
                    OB_SPEC_OFF_ILP32 as u64
                };
                array_slot(obj_t, obj_a, n_obj, obj_sz, off, index)
            }
            R_TEDINFO | R_TEPTEXT => array_slot(
                *RSC.ted_target.get(), *RSC.ted_access.get(),
                *RSC.n_ted.get(), *RSC.ted_size.get(), 0, index,
            ),
            R_TEPTMPLT => array_slot(
                *RSC.ted_target.get(), *RSC.ted_access.get(),
                *RSC.n_ted.get(), *RSC.ted_size.get(), ps, index,
            ),
            R_TEPVALID => array_slot(
                *RSC.ted_target.get(), *RSC.ted_access.get(),
                *RSC.n_ted.get(), *RSC.ted_size.get(), 2 * ps, index,
            ),
            R_ICONBLK | R_IBPMASK => array_slot(
                *RSC.ib_target.get(), *RSC.ib_access.get(),
                *RSC.n_ib.get(), *RSC.ib_size.get(), 0, index,
            ),
            R_IBPDATA => array_slot(
                *RSC.ib_target.get(), *RSC.ib_access.get(),
                *RSC.n_ib.get(), *RSC.ib_size.get(), ps, index,
            ),
            R_IBPTEXT => array_slot(
                *RSC.ib_target.get(), *RSC.ib_access.get(),
                *RSC.n_ib.get(), *RSC.ib_size.get(), 2 * ps, index,
            ),
            R_BITBLK | R_BIPDATA => array_slot(
                *RSC.bb_target.get(), *RSC.bb_access.get(),
                *RSC.n_bb.get(), *RSC.bb_size.get(), 0, index,
            ),
            R_FRSTR => array_slot(
                *RSC.frstr_target.get(), *RSC.frstr_access.get(),
                *RSC.n_frstr.get(), *RSC.ptr_size.get(), 0, index,
            ),
            R_FRIMG => array_slot(
                *RSC.frimg_target.get(), *RSC.frimg_access.get(),
                *RSC.n_frimg.get(), *RSC.ptr_size.get(), 0, index,
            ),
            // SAFETY: reads the frstr/frimg table via its kernel alias.
            R_STRING => deref_slot(
                *RSC.frstr_target.get(), *RSC.frstr_access.get(),
                *RSC.n_frstr.get(), *RSC.ptr_size.get(), index,
            ),
            R_IMAGEDATA => deref_slot(
                *RSC.frimg_target.get(), *RSC.frimg_access.get(),
                *RSC.n_frimg.get(), *RSC.ptr_size.get(), index,
            ),
            _ => None,
        }
    }
}

/// `rsrc_gaddr` (112) — address of resource object `int_in[1]` of `R_*` type
/// `int_in[0]` → `addr_out[0]`. Resolves all 17 EmuTOS `get_addr` selectors
/// (aesdefs.h:578-594) via [`get_addr`]: `R_TREE`/`R_OBJECT`, the
/// TEDINFO/ICONBLK/BITBLK array bases (`R_TEDINFO`/`R_ICONBLK`/`R_BITBLK`),
/// their sub-pointer aliases (`R_OBSPEC`/`R_TEP*`/`R_IBP*`/`R_BIPDATA`), the
/// free-string / free-image slot addresses (`R_FRSTR`/`R_FRIMG`), and the
/// dereferenced free data pointers (`R_STRING`/`R_IMAGEDATA`). An unknown type,
/// or a selector whose table is absent (a v1–v4 resource for the substruct/
/// free families) / index out of range, yields the documented "not found"
/// (`addr_out[0] = 0`, `int_out[0] = 0`).
pub fn rsrc_gaddr(pb: &Aespb) {
    let rtype = pb.intin(0);
    let index = pb.intin(1) as usize;
    // SAFETY: single-CPU AES; the descriptor was set by relocate_and_register.
    let addr = unsafe { get_addr(rtype, index) }.map_or(0, |s| s.target);
    pb.set_addrout(0, addr);
    pb.set_ret(if addr != 0 { 1 } else { 0 });
}

/// `rsrc_saddr` (113) — store the pointer `addr_in[0]` into the resource slot
/// designated by `R_*` type `int_in[0]` / index `int_in[1]`. Faithful to
/// EmuTOS `rs_saddr` (gemrslib.c:728-742): `psub = get_addr(rtype, rindex); if
/// (psub != -1) { *psub = rsaddr; return TRUE; } return FALSE;`. The store goes
/// through the slot's kernel-reachable alias at the caller's pointer width (the
/// slot is pointer-sized in every selector), so an EL0 app's `.RSC` — mapped at
/// a user VA — is updated without an EL1 access to that user VA. Returns TRUE
/// for a resolvable slot, FALSE for an unknown type / absent table / out-of-
/// range index (NewTOS's safer not-found, matching [`get_addr`]).
pub fn rsrc_saddr(pb: &Aespb) {
    let rtype = pb.intin(0);
    let index = pb.intin(1) as usize;
    // The pointer to store, read at the caller's ABI width (4 bytes ILP32 /
    // 8 bytes LP64) — exactly the width of the destination pointer slot.
    let rsaddr = pb.addrin(0);
    // SAFETY: single-CPU AES; `slot.access` is a kernel-reachable pointer slot
    // of `ptr_size` bytes, live until rsrc_free.
    let ok = unsafe {
        match get_addr(rtype, index) {
            Some(slot) => {
                let p = slot.access as *mut u8;
                match *RSC.ptr_size.get() {
                    4 => (p as *mut u32).write_unaligned(rsaddr as u32),
                    _ => (p as *mut u64).write_unaligned(rsaddr),
                }
                true
            }
            None => false,
        }
    };
    pb.set_ret(if ok { 1 } else { 0 });
}

/// `rsrc_obfix` (114) — convert object `int_in[0]`'s packed char-grid
/// coordinates (`ob_x`/`ob_y`/`ob_width`/`ob_height`) to pixels, in place.
///
/// Faithful to EmuTOS `rs_obfix` (`gemrslib.c:89-99`) + `fix_chpos`
/// (`gemrslib.c:53-83`): each of the four coordinate WORDs packs an unsigned
/// **character count in the low byte** and a **signed pixel offset in the high
/// byte**; the pixel value is `char_count * cell + signed_offset`, where the
/// cell is `gl_wchar` (8) for x/width and `gl_hchar` (16) for y/height, and a
/// `ob_width` whose char count is exactly 80 maps to the full screen width
/// (`gl_width`, `gemrslib.c:71-74`). The slots are `RS_TREE = addr_in[0]` and
/// `RS_OBJ = int_in[0]` (`aesdefs.h:574-575`, `gemsuper.c:375-377`), read at
/// the caller's ABI width via [`ObjTree`] exactly like every other object op.
///
/// **Scope — inline resources only, no double conversion.** This is applied
/// solely to a resource an app built itself (an `rs_object[]` compiled into
/// the program, still in char-grid form). A resource loaded through
/// `rsrc_load` is already in pixels — the host-side `mkrsc.py` pre-converts
/// and [`rsrc_load`] does not re-`ob_fix` — and a well-behaved GEM app never
/// re-`rsrc_obfix`es loaded objects (EmuTOS `rsrc_load` `ob_fix`es them once
/// at load, `gemrslib.c:606-637,815-819`), so converting here never touches
/// the loaded `.RSC` and cannot double-convert it.
///
/// `rs_obfix` is `void`; the AES `ret` stays `TRUE`, so this always reports 1
/// (`gemsuper.c:85,375-377`).
pub fn rsrc_obfix(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj < 0 {
        pb.set_ret(1);
        return;
    }
    // `gl_width` — the full-screen pixel width used by the `ob_width == 80`
    // special case (EmuTOS `gl_width`, the VDI workstation width). Fall back to
    // the classic 80-column full width (80 * gl_wchar = 640) if no raster is up,
    // which only matters for that one special case with no display.
    let gl_width = crate::host::host()
        .and_then(|h| h.raster())
        .map(|r| r.width() as i16)
        .unwrap_or(80 * CELL_W);
    let t = ObjTree::new(tree);
    t.set_x(obj, fix_chpos(t.x(obj), CELL_W, false, gl_width));
    t.set_y(obj, fix_chpos(t.y(obj), CELL_H, false, gl_width));
    t.set_width(obj, fix_chpos(t.width(obj), CELL_W, true, gl_width));
    t.set_height(obj, fix_chpos(t.height(obj), CELL_H, false, gl_width));
    pb.set_ret(1);
}

/// Convert one packed char-grid coordinate WORD to pixels (EmuTOS `fix_chpos`,
/// `gemrslib.c:53-83`). The **low byte** is the unsigned character count,
/// multiplied by the character cell size `cell` (`gl_wchar` for x/width,
/// `gl_hchar` for y/height); the **high byte** is a signed pixel offset added
/// afterwards (`> 128 ⇒ − 256`, matching `coffset > 128 ? coffset - 256 :
/// coffset`). `is_width` selects the `char == 80 → gl_width` full-screen case,
/// which EmuTOS applies to `ob_width` only. All arithmetic is done in `i32` and
/// truncated to `i16`, which is bit-identical to EmuTOS' `WORD` (16-bit) math
/// for the valid 0..=255 byte inputs (max `255 * 16 + 127`, never overflowing).
pub(super) fn fix_chpos(raw: i16, cell: i16, is_width: bool, gl_width: i16) -> i16 {
    let v = raw as u16;
    let coffset = (v >> 8) as i32; // high byte: signed pixel offset (0..=255)
    let charcnt = (v & 0x00ff) as i32; // low byte: unsigned char count (0..=255)
    let base = if is_width && charcnt == 80 {
        gl_width as i32
    } else {
        charcnt * cell as i32
    };
    let offset = if coffset > 128 { coffset - 256 } else { coffset };
    (base + offset) as i16
}

/// Size of one native `Object` record (for `R_OBJECT` indexing / tools).
pub const OBJECT_SIZE: usize = size_of::<Object>();
