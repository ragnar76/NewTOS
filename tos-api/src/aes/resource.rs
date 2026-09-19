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
//! Native `.RSC` layout (all little-endian). Four header generations coexist;
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
//!   resolve to real struct/field addresses (AUD-P6-002). Those three counts
//!   are also what the **relocation** walks: EmuTOS `rs_readit` fixes the
//!   sub-structure pointers array-wise, by `rsh_nted`/`rsh_nib`/`rsh_nbb`
//!   (`gemrslib.c:798-805`), so a record no object references is a pointer
//!   afterwards too. A pre-v5 blob declares no such arrays; it still loads if
//!   it holds no text/icon/image object, and is refused if it does, because
//!   its records could not be relocated (A36).
//! * **v7 (LP64) / v8 (ILP32)** — the same 68-byte header, but the **tree
//!   index** is a relocatable pointer table instead of a `[u32]` of root
//!   OBJECT indices: caller-pointer-wide slots holding a root's byte offset,
//!   rebased on load like the frstr/frimg tables. That is EmuTOS's
//!   `rsh_trindex` array, the one `fix_trindex` publishes through
//!   `AESGLOBAL.ap_ptree` (`gemrslib.c:591-603`) and `get_addr(R_TREE, i)`
//!   reads back (`:124`) — so `rsrc_load` finally has an array to name (A35).
//!   A v1–v6 blob keeps loading and keeps answering `rsrc_gaddr(R_TREE, i)`
//!   from the index form; it simply publishes no `ap_ptree`.
//!
//! ```text
//!   NrscHeader: magic "NRSC", version, n_trees, n_objects,
//!               off_trindex, off_objects, total_size            (v1/v2: 28 B)
//!               [, off_frstr, n_frstr, off_frimg, n_frimg]      (v3/v4: 44 B)
//!               [, off_ted, n_ted, off_ib, n_ib, off_bb, n_bb]  (v5–v8: 68 B)
//!   tree index       @ off_trindex  (v1–v6: [u32] root OBJECT index/tree;
//!                                    v7/v8: [ptr] offset → the root OBJECT)
//!   [Object; n_objects]          @ off_objects  (ob_spec = offset for the
//!                                                 string object types, else
//!                                                 a literal colour/char word)
//!   [TedInfo; n_ted] [BitBlk; n_bb] [IconBlk; n_ib]             (v5–v8)
//!   [ptr; n_frstr]   frstr table @ off_frstr    (v3+: offset → string data)
//!   [ptr; n_frimg]   frimg table @ off_frimg    (v3+: offset → a BITBLK)
//!   string / data bytes          (referenced by the ob_spec / table offsets)
//! ```
//!
//! The tree-index / frstr / frimg **slot** width matches `ob_spec`: 8 bytes
//! (LP64) or 4 bytes (ILP32), relocated at the calling app's ABI width like
//! the OBJECT array — the *file's* `version` only says which width its
//! producer wrote, the *caller's* ABI is what the loader relocates by.
//!
//! **Batch-1 scope:** the format, the relocation core
//! (`relocate_and_register`), `rsrc_gaddr`, `rsrc_obfix`, `rsrc_free`.
//! Loading from SD (`bdos::fs::read_file`), TEDINFO/ICONBLK/BITBLK
//! sub-pointer relocation, and `rsrc_saddr` are Batch 2.
//!
//! **What `rsrc_load` reports back (A35).** EmuTOS's `rs_readit` also reports
//! the load through the caller's `AESGLOBAL` — `ap_rscmem`, `ap_rsclen`
//! (`gemrslib.c:787-788`) and `ap_ptree` (`:595-597`) — and `rs_sglobe`
//! (`:675-679`) reads them back on every `rs_free`/`rs_gaddr`/`rs_saddr`.
//! NewTOS **writes** the three ([`report_load`]).
//!
//! **Where the descriptor lives (A39).** In EmuTOS those three words *are*
//! the descriptor, so the state is the application's and two AES processes
//! never touch each other's resource — the `AESPD` holds no resource field
//! (`aes/struct.h:119-155`), and `rs_load` frees nothing (`:786-788`), so a
//! process may hold several loaded blobs of which its `AESGLOBAL` names one.
//! NewTOS cannot read the state out of the application's memory (a blob is
//! reached through the kernel alias of the process window and needs the
//! ABI's record strides, [`Loaded`]), so it keeps its own descriptor —
//! **per AES process**, keyed by the caller's pid, in [`RSC`]: a list of the
//! process's loaded resources of which one is *selected*. `rsrc_load`
//! appends and selects the new entry; `rsrc_free`/`rsrc_gaddr`/`rsrc_saddr`
//! act on the selected one only; `appl_exit` touches nothing (`ap_exit`,
//! `aes/gemaplib.c:251-261`); at process end the process's entries go with
//! its memory ([`on_process_exit`], the counterpart of `ixterm` →
//! `free_all_owned`, `bdos/proc.c:136`). The model of the list is XaAES's
//! `client->resources` (`xaaes/src.km/xa_rsrc.c:895-900`), which stands in
//! for what EmuTOS gets from the application's own memory.
//!
//! **Stage 1 / stage 2 (A39).** This is stage 1: *selected* means "the
//! process's most recently loaded entry" — which is what `ap_rscmem` names
//! for every application that does not rewrite its `AESGLOBAL`. Stage 2
//! ([`selected`], after A60) reads the 4-byte `ap_rscmem` back out of the
//! caller's `AESGLOBAL` and matches it against the `blob_target` of the
//! caller's own entries, `rs_sglobe`'s binding; a value naming none of them
//! reads as "nothing loaded". The contract with both stages is
//! `docs/api/aes/resource-fileselector.md` § `rsrc_load` (A39 callout); the
//! reference comparison `docs/analysis/rsrc-per-app-state-2026-09-19.md`.
//!
//! Two things bound what can be written, and both are recorded at
//! [`report_load`]: the blob must carry a relocated tree table
//! ([`NRSC_VER_TRINDEX_PTR`]), and its base must fit the four bytes
//! `ap_ptree`/`ap_rscmem` have. The second excludes the in-kernel path, which
//! relocates in place in [`RSC_BUF`], a kernel high-half static — there the
//! three stay 0 rather than carry a truncated VA. That binds no caller on a
//! running system: **there is no EL1 caller of `rsrc_load`** (measured
//! 2026-09-13). Opcode 110 is reachable only through the `svc #2` dispatcher,
//! and of the four EL1 emitters only `kernel/src/selftest_objc.rs` issues rsrc
//! opcodes at all — its two functions that issue 110 have no caller, and
//! `run_rsrc`, reachable only from the callerless `aes_demo::showcase`,
//! registers its blob through `relocate_and_register` and never issues 110.
//! Traced in QEMU `raspi4b` (`--features aes-trace`): the built-in desktop
//! makes 28 AES calls, all with `slot = 0xFF`, none of them rsrc; TeraDesk
//! makes 188, all EL0, 68 of them rsrc, every blob address inside the process
//! window below `USER_TPA_MAX_VA` (32 MiB) and so exact in four bytes.
//! **The one caller that does drive the EL1 load path is the host test
//! harness** `tests/aes_rsrc_global_fields.rs`, which is not a running system
//! — and on a 64-bit host no address is ever 32-bit representable, so that
//! harness can only ever observe the refusal, never the write.
//!
//! EmuTOS's own desktop is the counter-case — `load_icon_file`
//! (`desk/deskapp.c:554`) calls `rsrc_load` and reads `AP_1RESV` back (`:571`)
//! — but NewTOS's `desk/` is not getting that path: ADR-022 is rescinded,
//! TeraDesk is the desktop. So the EL1 address question stays a
//! **conditional** constraint (`docs/api/aes/resource-fileselector.md`
//! §`rsrc_load`), not a blocker: it binds only if an EL1 caller is ever made
//! to load a resource itself.
//!
//! The tests stand in `tests/aes_rsrc_global_fields.rs`.

use super::object::{BitBlk, IconBlk, ObjTree, Object, TedInfo};
use super::shell::PATHBUF;
use super::Aespb;
use crate::bdos::process::{usermem, Abi};
use crate::vdi::font::{CELL_H, CELL_W};
use core::cell::UnsafeCell;
use core::mem::size_of;
use core::sync::atomic::{AtomicUsize, Ordering};
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

/// First NRSC `version` whose **tree index** is a relocatable pointer table
/// (v7 LP64, v8 ILP32) instead of a `[u32]` of root OBJECT indices. The header
/// is unchanged (68 bytes); only `off_trindex`'s content and slot width move:
/// each slot holds the byte **offset** of a tree's root OBJECT and is relocated
/// in place, at the caller's pointer width, exactly like the frstr/frimg tables
/// (`fix_nptrs64`/`fix_nptrs32`).
///
/// That is the array EmuTOS's `fix_trindex` (`aes/gemrslib.c:591-603`)
/// publishes to the application:
///
/// ```c
/// ptreebase = (LONG *)get_sub(0, rs_hdr->rsh_trindex, sizeof(LONG));
/// rs_global->ap_ptree = (OBJECT **)ptreebase;
/// for (ii = 0; ii < rs_hdr->rsh_ntree; ii++)
///     fix_long(ptreebase+ii);
/// ```
///
/// and the array `get_addr(R_TREE, i)` reads back (`return
/// rs_global->ap_ptree[rsindex];`, `gemrslib.c:124`) — so `ap_ptree[i]` and
/// `rsrc_gaddr(R_TREE, i)` are the same value by construction. Without it
/// [`rsrc_load`] has nothing to put in `ap_ptree` (A35).
///
/// A v1–v6 blob keeps the index form and keeps loading unchanged; its trees are
/// still registered and `rsrc_gaddr(R_TREE, i)` still works. What such a blob
/// cannot do is *publish* `ap_ptree`, because the array the field would name
/// does not exist in it — see [`rsrc_load`].
const NRSC_VER_TRINDEX_PTR: u32 = 7;

/// Slot width of the tree index for a blob of `version`, given the caller's
/// pointer width `ptr`: a relocated pointer from [`NRSC_VER_TRINDEX_PTR`] on,
/// a fixed 4-byte root OBJECT index before that.
#[inline]
fn trindex_stride(version: u32, ptr: usize) -> usize {
    if version >= NRSC_VER_TRINDEX_PTR {
        ptr
    } else {
        4
    }
}

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
    /// Offset to the tree index. From [`NRSC_VER_TRINDEX_PTR`] on this is a
    /// caller-pointer-wide table of root-OBJECT byte offsets, relocated in
    /// place (EmuTOS's `rsh_trindex` array); before that a `[u32; n_trees]` of
    /// root object *indices*.
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

/// One loaded resource — the descriptor of a single `rsrc_load`.
///
/// Stored as **target VAs + a record stride** rather than typed `*mut Object`
/// pointers, so an ILP32 caller's 24-byte OBJECT array is indexed at its own
/// stride (ADR-020 stage 4): the kernel `Object` type is the LP64 32-byte
/// layout, but an ILP32 app links a 24-byte OBJECT with a 4-byte `ob_spec`.
/// `rsrc_gaddr` hands back `obj_base + index * obj_size`, so the app always
/// receives records of its own width.
///
/// A plain value: [`relocate_and_register_core`] produces one, [`RSC`] keeps
/// it in the loading process's list.
#[derive(Clone, Copy)]
struct Loaded {
    /// Target VA of each tree's root OBJECT (0 = unused slot).
    trees: [u64; MAX_TREES],
    n_trees: usize,
    /// Target VA of the OBJECT array (0 = nothing loaded).
    obj_base: u64,
    /// Stride of one OBJECT record: 32 (LP64 caller) or 24 (ILP32 caller).
    obj_size: usize,
    n_objects: usize,
    /// Target VA of `frstr[0]` — the app-visible base returned by `R_FRSTR`
    /// (0 = no free-string table, i.e. a v1/v2 resource).
    frstr_target: u64,
    /// Kernel-reachable VA of `frstr[0]` — used to *dereference* a slot for
    /// `R_STRING` (equals `frstr_target` for a direct EL1 caller; the pool
    /// block's kernel alias for the EL0 path).
    frstr_access: u64,
    n_frstr: usize,
    /// Target VA of `frimg[0]` (`R_FRIMG`), 0 = no free-image table.
    frimg_target: u64,
    /// Kernel-reachable VA of `frimg[0]` (`R_IMAGEDATA` dereference).
    frimg_access: u64,
    n_frimg: usize,
    /// Width of one frstr/frimg pointer slot: 8 (LP64) or 4 (ILP32).
    ptr_size: usize,
    /// Kernel-reachable alias of `object[0]` (`obj_base` is the app-visible
    /// target VA). Equals `obj_base` for a direct EL1 caller; the pool block's
    /// kernel alias for the EL0 path. Used by `rsrc_saddr`
    /// (R_OBJECT/R_OBSPEC/R_TREE) to write a slot through a kernel VA.
    obj_access: u64,
    /// Target VA of `tedinfo[0]` (`R_TEDINFO`/`R_TEP*`), 0 = no table (v1–v4).
    ted_target: u64,
    /// Kernel-reachable alias of `tedinfo[0]` (for `rsrc_saddr`).
    ted_access: u64,
    n_ted: usize,
    /// Stride of one TEDINFO record: 40 (LP64) or 28 (ILP32).
    ted_size: usize,
    /// Target VA of `iconblk[0]` (`R_ICONBLK`/`R_IBP*`), 0 = no table.
    ib_target: u64,
    /// Kernel-reachable alias of `iconblk[0]`.
    ib_access: u64,
    n_ib: usize,
    /// Stride of one ICONBLK record: 48 (LP64) or 36 (ILP32).
    ib_size: usize,
    /// Target VA of `bitblk[0]` (`R_BITBLK`/`R_BIPDATA`), 0 = no table.
    bb_target: u64,
    /// Kernel-reachable alias of `bitblk[0]`.
    bb_access: u64,
    n_bb: usize,
    /// Stride of one BITBLK record: 24 (LP64) or 16 (ILP32).
    bb_size: usize,
    /// Target VA of the blob's **first byte** — EmuTOS's `rs_hdr`, reported as
    /// `ap_rscmem` (`gemrslib.c:787`). 0 = nothing loaded.
    blob_target: u64,
    /// Declared blob length in bytes (`total_size`) — EmuTOS's `rslsize`,
    /// reported as `ap_rsclen` (`gemrslib.c:788`).
    blob_len: u32,
    /// Target VA of the relocated tree-index array — EmuTOS's `ptreebase`,
    /// reported as `ap_ptree` (`gemrslib.c:595-597`). 0 when the blob predates
    /// [`NRSC_VER_TRINDEX_PTR`] and therefore carries no pointer table.
    trindex_target: u64,
}

impl Loaded {
    const EMPTY: Loaded = Loaded {
        trees: [0u64; MAX_TREES],
        n_trees: 0,
        obj_base: 0,
        obj_size: 0,
        n_objects: 0,
        frstr_target: 0,
        frstr_access: 0,
        n_frstr: 0,
        frimg_target: 0,
        frimg_access: 0,
        n_frimg: 0,
        ptr_size: 0,
        obj_access: 0,
        ted_target: 0,
        ted_access: 0,
        n_ted: 0,
        ted_size: 0,
        ib_target: 0,
        ib_access: 0,
        n_ib: 0,
        ib_size: 0,
        bb_target: 0,
        bb_access: 0,
        n_bb: 0,
        bb_size: 0,
        blob_target: 0,
        blob_len: 0,
        trindex_target: 0,
    };
}

// ─── The per-process resource lists (A39) ────────────────────────────────
//
// EmuTOS keeps the resource state in the application's `AESGLOBAL` and
// re-binds to the caller's copy on every call (`rs_sglobe`,
// `aes/gemrslib.c:673-679`), so each AES process has its own state and may
// hold several loaded blobs, of which its array names one. NewTOS keeps the
// equivalent here: every entry is one `rsrc_load` of one process, keyed by the
// loading process's AES pid (`appl::current_pid`, GEMDOS slot + 1), and one
// entry per pid is *selected*. XaAES's per-client `resources` list
// (`xaaes/src.km/xa_rsrc.c:895-900`) is the model for the list itself.

/// One entry of the per-process resource lists.
#[derive(Clone, Copy)]
struct RscEntry {
    /// AES pid of the process that loaded it; 0 = free entry.
    pid: i16,
    /// The entry the pid's `rsrc_free`/`rsrc_gaddr`/`rsrc_saddr` act on. At
    /// most one entry per pid. Set by the load that created the entry, cleared
    /// by `rsrc_free` — stage 1's stand-in for the `ap_rscmem` selector.
    selected: bool,
    /// Pool `m_start` of the EL0 process's `.RSC` block ([`load_into_user`]),
    /// for `Mfree` at `rsrc_free` and to identify the `usermem` window entry
    /// to drop; `0` for the in-place kernel path. A [`Long`] (Atari 32-bit
    /// pool address) so it feeds `mfree` directly.
    raw: Long,
    /// The descriptor [`relocate_and_register_core`] produced for the blob.
    desc: Loaded,
}

impl RscEntry {
    const FREE: RscEntry = RscEntry { pid: 0, selected: false, raw: 0, desc: Loaded::EMPTY };
}

/// Capacity of the resource lists, over all processes together. EmuTOS has no
/// such bound (each load is a `dos_alloc_anyram`, `gemrslib.c:775`); what it
/// has is the failure mode, `rsrc_load` = 0 when the allocation fails
/// (`:776`), and that is what a full table reports. Sized for every AES
/// process (`pd::MAX_APPS` = 8) holding two blobs at once — TeraDesk's
/// `load_icons` needs `desktop.rsc` and `cicons.rsc` together
/// (`sdk/apps/teradesk/icon.c:2705-2766`).
const MAX_LOADED: usize = 16;

struct RscTable(UnsafeCell<[RscEntry; MAX_LOADED]>);
// SAFETY: single boot CPU only touches the AES (the single-trap-path
// discipline of the AES descriptor tables).
unsafe impl Sync for RscTable {}
static RSC: RscTable = RscTable(UnsafeCell::new([RscEntry::FREE; MAX_LOADED]));

/// Borrow the resource lists.
///
/// # Safety
/// Single trap path (no re-entrancy).
#[allow(clippy::mut_from_ref)]
unsafe fn table() -> &'static mut [RscEntry; MAX_LOADED] {
    // SAFETY: see `RscTable`'s Sync impl.
    unsafe { &mut *RSC.0.get() }
}

/// The AES pid of the process making the current call — the key of its
/// resource list. The same id `appl_init` reports (`appl::current_pid`):
/// GEMDOS slot + 1 for an EL0 caller, 1 for the kernel boot context, which
/// shares the id with the first program launched into slot 0 (A29).
fn caller_pid() -> i16 {
    super::appl::current_pid()
}

/// Index of the entry `pid`'s resource calls act on — EmuTOS's `rs_sglobe`:
/// the blob the caller's `AESGLOBAL` names (`gemrslib.c:677-678`). `None` is
/// "nothing loaded" (`rs_free` → 0, `:685`).
///
/// **Stage 1 (this):** the entry the pid's latest `rsrc_load` marked
/// `selected`, until its `rsrc_free` — for an application that leaves its
/// `AESGLOBAL` alone that is the blob `ap_rscmem` names (`:786-788`).
///
/// **Stage 2 (A39, after A60 — not built):** read the 4-byte `ap_rscmem`
/// (byte 14) out of the caller's `AESGLOBAL` and return the entry of *this*
/// pid whose `desc.blob_target` equals it; 0, a null `global`, another
/// process's blob or any foreign address → `None`. That is the selector an
/// application rewrites (TeraDesk `load_icons`, `icon.c:2707-2763`), and it
/// needs A60 first because TeraDesk-LP64 restores it at the wrong bytes
/// (spec § `rsrc_load`, A39 callout, item 4; analysis § 3).
///
/// # Safety
/// Single trap path.
unsafe fn selected(pid: i16) -> Option<usize> {
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    t.iter().position(|e| e.pid == pid && e.selected)
}

/// Enter a completed load into `pid`'s list and select it — the list half
/// of `rs_readit`'s "init global" (`gemrslib.c:786-788`): the new blob is
/// the one the process's later calls act on, and no earlier load of the
/// process is freed. `false` when the table is full (the load must then be
/// undone by the caller; `rsrc_load` reports 0 as for EmuTOS's failed
/// `dos_alloc_anyram`, `:775-776`).
///
/// # Safety
/// Single trap path.
unsafe fn register(pid: i16, raw: Long, desc: Loaded) -> bool {
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    let Some(idx) = t.iter().position(|e| e.pid == 0) else {
        return false;
    };
    for e in t.iter_mut() {
        if e.pid == pid {
            e.selected = false;
        }
    }
    t[idx] = RscEntry { pid, selected: true, raw, desc };
    true
}

/// Drop every entry of the process with AES pid `pid` — the AES side of
/// process termination, called from `Pterm` and `Ptermres`
/// (`bdos::process::pterm::terminate_inner`). The blobs themselves are the
/// process's pool blocks and are reclaimed there by owner (`free_all_owned`,
/// EmuTOS `ixterm` → `free_all_owned`, `bdos/proc.c:136`); the process's
/// window table is re-seeded for its next tenant (`usermem::seed_tpa`). What
/// must go here is the **descriptor**: the next program in the same GEMDOS
/// slot gets the same pid, and `appl_init` reuses the same AES descriptor
/// (`pd::open`, A29) — without this it would inherit entries whose kernel
/// aliases point at freed pool memory. Only `pid`'s entries; every other
/// process keeps its state. `appl_exit` does not call this (`ap_exit`,
/// `aes/gemaplib.c:251-261`).
pub fn on_process_exit(pid: i16) {
    // SAFETY: called on the single trap path (`Pterm` under the BDOS giant
    // lock), no AES call in flight.
    let t = unsafe { table() };
    for e in t.iter_mut() {
        if e.pid == pid {
            *e = RscEntry::FREE;
        }
    }
}

/// ILP32 OBJECT record stride (4-byte `ob_spec` @ +12; see `mt_gem.h`
/// `sizeof(OBJECT) == 24`). The LP64 stride is [`OBJECT_SIZE`] (32).
const OBJECT_SIZE_ILP32: usize = 24;
/// Byte offset of `ob_type` within an OBJECT (same in both ABIs).
const OB_TYPE_OFF: usize = 6;
/// Byte offset of `ob_spec` within an ILP32 OBJECT (4-byte field).
const OB_SPEC_OFF_ILP32: usize = 12;
/// Byte offset of `te_txtlen` within an **LP64** TEDINFO: three 8-byte
/// pointers then six WORDs (`te_font`…`te_thickness`).
const TE_TXTLEN_OFF: usize = 36;
/// Byte offset of `te_tmplen` within an LP64 TEDINFO (the word after
/// [`TE_TXTLEN_OFF`]).
const TE_TMPLEN_OFF: usize = 38;
/// Byte offset of `te_txtlen` within an ILP32 TEDINFO: three 4-byte pointers
/// then six WORDs (`te_font`…`te_thickness`).
const TE_TXTLEN_OFF_ILP32: usize = 24;
/// Byte offset of `te_tmplen` within an ILP32 TEDINFO (the word after
/// [`TE_TXTLEN_OFF_ILP32`]).
const TE_TMPLEN_OFF_ILP32: usize = 26;

/// Check a candidate blob's header against the bytes that really exist at
/// `access_base`, *before* anything writes through the offsets it declares.
///
/// This is EmuTOS's short-read guard, which NewTOS has to state explicitly.
/// `rs_readit` (`aes/gemrslib.c:775-783`) allocates exactly the declared
/// `rslsize` and refuses the file unless `dos_read` delivers all of it, so
/// `fix_objects` & co. can never step outside the allocation. NewTOS reads
/// into a fixed buffer through `bdos::fs::read_file`, which stops at the
/// buffer's end and reports only how much it copied — so here the same
/// guarantee has to be *checked*: `len` is what was read, `total_size` is what
/// the blob claims, and the fix-up walk writes at every offset the header
/// declares.
///
/// Refused (false → `rsrc_load` = 0, EmuTOS's FALSE):
/// * fewer bytes than the header itself, or no `NRSC` magic,
/// * `total_size > len` — a short read, and equally a file too large for
///   [`RSC_CAP`], which `read_file` would otherwise truncate in silence,
/// * a declared array that does not lie wholly inside `total_size`.
///
/// The record strides depend on the caller's OBJECT width, hence `abi`.
///
/// # Safety
/// `access_base` must point at `len` readable bytes.
unsafe fn header_is_sound(access_base: *const u8, len: usize, abi: Abi) -> bool {
    // The header is read as a whole `NrscHeader`, so that many bytes must be
    // there even for a file generation that only defines the first 28 or 44 of
    // them — the `version` gates below decide what may be *interpreted*.
    if len < size_of::<NrscHeader>() {
        return false;
    }
    // SAFETY: `len` covers the header just checked for.
    let h = unsafe { &*(access_base as *const NrscHeader) };
    if &h.magic != b"NRSC" {
        return false;
    }
    // The short-read guard itself: the blob must not claim more bytes than
    // were read, nor fewer than its own header.
    let size = h.total_size as u64;
    if size > len as u64 || size < size_of::<NrscHeader>() as u64 {
        return false;
    }
    let (obj, ted, ib, bb, ptr) = match abi {
        Abi::Ilp32 => (
            OBJECT_SIZE_ILP32,
            TEDINFO_SIZE_ILP32,
            ICONBLK_SIZE_ILP32,
            BITBLK_SIZE_ILP32,
            4usize,
        ),
        Abi::Lp64 => (OBJECT_SIZE, TEDINFO_SIZE, ICONBLK_SIZE, BITBLK_SIZE, 8usize),
    };
    // Every array the fix-up walk indexes must end inside the blob.
    let fits = |off: u32, count: u32, stride: usize| -> bool {
        (count as u64)
            .checked_mul(stride as u64)
            .and_then(|bytes| bytes.checked_add(off as u64))
            .is_some_and(|end| end <= size)
    };
    // The tree index widened with [`NRSC_VER_TRINDEX_PTR`]: pointer slots from
    // v7 on, 4-byte root indices before that.
    if !fits(h.off_trindex, h.n_trees, trindex_stride(h.version, ptr))
        || !fits(h.off_objects, h.n_objects, obj)
    {
        return false;
    }
    // v1/v2 leave the table fields undefined (they overlap the tree index),
    // so they are only checked — and later only read — from v3 on.
    if h.version >= NRSC_VER_TABLES
        && (!fits(h.off_frstr, h.n_frstr, ptr) || !fits(h.off_frimg, h.n_frimg, ptr))
    {
        return false;
    }
    // Likewise the sub-struct array bases, which exist from v5 on.
    if h.version >= NRSC_VER_SUBSTRUCTS
        && (!fits(h.off_ted, h.n_ted, ted)
            || !fits(h.off_ib, h.n_ib, ib)
            || !fits(h.off_bb, h.n_bb, bb))
    {
        return false;
    }
    true
}

/// Relocate a native `.RSC` blob in place (offsets → absolute pointers)
/// and register its trees for the **calling process** ([`caller_pid`]),
/// selecting it as that process's resource — the in-kernel branch of
/// [`rsrc_load`]. `base` points at the start of the blob, `len` is the
/// number of bytes that really exist there — the bytes read from the file,
/// not the capacity of the buffer holding them. The blob must remain live
/// until `rsrc_free` or the process's end.
///
/// Returns false, having written nothing, when the header does not hold up
/// against `len` (see [`header_is_sound`]), and false when the resource
/// lists are full ([`register`]) — the blob is then relocated but not
/// registered.
///
/// # Safety
/// `base` must point at `len` readable and writable bytes.
pub unsafe fn relocate_and_register(base: *mut u8, len: usize) -> bool {
    // In-place kernel path: access base == target base, LP64 (desktop / the
    // selftests always build 32-byte native resources).
    // SAFETY: caller guarantees `len` accessible bytes.
    let Some(desc) = (unsafe { relocate_and_register_core(base, len, base as u64, Abi::Lp64) })
    else {
        return false;
    };
    // SAFETY: single trap path.
    unsafe { register(caller_pid(), 0, desc) }
}

/// The relocation + descriptor half of a load, with the access/target-base
/// split of [`relocate_blob_core`]: the descriptor's tree/object pointers
/// resolve to `target_base`, while every field is touched through
/// `access_base`. Used by the EL0 `rsrc_load` path where `target_base` is
/// the user VA the blob is mapped at and `access_base` is the pool block's
/// kernel alias, and by [`relocate_and_register`] with the two equal.
///
/// Returns the [`Loaded`] descriptor of the relocated blob — which the
/// caller enters into the loading process's list ([`register`]) — or
/// `None`, having written nothing, when the header does not hold up against
/// `len`.
///
/// # Safety
/// As [`relocate_and_register`], with `access_base` reachable from the
/// kernel and the blob eventually visible at `target_base`.
unsafe fn relocate_and_register_core(
    access_base: *mut u8,
    len: usize,
    target_base: u64,
    abi: Abi,
) -> Option<Loaded> {
    // SAFETY: caller guarantees `len` accessible bytes.
    unsafe {
        // Relocate offsets → target VAs at the caller's OBJECT width. Both
        // cores validate the header against `len` first, so everything below
        // — the tree index included — stays inside the bytes that were read.
        let ok = match abi {
            Abi::Ilp32 => relocate_blob_core_ilp32(access_base, len, target_base),
            Abi::Lp64 => relocate_blob_core(access_base, len, target_base),
        };
        if !ok {
            return None;
        }
        // `rs_readit`'s `fix_tedinfo_std()` (gemrslib.c:798): recompute every
        // relocated TEDINFO's `te_txtlen`/`te_tmplen` from the strings, so no
        // length word can exceed the bytes the blob really holds.
        match abi {
            Abi::Ilp32 => fix_tedinfo_std_ilp32(access_base, target_base),
            Abi::Lp64 => fix_tedinfo_std(access_base, target_base),
        }
        let (obj_size, ptr_size) = match abi {
            Abi::Ilp32 => (OBJECT_SIZE_ILP32, 4usize),
            Abi::Lp64 => (OBJECT_SIZE, 8usize),
        };
        let h = &*(access_base as *const NrscHeader);
        let mut d = Loaded::EMPTY;
        // Target VA of the object array (what callers dereference).
        let objs_target = target_base + h.off_objects as u64;
        // The tree index is read through the kernel alias. From
        // [`NRSC_VER_TRINDEX_PTR`] on it already holds the relocated root
        // addresses (the relocation core above rebased every slot), so the
        // registered tree VA *is* the slot — which is exactly EmuTOS's
        // `get_addr(R_TREE, i) == ap_ptree[i]` (`gemrslib.c:124`). An older
        // blob holds a `[u32]` of root OBJECT *indices*; the byte VA of a root
        // is then `objs_target + index * obj_size`.
        let tri = access_base.add(h.off_trindex as usize);
        let n = (h.n_trees as usize).min(MAX_TREES);
        let ptr_slot = h.version >= NRSC_VER_TRINDEX_PTR;
        for (t, slot) in d.trees.iter_mut().enumerate().take(n) {
            *slot = if ptr_slot {
                let p = tri.add(t * ptr_size);
                match ptr_size {
                    4 => rd32(p) as u64,
                    _ => (p as *const u64).read_unaligned(),
                }
            } else {
                objs_target + rd32(tri.add(t * 4)) as u64 * obj_size as u64
            };
        }
        d.n_trees = n;
        d.obj_base = objs_target;
        // Kernel-reachable alias of the object array, for `rsrc_saddr`'s
        // R_OBJECT/R_OBSPEC/R_TREE writes (equals `objs_target` for the in-place
        // EL1 path; the pool block's kernel alias on the EL0 path).
        d.obj_access = access_base as u64 + h.off_objects as u64;
        d.obj_size = obj_size;
        d.n_objects = h.n_objects as usize;

        // Free-string / free-image pointer tables (NRSC >= v3). The slots were
        // already rebased onto `target_base` by the relocation core; here we
        // record the app-visible base (`*_target`, returned by R_FRSTR/R_FRIMG)
        // and the kernel-reachable base (`*_access`, used to dereference a slot
        // for R_STRING/R_IMAGEDATA). An older file leaves the counts at 0, so
        // those selectors report the documented "not found".
        d.ptr_size = ptr_size;
        if h.version >= NRSC_VER_TABLES {
            d.frstr_target = target_base + h.off_frstr as u64;
            d.frstr_access = access_base as u64 + h.off_frstr as u64;
            d.n_frstr = h.n_frstr as usize;
            d.frimg_target = target_base + h.off_frimg as u64;
            d.frimg_access = access_base as u64 + h.off_frimg as u64;
            d.n_frimg = h.n_frimg as usize;
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
            d.ted_target = target_base + h.off_ted as u64;
            d.ted_access = access_base as u64 + h.off_ted as u64;
            d.n_ted = h.n_ted as usize;
            d.ted_size = ted_size;
            d.ib_target = target_base + h.off_ib as u64;
            d.ib_access = access_base as u64 + h.off_ib as u64;
            d.n_ib = h.n_ib as usize;
            d.ib_size = ib_size;
            d.bb_target = target_base + h.off_bb as u64;
            d.bb_access = access_base as u64 + h.off_bb as u64;
            d.n_bb = h.n_bb as usize;
            d.bb_size = bb_size;
        }

        // The blob's own identity — EmuTOS's `rs_hdr` / `rslsize` / `ptreebase`
        // (`gemrslib.c:787-788`, `:595-597`). [`rsrc_load`] reports these three
        // through the caller's `AESGLOBAL`; every other entry point leaves the
        // caller's copy alone, exactly as EmuTOS only writes them in
        // `rs_readit`. `trindex_target` stays 0 for a blob older than
        // [`NRSC_VER_TRINDEX_PTR`], which has no pointer table to publish.
        d.blob_target = target_base;
        d.blob_len = h.total_size;
        d.trindex_target = if h.version >= NRSC_VER_TRINDEX_PTR {
            target_base + h.off_trindex as u64
        } else {
            0
        };
        Some(d)
    }
}

// ─── fix_tedinfo_std: the TEDINFO length words ──────────────────────────
//
// EmuTOS `fix_tedinfo_std` (`aes/gemrslib.c:655-669`), called by `rs_readit`
// right after the offsets have become pointers (`gemrslib.c:798`):
//
//     if (fix_ptr(R_TEPTEXT,  ii)) ted->te_txtlen = strlen(ted->te_ptext)  + 1;
//     if (fix_ptr(R_TEPTMPLT, ii)) ted->te_tmplen = strlen(ted->te_ptmplt) + 1;
//
// The two *length* words are recomputed from the strings instead of trusted
// from the file. That is what keeps `te_txtlen` inside the bytes that actually
// sit behind `te_ptext`, because every writer bounds itself by `te_txtlen` and
// not by the buffer: `ins_char` (`object.rs`, EmuTOS `gemobed.c:117`) writes up
// to index `te_txtlen - 1`, and `inf_sset` (EmuTOS `util/optimize.c:103`) does
// `strlcpy(te_ptext, pstr, te_txtlen)`. A `te_txtlen` larger than the buffer is
// therefore a write past its end.
//
// Only the `rs_readit` path runs this. EmuTOS's *built-in* AES resource is
// initialised by `gem_rsc_init` (`aes/gem_rsc.c`), which deliberately does not:
// `erd` reserves the full `te_txtlen` bytes per TEDINFO in `rs_te_text[]`
// (`tools/erd.c:3007`, `te_ptext_len += get_short(&ted->te_txtlen)`), so the
// file's lengths are already correct there. [`relocate_blob`] — NewTOS's
// counterpart for the embedded GEM.RSC — is left alone to match.
//
// This is the one place NewTOS splits an EmuTOS function: EmuTOS's
// `fix_tedinfo_std` both relocates the three TEDINFO string pointers and
// recomputes the two length words, because only a loaded resource ever runs
// it. NewTOS's *embedded* GEM.RSC is a converted file too, so it still needs
// the pointer half — that half therefore lives in `relocate_blob_core`'s array
// walk, which both entry points run, while the length half stays here, which
// only `relocate_and_register` (the `rs_readit` path) runs. The observable
// split is EmuTOS's own.
//
// The walk goes over the TEDINFO array by the header's `n_ted`, exactly like
// `fix_tedinfo_std`'s `for (ii = 0; ii < rs_hdr->rsh_nted; ii++)`, so a record
// no object references is recomputed as well.

/// `strlen(blob + off) + 1` for a TEDINFO length word.
///
/// EmuTOS calls plain `strlen`. NewTOS bounds the scan by the blob's declared
/// `total_size`, so a malformed resource cannot walk out of the buffer; for a
/// well-formed one — every string NUL-terminated inside the blob — the two
/// agree exactly. With no NUL in range the string is taken to end at the
/// blob's last byte, which still leaves `te_txtlen` inside the bytes that
/// exist. Clamped to `i16`, the width of the length word.
///
/// # Safety
/// `base` must address at least `size` readable bytes.
unsafe fn ted_len(base: *const u8, size: usize, off: usize) -> i16 {
    if off >= size {
        return 1;
    }
    let mut n = 0usize;
    // SAFETY: the scan stops at `size`, inside the caller's span.
    while off + n < size && unsafe { *base.add(off + n) } != 0 {
        n += 1;
    }
    if off + n >= size {
        // Unterminated: treat the blob's last byte as the terminator.
        n = size - off - 1;
    }
    (n + 1).min(i16::MAX as usize) as i16
}

/// LP64 `fix_tedinfo_std`, length half: recompute `te_txtlen`/`te_tmplen` of
/// every TEDINFO the header declares. See the section comment above.
///
/// A blob older than [`NRSC_VER_SUBSTRUCTS`] declares no TEDINFO array and has
/// none to walk (`relocate_blob_core` refuses such a file if an object needs
/// one at all).
///
/// # Safety
/// `access_base` must point at the blob already relocated onto `target_base`
/// by [`relocate_blob_core`].
unsafe fn fix_tedinfo_std(access_base: *mut u8, target_base: u64) {
    // SAFETY: caller guarantees a relocated blob.
    unsafe {
        let h = &*(access_base as *const NrscHeader);
        if h.version < NRSC_VER_SUBSTRUCTS {
            return;
        }
        let size = h.total_size as usize;
        let teds = access_base.add(h.off_ted as usize);
        for i in 0..h.n_ted as usize {
            let ted = teds.add(i * TEDINFO_SIZE);
            // `fix_ptr` returned FALSE for an absent pointer — leave the
            // length word alone, exactly as EmuTOS does.
            let ptext = (ted as *const u64).read_unaligned();
            if ptext != 0 {
                let off = ptext.wrapping_sub(target_base) as usize;
                (ted.add(TE_TXTLEN_OFF) as *mut i16)
                    .write_unaligned(ted_len(access_base, size, off));
            }
            let ptmplt = (ted.add(8) as *const u64).read_unaligned();
            if ptmplt != 0 {
                let off = ptmplt.wrapping_sub(target_base) as usize;
                (ted.add(TE_TMPLEN_OFF) as *mut i16)
                    .write_unaligned(ted_len(access_base, size, off));
            }
        }
    }
}

/// ILP32 twin of [`fix_tedinfo_std`]: 4-byte TEDINFO pointers, `te_txtlen` at
/// +24 and `te_tmplen` at +26 of the 28-byte record.
///
/// # Safety
/// As [`fix_tedinfo_std`], for a blob relocated by
/// [`relocate_blob_core_ilp32`].
unsafe fn fix_tedinfo_std_ilp32(access_base: *mut u8, target_base: u64) {
    // SAFETY: caller guarantees a relocated blob.
    unsafe {
        let h = &*(access_base as *const NrscHeader);
        if h.version < NRSC_VER_SUBSTRUCTS {
            return;
        }
        let size = h.total_size as usize;
        let tb = target_base as u32;
        let teds = access_base.add(h.off_ted as usize);
        for i in 0..h.n_ted as usize {
            let ted = teds.add(i * TEDINFO_SIZE_ILP32);
            let ptext = rd32(ted);
            if ptext != 0 {
                let off = ptext.wrapping_sub(tb) as usize;
                (ted.add(TE_TXTLEN_OFF_ILP32) as *mut i16)
                    .write_unaligned(ted_len(access_base, size, off));
            }
            let ptmplt = rd32(ted.add(4));
            if ptmplt != 0 {
                let off = ptmplt.wrapping_sub(tb) as usize;
                (ted.add(TE_TMPLEN_OFF_ILP32) as *mut i16)
                    .write_unaligned(ted_len(access_base, size, off));
            }
        }
    }
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

/// ILP32 twin of [`fix_nptrs64`] — EmuTOS `fix_nptrs` over 4-byte pointer
/// slots: relocate the field at byte `field` of each `stride`-byte record,
/// across `count` records of the array at `base`, leaving 0 as "absent".
/// Slots are read/written unaligned.
///
/// # Safety
/// `base` must point at ≥ `count * stride` readable/writable bytes, with
/// `field + 4 <= stride`.
unsafe fn fix_nptrs32(base: *mut u8, count: usize, stride: usize, field: usize, tb: u32) {
    for i in 0..count {
        // SAFETY: i < count; the caller guarantees the record span.
        let p = unsafe { base.add(i * stride + field) } as *mut u32;
        // SAFETY: 4 readable/writable bytes at `p`.
        unsafe {
            let v = p.read_unaligned();
            if v != 0 {
                p.write_unaligned(v.wrapping_add(tb));
            }
        }
    }
}

/// ILP32 twin of [`relocate_blob_core`]: relocate the 24-byte OBJECT array and
/// its 4-byte-pointer sub-structs from blob offsets to `target_base` VAs.
///
/// # Safety
/// `access_base` must point at `len` readable and writable bytes.
unsafe fn relocate_blob_core_ilp32(access_base: *mut u8, len: usize, target_base: u64) -> bool {
    // SAFETY: caller guarantees `len` accessible bytes.
    unsafe {
        // Nothing below may be touched until the header holds up against the
        // bytes that were actually read (EmuTOS's short-read guard).
        if !header_is_sound(access_base, len, Abi::Ilp32) {
            return false;
        }
        let h = &*(access_base as *const NrscHeader);
        let size = h.total_size as u64;
        let objs = access_base.add(h.off_objects as usize);
        let tb = target_base as u32;
        let n = h.n_objects as usize;
        // See the LP64 twin: a pre-v5 blob declares no sub-structure arrays,
        // so a record an object points at could not be relocated by count and
        // the file is refused rather than handed out with raw offsets in it.
        if h.version < NRSC_VER_SUBSTRUCTS {
            for i in 0..n {
                let ty = rd16(objs.add(i * OBJECT_SIZE_ILP32 + OB_TYPE_OFF)) & 0x00ff;
                if has_substruct(ty) {
                    return false;
                }
            }
        }
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
                // Every other type's `ob_spec` is a blob offset; the records
                // it may name have their own pointers fixed up by the array
                // walk below, not from here (EmuTOS separates `fix_objects`
                // from `rs_readit`'s array walks). The record still has to lie
                // inside the blob — see the LP64 twin.
                ty => {
                    if let Some(bytes) = substruct_bytes(
                        ty,
                        TEDINFO_SIZE_ILP32,
                        ICONBLK_SIZE_ILP32,
                        BITBLK_SIZE_ILP32,
                    ) {
                        if !record_fits(spec as u64, bytes, size) {
                            return false;
                        }
                    }
                    (obj.add(OB_SPEC_OFF_ILP32) as *mut u32)
                        .write_unaligned(spec.wrapping_add(tb));
                }
            }
        }
        // The sub-structure arrays by header count (EmuTOS `gemrslib.c:798-805`,
        // see the LP64 twin). ILP32 records: TEDINFO te_ptext@0/te_ptmplt@4/
        // te_pvalid@8, ICONBLK ib_pmask@0/ib_pdata@4/ib_ptext@8, BITBLK
        // bi_pdata@0.
        if h.version >= NRSC_VER_SUBSTRUCTS {
            let ted = access_base.add(h.off_ted as usize);
            let ib = access_base.add(h.off_ib as usize);
            for f in 0..SUBPTR_FIELDS {
                fix_nptrs32(ted, h.n_ted as usize, TEDINFO_SIZE_ILP32, f * 4, tb);
                fix_nptrs32(ib, h.n_ib as usize, ICONBLK_SIZE_ILP32, f * 4, tb);
            }
            fix_nptrs32(
                access_base.add(h.off_bb as usize),
                h.n_bb as usize,
                BITBLK_SIZE_ILP32,
                0,
                tb,
            );
        }
        // Free-string / free-image pointer tables (NRSC >= v3, 4-byte slots).
        if h.version >= NRSC_VER_TABLES {
            fix_nptrs32(access_base.add(h.off_frstr as usize), h.n_frstr as usize, 4, 0, tb);
            fix_nptrs32(access_base.add(h.off_frimg as usize), h.n_frimg as usize, 4, 0, tb);
        }
        // The tree index (NRSC >= v7), same treatment: EmuTOS's `fix_trindex`
        // relocates each slot of `rsh_trindex` in place (`gemrslib.c:599-602`).
        if h.version >= NRSC_VER_TRINDEX_PTR {
            fix_nptrs32(access_base.add(h.off_trindex as usize), h.n_trees as usize, 4, 0, tb);
        }
    }
    true
}

/// Relocate a native `.RSC` blob in place (offsets → pointers) **without**
/// touching the global tree table — for a second resource (e.g. the AES's
/// own GEM.RSC behind the file selector) that must coexist with the app's.
///
/// # Safety
/// As [`relocate_and_register`].
pub unsafe fn relocate_blob(base: *mut u8, len: usize) -> bool {
    // In-place relocation: the blob is accessed and dereferenced through
    // the same pointer, so the access base and the target base coincide.
    // SAFETY: caller guarantees `len` accessible bytes.
    unsafe { relocate_blob_core(base, len, base as u64) }
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
/// `access_base` must point at `len` readable and writable bytes.
unsafe fn relocate_blob_core(access_base: *mut u8, len: usize, target_base: u64) -> bool {
    // SAFETY: caller guarantees `len` accessible bytes.
    unsafe {
        // Nothing below may be touched until the header holds up against the
        // bytes that were actually read (EmuTOS's short-read guard).
        if !header_is_sound(access_base, len, Abi::Lp64) {
            return false;
        }
        let h = &*(access_base as *const NrscHeader);
        let size = h.total_size as u64;
        let objs = access_base.add(h.off_objects as usize) as *mut Object;
        let tb = target_base;
        // A pre-v5 blob carries no sub-structure array table (see
        // [`NRSC_VER_SUBSTRUCTS`]), so its TEDINFO/ICONBLK/BITBLK records
        // cannot be walked by count the way `rs_readit` walks them
        // (`gemrslib.c:798-805`). Following `ob_spec` to them instead was this
        // loader's old route and is exactly the defect that is being removed —
        // it reaches only the *referenced* records. Handing an object a record
        // whose internal pointers are still file offsets would be a wild
        // pointer, so the file is refused instead, which is `rs_readit`'s own
        // answer to a resource it cannot read whole (`gemrslib.c:782-783`,
        // FALSE). `mkrsc.py` emits v5/v6 exclusively, so no resource NewTOS
        // produces takes this path.
        if h.version < NRSC_VER_SUBSTRUCTS {
            for i in 0..h.n_objects as usize {
                if has_substruct((*objs.add(i)).ob_type & 0x00ff) {
                    return false;
                }
            }
        }
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
                // Every other type's `ob_spec` is a blob offset — a TEDINFO /
                // ICONBLK / BITBLK record for the text/icon/image types, a
                // string or user data otherwise. All of them are relocated the
                // same way, `fix_long(&obj->ob_spec)`; the records' *internal*
                // pointers are not reached from here but from their arrays
                // below, exactly as EmuTOS separates `fix_objects` from
                // `rs_readit`'s `fix_tedinfo_std`/`fix_nptrs` walks.
                ty => {
                    // A record `objc_draw` will dereference must lie inside
                    // the blob (see [`substruct_bytes`]).
                    if let Some(bytes) =
                        substruct_bytes(ty, TEDINFO_SIZE, ICONBLK_SIZE, BITBLK_SIZE)
                    {
                        if !record_fits(o.ob_spec, bytes, size) {
                            return false;
                        }
                    }
                    o.ob_spec += tb;
                }
            }
        }
        // The sub-structure arrays, by the counts the header declares —
        // EmuTOS `rs_readit` (`gemrslib.c:798-805`):
        //
        //     fix_tedinfo_std();                       /* rsh_nted   */
        //     ibcnt = rs_hdr->rsh_nib;
        //     fix_nptrs(ibcnt, R_IBPMASK);
        //     fix_nptrs(ibcnt, R_IBPDATA);
        //     fix_nptrs(ibcnt, R_IBPTEXT);
        //     fix_nptrs(rs_hdr->rsh_nbb, R_BIPDATA);
        //     fix_nptrs(rs_hdr->rsh_nstring, R_FRSTR);
        //     fix_nptrs(rs_hdr->rsh_nimages, R_FRIMG);
        //
        // Every declared record is relocated whether or not an object points
        // at it: a free image, a spare TEDINFO or an icon reached only through
        // `rsrc_gaddr` is a pointer afterwards, not a file offset. `get_addr`
        // places the relocatable pointers at the start of each record — the
        // three TEDINFO strings and the three ICONBLK pointers at
        // `0`/`ps`/`2*ps`, the single BITBLK raster at `0`
        // (`gemrslib.c:127-157`) — which is what the field loops below index.
        // The length words `fix_tedinfo_std` recomputes belong to the
        // `rs_readit` path only and are applied by [`fix_tedinfo_std`] from
        // [`relocate_and_register_core`]; see that function for why the
        // embedded resource must not get them.
        if h.version >= NRSC_VER_SUBSTRUCTS {
            let ted = access_base.add(h.off_ted as usize);
            let ib = access_base.add(h.off_ib as usize);
            for f in 0..SUBPTR_FIELDS {
                fix_nptrs64(ted, h.n_ted as usize, TEDINFO_SIZE, f * 8, tb);
                fix_nptrs64(ib, h.n_ib as usize, ICONBLK_SIZE, f * 8, tb);
            }
            fix_nptrs64(access_base.add(h.off_bb as usize), h.n_bb as usize, BITBLK_SIZE, 0, tb);
        }
        // Free-string / free-image pointer tables (NRSC >= v3, 8-byte slots):
        // each slot is a blob offset (string data for frstr, a BITBLK for
        // frimg) rebased onto `target_base`, leaving 0 as "empty".
        if h.version >= NRSC_VER_TABLES {
            fix_nptrs64(access_base.add(h.off_frstr as usize), h.n_frstr as usize, 8, 0, tb);
            fix_nptrs64(access_base.add(h.off_frimg as usize), h.n_frimg as usize, 8, 0, tb);
        }
        // The tree index (NRSC >= v7): a pointer table like the two above, each
        // slot a root-OBJECT byte offset rebased onto `target_base`. EmuTOS's
        // `fix_trindex` does exactly this over `rsh_ntree`
        // (`gemrslib.c:599-602`, `fix_long(ptreebase+ii)`); it is what makes
        // `ap_ptree[i] == rsrc_gaddr(R_TREE, i)` true (`get_addr`, `:124`).
        if h.version >= NRSC_VER_TRINDEX_PTR {
            fix_nptrs64(access_base.add(h.off_trindex as usize), h.n_trees as usize, 8, 0, tb);
        }
    }
    true
}

/// Number of relocatable pointer fields at the head of a TEDINFO
/// (`te_ptext`/`te_ptmplt`/`te_pvalid`) and of an ICONBLK
/// (`ib_pmask`/`ib_pdata`/`ib_ptext`). They are the records' first three
/// members in both ABIs, which is why `get_addr` addresses them as
/// `0`/`ps`/`2*ps` (`gemrslib.c:145-157`, `aesdefs.h:578-594`).
const SUBPTR_FIELDS: usize = 3;

/// EmuTOS `fix_nptrs(cnt, type)` (`gemrslib.c:640-646`):
///
/// ```c
/// for (i = 0; i < cnt; i++)
///     fix_long(get_addr(type, i));
/// ```
///
/// Relocate one 8-byte pointer field — the one at byte `field` of each
/// `stride`-byte record — across `count` records of the array at `base`,
/// leaving 0 as "absent" (EmuTOS's `-1L`; the native format uses 0). `count`
/// is the header's own record count, never anything derived from an object.
///
/// The slots are read and written unaligned, so a blob whose array base is not
/// 8-aligned is relocated correctly instead of faulting — the loader must cope
/// with any byte offset a file declares.
///
/// # Safety
/// `base` must point at ≥ `count * stride` readable/writable bytes, with
/// `field + 8 <= stride`.
unsafe fn fix_nptrs64(base: *mut u8, count: usize, stride: usize, field: usize, tb: u64) {
    for i in 0..count {
        // SAFETY: i < count; the caller guarantees the record span.
        let p = unsafe { base.add(i * stride + field) } as *mut u64;
        // SAFETY: 8 readable/writable bytes at `p`.
        unsafe {
            let v = p.read_unaligned();
            if v != 0 {
                p.write_unaligned(v + tb);
            }
        }
    }
}

/// Pointer to the root object of tree `idx` in a blob relocated **in place**
/// (LP64, `access_base == target_base` — [`relocate_blob`] /
/// [`relocate_and_register`]).
///
/// From [`NRSC_VER_TRINDEX_PTR`] on the tree index already holds the relocated
/// root address, so the slot is the answer (EmuTOS `ap_ptree[idx]`,
/// `gemrslib.c:124`); an older blob holds a root OBJECT *index* into the
/// object array.
///
/// # Safety
/// `base` must point at a relocated native `.RSC` blob with `idx` < n_trees.
pub unsafe fn tree_root(base: *mut u8, idx: usize) -> *mut Object {
    // SAFETY: caller guarantees a relocated blob.
    unsafe {
        let h = &*(base as *const NrscHeader);
        let tri = base.add(h.off_trindex as usize);
        if h.version >= NRSC_VER_TRINDEX_PTR {
            (tri.add(idx * 8) as *const u64).read_unaligned() as *mut Object
        } else {
            let objs = base.add(h.off_objects as usize) as *mut Object;
            objs.add(rd32(tri.add(idx * 4)) as usize)
        }
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

/// Size of the sub-structure record an `ob_spec` of base type `base_type`
/// points at — `ted`/`ib`/`bb` are the caller ABI's TEDINFO / ICONBLK / BITBLK
/// record strides — or `None` for a type whose `ob_spec` is not a record
/// pointer at all (a string, user data, a literal colour word).
///
/// The relocation does not *reach* those records this way any more: it walks
/// the header's TEDINFO/ICONBLK/BITBLK arrays by count, as `rs_readit` does
/// (`gemrslib.c:798-805`). What is left is the bound — `objc_draw`
/// dereferences `ob_spec` as one of these records, and a resource may not
/// steer that read outside the bytes it declares. Before the array walk this
/// bound came for free, because the record's own pointers were written through
/// `ob_spec`; it is stated explicitly now so the refusal does not quietly
/// disappear. EmuTOS has no equivalent (`fix_objects` just relocates) because
/// it has no memory protection to honour.
#[inline]
fn substruct_bytes(base_type: u16, ted: usize, ib: usize, bb: usize) -> Option<usize> {
    match base_type {
        G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => Some(ted),
        G_ICON => Some(ib),
        G_IMAGE => Some(bb),
        _ => None,
    }
}

/// Whether an object of `base_type` points at a sub-structure record at all.
/// Used to refuse a pre-v5 blob, whose header declares no sub-structure arrays
/// and whose records therefore cannot be relocated by count.
#[inline]
fn has_substruct(base_type: u16) -> bool {
    substruct_bytes(base_type, 0, 0, 0).is_some()
}

/// Whether a sub-structure record of `bytes` bytes at blob offset `off` lies
/// wholly inside a blob of `size` bytes.
#[inline]
fn record_fits(off: u64, bytes: usize, size: u64) -> bool {
    off.checked_add(bytes as u64).is_some_and(|end| end <= size)
}

/// Capacity of the single resource buffer (one loaded `.RSC` at a time), and
/// therefore the largest resource `rsrc_load` can hold.
///
/// EmuTOS has no such limit — `rs_readit` asks `dos_alloc_anyram(rslsize)` for
/// exactly the declared size (`aes/gemrslib.c:775`) — but it *does* have the
/// same failure mode: no memory means `return FALSE`, i.e. `rsrc_load` = 0.
/// NewTOS's fixed buffer moves that boundary earlier; what must not differ is
/// that the refusal is honest. A file larger than this is rejected by the
/// declared-size check in [`header_is_sound`], never truncated and relocated.
pub const RSC_CAP: usize = 64 * 1024;

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

/// Directory of the currently-launched program (e.g. `C:\APPS\QED\`) —
/// NewTOS's `rlr->p_appdir`, the first place `sh_find` looks
/// (`findfile` stage 1, `aes/gemshlib.c:415-423`). Set by the kernel launcher
/// ([`set_app_dir`], EmuTOS `p_setappdir`, `aes/gempd.c:90-107`) from the
/// launch path before the program enters EL0.
///
/// Capacity is EmuTOS's `p_appdir[LEN_ZPATH+2]` (`aes/struct.h:153`), so the
/// directory portion of any `MAXPATHLEN` path fits whole; the previous 64
/// bytes truncated a deep application directory in silence.
pub const APP_DIR_CAP: usize = 116;
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
///
/// EmuTOS `rs_load` (`aes/gemrslib.c:826-850`) copies the caller's spec into
/// its own `tmprsfname[MAXPATHLEN]` scratch and resolves it with `sh_find`:
///
/// ```c
/// strcpy(tmprsfname, rsfname);
/// if (!sh_find(tmprsfname)) return FALSE;
/// dosrc = dos_open(tmprsfname, 0);
/// ```
///
/// `sh_find` → `findfile` (`aes/gemshlib.c:406-472`) searches in four stages:
/// (1) `rlr->p_appdir` + the **name portion** of the spec (`sh_name`),
/// (2) the spec **unchanged** — the current directory, and the stage that
/// makes any qualified path, absolute or relative, resolvable,
/// (3) `\<name>` on the **current** drive, (4) the AES `PATH=`.
/// The application's own buffer is never rewritten, only the scratch.
///
/// NewTOS runs exactly that search through [`crate::aes::shell::sh_find`]
/// (whose stage 4 is documented-absent for want of a loader-populated AES
/// environment) and then reads the resolved path. Returns 1 on success,
/// 0 when the file is not found, is short, or does not relocate.
pub fn rsrc_load(pb: &Aespb) {
    let fname = pb.addrin(0) as *const u8;
    if fname.is_null() {
        pb.set_ret(0);
        return;
    }
    // `strcpy(tmprsfname, rsfname)` (`gemrslib.c:835`) — a MAXPATHLEN scratch,
    // so `sh_find` rewrites ours and not the application's buffer.
    let mut spec = [0u8; PATHBUF];
    let mut nlen = 0;
    // SAFETY: trusted GEM pointer to a NUL-terminated string.
    unsafe {
        while nlen < spec.len() - 1 {
            let c = *fname.add(nlen);
            if c == 0 {
                break;
            }
            spec[nlen] = c;
            nlen += 1;
        }
    }

    // `if (!sh_find(tmprsfname)) return FALSE;` (`gemrslib.c:836-837`).
    if !crate::aes::shell::sh_find(&mut spec) {
        pb.set_ret(0);
        return;
    }
    let n = spec.iter().position(|&c| c == 0).unwrap_or(spec.len());
    let Ok(path_str) = core::str::from_utf8(&spec[..n]) else {
        pb.set_ret(0);
        return;
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

    // SAFETY: single-CPU AES; exclusive logical owner of the buffer.
    let buf = unsafe { &mut *RSC_BUF.0.get() };
    let Some(len) = crate::bdos::fs::read_file(path_str, buf) else {
        pb.set_ret(0);
        return;
    };
    if len < size_of::<NrscHeader>() {
        pb.set_ret(0);
        return;
    }
    // `len` is what `read_file` copied — bounded by the file's own size *and*
    // by `RSC_CAP`, silently (`bdos/fs/mod.rs`, `min(entry.size, out.len())`).
    // It is therefore the only honest statement about how many bytes exist,
    // and every path below is bounded by it: a file that was cut short, or one
    // too large for the buffer, fails `header_is_sound` and is refused, the
    // way EmuTOS refuses a short `dos_read` (`gemrslib.c:782-783`).
    // The load is entered into the **calling process's** resource list and
    // becomes its selected resource (A39, [`register`]); nothing already
    // loaded — by this process or any other — is freed, as nothing in
    // `rs_load` frees (`gemrslib.c:786-788`).
    let pid = caller_pid();
    // SAFETY: `buf` now holds `len` file bytes; both relocation paths
    // validate the `NRSC` header against `len` before touching anything.
    let loaded = if run.is_null() {
        // Kernel-side caller: relocate in place, kernel-VA pointers (LP64).
        // One buffer serves this path, so a second in-place load while the
        // first is still registered would relocate over its bytes — no
        // caller does this (there is no EL1 `rsrc_load` caller at all, see
        // the module comment).
        let base = buf.as_mut_ptr();
        unsafe { relocate_and_register_core(base, len, base as u64, Abi::Lp64) }
            .filter(|&desc| unsafe { register(pid, 0, desc) })
    } else {
        // EL0 app caller: copy into per-process EL0-mapped memory and
        // relocate to user VAs at the caller's OBJECT width.
        unsafe { load_into_user(run, pid, buf, len, abi) }
    };
    // `rs_readit`'s "init global" (`gemrslib.c:786-788`) + `fix_trindex`'s
    // `ap_ptree` (`:595-597`): report the load back through the caller's
    // `AESGLOBAL`. Only on success — EmuTOS reaches those lines only after the
    // whole file is in memory.
    if let Some(desc) = loaded {
        // SAFETY: `global` is the caller's AESGLOBAL, the same array
        // `appl_init` fills (`aes/appl.rs`).
        unsafe { report_load(pb.global_ptr(), &desc) };
    }
    pb.set_ret(i16::from(loaded.is_some()));
}

/// Place a candidate native `.RSC` blob (`src[..len]`) into the calling
/// process's per-process resource window, register it in the list of the
/// process with AES id `pid` with **user** VAs — so an EL0 `rsrc_gaddr`
/// returns dereferenceable pointers — and select it. Returns the
/// descriptor, or `None` — leaving nothing allocated — on a bad header, no
/// slot, an exhausted window, `ENSMEM`, a mapping error, or full resource
/// lists.
///
/// Protected-memory realisation of EmuTOS `rs_readit` (gemrslib.c:775): the
/// RSC lives in per-application memory (a BDOS-pool block, `Mxalloc`),
/// separate from the app's TPA, reclaimed at `rsrc_free` / the process's
/// `Pterm` (owner tag).
///
/// # Safety
/// Single-CPU trap context (the implicit BDOS giant lock); `run` is the
/// current process basepage; `src[..len]` is a candidate blob.
unsafe fn load_into_user(
    run: *mut BasePage,
    pid: i16,
    src: &[u8; RSC_CAP],
    len: usize,
    abi: Abi,
) -> Option<Loaded> {
    // Reject an unusable blob before any allocation: not `NRSC`, cut short, or
    // claiming arrays outside the bytes that were read. The relocation core
    // checks the same thing again on the copy, but there is no reason to take
    // a pool block for a file we already know we will refuse.
    // SAFETY: `src` holds `len` readable bytes.
    if !unsafe { header_is_sound(src.as_ptr(), len, abi) } {
        return None;
    }
    let slot = crate::bdos::process::slot_for(run)?;

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
        return None;
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
        return None;
    };

    // Copy the raw file bytes into the block via its kernel alias, then
    // relocate offsets → user VAs (accessed through the alias, resolved to
    // `app_va`).
    // SAFETY: `base_kva` aliases `pages` live RW pool bytes; `src` is valid.
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr(), base_kva, len);
    }
    // The copy is `len` bytes long, so `len` also bounds the relocation here:
    // the block is larger (page-rounded), but only those bytes came from the
    // file and only they may be fixed up.
    // SAFETY: `base_kva` holds the `len`-byte copy; header re-checked inside.
    let desc = unsafe { relocate_and_register_core(base_kva, len, app_va, abi) };
    // Entering the load into the process's list can fail only when the lists
    // are full — the same "no room" `rsrc_load` = 0 as EmuTOS's failed
    // `dos_alloc_anyram` (`gemrslib.c:775-776`), undone the same way as a
    // refused blob.
    // SAFETY: single trap path.
    let desc = desc.filter(|&d| unsafe { register(pid, raw, d) });
    if desc.is_none() {
        // Drop the window entry and free the pool block.
        usermem::free_by_mstart(slot, raw);
        // SAFETY: `raw` is on the allocated list.
        unsafe { crate::bdos::mem::traps::mfree(raw) };
        return None;
    }

    // Order the blob writes (made through the kernel alias) before the app
    // reads them at `app_va`; `usermem::map` already published the PTEs, and
    // the window VAs were previously invalid so no TLBI is needed (QUIRK-001).
    // SAFETY: barrier-only.
    unsafe { usermem::publish() };

    desc
}

/// Byte offset of `ap_ptree` in the caller's `AESGLOBAL` (`global[5]`), a
/// 4-byte field on every ABI — see `docs/api/aes/application.md` §`AESGLOBAL`
/// and [`crate::aes::appl`]'s module note for why the pointer fields keep the
/// m68k widths.
const AP_PTREE_OFF: usize = 10;
/// Byte offset of `ap_rscmem` (`global[7]`), 4 bytes.
const AP_RSCMEM_OFF: usize = 14;
/// Byte offset of `ap_rsclen` (`global[9]`), a `UWORD` — 2 bytes, as
/// `gemrslib.h:22` declares it (gemlib: "only valid for resource files <64K").
const AP_RSCLEN_OFF: usize = 18;

/// Report a completed load back through the caller's `AESGLOBAL`, the way
/// EmuTOS's `rs_readit` does (`aes/gemrslib.c:786-788`):
///
/// ```c
/// rs_global = pglobal;
/// rs_global->ap_rscmem = rs_hdr;      /* the blob base   */
/// rs_global->ap_rsclen = rslsize;     /* its byte length */
/// ```
///
/// plus the tree table `fix_trindex` publishes a few lines later
/// (`:595-597`, `rs_global->ap_ptree = (OBJECT **)ptreebase`). Only
/// `rs_readit` writes these; `rs_free`/`rs_gaddr`/`rs_saddr` merely read them
/// back (`rs_sglobe`, `:675-679`), which is why [`rsrc_free`] leaves the
/// caller's copy untouched. NewTOS binds those three calls to the caller's
/// *selected* entry in its per-process list ([`selected`]) — stage 1 of
/// A39; reading the words back as the selector is stage 2, after A60 (see
/// the module comment).
///
/// The values are the **target** VAs — the same addresses [`rsrc_gaddr`]
/// hands out — so `ap_ptree[i] == rsrc_gaddr(R_TREE, i)`, EmuTOS's binding in
/// `get_addr` (`gemrslib.c:124`). The *slots* of that array are the caller's
/// pointer width (8 bytes LP64, 4 ILP32), because `_AESrscfile[i]` is a native
/// `OBJECT *` (`sdk/gemlib/overlay/gem.h`); the *field* that names the array
/// is 4 bytes on both ABIs, because `AESGLOBAL` is addressed by WORD index as
/// well as by byte offset.
///
/// **When nothing is written, and why.** Both address fields are four bytes,
/// so they can only name a blob that lives below 4 GiB in the caller's address
/// space. That holds by construction for every real caller: an EL0 app's blob
/// sits in its per-process window under `USER_TPA_MAX_VA` (32 MiB,
/// `bdos::process::mmu`). It does **not** hold for the in-kernel path, which
/// relocates in place in [`RSC_BUF`], a kernel high-half static: the low 32
/// bits of such a VA are a physical address that resolves to nothing at EL1
/// (after the high-half switch `TTBR0_EL1` maps nothing until a process is
/// installed — `kernel/src/arch/aarch64/mmu.rs`). Writing that truncation
/// would hand back a pointer that is wrong rather than absent, so this path
/// writes **none** of the three and the caller keeps its zeros — EmuTOS's
/// "nothing loaded" reading (`rs_free`, `gemrslib.c:685`). The load itself
/// still succeeds; the entry in the process's list ([`RSC`]) is what stage 1
/// works from, and `rsrc_gaddr` keeps answering at full width. There is no
/// EL1 caller of `rsrc_load` on a running system (measured 2026-09-13; see
/// the module comment), so this branch binds only the host test harness.
///
/// Likewise `ap_ptree` needs the blob to *have* a relocated tree table, i.e.
/// [`NRSC_VER_TRINDEX_PTR`] or newer. A v1–v6 blob carries root OBJECT
/// indices instead of addresses; there is no array whose slots are pointers,
/// so nothing is published. The three land together either way: a non-zero
/// `ap_rsclen` beside a zero `ap_rscmem` would state two contradictory things.
///
/// `desc` is the descriptor of the load just completed.
///
/// # Safety
/// `global` is the caller's `AESGLOBAL`: null, or writable for at least the
/// 30 bytes / 15 WORDs of `gemrslib.h:15-27`.
unsafe fn report_load(global: *mut i16, desc: &Loaded) {
    if global.is_null() {
        return;
    }
    let (base, ptree, len) = (desc.blob_target, desc.trindex_target, desc.blob_len);
    // `fits` is the whole rule: an address this narrow field cannot carry is
    // not carried at all.
    let fits = |v: u64| v != 0 && v <= u32::MAX as u64;
    if !fits(base) || !fits(ptree) {
        return;
    }
    let g = global as *mut u8;
    // SAFETY: `global` addresses a caller-allocated `AESGLOBAL` of at least
    // 30 bytes / 15 WORDs (`gemrslib.h:15-27`, `desk/gembind.h:27`); the last
    // byte touched is 19. The 4-byte fields at 10 and 14 are only 2-byte
    // aligned, hence `write_unaligned`.
    unsafe {
        (g.add(AP_PTREE_OFF) as *mut u32).write_unaligned(ptree as u32);
        (g.add(AP_RSCMEM_OFF) as *mut u32).write_unaligned(base as u32);
        // `ap_rsclen` is a UWORD and `rslsize` a LONG in EmuTOS too, so a
        // resource of 64 KiB or more reports its low word — the same value
        // EmuTOS reports, and the reason gemlib documents the field as "only
        // valid for resource files <64K". [`RSC_CAP`] bounds ours at 64 KiB.
        (g.add(AP_RSCLEN_OFF) as *mut u16).write_unaligned(len as u16);
    }
}

/// `rsrc_free` (111) — free the calling process's selected resource.
///
/// EmuTOS `rs_free` (`aes/gemrslib.c:685-708`): `rs_sglobe(pglobal)` binds
/// to the blob the **caller's** `AESGLOBAL` names; nothing named → 0;
/// otherwise the blob is `dos_free`d and the result is 1, or 0 if that free
/// failed. NewTOS acts on the caller's selected entry ([`selected`]): the
/// EL0 pool block goes back to the pool and its `usermem` window entry is
/// dropped, the entry leaves the process's list, and the process has no
/// selected resource until its next `rsrc_load`. An earlier load of the same
/// process stays loaded and unselected — as on EmuTOS, where the words keep
/// naming the freed blob, a second `rsrc_free` fails (`:699-704`), and the
/// earlier blob is the process's GEMDOS memory until it restores the words
/// (stage 2) or ends (R6). No other process's state is touched.
///
/// Returns 1 only if a resource was selected; 0 when nothing was, on a
/// double free, and when the pool refused the block (spec verification
/// criterion resource-fileselector.md § `rsrc_free`).
pub fn rsrc_free(pb: &Aespb) {
    let pid = caller_pid();
    // SAFETY: single trap path.
    let Some(idx) = (unsafe { selected(pid) }) else {
        pb.set_ret(0);
        return;
    };
    // SAFETY: single trap path; `idx` was just found in the table.
    let entry = unsafe { table()[idx] };
    // SAFETY: single trap path.
    unsafe { table()[idx] = RscEntry::FREE };

    // Return the EL0 RSC block to the pool and drop its `usermem` window
    // entry. The window *mapping* is intentionally left in place: the app
    // holds no live pointers after the free, and the bump cursor never
    // re-hands the VA, so the stale mapping is never remapped (no TLBI) nor
    // read. The block's owner tag makes a later `Pterm` a no-op for it (it is
    // now off the allocated list, so `free_all_owned` / a second `Mfree` skip
    // it). `raw == 0` is the in-place kernel path, which owns no pool block.
    let mut rc = 1;
    if entry.raw != 0 {
        if let Some(slot) =
            crate::bdos::process::slot_for(crate::bdos::process::current())
        {
            usermem::free_by_mstart(slot, entry.raw);
        }
        // SAFETY: single-CPU trap context (implicit BDOS lock); `raw` came
        // from `mxalloc` in `load_into_user` and has not been freed.
        if unsafe { crate::bdos::mem::traps::mfree(entry.raw) } != 0 {
            rc = 0; // `if (dos_free(...)) rc = 0;` (`gemrslib.c:699-700`)
        }
    }
    // The caller's `AESGLOBAL` is deliberately **not** cleared: EmuTOS's
    // `rs_free` frees the blob and leaves `ap_rscmem`/`ap_rsclen`/`ap_ptree`
    // holding their stale values (`gemrslib.c:685-708` writes none of them),
    // which is why the spec notes a program must not `rsrc_gaddr` after
    // `rsrc_free`. XaAES rewrites the words to the next entry of its list
    // (`xa_rsrc.c:1347-1362`); not adopted.
    pb.set_ret(rc);
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

/// Resolve the slot of resource object `index` of `R_*` type `rtype` in the
/// loaded resource `d`, faithful to EmuTOS `get_addr` (gemrslib.c:112-176)
/// and the R_* table (aesdefs.h:578-594). Shared by [`rsrc_gaddr`] (which
/// returns `Slot::target`) and [`rsrc_saddr`] (which writes through
/// `Slot::access`). The sub-pointer aliases resolve to a pointer field
/// *within* the indexed record at the caller's ABI width: `ob_spec` @16
/// (LP64) / @12 (ILP32); the TEDINFO/ICONBLK sub-pointers at `ptr_size`
/// strides (`te_ptmplt`/`ib_pdata` @`ptr_size`, `te_pvalid`/`ib_ptext`
/// @`2*ptr_size`). `None` is EmuTOS's `-1L` (unknown type) or NewTOS's safer
/// not-found (absent table / out-of-range index).
///
/// # Safety
/// Single-CPU AES; `d` was produced by [`relocate_and_register_core`] for a
/// blob that is still live.
unsafe fn get_addr(d: &Loaded, rtype: i16, index: usize) -> Option<Slot> {
    let ps = d.ptr_size as u64;
    let obj_t = d.obj_base;
    let obj_a = d.obj_access;
    let n_obj = d.n_objects;
    let obj_sz = d.obj_size;
    match rtype {
        R_TREE => {
            if index >= d.n_trees {
                return None;
            }
            // The tree root lives inside the OBJECT array, so its kernel
            // alias uses the same blob delta as the object array.
            let target = d.trees[index];
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
        R_TEDINFO | R_TEPTEXT => {
            array_slot(d.ted_target, d.ted_access, d.n_ted, d.ted_size, 0, index)
        }
        R_TEPTMPLT => {
            array_slot(d.ted_target, d.ted_access, d.n_ted, d.ted_size, ps, index)
        }
        R_TEPVALID => {
            array_slot(d.ted_target, d.ted_access, d.n_ted, d.ted_size, 2 * ps, index)
        }
        R_ICONBLK | R_IBPMASK => {
            array_slot(d.ib_target, d.ib_access, d.n_ib, d.ib_size, 0, index)
        }
        R_IBPDATA => array_slot(d.ib_target, d.ib_access, d.n_ib, d.ib_size, ps, index),
        R_IBPTEXT => {
            array_slot(d.ib_target, d.ib_access, d.n_ib, d.ib_size, 2 * ps, index)
        }
        R_BITBLK | R_BIPDATA => {
            array_slot(d.bb_target, d.bb_access, d.n_bb, d.bb_size, 0, index)
        }
        R_FRSTR => {
            array_slot(d.frstr_target, d.frstr_access, d.n_frstr, d.ptr_size, 0, index)
        }
        R_FRIMG => {
            array_slot(d.frimg_target, d.frimg_access, d.n_frimg, d.ptr_size, 0, index)
        }
        // SAFETY: reads the frstr/frimg table via its kernel alias, live per
        // this function's contract.
        R_STRING => unsafe {
            deref_slot(d.frstr_target, d.frstr_access, d.n_frstr, d.ptr_size, index)
        },
        R_IMAGEDATA => unsafe {
            deref_slot(d.frimg_target, d.frimg_access, d.n_frimg, d.ptr_size, index)
        },
        _ => None,
    }
}

/// `rsrc_gaddr` (112) — address of resource object `int_in[1]` of `R_*` type
/// `int_in[0]` → `addr_out[0]`, resolved in the **calling process's selected
/// resource** (EmuTOS `rs_gaddr`, `gemrslib.c:715-721`: `rs_sglobe(pglobal)`
/// then `get_addr`; here [`selected`] then [`get_addr`]). Resolves all 17
/// EmuTOS `get_addr` selectors (aesdefs.h:578-594): `R_TREE`/`R_OBJECT`, the
/// TEDINFO/ICONBLK/BITBLK array bases (`R_TEDINFO`/`R_ICONBLK`/`R_BITBLK`),
/// their sub-pointer aliases (`R_OBSPEC`/`R_TEP*`/`R_IBP*`/`R_BIPDATA`), the
/// free-string / free-image slot addresses (`R_FRSTR`/`R_FRIMG`), and the
/// dereferenced free data pointers (`R_STRING`/`R_IMAGEDATA`). No selected
/// resource (nothing loaded, or freed), an unknown type, or a selector whose
/// table is absent (a v1–v4 resource for the substruct/free families) /
/// index out of range, yields the documented "not found" (`addr_out[0] = 0`,
/// `int_out[0] = 0`).
pub fn rsrc_gaddr(pb: &Aespb) {
    let rtype = pb.intin(0);
    let index = pb.intin(1) as usize;
    // SAFETY: single trap path; a selected entry's descriptor is live until
    // its `rsrc_free` / the process's end.
    let addr = unsafe {
        selected(caller_pid())
            .and_then(|idx| get_addr(&table()[idx].desc, rtype, index))
            .map_or(0, |s| s.target)
    };
    pb.set_addrout(0, addr);
    pb.set_ret(if addr != 0 { 1 } else { 0 });
}

/// `rsrc_saddr` (113) — store the pointer `addr_in[0]` into the resource slot
/// designated by `R_*` type `int_in[0]` / index `int_in[1]` of the **calling
/// process's selected resource**. Faithful to EmuTOS `rs_saddr`
/// (gemrslib.c:728-742): `rs_sglobe(pglobal); psub = get_addr(rtype, rindex);
/// if (psub != -1) { *psub = rsaddr; return TRUE; } return FALSE;`. The store
/// goes through the slot's kernel-reachable alias at the caller's pointer
/// width (the slot is pointer-sized in every selector), so an EL0 app's
/// `.RSC` — mapped at a user VA — is updated without an EL1 access to that
/// user VA. Returns TRUE for a resolvable slot, FALSE for no selected
/// resource / an unknown type / absent table / out-of-range index (NewTOS's
/// safer not-found, matching [`get_addr`]).
pub fn rsrc_saddr(pb: &Aespb) {
    let rtype = pb.intin(0);
    let index = pb.intin(1) as usize;
    // The pointer to store, read at the caller's ABI width (4 bytes ILP32 /
    // 8 bytes LP64) — exactly the width of the destination pointer slot.
    let rsaddr = pb.addrin(0);
    // SAFETY: single trap path; `slot.access` is a kernel-reachable pointer
    // slot of `ptr_size` bytes, live until rsrc_free.
    let ok = unsafe {
        match selected(caller_pid()).map(|idx| &table()[idx].desc) {
            Some(d) => match get_addr(d, rtype, index) {
                Some(slot) => {
                    let p = slot.access as *mut u8;
                    match d.ptr_size {
                        4 => (p as *mut u32).write_unaligned(rsaddr as u32),
                        _ => (p as *mut u64).write_unaligned(rsaddr),
                    }
                    true
                }
                None => false,
            },
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

#[cfg(test)]
mod tests {
    //! A35 — what `rsrc_load` reports back through the caller's `AESGLOBAL`.
    //!
    //! These live **in-crate** and not in `tests/aes_rsrc_global_fields.rs`
    //! for one reason: the interesting branch is the one where the blob's
    //! address fits the four bytes `ap_ptree`/`ap_rscmem` have, and that is
    //! the EL0 branch — `load_into_user` relocates the blob onto a **user VA**
    //! (`app_va`, under `USER_TPA_MAX_VA` = 32 MiB) while touching it through
    //! the pool block's kernel alias. A host integration test cannot reach it:
    //! it has no page tables to map a user window with, and on a 64-bit host
    //! every real address is above 4 GiB, so the in-place path it *can* drive
    //! always takes the refusal branch.
    //!
    //! [`relocate_and_register_core`] is exactly the access/target split that
    //! makes the EL0 path work, so these tests use it the way
    //! `load_into_user` does: `access_base` = a buffer they own, `target_base`
    //! = the user VA the blob will be visible at. Nothing is faked — the same
    //! function, the same arguments' shape, only the memory is the test's.

    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// The AES's own file-selector resource: a genuine NRSC v7 blob (LP64,
    /// relocated tree index), the file `xtask gen-fsel-rsc` produces.
    const FSEL: &[u8] = include_bytes!("gem_fsel.nrsc");

    /// The tests hand `relocate_and_register_core` their own buffers and
    /// read the returned descriptor; nothing is entered into [`RSC`]. The
    /// lock only keeps the blob buffers from being relocated concurrently on
    /// the same static input.
    static RSC_LOCK: Mutex<()> = Mutex::new(());

    fn lock<'a>() -> MutexGuard<'a, ()> {
        RSC_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The user VA shape `load_into_user` hands `relocate_and_register_core`:
    /// the per-process EL0 window base (`usermem::WINDOW_BASE` = the 4 MiB
    /// `USER_TPA_TOP`). Measured on QEMU `raspi4b` for TeraDesk's 68 rsrc
    /// calls: `0x4000ac … 0x409381`.
    const APP_VA: u64 = 0x0040_0000;

    /// A `u32` field of the caller's `AESGLOBAL`, read at its byte offset.
    fn g32(global: &[i16; 16], off: usize) -> u32 {
        // SAFETY: `global` is 32 bytes and `off + 4 <= 32` at every call site.
        let b = unsafe { std::slice::from_raw_parts(global.as_ptr() as *const u8, 32) };
        u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// An 8-byte-aligned copy of `blob`. The blob's OBJECT / TEDINFO /
    /// ICONBLK / BITBLK records are `repr(C)` structs the loader dereferences
    /// in place, so the buffer that holds them must be at least as aligned as
    /// a pool page or [`RSC_BUF`] is — a plain `Vec<u8>` is not.
    struct Blob {
        words: Vec<u64>,
        len: usize,
    }

    impl Blob {
        fn new(blob: &[u8]) -> Self {
            let mut words = vec![0u64; blob.len().div_ceil(8)];
            // SAFETY: `words` holds at least `blob.len()` bytes.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    blob.as_ptr(),
                    words.as_mut_ptr() as *mut u8,
                    blob.len(),
                );
            }
            Self { words, len: blob.len() }
        }
        fn as_mut_ptr(&mut self) -> *mut u8 {
            self.words.as_mut_ptr() as *mut u8
        }
        fn bytes(&self) -> &[u8] {
            // SAFETY: `words` holds `len` initialised bytes.
            unsafe { std::slice::from_raw_parts(self.words.as_ptr() as *const u8, self.len) }
        }
    }

    /// Load `blob` as if it were mapped at `target`, exactly as
    /// `load_into_user` does, and return the owned access buffer (which must
    /// outlive every later `get_addr`), the descriptor, and the caller's
    /// `AESGLOBAL` after [`report_load`].
    fn load_at(blob: &[u8], target: u64) -> (Blob, Loaded, [i16; 16]) {
        let mut buf = Blob::new(blob);
        let len = buf.len;
        let p = buf.as_mut_ptr();
        // SAFETY: `buf` holds `len` readable/writable bytes; the core
        // validates the header against `len` before touching anything.
        let desc = unsafe { relocate_and_register_core(p, len, target, Abi::Lp64) }
            .expect("the blob must relocate");
        let mut global = [0i16; 16];
        // SAFETY: a 32-byte caller array, more than the 30-byte AESGLOBAL.
        unsafe { report_load(global.as_mut_ptr(), &desc) };
        (buf, desc, global)
    }

    /// A hand-built blob of an **older** generation: `version` with the
    /// pre-v7 tree index (`[u32]` of root OBJECT indices) and a single G_BOX
    /// object, whose `ob_spec` is a literal colour word. v1 is what
    /// `kernel/src/selftest_objc.rs::run_rsrc` builds; v5 is the generation
    /// every committed resource had before A35.
    fn old_blob(version: u32) -> Vec<u8> {
        const HDR: usize = size_of::<NrscHeader>(); // 68
        let off_tri = HDR;
        let off_obj = off_tri + 4;
        let total = off_obj + OBJECT_SIZE;
        let mut b = vec![0u8; total];
        b[0..4].copy_from_slice(b"NRSC");
        let put = |b: &mut Vec<u8>, i: usize, v: u32| {
            b[4 + i * 4..8 + i * 4].copy_from_slice(&v.to_le_bytes());
        };
        put(&mut b, 0, version);
        put(&mut b, 1, 1); // n_trees
        put(&mut b, 2, 1); // n_objects
        put(&mut b, 3, off_tri as u32);
        put(&mut b, 4, off_obj as u32);
        put(&mut b, 5, total as u32);
        // The v3+/v5+ table fields stay 0/0, i.e. empty tables inside the blob.
        b[off_tri..off_tri + 4].copy_from_slice(&0u32.to_le_bytes()); // root = obj 0
        let o = off_obj;
        b[o..o + 2].copy_from_slice(&(-1i16).to_le_bytes()); // ob_next
        b[o + 2..o + 4].copy_from_slice(&(-1i16).to_le_bytes()); // ob_head
        b[o + 4..o + 6].copy_from_slice(&(-1i16).to_le_bytes()); // ob_tail
        b[o + 6..o + 8].copy_from_slice(&G_BOX.to_le_bytes());
        b[o + 16..o + 24].copy_from_slice(&0x0001_0000u64.to_le_bytes()); // ob_spec
        b
    }

    /// EmuTOS `rs_readit` (`gemrslib.c:787-788`) + `fix_trindex` (`:595-597`):
    /// a load whose blob is reachable in four bytes reports all three fields.
    #[test]
    fn an_el0_load_reports_all_three_fields() {
        let _g = lock();
        let (_buf, _desc, global) = load_at(FSEL, APP_VA);
        let off_trindex = u32::from_le_bytes(FSEL[16..20].try_into().unwrap());
        let total = u32::from_le_bytes(FSEL[24..28].try_into().unwrap());
        assert_eq!(
            g32(&global, AP_PTREE_OFF),
            APP_VA as u32 + off_trindex,
            "ap_ptree must name the relocated tree index inside the blob"
        );
        assert_eq!(g32(&global, AP_RSCMEM_OFF), APP_VA as u32, "ap_rscmem");
        assert_eq!(global[9] as u16 as u32, total, "ap_rsclen");
    }

    /// `appl_init`'s six fields and the caller's own bytes must survive: a
    /// load writes bytes 10..=19 and nothing else, so `ap_planes`
    /// (`global[10]`, byte 20) and `ap_3resv` (byte 22) stay put. This is the
    /// index half of the `AESGLOBAL` contract (`application.md`
    /// §`AESGLOBAL`).
    #[test]
    fn a_load_writes_only_its_own_ten_bytes() {
        let _g = lock();
        let mut buf = Blob::new(FSEL);
        let len = buf.len;
        let p = buf.as_mut_ptr();
        // SAFETY: `buf` holds `len` readable/writable bytes.
        let desc = unsafe { relocate_and_register_core(p, len, APP_VA, Abi::Lp64) }
            .expect("the blob must relocate");
        let mut global = [0x5A5Ai16; 16];
        // SAFETY: a 32-byte caller array.
        unsafe { report_load(global.as_mut_ptr(), &desc) };
        for (i, w) in global.iter().enumerate() {
            if (5..=9).contains(&i) {
                continue; // ap_ptree / ap_rscmem / ap_rsclen
            }
            assert_eq!(*w, 0x5A5Ai16, "global[{i}] must not be touched");
        }
    }

    /// EmuTOS `get_addr(R_TREE, i)` **is** `ap_ptree[i]` (`gemrslib.c:124`).
    /// The relocated tree index must therefore hold, in caller-pointer-wide
    /// slot `i`, exactly the address `rsrc_gaddr(R_TREE, i)` returns.
    #[test]
    fn ap_ptree_slots_are_what_rsrc_gaddr_returns() {
        let _g = lock();
        let (buf, desc, global) = load_at(FSEL, APP_VA);
        let off_trindex = u32::from_le_bytes(FSEL[16..20].try_into().unwrap()) as usize;
        let n_trees = u32::from_le_bytes(FSEL[8..12].try_into().unwrap()) as usize;
        assert!(n_trees >= 3, "gem.rsc declares three trees");
        assert_eq!(g32(&global, AP_PTREE_OFF) as u64, APP_VA + off_trindex as u64);
        for i in 0..n_trees {
            // The slot, read through the blob's access alias (the kernel's
            // view); its *value* is a target VA, which is what an application
            // reading `ap_ptree[i]` would get.
            let d = buf.bytes();
            let slot = u64::from_le_bytes(
                d[off_trindex + i * 8..off_trindex + i * 8 + 8].try_into().unwrap(),
            );
            // SAFETY: `desc` describes `buf`, which is still live.
            let via_gaddr = unsafe { get_addr(&desc, R_TREE, i) }.expect("tree in range").target;
            assert_eq!(slot, via_gaddr, "ap_ptree[{i}] != rsrc_gaddr(R_TREE, {i})");
            assert!(slot >= APP_VA && slot < APP_VA + buf.len as u64);
        }
    }

    /// A blob the four-byte fields cannot name reports **nothing** — not a
    /// truncated address. That is the in-kernel (EL1) path, where the blob
    /// sits in [`RSC_BUF`], a kernel high-half static.
    #[test]
    fn a_blob_beyond_four_gigabytes_reports_nothing() {
        let _g = lock();
        let (_buf, _desc, global) = load_at(FSEL, 0xFFFF_0000_0015_B300);
        assert_eq!(g32(&global, AP_PTREE_OFF), 0, "ap_ptree");
        assert_eq!(g32(&global, AP_RSCMEM_OFF), 0, "ap_rscmem");
        assert_eq!(global[9], 0, "ap_rsclen");
    }

    /// Backward compatibility: a v1 and a v5 blob still load and still
    /// register their trees — only they publish no `ap_ptree`, because their
    /// tree index holds root *indices*, not addresses. v1 is the generation
    /// `kernel/src/selftest_objc.rs::run_rsrc` builds by hand.
    #[test]
    fn a_pre_v7_blob_still_loads_and_publishes_nothing() {
        let _g = lock();
        for version in [1u32, 5] {
            let (buf, desc, global) = load_at(&old_blob(version), APP_VA);
            let off_obj = u32::from_le_bytes(buf.bytes()[20..24].try_into().unwrap()) as u64;
            // SAFETY: `desc` describes `buf`, which is still live.
            let tree = unsafe { get_addr(&desc, R_TREE, 0) }.expect("tree 0").target;
            assert_eq!(tree, APP_VA + off_obj, "v{version}: tree 0 must register");
            assert_eq!(g32(&global, AP_PTREE_OFF), 0, "v{version}: ap_ptree");
            assert_eq!(g32(&global, AP_RSCMEM_OFF), 0, "v{version}: ap_rscmem");
            assert_eq!(global[9], 0, "v{version}: ap_rsclen");
        }
    }
}
