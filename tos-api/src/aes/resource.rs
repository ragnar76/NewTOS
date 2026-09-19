//! AES Resource Manager (`rsrc_*`, opcodes 110–114) — Batch 1.
//!
//! Spec: [`docs/api/aes/resource-fileselector.md`]. **NewTOS uses a native
//! resource format** (decision 2026-06-21): the on-disk `.RSC` stores our
//! own 64-bit `OBJECT` layout, little-endian, with internal pointers held
//! as **byte offsets from the blob base**. Loading is therefore a pure
//! offset→pointer relocation — no endianness swap and no 16→64-bit
//! expansion (unlike a classic Atari `.RSC`). The OBJECT coordinates are
//! the classic file's **packed char-grid words** (from NRSC v9/v10 on) and
//! are turned into pixels **at load**, per object, with the live
//! `gl_wchar`/`gl_hchar`/`gl_width` — EmuTOS's `rs_fixit` → `fix_objects`
//! → `rs_obfix` (`gemrslib.c:816-820`, `:606-637`, `:89-99`), see
//! [`fix_objects_coords`] and [`rs_obfix`]. `rsrc_obfix` (114) is the same
//! `rs_obfix`, applied by an application to an object of its own.
//!
//! Native `.RSC` layout (all little-endian). Five header generations coexist;
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
//! * **v9 (LP64) / v10 (ILP32)** — the same 68-byte header, but the OBJECT
//!   coordinates are the classic file's **packed char-grid words** (char
//!   count in the low byte, signed pixel offset in the high byte) instead
//!   of pixels: the loader runs `rs_obfix` over every object at load with
//!   the live `gl_wchar`/`gl_hchar`/`gl_width` (EmuTOS `fix_objects`,
//!   `gemrslib.c:606-637`), so the layout follows the machine the file is
//!   loaded on, not the one the converter assumed (A40). A v1–v8 blob holds
//!   pixels and is not converted — the tool of its day did that with
//!   `8 × 16 / 640` baked in.
//! * **v11 (LP64) / v12 (ILP32)** — an 84-byte header that additionally
//!   carries the **CICONBLK / CICON** array bases + counts of a new-format
//!   (colour-icon) `.rsc` (A41, spec §5). The records are the application's
//!   C structs (`CICONBLK { ICONBLK monoblk; CICON *mainlist; }`, `CICON {
//!   num_planes; col_data; col_mask; sel_data; sel_mask; next_res; }`,
//!   `obdefs.h:203-217`) with blob offsets in every pointer field, and a
//!   `G_CICON` object's `ob_spec` is the byte offset of its CICONBLK — so the
//!   relocation is the generic one. After it the loader runs EmuTOS's
//!   `fix_cicons` (`gemrslib.c:549-573`): `best_match` picks one variant per
//!   block for the live plane count and the colour data is turned into the
//!   device form ([`fix_cicons`]). A pre-v11 blob declares no such arrays;
//!   its `G_CICON` objects (which the tool of the day zeroed) stay empty.
//!
//! ```text
//!   NrscHeader: magic "NRSC", version, n_trees, n_objects,
//!               off_trindex, off_objects, total_size            (v1/v2: 28 B)
//!               [, off_frstr, n_frstr, off_frimg, n_frimg]      (v3/v4: 44 B)
//!               [, off_ted, n_ted, off_ib, n_ib, off_bb, n_bb]  (v5–v10: 68 B)
//!               [, off_cib, n_cib, off_cic, n_cic]              (v11+: 84 B)
//!   tree index       @ off_trindex  (v1–v6: [u32] root OBJECT index/tree;
//!                                    v7/v8: [ptr] offset → the root OBJECT)
//!   [Object; n_objects]          @ off_objects  (ob_spec = offset for the
//!                                                 string object types, else
//!                                                 a literal colour/char word;
//!                                                 v9/v10: ob_x/y/w/h packed
//!                                                 char-grid words, else px)
//!   [TedInfo; n_ted] [BitBlk; n_bb] [IconBlk; n_ib]             (v5+)
//!   [CiconBlk; n_cib] [Cicon; n_cic]                            (v11+)
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
//! process's loaded resources, **and the caller's `AESGLOBAL` says which one
//! a call acts on** — `rs_sglobe`'s binding (`:675-679`). `rsrc_load`
//! appends the new entry and names it through the caller's words
//! ([`report_load`]); `rsrc_free`/`rsrc_gaddr`/`rsrc_saddr` read the 4-byte
//! `ap_rscmem` back out of the caller's array and act on the entry of the
//! caller's own list it names ([`selected`]) — a value naming none of them
//! (0, another process's blob, a foreign or application-built address, a
//! null `global`) reads as "nothing loaded", because the kernel must not
//! parse a header it did not load; `appl_exit` touches nothing (`ap_exit`,
//! `aes/gemaplib.c:251-261`); at process end the process's entries go with
//! its memory ([`on_process_exit`], the counterpart of `ixterm` →
//! `free_all_owned`, `bdos/proc.c:136`). The model of the list is XaAES's
//! `client->resources` (`xaaes/src.km/xa_rsrc.c:895-900`), which stands in
//! for what EmuTOS gets from the application's own memory. That is the
//! whole of A39 (stage 1: the per-process list, `d7ab92d`; stage 2: the
//! selector read back, after A60 put TeraDesk's words at the TOS bytes,
//! `fc1bf02`). The contract is `docs/api/aes/resource-fileselector.md`
//! § `rsrc_load` (A39 callout, items 1–7); the reference comparison
//! `docs/analysis/rsrc-per-app-state-2026-09-19.md`.
//!
//! **A blob the four bytes cannot name.** `report_load` publishes only a
//! base that fits the field (below); a load it could not name — the
//! in-place kernel path, a pre-v7 blob without a tree table — leaves the
//! caller's `ap_rscmem` at 0, and 0 is then the name the entry answers to:
//! the zero selector reaches the process's latest such load and nothing
//! else. A named blob is never reached by 0, an unnamed one never by a
//! non-zero value. That keeps the A35 rule "the load itself still succeeds
//! and `rsrc_gaddr` still answers at full width" true for the one path that
//! has such loads (the host test harness; there is no EL1 caller on a
//! running system), without ever resolving a value the kernel did not
//! publish.
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

/// Native `CICONBLK` record stride — the application's C struct
/// (`CICONBLK { ICONBLK monoblk; CICON *mainlist; }`, `obdefs.h:213-217`,
/// gemlib `mt_gem.h`): the ICONBLK head followed by one pointer. LP64
/// 48 + 8 = 56, ILP32 36 + 4 = 40. `mainlist` sits at the ICONBLK size.
const CICONBLK_SIZE: usize = ICONBLK_SIZE + 8;
const CICONBLK_SIZE_ILP32: usize = ICONBLK_SIZE_ILP32 + 4;
/// Native `CICON` record stride (`obdefs.h:203-211`): a WORD `num_planes`,
/// padding to the pointer width, then five pointers `col_data`, `col_mask`,
/// `sel_data`, `sel_mask`, `next_res` — each at `k * ptr` for `k` in 1..=5.
/// LP64 8 + 5 × 8 = 48, ILP32 4 + 5 × 4 = 24.
const CICON_SIZE: usize = 48;
const CICON_SIZE_ILP32: usize = 24;
/// Number of pointer fields of a CICON record (the five after `num_planes`).
const CICON_PTR_FIELDS: usize = 5;

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

/// First NRSC `version` whose OBJECT coordinates are the classic file's
/// **packed char-grid words** (v9 LP64, v10 ILP32) — `ob_x`/`ob_y`/
/// `ob_width`/`ob_height` each an unsigned character count in the low byte
/// and a signed pixel offset in the high byte, exactly the bytes of the
/// classic `.rsc`. The header is unchanged (68 bytes); what moves is *when*
/// the char→pixel arithmetic happens: at load, in [`fix_objects_coords`],
/// with the AES's live `gl_wchar`/`gl_hchar`/`gl_width` — which is EmuTOS's
/// `rs_fixit` → `fix_objects` → `rs_obfix(obj, 0)` for every `rsh_nobs`
/// object (`aes/gemrslib.c:816-820`, `:606-637`, `:89-99`), run by
/// `rs_load` right after `rs_readit` (`:846`).
///
/// Before this generation the converter (`tools/mkrsc.py`) did that
/// arithmetic itself with `WCHAR, HCHAR, SCREEN_W = 8, 16, 640` baked in, so
/// every resource carried the pixels of a 640-wide 8×16 screen onto every
/// machine — an 80-character root stayed 640 px on a 1920-wide workstation
/// (audit B10, `docs/analysis/audit-applinit-rsrc-load-2026-09-11.md`).
/// A v1–v8 blob still holds those pixels and is loaded without conversion.
const NRSC_VER_CHARGRID: u32 = 9;

/// First NRSC `version` whose header carries the **CICONBLK / CICON** array
/// bases + counts (v11 LP64, v12 ILP32; 84-byte header) — the colour icons
/// of a new-format `.rsc` (TASK/A41, spec §5). The records are the
/// application's C structs with blob-offset pointer fields; a `G_CICON`
/// object's `ob_spec` is the byte offset of its CICONBLK, relocated by the
/// generic rule. The loader then runs [`fix_cicons`] over the arrays. A
/// pre-v11 blob declares no such arrays, and its `G_CICON` objects — whose
/// `ob_spec` the converter of the day zeroed — are left as they are.
const NRSC_VER_CICON: u32 = 11;

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
/// bytes for v5–v10, 84 bytes from v11. Each generation's extra fields exist
/// only for a high enough `version` (`>= NRSC_VER_TABLES` for the
/// free-string/-image table fields, `>= NRSC_VER_SUBSTRUCTS` for the
/// TEDINFO/ICONBLK/BITBLK fields, `>= NRSC_VER_CICON` for the CICONBLK/CICON
/// fields); for an older file those trailing bytes overlap the tree index /
/// objects and MUST NOT be interpreted (the loader gates on `version`).
#[repr(C)]
pub struct NrscHeader {
    /// `b"NRSC"`.
    pub magic: [u8; 4],
    /// Format version: 1 = LP64/28 B, 2 = ILP32/28 B, 3 = LP64/44 B (+tables),
    /// 4 = ILP32/44 B (+tables), 5 = LP64/68 B (+substruct bases), 6 = ILP32/
    /// 68 B (+substruct bases), 7/8 = pointer tree index
    /// ([`NRSC_VER_TRINDEX_PTR`]), 9/10 = char-grid coordinates
    /// ([`NRSC_VER_CHARGRID`]), 11/12 = 84-byte header with the CICONBLK /
    /// CICON arrays ([`NRSC_VER_CICON`]). Odd = LP64, even = ILP32.
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
    /// Offset to the `CICONBLK` array (v11+ only, else undefined).
    pub off_cib: u32,
    /// Number of `CICONBLK` records (v11+ only, else undefined).
    pub n_cib: u32,
    /// Offset to the `CICON` array (v11+ only, else undefined).
    pub off_cic: u32,
    /// Number of `CICON` records (v11+ only, else undefined).
    pub n_cic: u32,
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
    /// The device-form buffer of the blob's colour icons ([`fix_cicons`]),
    /// all-zero when there is none — what `rsrc_free` releases in place of
    /// EmuTOS's `free_cicon_buffers` (`gemrslib.c:515-536`).
    cicon: CiconBuf,
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
        cicon: CiconBuf { access: 0, len: 0 },
    };
}

// ─── The per-process resource lists (A39) ────────────────────────────────
//
// EmuTOS keeps the resource state in the application's `AESGLOBAL` and
// re-binds to the caller's copy on every call (`rs_sglobe`,
// `aes/gemrslib.c:673-679`), so each AES process has its own state and may
// hold several loaded blobs, of which its array names one. NewTOS keeps the
// equivalent here: every entry is one `rsrc_load` of one process, keyed by the
// loading process's AES pid (`appl::current_pid`, GEMDOS slot + 1), and the
// caller's `ap_rscmem` — matched against the entries' names — says which one
// a call acts on. XaAES's per-client `resources` list
// (`xaaes/src.km/xa_rsrc.c:895-900`) is the model for the list itself.

/// One entry of the per-process resource lists.
#[derive(Clone, Copy)]
struct RscEntry {
    /// AES pid of the process that loaded it; 0 = free entry.
    pid: i16,
    /// What the loading process received as `ap_rscmem` for this blob
    /// ([`published_name`]): the 4-byte blob base, or 0 when
    /// [`report_load`] could publish nothing (the base or the tree table
    /// does not fit the field). The caller's selector is matched against
    /// this — EmuTOS's `rs_hdr = pglobal->ap_rscmem` (`gemrslib.c:677`).
    name: u32,
    /// Among the pid's entries **without** a name, the latest loaded — the
    /// one a zero selector reaches (module comment, "a blob the four bytes
    /// cannot name"). Set by the load, gone with the entry, never re-set:
    /// after its `rsrc_free` the zero selector reaches nothing, the way the
    /// stale words of a freed blob reach nothing. Meaningless for a named
    /// entry.
    latest_unnamed: bool,
    /// Pool `m_start` of the EL0 process's `.RSC` block ([`load_into_user`]),
    /// for `Mfree` at `rsrc_free` and to identify the `usermem` window entry
    /// to drop; `0` for the in-place kernel path. A [`Long`] (Atari 32-bit
    /// pool address) so it feeds `mfree` directly.
    raw: Long,
    /// Pool `m_start` of the EL0 process's colour-icon device-form block
    /// (`desc.cicon`, [`load_into_user`]); `0` when there is none or when the
    /// buffer is the kernel heap's (the in-place path, freed by
    /// [`kernel_cicon_free`]).
    cicon_raw: Long,
    /// The descriptor [`relocate_and_register_core`] produced for the blob.
    desc: Loaded,
}

impl RscEntry {
    const FREE: RscEntry = RscEntry {
        pid: 0,
        name: 0,
        latest_unnamed: false,
        raw: 0,
        cicon_raw: 0,
        desc: Loaded::EMPTY,
    };
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

/// Index of the entry `pid`'s resource call acts on — EmuTOS's `rs_sglobe`
/// (`gemrslib.c:675-679`):
///
/// ```c
/// rs_global = pglobal;
/// rs_hdr = (RSHDR *)(rs_global->ap_rscmem);
/// ```
///
/// The 4-byte `ap_rscmem` (byte 14) is read out of the caller's `AESGLOBAL`
/// `global` and matched against the names of *this* pid's entries
/// ([`RscEntry::name`]): the blob the caller's words name, if the AES
/// loaded it for this process. `None` is "nothing loaded" (`rs_free` → 0,
/// `:685`) — for 0 while every entry is named, a null `global`, another
/// process's blob, and any foreign or application-built address: EmuTOS
/// would parse whatever header stands there (`get_addr`, `:100-104`), XaAES
/// follows the pointer and range-checks the result against that header
/// (`xa_rsrc.c:1839-1855`); NewTOS moves the check to the selector because
/// it must not parse a header it did not load (spec § `rsrc_load`, A39
/// callout, item 4). Base comparison, not range: TeraDesk's pre-A60 hybrid
/// `0x407000` lay inside `desktop.rsc` and named no blob
/// (`TASK/A60-STATUS-DONE.MD` § 3). Among the pid's *unnamed* entries — the
/// zero the caller was left with — the latest loaded answers (module
/// comment).
///
/// This is the selector an application rewrites and expects to be followed:
/// TeraDesk `load_icons` (`sdk/apps/teradesk/icon.c:2707-2708`,
/// `:2762-2763`) — since A60 at the TOS bytes on LP64 too.
///
/// # Safety
/// Single trap path; `global` is null or the caller's `AESGLOBAL`, readable
/// for the 30 bytes / 15 WORDs of `gemrslib.h:15-27`.
unsafe fn selected(pid: i16, global: *const i16) -> Option<usize> {
    if global.is_null() {
        return None;
    }
    // SAFETY: `global` addresses a caller-allocated `AESGLOBAL` of at least
    // 30 bytes; the 4-byte field at 14 is only 2-byte aligned, hence
    // `read_unaligned`.
    let sel = unsafe { ((global as *const u8).add(AP_RSCMEM_OFF) as *const u32).read_unaligned() };
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    t.iter()
        .position(|e| e.pid == pid && e.name == sel && (sel != 0 || e.latest_unnamed))
}

/// Enter a completed load into `pid`'s list under the name the caller
/// receives for it ([`published_name`]) — the list half of `rs_readit`'s
/// "init global" (`gemrslib.c:786-788`), whose `AESGLOBAL` half is
/// [`report_load`]: the new blob is the one the caller's words now name,
/// and no earlier load of the process is freed. A load the words cannot
/// name becomes the pid's latest unnamed entry. `false` when the table is
/// full (the load must then be undone by the caller; `rsrc_load` reports 0
/// as for EmuTOS's failed `dos_alloc_anyram`, `:775-776`).
///
/// # Safety
/// Single trap path.
unsafe fn register(pid: i16, raw: Long, cicon_raw: Long, desc: Loaded) -> bool {
    // SAFETY: caller upholds the single-trap-path invariant.
    let t = unsafe { table() };
    let Some(idx) = t.iter().position(|e| e.pid == 0) else {
        return false;
    };
    let name = published_name(&desc);
    if name == 0 {
        for e in t.iter_mut() {
            if e.pid == pid {
                e.latest_unnamed = false;
            }
        }
    }
    t[idx] = RscEntry { pid, name, latest_unnamed: name == 0, raw, cicon_raw, desc };
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
            // A colour-icon buffer of the in-place path is the kernel
            // heap's, not the process's pool memory, and goes with the
            // entry; a pool block (`cicon_raw != 0`) is reclaimed by owner
            // like the blob itself.
            if e.cicon_raw == 0 {
                // SAFETY: the buffer `kernel_cicon_alloc` gave this entry,
                // freed exactly once (the entry is cleared right after).
                unsafe { kernel_cicon_free(e.desc.cicon) };
            }
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
    // The CICONBLK / CICON arrays (v11+) — and, because the *loader* reads
    // the colour rasters through the kernel alias ([`fix_cicons`]), every
    // record's chain links and raster spans as well ([`cicon_tables_are_sound`]).
    if h.version >= NRSC_VER_CICON {
        let (cib, cic) = match abi {
            Abi::Ilp32 => (CICONBLK_SIZE_ILP32, CICON_SIZE_ILP32),
            Abi::Lp64 => (CICONBLK_SIZE, CICON_SIZE),
        };
        if !fits(h.off_cib, h.n_cib, cib) || !fits(h.off_cic, h.n_cic, cic) {
            return false;
        }
        // SAFETY: both arrays were just shown to lie inside `size <= len`.
        if !unsafe { cicon_tables_are_sound(access_base, h, abi) } {
            return false;
        }
    }
    true
}

/// Read a blob-offset pointer field at `p` at the caller's width (the
/// pre-relocation form: a `u32` for ILP32, a `u64` for LP64), unaligned.
///
/// # Safety
/// `p` must point at ≥ 4 (ILP32) / 8 (LP64) readable bytes.
#[inline]
unsafe fn rd_ptr(p: *const u8, abi: Abi) -> u64 {
    // SAFETY: caller guarantees the field's bytes.
    unsafe {
        match abi {
            Abi::Ilp32 => rd32(p) as u64,
            Abi::Lp64 => (p as *const u64).read_unaligned(),
        }
    }
}

/// Write a pointer field at `p` at the caller's width, unaligned (the
/// inverse of [`rd_ptr`]; an ILP32 store keeps the low 4 bytes).
///
/// # Safety
/// `p` must point at ≥ 4 (ILP32) / 8 (LP64) writable bytes.
#[inline]
unsafe fn wr_ptr(p: *mut u8, abi: Abi, v: u64) {
    // SAFETY: caller guarantees the field's bytes.
    unsafe {
        match abi {
            Abi::Ilp32 => (p as *mut u32).write_unaligned(v as u32),
            Abi::Lp64 => (p as *mut u64).write_unaligned(v),
        }
    }
}

/// Pointer width of `abi` in bytes.
#[inline]
fn ptr_size(abi: Abi) -> usize {
    match abi {
        Abi::Ilp32 => 4,
        Abi::Lp64 => 8,
    }
}

/// Number of raster WORDs in one plane of a CICONBLK's icons — EmuTOS's
/// `mono_words = muls(ib_wicon/16, ib_hicon)` (`gemrslib.c:235`), the unit
/// every CICON raster is sized in (`fixup_colour_icons`, `:196-208`).
#[inline]
fn mono_words(wicon: i16, hicon: i16) -> u64 {
    (wicon.max(0) as u64 / 16) * hicon.max(0) as u64
}

/// The CICONBLK / CICON records of a v11+ blob, checked **before** anything
/// is relocated: every `mainlist` / `next_res` link names a record of the
/// CICON array (so the chain walks stay inside it and a cycle is caught by
/// the step bound), every variant's plane count is one a device could hold
/// (`1..=16`), its `col_data`/`col_mask` are present and its four raster
/// spans — `mono_words × num_planes` WORDs of data, `mono_words` of mask,
/// the same pair again for the SELECTED image, which comes as a pair or not
/// at all (`fixup_colour_icons`, `gemrslib.c:204-209`) — lie inside
/// `total_size`. EmuTOS checks none of this (`fix_cicons` walks whatever the
/// file says, `:549-573`); NewTOS must, because [`fix_cicons`] reads those
/// rasters through the kernel alias to build the device form. Refusal is
/// `rs_readit`'s FALSE — the file is not loaded, not partially fixed up.
///
/// # Safety
/// The header's CICONBLK and CICON arrays must lie inside `total_size`
/// readable bytes at `base` (checked by the caller).
unsafe fn cicon_tables_are_sound(base: *const u8, h: &NrscHeader, abi: Abi) -> bool {
    let size = h.total_size as u64;
    let ps = ptr_size(abi);
    let (cib, cic, ib) = match abi {
        Abi::Ilp32 => (CICONBLK_SIZE_ILP32, CICON_SIZE_ILP32, ICONBLK_SIZE_ILP32),
        Abi::Lp64 => (CICONBLK_SIZE, CICON_SIZE, ICONBLK_SIZE),
    };
    let n_cic = h.n_cic as u64;
    // A CICON link is a blob offset that must be one of the array's records.
    let cicon_record = |off: u64| -> Option<*const u8> {
        let rel = off.checked_sub(h.off_cic as u64)?;
        if rel % cic as u64 != 0 || rel / cic as u64 >= n_cic {
            return None;
        }
        // SAFETY: `off` names a record of the array the caller bounded.
        Some(unsafe { base.add(off as usize) })
    };
    for i in 0..h.n_cib as usize {
        // SAFETY: record `i` of the bounded CICONBLK array.
        let rec = unsafe { base.add(h.off_cib as usize + i * cib) };
        // The eleven ICONBLK WORDs follow the three pointers; `ib_wicon` /
        // `ib_hicon` are the 6th and 7th (`obdefs.h`).
        // SAFETY: inside the record.
        let (wicon, hicon) = unsafe { (rd16(rec.add(3 * ps + 10)) as i16, rd16(rec.add(3 * ps + 12)) as i16) };
        let words = mono_words(wicon, hicon);
        // SAFETY: `mainlist` is the pointer right after the ICONBLK head.
        let mut link = unsafe { rd_ptr(rec.add(ib), abi) };
        let mut steps = 0u64;
        while link != 0 {
            let Some(c) = cicon_record(link) else {
                return false;
            };
            steps += 1;
            if steps > n_cic {
                return false; // a cycle: longer than the array itself
            }
            // SAFETY: `c` is a whole CICON record inside the array.
            let planes = unsafe { rd16(c) as i16 };
            if !(1..=16).contains(&planes) {
                return false;
            }
            // SAFETY: as above — the five pointer fields at `k * ps`.
            let field = |k: usize| unsafe { rd_ptr(c.add(k * ps), abi) };
            let (col_data, col_mask, sel_data, sel_mask, next) =
                (field(1), field(2), field(3), field(4), field(5));
            let Some(data_bytes) = words.checked_mul(planes as u64).and_then(|w| w.checked_mul(2))
            else {
                return false;
            };
            let mask_bytes = words * 2;
            if col_data == 0 || col_mask == 0 {
                return false;
            }
            let span = |off: u64, bytes: u64| off.checked_add(bytes).is_some_and(|end| end <= size);
            if !span(col_data, data_bytes) || !span(col_mask, mask_bytes) {
                return false;
            }
            if (sel_data == 0) != (sel_mask == 0) {
                return false;
            }
            if sel_data != 0 && (!span(sel_data, data_bytes) || !span(sel_mask, mask_bytes)) {
                return false;
            }
            link = next;
        }
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
    // selftests always build 32-byte native resources); a colour-icon buffer
    // comes from the kernel heap, reachable at one address.
    // SAFETY: caller guarantees `len` accessible bytes.
    let Some(desc) = (unsafe {
        relocate_and_register_core(base, len, base as u64, Abi::Lp64, &mut kernel_cicon_alloc)
    }) else {
        return false;
    };
    // SAFETY: single trap path.
    if unsafe { register(caller_pid(), 0, 0, desc) } {
        return true;
    }
    // SAFETY: the buffer this load's `kernel_cicon_alloc` gave it; the
    // descriptor is dropped here, so it is freed exactly once.
    unsafe { kernel_cicon_free(desc.cicon) };
    false
}

/// The relocation + descriptor half of a load, with the access/target-base
/// split of [`relocate_blob_core`]: the descriptor's tree/object pointers
/// resolve to `target_base`, while every field is touched through
/// `access_base`. Used by the EL0 `rsrc_load` path where `target_base` is
/// the user VA the blob is mapped at and `access_base` is the pool block's
/// kernel alias, and by [`relocate_and_register`] with the two equal.
///
/// `cicon_alloc(bytes)` provides the device-form buffer for the blob's
/// colour icons ([`fix_cicons`]) as `(kernel-reachable, app-visible)`
/// addresses of `bytes` bytes, or `None` — EmuTOS's `dos_alloc_anyram`
/// (`gemrslib.c:436`), which the in-place path takes from the kernel heap and
/// the EL0 path from a mapped pool block. It is called at most once, only for
/// a blob that has colour icons a variant was found for.
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
    cicon_alloc: &mut dyn FnMut(usize) -> Option<(u64, u64)>,
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
        // `rs_readit`'s `fix_cicons()` (gemrslib.c:795-797), right after the
        // pointers are in place: choose and transform the colour icons for
        // the live device.
        let cicon = fix_cicons(access_base, target_base, abi, &mut CiconDevice::live, cicon_alloc);
        // `rs_readit`'s `fix_tedinfo_std()` (gemrslib.c:798): recompute every
        // relocated TEDINFO's `te_txtlen`/`te_tmplen` from the strings, so no
        // length word can exceed the bytes the blob really holds.
        match abi {
            Abi::Ilp32 => fix_tedinfo_std_ilp32(access_base, target_base),
            Abi::Lp64 => fix_tedinfo_std(access_base, target_base),
        }
        // `rs_fixit` → `fix_objects` (gemrslib.c:816-820, :606-637): the
        // char→pixel fix of every object, with the live font cell and screen
        // width — `rs_load` runs it right after `rs_readit` (:846).
        fix_objects_coords(access_base, abi);
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
        d.cicon = cicon;
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
                // A pre-v11 blob has no CICONBLK array for a colour icon's
                // `ob_spec` to name (the converter of the day wrote 0), so
                // the word is left as it is (P6-012). From v11 on it is the
                // byte offset of the CICONBLK record — the LP64 twin — and
                // takes the generic arm below.
                G_CICON if h.version < NRSC_VER_CICON => {}
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
                        CICONBLK_SIZE_ILP32,
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
        // The CICONBLK / CICON arrays (NRSC >= v11): the three ICONBLK
        // pointers and `mainlist` of every CICONBLK, the five pointers of
        // every CICON — see the LP64 twin.
        if h.version >= NRSC_VER_CICON {
            let cib = access_base.add(h.off_cib as usize);
            for f in 0..SUBPTR_FIELDS {
                fix_nptrs32(cib, h.n_cib as usize, CICONBLK_SIZE_ILP32, f * 4, tb);
            }
            fix_nptrs32(cib, h.n_cib as usize, CICONBLK_SIZE_ILP32, ICONBLK_SIZE_ILP32, tb);
            let cic = access_base.add(h.off_cic as usize);
            for f in 1..=CICON_PTR_FIELDS {
                fix_nptrs32(cic, h.n_cic as usize, CICON_SIZE_ILP32, f * 4, tb);
            }
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
    unsafe {
        if !relocate_blob_core(base, len, base as u64) {
            return false;
        }
        // EmuTOS runs `rs_obfix` over its embedded resource as well, once
        // the workstation is open: `gem_rsc_fixit` (`aes/gem_rsc.c:636-641`,
        // called from `geminit.c:711`). Native LP64: the embedded blob and
        // every in-kernel caller of this entry are 32-byte OBJECTs.
        fix_objects_coords(base, Abi::Lp64);
    }
    true
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
                // Colour icon of a **pre-v11** blob: in EmuTOS the classic
                // `ob_spec` is an index into the CICONBLK pointer table
                // (`ob_spec = ciconblkptr[ob_spec]`, `gemrslib.c:623-626`),
                // and the converter of those generations, having no CICON
                // path, wrote 0 — there is no table to resolve it against,
                // so the word is left as it is (AUD-P6-012). From
                // [`NRSC_VER_CICON`] on the converter has already turned the
                // index into the **byte offset** of the native CICONBLK
                // record, so the object takes the generic arm below like a
                // G_ICON — the CICONBLK array's own pointers are fixed up by
                // the array walk, and [`fix_cicons`] then does what EmuTOS's
                // `fix_cicons` does to the resolved records.
                G_CICON if h.version < NRSC_VER_CICON => {}
                // Every other type's `ob_spec` is a blob offset — a TEDINFO /
                // ICONBLK / BITBLK / CICONBLK record for the text/icon/image
                // types, a string or user data otherwise. All of them are
                // relocated the same way, `fix_long(&obj->ob_spec)`; the
                // records' *internal* pointers are not reached from here but
                // from their arrays below, exactly as EmuTOS separates
                // `fix_objects` from `rs_readit`'s `fix_tedinfo_std`/
                // `fix_nptrs` walks.
                ty => {
                    // A record `objc_draw` will dereference must lie inside
                    // the blob (see [`substruct_bytes`]).
                    if let Some(bytes) =
                        substruct_bytes(ty, TEDINFO_SIZE, ICONBLK_SIZE, BITBLK_SIZE, CICONBLK_SIZE)
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
        // The CICONBLK / CICON arrays (NRSC >= v11) — the pointers EmuTOS's
        // `fixup_all_ciconblks` / `fixup_colour_icons` (`gemrslib.c:226-256`,
        // `:185-217`) compute in place: every CICONBLK's three ICONBLK
        // pointers (mono mask / data / label, at `0`/`ps`/`2*ps` like an
        // ICONBLK's) and its `mainlist` (right after the ICONBLK head), every
        // CICON's `col_data`/`col_mask`/`sel_data`/`sel_mask`/`next_res`
        // (at `k * ps`, `k` = 1..=5). The converter stored them as blob
        // offsets with 0 for "absent" / "end of chain"; here they become the
        // pointers EmuTOS has — the linked shape `fix_cicons` then walks.
        if h.version >= NRSC_VER_CICON {
            let cib = access_base.add(h.off_cib as usize);
            for f in 0..SUBPTR_FIELDS {
                fix_nptrs64(cib, h.n_cib as usize, CICONBLK_SIZE, f * 8, tb);
            }
            fix_nptrs64(cib, h.n_cib as usize, CICONBLK_SIZE, ICONBLK_SIZE, tb);
            let cic = access_base.add(h.off_cic as usize);
            for f in 1..=CICON_PTR_FIELDS {
                fix_nptrs64(cic, h.n_cic as usize, CICON_SIZE, f * 8, tb);
            }
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
fn substruct_bytes(base_type: u16, ted: usize, ib: usize, bb: usize, cib: usize) -> Option<usize> {
    match base_type {
        G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => Some(ted),
        G_ICON => Some(ib),
        G_IMAGE => Some(bb),
        G_CICON => Some(cib),
        _ => None,
    }
}

/// Whether an object of `base_type` points at a sub-structure record at all.
/// Used to refuse a pre-v5 blob, whose header declares no sub-structure arrays
/// and whose records therefore cannot be relocated by count. `G_CICON` is
/// not in the list: a pre-v11 blob's colour icons carry no record (their
/// `ob_spec` is 0) and such blobs kept loading before — the v11 gate in the
/// relocation cores is what makes a CICONBLK a record.
#[inline]
fn has_substruct(base_type: u16) -> bool {
    matches!(base_type, G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT | G_ICON | G_IMAGE)
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
    // The load is entered into the **calling process's** resource list
    // (A39, [`register`]) and named through the caller's `AESGLOBAL`
    // ([`report_load`], below); nothing already loaded — by this process or
    // any other — is freed, as nothing in `rs_load` frees
    // (`gemrslib.c:786-788`).
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
        unsafe {
            relocate_and_register_core(base, len, base as u64, Abi::Lp64, &mut kernel_cicon_alloc)
        }
        .filter(|&desc| {
            // SAFETY: single trap path; a refused registration frees the
            // buffer this load allocated, exactly once.
            unsafe {
                if register(pid, 0, 0, desc) {
                    true
                } else {
                    kernel_cicon_free(desc.cicon);
                    false
                }
            }
        })
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
    // The colour icons' device-form buffer, should the blob have any: a
    // second pool block of the process, mapped into its window like the
    // blob (EmuTOS's `dos_alloc_anyram`, `gemrslib.c:436`, in the
    // application's memory) — one block for all of them ([`fix_cicons`]'s
    // section comment). Its pool key is kept for `rsrc_free`.
    let mut cicon_raw: Long = 0;
    let mut cicon_alloc = |bytes: usize| -> Option<(u64, u64)> {
        let pages = (bytes as u64 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        // SAFETY: single-CPU trap context holds the implicit BDOS lock.
        let raw = unsafe { crate::bdos::mem::traps::mxalloc((pages + PAGE_SIZE) as Long, MX_PREFTTRAM) };
        if raw == 0 {
            return None;
        }
        let pa = (raw as u64 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        // SAFETY: `raw` addresses `pages + one page` live pool bytes owned
        // by the calling process.
        let Some(va) = (unsafe { usermem::map(slot, raw, pages, usermem::Kind::Rsc) }) else {
            // SAFETY: `raw` is on the allocated list.
            unsafe { crate::bdos::mem::traps::mfree(raw) };
            return None;
        };
        cicon_raw = raw;
        Some((pa.wrapping_add(VA_OFFSET), va))
    };
    // The copy is `len` bytes long, so `len` also bounds the relocation here:
    // the block is larger (page-rounded), but only those bytes came from the
    // file and only they may be fixed up.
    // SAFETY: `base_kva` holds the `len`-byte copy; header re-checked inside.
    let desc = unsafe { relocate_and_register_core(base_kva, len, app_va, abi, &mut cicon_alloc) };
    // Entering the load into the process's list can fail only when the lists
    // are full — the same "no room" `rsrc_load` = 0 as EmuTOS's failed
    // `dos_alloc_anyram` (`gemrslib.c:775-776`), undone the same way as a
    // refused blob.
    // SAFETY: single trap path.
    let desc = desc.filter(|&d| unsafe { register(pid, raw, cicon_raw, d) });
    if desc.is_none() {
        // Drop the window entries and free the pool blocks.
        if cicon_raw != 0 {
            usermem::free_by_mstart(slot, cicon_raw);
            // SAFETY: `cicon_raw` is on the allocated list.
            unsafe { crate::bdos::mem::traps::mfree(cicon_raw) };
        }
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

/// The `ap_rscmem` a load publishes for `desc` — the blob's 4-byte base —
/// or 0 when [`report_load`] publishes nothing: the base or the tree table
/// (`ap_ptree`, absent before [`NRSC_VER_TRINDEX_PTR`]) does not fit the
/// four bytes the field has; the three land together (see `report_load`).
/// One rule, read by both sides of the binding: `report_load` writes it,
/// [`register`] stores it as the entry's [`RscEntry::name`], and
/// [`selected`] matches the caller's word against it.
fn published_name(desc: &Loaded) -> u32 {
    // `fits` is the whole rule: an address this narrow field cannot carry is
    // not carried at all.
    let fits = |v: u64| v != 0 && v <= u32::MAX as u64;
    if fits(desc.blob_target) && fits(desc.trindex_target) {
        desc.blob_target as u32
    } else {
        0
    }
}

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
/// back (`rs_sglobe`, `:675-679`) — and so does NewTOS: `ap_rscmem` is the
/// selector [`selected`] matches against the entry's name, which is the
/// very value written here ([`published_name`], one rule for both), and
/// [`rsrc_free`] leaves the caller's copy untouched.
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
/// still succeeds: its entry in the process's list ([`RSC`]) carries the
/// name 0 the caller was left with, the zero selector reaches it, and
/// `rsrc_gaddr` keeps answering at full width (module comment, "a blob the
/// four bytes cannot name"). There is no EL1 caller of `rsrc_load` on a
/// running system (measured 2026-09-13; see the module comment), so this
/// branch binds only the host test harness.
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
    let name = published_name(desc);
    if name == 0 {
        return;
    }
    let (ptree, len) = (desc.trindex_target, desc.blob_len);
    let g = global as *mut u8;
    // SAFETY: `global` addresses a caller-allocated `AESGLOBAL` of at least
    // 30 bytes / 15 WORDs (`gemrslib.h:15-27`, `desk/gembind.h:27`); the last
    // byte touched is 19. The 4-byte fields at 10 and 14 are only 2-byte
    // aligned, hence `write_unaligned`.
    unsafe {
        (g.add(AP_PTREE_OFF) as *mut u32).write_unaligned(ptree as u32);
        (g.add(AP_RSCMEM_OFF) as *mut u32).write_unaligned(name);
        // `ap_rsclen` is a UWORD and `rslsize` a LONG in EmuTOS too, so a
        // resource of 64 KiB or more reports its low word — the same value
        // EmuTOS reports, and the reason gemlib documents the field as "only
        // valid for resource files <64K". [`RSC_CAP`] bounds ours at 64 KiB.
        (g.add(AP_RSCLEN_OFF) as *mut u16).write_unaligned(len as u16);
    }
}

/// `rsrc_free` (111) — free the resource the caller's `AESGLOBAL` names.
///
/// EmuTOS `rs_free` (`aes/gemrslib.c:685-708`): `rs_sglobe(pglobal)` binds
/// to the blob the **caller's** `AESGLOBAL` names; nothing named → 0;
/// otherwise the blob is `dos_free`d and the result is 1, or 0 if that free
/// failed. NewTOS acts on the entry of the caller's list its `ap_rscmem`
/// names ([`selected`]): the EL0 pool block goes back to the pool and its
/// `usermem` window entry is dropped, and the entry leaves the process's
/// list. An earlier load of the same process stays loaded — as on EmuTOS,
/// where the words keep naming the freed blob, a second `rsrc_free` fails
/// (`:699-704`), and the earlier blob is the process's GEMDOS memory until
/// it restores the words (TeraDesk `load_icons`) or ends (R6). A selector
/// naming none of the caller's blobs frees nothing and reports 0. No other
/// process's state is touched.
///
/// Returns 1 only if the words named a loaded resource; 0 when they named
/// nothing, on a double free, and when the pool refused the block (spec
/// verification criterion resource-fileselector.md § `rsrc_free`).
pub fn rsrc_free(pb: &Aespb) {
    let pid = caller_pid();
    // SAFETY: single trap path; `global` is the caller's AESGLOBAL.
    let Some(idx) = (unsafe { selected(pid, pb.global_ptr()) }) else {
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
    // The colour icons' device-form buffer first — `rs_free`'s
    // `free_cicon_buffers` (`gemrslib.c:694-697`, `:515-536`), which frees
    // every chosen variant's `col_data` buffer and turns the return code to
    // 0 when a free fails. NewTOS has one buffer per resource: a pool block
    // of the process (`cicon_raw`), or the kernel heap's on the in-place
    // path.
    if entry.cicon_raw != 0 {
        if let Some(slot) = crate::bdos::process::slot_for(crate::bdos::process::current()) {
            usermem::free_by_mstart(slot, entry.cicon_raw);
        }
        // SAFETY: single-CPU trap context; `cicon_raw` came from `mxalloc`
        // in `load_into_user` and has not been freed.
        if unsafe { crate::bdos::mem::traps::mfree(entry.cicon_raw) } != 0 {
            rc = 0; // `if (dos_free(cicon->col_data)) rc = -1;` (`:531`)
        }
    } else {
        // SAFETY: the entry's kernel-heap buffer, freed exactly once (the
        // entry was just cleared).
        unsafe { kernel_cicon_free(entry.desc.cicon) };
    }
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
/// `int_in[0]` → `addr_out[0]`, resolved in the **resource the caller's
/// `AESGLOBAL` names** (EmuTOS `rs_gaddr`, `gemrslib.c:715-721`:
/// `rs_sglobe(pglobal)` then `get_addr`; here [`selected`] then
/// [`get_addr`]). `R_TREE` is answered from the tree table of that blob,
/// which from NRSC v7/v8 on is literally the `ap_ptree[i]` array the load
/// published (`get_addr`, `:124`). Resolves all 17
/// EmuTOS `get_addr` selectors (aesdefs.h:578-594): `R_TREE`/`R_OBJECT`, the
/// TEDINFO/ICONBLK/BITBLK array bases (`R_TEDINFO`/`R_ICONBLK`/`R_BITBLK`),
/// their sub-pointer aliases (`R_OBSPEC`/`R_TEP*`/`R_IBP*`/`R_BIPDATA`), the
/// free-string / free-image slot addresses (`R_FRSTR`/`R_FRIMG`), and the
/// dereferenced free data pointers (`R_STRING`/`R_IMAGEDATA`). Words naming
/// no loaded blob of the caller (nothing loaded, freed, a foreign value), an
/// unknown type, or a selector whose table is absent (a v1–v4 resource for
/// the substruct/free families) / index out of range, yields the documented
/// "not found" (`addr_out[0] = 0`, `int_out[0] = 0`).
pub fn rsrc_gaddr(pb: &Aespb) {
    let rtype = pb.intin(0);
    let index = pb.intin(1) as usize;
    // SAFETY: single trap path; `global` is the caller's AESGLOBAL; a named
    // entry's descriptor is live until its `rsrc_free` / the process's end.
    let addr = unsafe {
        selected(caller_pid(), pb.global_ptr())
            .and_then(|idx| get_addr(&table()[idx].desc, rtype, index))
            .map_or(0, |s| s.target)
    };
    pb.set_addrout(0, addr);
    pb.set_ret(if addr != 0 { 1 } else { 0 });
}

/// `rsrc_saddr` (113) — store the pointer `addr_in[0]` into the resource slot
/// designated by `R_*` type `int_in[0]` / index `int_in[1]` of the **resource
/// the caller's `AESGLOBAL` names**. Faithful to EmuTOS `rs_saddr`
/// (gemrslib.c:728-742): `rs_sglobe(pglobal); psub = get_addr(rtype, rindex);
/// if (psub != -1) { *psub = rsaddr; return TRUE; } return FALSE;`. The store
/// goes through the slot's kernel-reachable alias at the caller's pointer
/// width (the slot is pointer-sized in every selector), so an EL0 app's
/// `.RSC` — mapped at a user VA — is updated without an EL1 access to that
/// user VA. Returns TRUE for a resolvable slot, FALSE for words naming no
/// loaded blob of the caller / an unknown type / absent table / out-of-range
/// index (NewTOS's safer not-found, matching [`get_addr`]).
pub fn rsrc_saddr(pb: &Aespb) {
    let rtype = pb.intin(0);
    let index = pb.intin(1) as usize;
    // The pointer to store, read at the caller's ABI width (4 bytes ILP32 /
    // 8 bytes LP64) — exactly the width of the destination pointer slot.
    let rsaddr = pb.addrin(0);
    // SAFETY: single trap path; `global` is the caller's AESGLOBAL;
    // `slot.access` is a kernel-reachable pointer slot of `ptr_size` bytes,
    // live until rsrc_free.
    let ok = unsafe {
        match selected(caller_pid(), pb.global_ptr()).map(|idx| &table()[idx].desc) {
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
/// EmuTOS `rs_obfix` (`gemrslib.c:89-99`), dispatched at `gemsuper.c:375-377`
/// with `RS_TREE = addr_in[0]` and `RS_OBJ = int_in[0]` (`aesdefs.h:574-575`);
/// the object is read at the caller's ABI width via [`ObjTree`] exactly like
/// every other object op. The arithmetic is [`rs_obfix`], the very function
/// the loader applies to every object of a char-grid resource
/// ([`fix_objects_coords`]) — so an application that `rsrc_obfix`es an
/// object it built itself (an inline `rs_object[]`) gets the same pixels the
/// same object would have got out of `rsrc_load`. EmuTOS applies it once at
/// load (`gemrslib.c:606-637,815-819`); an application does not re-`obfix` a
/// loaded object, and NewTOS, like EmuTOS, does not guard against one that
/// does.
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
    rs_obfix(ObjTree::new(tree), obj);
    pb.set_ret(1);
}

/// EmuTOS `rs_obfix(tree, curob)` (`aes/gemrslib.c:89-99`):
///
/// ```c
/// p = &tree[curob].ob_x;
/// for (offset=0; offset<4; offset++)
///     fix_chpos(p+offset, offset);
/// ```
///
/// The four packed words of object `obj` become pixels, in place, with the
/// AES's live metrics: `gl_wchar` for `ob_x`/`ob_width`, `gl_hchar` for
/// `ob_y`/`ob_height`, and `gl_width` for an `ob_width` of exactly 80
/// characters ([`super::graf::gl_wchar`], [`super::graf::gl_hchar`],
/// [`super::graf::gl_width`] — `gemgraf.c:37-41`, set in `gsx_start`).
/// The per-word rule is [`fix_chpos`].
pub(super) fn rs_obfix(t: ObjTree, obj: i16) {
    let (wchar, hchar) = (super::graf::gl_wchar(), super::graf::gl_hchar());
    let width = super::graf::gl_width();
    t.set_x(obj, fix_chpos(t.x(obj), wchar, false, width));
    t.set_y(obj, fix_chpos(t.y(obj), hchar, false, width));
    t.set_width(obj, fix_chpos(t.width(obj), wchar, true, width));
    t.set_height(obj, fix_chpos(t.height(obj), hchar, false, width));
}

/// EmuTOS `fix_objects`' coordinate half (`aes/gemrslib.c:606-637`):
///
/// ```c
/// for (ii = 0; ii < rs_hdr->rsh_nobs; ii++) {
///     obj = (OBJECT *)get_addr(R_OBJECT, ii);
///     rs_obfix(obj, 0);
///     …
/// ```
///
/// applied to every object of a **char-grid** blob (`version >=`
/// [`NRSC_VER_CHARGRID`]), through the kernel alias `access_base` at the
/// caller's record width. A blob of an earlier generation already holds
/// pixels (its converter did this arithmetic) and is left exactly as it is —
/// converting it again would turn pixel counts into character counts.
///
/// Runs from both entry points, as EmuTOS runs it for both kinds of
/// resource: `rs_fixit` for a loaded file (`:816-820`, from `rs_load`
/// `:846`) and `gem_rsc_fixit` for the embedded one (`aes/gem_rsc.c:636-641`).
/// The `ob_spec` half of `fix_objects` is the relocation cores' object walk.
///
/// # Safety
/// `access_base` must address a blob whose header passed [`header_is_sound`]
/// for `abi` (the object array lies inside the bytes that exist).
unsafe fn fix_objects_coords(access_base: *mut u8, abi: Abi) {
    // SAFETY: the header and the object array were validated by the caller.
    unsafe {
        let h = &*(access_base as *const NrscHeader);
        if h.version < NRSC_VER_CHARGRID {
            return;
        }
        let objs = access_base.add(h.off_objects as usize) as *const Object;
        // Only the four coordinate words are touched, at `abi`'s record
        // width; the accessor's origin flag plays no part here.
        let t = ObjTree::app_at(objs, abi);
        for i in 0..(h.n_objects as usize).min(i16::MAX as usize) {
            rs_obfix(t, i as i16);
        }
    }
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
///
/// The rule alone, with explicit metrics; [`rs_obfix`] feeds it the live ones.
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

// ─── fix_cicons: colour icons (TASK/A41, spec §5) ─────────────────────────
//
// EmuTOS `fix_cicons` (`aes/gemrslib.c:549-573`), run by `rs_readit` right
// after `fix_trindex` (`:795-797`): with the pointers of every CICONBLK and
// CICON in place, `transform_all_cicons` (`:403-477`) picks, per CICONBLK,
// the variant `best_match` (`:267-282`) selects for the live plane count
// `gl_nplanes`, stores it in `mainlist` (or NULL — the draw-time mono
// fallback), expands it to the device's plane count when it has fewer
// (`expand_cicondata`, `:284-343`), transforms the result to the device form
// (`transform_cicon`, `:348-396`) into a freshly allocated buffer, points
// `col_data` (and `sel_data`, in the second half of the same buffer) at it,
// and stamps `num_planes = gl_nplanes`, `next_res = NULL`.
//
// NewTOS has one device form: the 16-bit RGB565-chunky memory MFDB
// (`vdi/raster.rs` module docs, `CHUNKY_PLANES`) — the Falcon truecolor form
// EmuTOS's own `transform_cicon` produces when `planes > 8` (`:359-395`):
// `vr_trnfm` to chunky words with plane `p` at bit `15 - p` (our
// `vr_trnfm16` does the same, `raster.rs`), the top byte reversed into an
// index `n` 0..=255, and `n` resolved through the physical workstation's
// palette (`phys_work.ext->palette[n]`). That chain is a pure function of
// each pixel's plane bits, its mask bit and the palette, which is how
// [`cicon_pixel`] computes it — expansion and transform folded into one
// step per pixel, no intermediate standard-form buffer.
//
// One deviation is deliberate and stated: EmuTOS allocates one buffer per
// icon and falls back to mono per icon when an allocation fails (`:436-449`).
// NewTOS makes **one** allocation per resource for all its icons, because on
// the EL0 path every allocation is a mapped block of the process's window,
// and that table holds [`usermem::MAX_USER_BLOCKS`] (32) entries — fewer than
// TeraDesk's 38 icons. The fallback is therefore per resource: no buffer,
// no colour icon, the load itself still succeeds (§5.6 point 2's shape, at
// resource granularity). `rsrc_free` releases the one buffer
// (`free_cicon_buffers`, `:515-536`, walks EmuTOS's many).

/// What `fix_cicons` needs to know about the device: the plane count
/// `best_match` compares against (`gl_nplanes` = `INQ_TAB[4]`,
/// `gemgraf.c:395`), the pen count that shapes EmuTOS's `MAP_COL`
/// (`vdi_col.c:736-747`), and the physical workstation's pseudo-palette
/// (`phys_work.ext->palette`, one RGB565 word per pen — NewTOS keeps it
/// indexed by VDI pen, see [`CiconDevice::rev_map_col`]).
#[derive(Clone, Copy)]
pub(crate) struct CiconDevice {
    /// `gl_nplanes`.
    pub planes: i16,
    /// `DEV_TAB[13]`, EmuTOS `numcolors`.
    pub num_colors: u16,
    /// The pseudo-palette, indexed by VDI pen.
    pub palette: [u16; 256],
}

impl CiconDevice {
    /// The live device — the raster backend's plane count, pen count and
    /// requested colours, packed the way a workstation's pseudo-palette is
    /// seeded (`init_wk`, `vdi_control.c:354-363`: pens 0..=254 through
    /// `set_color16`; pen 255 stays 0, `vdi/workstation.rs`). That seeded
    /// table is what the physical workstation holds in EmuTOS. Without a
    /// backend (a hostless test) the fallback is the one `appl_init`
    /// reports for `ap_planes`: 4 planes, 16 pens, an all-black palette.
    pub fn live() -> CiconDevice {
        let mut dev = CiconDevice { planes: 4, num_colors: 16, palette: [0; 256] };
        if let Some(r) = crate::host::host().and_then(|h| h.raster()) {
            dev.planes = r.planes() as i16;
            dev.num_colors = r.num_colors();
            for (pen, slot) in dev.palette.iter_mut().enumerate().take(255) {
                let (red, green, blue) = r.query_color(pen as u16);
                *slot = crate::vdi::pack_color16([red as i16, green as i16, blue as i16]);
            }
        }
        dev
    }

    /// EmuTOS's `REV_MAP_COL` — hardware colour register → VDI pen
    /// (`vdi_col.c:25-29`, built in `init_colors` `:736-747`): `MAP_COL` is
    /// the ROM table `{0, 15, 1, 2, 4, 6, 3, 5, 7, 8, 9, 10, 12, 14, 11, 13}`
    /// with `MAP_COL[1] = numcolors - 1`, the identity for 16..=254 and
    /// `MAP_COL[255] = 15` (`EXTENDED_PALETTE`), inverted over `i <
    /// numcolors`; a register no pen maps to reads 0.
    ///
    /// Why it is needed here: a CICON's planes hold **hardware** colour
    /// indices (the device form of a palette screen), and EmuTOS's
    /// `transform_cicon` accordingly indexes `phys_work.ext->palette[n]` by
    /// hardware register — that table is written at `palette[MAP_COL[pen]]`
    /// (`set_color16`, `:601-614`). NewTOS folds `MAP_COL` to the identity
    /// and keeps the pseudo-palette by VDI pen (`vdi/vwk.rs`, `VwkExt`), so
    /// the colour EmuTOS finds at register `n` is at pen `REV_MAP_COL[n]`
    /// here. The observable result — which RGB word a plane index becomes —
    /// is the same; e.g. index 15 of a 16-colour icon, expanded to 255 on a
    /// 16-plane device, reaches pen 1 (black) both ways.
    fn rev_map_col(&self) -> [u8; 256] {
        const MAP_COL_ROM: [u8; 16] = [0, 15, 1, 2, 4, 6, 3, 5, 7, 8, 9, 10, 12, 14, 11, 13];
        let numcolors = (self.num_colors as usize).clamp(2, 256);
        let mut map = [0u8; 256];
        map[..16].copy_from_slice(&MAP_COL_ROM);
        map[1] = (numcolors - 1) as u8;
        for (i, m) in map.iter_mut().enumerate().skip(16) {
            *m = i as u8;
        }
        map[255] = 15;
        let mut rev = [0u8; 256];
        for (pen, &reg) in map.iter().enumerate().take(numcolors) {
            rev[reg as usize] = pen as u8;
        }
        rev
    }
}

/// The device-form buffer of one resource's colour icons — where `fix_cicons`
/// put them: the kernel-reachable address of the one allocation (the address
/// the CICONs' `col_data`/`sel_data` name is the app-visible one, written
/// into the records) and its byte length. All zero when the resource has no
/// colour icons, or when none could be given a buffer (the mono fallback).
#[derive(Clone, Copy, Default)]
struct CiconBuf {
    access: u64,
    len: usize,
}

/// One pixel of the device form — EmuTOS `expand_cicondata` +
/// `transform_cicon` (`gemrslib.c:284-343`, `:348-396`) for one pixel:
///
/// * `index` is the pixel's value across the variant's `src_planes` planes
///   (plane `p` = bit `p`), `mask` its bit of the variant's mask plane;
/// * when the variant has **fewer** planes than the device (`src_planes <
///   dev_planes`), the expansion applies: the source planes are kept, every
///   further plane up to `dev_planes` is the AND of all source planes
///   (`:307-329`), and the mask plane is ANDed into every plane (`:331-341`).
///   An exact match is transformed as it is — no expansion, no masking
///   (`transform_all_cicons`, `expand` false, `:418`);
/// * `vr_trnfm` to chunky puts plane `p` at bit `15 - p`
///   (`vr_trnfm16`), and the reversal in `transform_cicon` (`:359-390`)
///   reads planes 0..=7 back as the index `n`; planes 8.. are dropped;
/// * `n` is a hardware register: `phys_work.ext->palette[n]` is
///   `palette[rev_map_col[n]]` here ([`CiconDevice::rev_map_col`]).
#[inline]
fn cicon_pixel(index: u32, mask: bool, src_planes: u32, dev_planes: u32, rev: &[u8; 256], palette: &[u16; 256]) -> u16 {
    let n = if src_planes < dev_planes {
        let all_src = (1u32 << src_planes) - 1;
        let all_dev = if dev_planes >= 32 { u32::MAX } else { (1u32 << dev_planes) - 1 };
        let mut e = index;
        if index == all_src {
            e |= all_dev & !all_src; // the ANDed plane, copied to the rest
        }
        if !mask {
            e = 0;
        }
        e & 0xff
    } else {
        index & 0xff
    };
    palette[rev[n as usize] as usize]
}

/// The device of one `fix_cicons` run, with its `REV_MAP_COL` computed once.
struct CiconXform<'a> {
    dev: &'a CiconDevice,
    rev: [u8; 256],
}

/// Transform one standard-form image (`src_planes` plane-sequential planes of
/// `words` WORDs each, then its one-plane `mask`) of a `wd`-word-wide,
/// `h`-row icon into `wd * 16 × h` device-form pixels at `dst`
/// ([`cicon_pixel`] per pixel).
///
/// # Safety
/// `src` must address `words * src_planes` readable WORDs and `mask` `words`
/// readable WORDs (both checked against the blob by
/// [`cicon_tables_are_sound`]); `dst` must address `wd * 16 * h` writable
/// `u16`s. None of the three need alignment.
unsafe fn transform_cicon(
    src: *const u8,
    mask: *const u8,
    src_planes: u32,
    wd: usize,
    h: usize,
    xf: &CiconXform,
    dst: *mut u16,
) {
    let words = wd * h;
    let dev_planes = xf.dev.planes.max(0) as u32;
    for row in 0..h {
        for col in 0..wd * 16 {
            let word = row * wd + col / 16;
            let bit = 0x8000u16 >> (col & 15);
            let mut index = 0u32;
            for p in 0..src_planes as usize {
                // SAFETY: `p * words + word < words * src_planes`.
                if unsafe { rd16(src.add((p * words + word) * 2)) } & bit != 0 {
                    index |= 1 << p;
                }
            }
            // SAFETY: `word < words`.
            let m = unsafe { rd16(mask.add(word * 2)) } & bit != 0;
            let px = cicon_pixel(index, m, src_planes, dev_planes, &xf.rev, &xf.dev.palette);
            // SAFETY: `row * wd * 16 + col < wd * 16 * h`.
            unsafe { dst.add(row * wd * 16 + col).write_unaligned(px) };
        }
    }
}

/// EmuTOS `best_match` (`gemrslib.c:267-282`) over the CICON chain that
/// starts at the (relocated) `mainlist` value `first` of one CICONBLK:
///
/// ```c
/// for (p = start->mainlist; p; p = p->next_res)
/// {
///     if (p->num_planes > gl_nplanes)     /* too many planes */
///         continue;
///     if (p->num_planes == gl_nplanes)    /* exact match */
///         return p;
///     if (!found || (p->num_planes > found->num_planes))
///         found = p;                      /* best so far */
/// }
/// return found;
/// ```
///
/// An exact match wins at once; else the deepest variant not deeper than the
/// device; a deeper one is never chosen; none at all → `None` (mono). The
/// links are target VAs (the relocation has run); each is read through the
/// kernel alias at `access_base + (va - target_base)`. Returns the chosen
/// record's `(access pointer, target VA)`.
///
/// # Safety
/// The chain must have been validated by [`cicon_tables_are_sound`] and
/// relocated onto `target_base`.
unsafe fn best_match(
    access_base: *mut u8,
    target_base: u64,
    abi: Abi,
    first: u64,
    gl_nplanes: i16,
) -> Option<(*mut u8, u64)> {
    let ps = ptr_size(abi);
    let mut found: Option<(*mut u8, u64, i16)> = None;
    let mut link = first;
    while link != 0 {
        // SAFETY: a validated chain link, relocated onto `target_base`.
        let rec = unsafe { access_base.add(link.wrapping_sub(target_base) as usize) };
        // SAFETY: a whole CICON record.
        let planes = unsafe { rd16(rec) as i16 };
        // SAFETY: `next_res` is the fifth pointer field.
        let next = unsafe { rd_ptr(rec.add(CICON_PTR_FIELDS * ps), abi) };
        if planes <= gl_nplanes {
            if planes == gl_nplanes {
                return Some((rec, link));
            }
            if found.is_none_or(|(_, _, best)| planes > best) {
                found = Some((rec, link, planes));
            }
        }
        link = next;
    }
    found.map(|(rec, va, _)| (rec, va))
}

/// EmuTOS `fix_cicons` → `transform_all_cicons` (`gemrslib.c:549-573`,
/// `:403-477`) for a relocated v11+ blob (see the section comment above):
/// per CICONBLK, `best_match` chooses the variant for `dev.planes`,
/// `mainlist` records the choice (or 0), and every chosen variant's image —
/// and its SELECTED image, when it has one — is transformed into one
/// device-form buffer that `alloc(bytes)` provides as `(access, target)`
/// addresses; the variant's `col_data`/`sel_data` then point at its images,
/// `num_planes` becomes `dev.planes` and `next_res` 0, as EmuTOS leaves
/// them (`:470-471`). `col_mask`/`sel_mask` stay the file's one-plane masks
/// (EmuTOS transforms only the data, `:452-467`). No buffer (`alloc` →
/// `None`) means every `mainlist` is 0 — the icons draw mono, the load goes
/// on (`:441-449`, at resource granularity). Returns the buffer for the
/// descriptor, all-zero when there is none.
///
/// A blob older than [`NRSC_VER_CICON`] has no arrays and nothing happens.
///
/// # Safety
/// `access_base` must point at a blob whose header held up
/// ([`header_is_sound`]) and that has been relocated onto `target_base`.
unsafe fn fix_cicons(
    access_base: *mut u8,
    target_base: u64,
    abi: Abi,
    device: &mut dyn FnMut() -> CiconDevice,
    alloc: &mut dyn FnMut(usize) -> Option<(u64, u64)>,
) -> CiconBuf {
    // SAFETY: caller guarantees a validated, relocated blob.
    let h = unsafe { &*(access_base as *const NrscHeader) };
    if h.version < NRSC_VER_CICON || h.n_cib == 0 {
        return CiconBuf::default();
    }
    // The device is asked only now — a blob without colour icons never
    // touches it (the plane count and palette are the backend's).
    let dev = &device();
    let ps = ptr_size(abi);
    let (cib, ib) = match abi {
        Abi::Ilp32 => (CICONBLK_SIZE_ILP32, ICONBLK_SIZE_ILP32),
        Abi::Lp64 => (CICONBLK_SIZE, ICONBLK_SIZE),
    };
    // Per block: the record, the chosen variant, and the icon's geometry in
    // words per row / rows — `w/16` and `h`, EmuTOS's `data_size =
    // w/8 * gl_nplanes * h` bytes being `wd * 16 * h` 16-bit pixels for the
    // 16-plane device form (`:417`).
    let block = |i: usize| -> (*mut u8, Option<(*mut u8, u64)>, usize, usize) {
        // SAFETY: record `i` of the bounded CICONBLK array.
        let rec = unsafe { access_base.add(h.off_cib as usize + i * cib) };
        // SAFETY: `ib_wicon`/`ib_hicon` are the 6th/7th ICONBLK WORDs.
        let (wicon, hicon) = unsafe { (rd16(rec.add(3 * ps + 10)) as i16, rd16(rec.add(3 * ps + 12)) as i16) };
        // SAFETY: `mainlist` follows the ICONBLK head.
        let first = unsafe { rd_ptr(rec.add(ib), abi) };
        // SAFETY: validated chain.
        let chosen = unsafe { best_match(access_base, target_base, abi, first, dev.planes) };
        (rec, chosen, wicon.max(0) as usize / 16, hicon.max(0) as usize)
    };
    // Pass 1: how much device form is needed (`:421-426`, `:436`, summed).
    let mut need = 0usize;
    for i in 0..h.n_cib as usize {
        let (_, chosen, wd, rows) = block(i);
        if let Some((rec, _)) = chosen {
            let bytes = wd * 16 * rows * 2;
            // SAFETY: `sel_data` is the third pointer field.
            let sel = unsafe { rd_ptr(rec.add(3 * ps), abi) } != 0;
            need += if sel { 2 * bytes } else { bytes };
        }
    }
    let buf = if need == 0 { None } else { alloc(need) };
    let xf = CiconXform { dev, rev: dev.rev_map_col() };
    // Pass 2: choose, transform, patch.
    let mut cursor = 0usize;
    for i in 0..h.n_cib as usize {
        let (rec, chosen, wd, rows) = block(i);
        let Some((cic, cic_va)) = chosen.filter(|_| buf.is_some()) else {
            // Nothing suitable, or no buffer: no colour for this icon
            // (`ciconblk->mainlist = NULL`, `:411`, `:441`, `:449`).
            // SAFETY: `mainlist` follows the ICONBLK head.
            unsafe { wr_ptr(rec.add(ib), abi, 0) };
            continue;
        };
        let (buf_access, buf_target) = buf.unwrap_or_default();
        // SAFETY: `mainlist` follows the ICONBLK head.
        unsafe { wr_ptr(rec.add(ib), abi, cic_va) };
        // SAFETY: a whole CICON record: `num_planes` @0, pointers @ k*ps.
        let src_planes = unsafe { rd16(cic) as i16 }.max(0) as u32;
        let field = |k: usize| unsafe { rd_ptr(cic.add(k * ps), abi) };
        let to_access = |va: u64| unsafe { access_base.add(va.wrapping_sub(target_base) as usize) as *const u8 };
        let (col_data, col_mask, sel_data, sel_mask) = (field(1), field(2), field(3), field(4));
        let bytes = wd * 16 * rows * 2;
        // SAFETY: the rasters were bounds-checked against the blob; the
        // destination is inside the `need`-byte buffer.
        unsafe {
            transform_cicon(
                to_access(col_data),
                to_access(col_mask),
                src_planes,
                wd,
                rows,
                &xf,
                (buf_access as usize + cursor) as *mut u16,
            );
            wr_ptr(cic.add(ps), abi, buf_target + cursor as u64);
        }
        cursor += bytes;
        if sel_data != 0 {
            // SAFETY: as above, `selbuf = colbuf + data_size` (`:455`).
            unsafe {
                transform_cicon(
                    to_access(sel_data),
                    to_access(sel_mask),
                    src_planes,
                    wd,
                    rows,
                    &xf,
                    (buf_access as usize + cursor) as *mut u16,
                );
                wr_ptr(cic.add(3 * ps), abi, buf_target + cursor as u64);
            }
            cursor += bytes;
        }
        // SAFETY: `num_planes` @0 (`:470`, "neatness only"), `next_res` @5*ps (`:471`).
        unsafe {
            (cic as *mut i16).write_unaligned(dev.planes);
            wr_ptr(cic.add(CICON_PTR_FIELDS * ps), abi, 0);
        }
    }
    match buf {
        Some((access, _)) => CiconBuf { access, len: need },
        None => CiconBuf::default(),
    }
}

/// Release a device-form buffer the **kernel heap** provided ([`kernel_cicon_alloc`]).
///
/// # Safety
/// `buf` must be one returned by [`kernel_cicon_alloc`] and not yet freed.
unsafe fn kernel_cicon_free(buf: CiconBuf) {
    if buf.access != 0 && buf.len != 0 {
        // SAFETY: the layout `kernel_cicon_alloc` used.
        unsafe {
            alloc::alloc::dealloc(
                buf.access as *mut u8,
                alloc::alloc::Layout::from_size_align_unchecked(buf.len, CICON_BUF_ALIGN),
            )
        };
    }
}

/// Alignment of a kernel-heap device-form buffer (`u16` pixels; 8 keeps the
/// layout trivially valid for any pixel width).
const CICON_BUF_ALIGN: usize = 8;

/// The device-form buffer of the in-place kernel path ([`relocate_and_register`]):
/// the kernel heap, reachable at one address (EmuTOS `dos_alloc_anyram`).
fn kernel_cicon_alloc(bytes: usize) -> Option<(u64, u64)> {
    let layout = alloc::alloc::Layout::from_size_align(bytes, CICON_BUF_ALIGN).ok()?;
    if layout.size() == 0 {
        return None;
    }
    // SAFETY: a non-zero layout.
    let p = unsafe { alloc::alloc::alloc_zeroed(layout) };
    if p.is_null() {
        return None;
    }
    Some((p as u64, p as u64))
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

    pub(super) fn lock<'a>() -> MutexGuard<'a, ()> {
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
    pub(super) struct Blob {
        words: Vec<u64>,
        pub(super) len: usize,
    }

    impl Blob {
        pub(super) fn new(blob: &[u8]) -> Self {
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
        pub(super) fn as_mut_ptr(&mut self) -> *mut u8 {
            self.words.as_mut_ptr() as *mut u8
        }
        pub(super) fn bytes(&self) -> &[u8] {
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
        let desc = unsafe { relocate_and_register_core(p, len, target, Abi::Lp64, &mut kernel_cicon_alloc) }
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
        let desc = unsafe { relocate_and_register_core(p, len, APP_VA, Abi::Lp64, &mut kernel_cicon_alloc) }
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

    /// The ILP32 twin (R4): the slots of the published table are the
    /// caller's pointer width — 4 bytes — and slot `i` still holds exactly
    /// what `rsrc_gaddr(R_TREE, i)` returns; the *field* `ap_ptree` stays 4
    /// bytes at byte 10 (`application.md` § `AESGLOBAL`).
    #[test]
    fn ap_ptree_slots_are_what_rsrc_gaddr_returns_for_an_ilp32_caller() {
        let _g = lock();
        // A v8 blob: pointer tree index, pixel coordinates (no conversion).
        let blob = ilp32_blob(NRSC_VER_TRINDEX_PTR + 1, [1, 2, 3, 4]);
        let mut buf = Blob::new(&blob);
        let len = buf.len;
        // SAFETY: `buf` holds `len` readable/writable bytes.
        let desc = unsafe { relocate_and_register_core(buf.as_mut_ptr(), len, APP_VA, Abi::Ilp32, &mut kernel_cicon_alloc) }
            .expect("the ILP32 blob must relocate");
        assert_eq!(desc.ptr_size, 4, "ILP32 slots are 4 bytes");
        let mut global = [0i16; 16];
        // SAFETY: a 32-byte caller array.
        unsafe { report_load(global.as_mut_ptr(), &desc) };
        let off_trindex = u32::from_le_bytes(blob[16..20].try_into().unwrap()) as usize;
        let off_obj = u32::from_le_bytes(blob[20..24].try_into().unwrap()) as u64;
        assert_eq!(g32(&global, AP_PTREE_OFF) as u64, APP_VA + off_trindex as u64, "ap_ptree");
        assert_eq!(g32(&global, AP_RSCMEM_OFF) as u64, APP_VA, "ap_rscmem");
        let slot = u32::from_le_bytes(buf.bytes()[off_trindex..off_trindex + 4].try_into().unwrap()) as u64;
        // SAFETY: `desc` describes `buf`, which is still live.
        let via_gaddr = unsafe { get_addr(&desc, R_TREE, 0) }.expect("tree 0").target;
        assert_eq!(slot, via_gaddr, "ap_ptree[0] (4-byte slot) != rsrc_gaddr(R_TREE, 0)");
        assert_eq!(slot, APP_VA + off_obj, "the root object at the caller's VA");
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

    // ── A40: the ILP32 twin of the load-time coordinate fix ────────────────
    //
    // The LP64 path is driven end to end (through `relocate_and_register`
    // and the dispatcher's `rsrc_gaddr`) in `tests/aes_rsrc_obfix_at_load.rs`;
    // the ILP32 record layout is reachable only through
    // `relocate_and_register_core(…, Abi::Ilp32)`, the `load_into_user`
    // shape, hence here. No host is installed in this binary, so `gl_width`
    // is the 80-column stand-in `80 × gl_wchar`; the arithmetic under test
    // is the same either way.

    /// A one-object NRSC blob at the ILP32 record width (24-byte OBJECT,
    /// the coordinate words at +16), with the packed char-grid words `raw`.
    fn ilp32_blob(version: u32, raw: [u16; 4]) -> Vec<u8> {
        const HDR: usize = size_of::<NrscHeader>(); // 68
        let off_tri = HDR;
        let off_obj = off_tri + 4; // one 4-byte pointer slot, 72 = 8-aligned
        let total = off_obj + OBJECT_SIZE_ILP32;
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
        // The v3+/v5+ table fields stay 0/0 (empty tables); the v7+ tree
        // index slot holds the root's byte offset.
        b[off_tri..off_tri + 4].copy_from_slice(&(off_obj as u32).to_le_bytes());
        let o = off_obj;
        b[o..o + 2].copy_from_slice(&(-1i16).to_le_bytes()); // ob_next
        b[o + 2..o + 4].copy_from_slice(&(-1i16).to_le_bytes()); // ob_head
        b[o + 4..o + 6].copy_from_slice(&(-1i16).to_le_bytes()); // ob_tail
        b[o + 6..o + 8].copy_from_slice(&G_BOX.to_le_bytes());
        b[o + 12..o + 16].copy_from_slice(&0x0001_0000u32.to_le_bytes()); // ob_spec
        for (k, w) in raw.iter().enumerate() {
            b[o + 16 + 2 * k..o + 18 + 2 * k].copy_from_slice(&w.to_le_bytes());
        }
        b
    }

    /// Load an ILP32 blob at `APP_VA` and return its one object's four
    /// coordinate words as the application would read them.
    fn ilp32_coords(blob: &[u8]) -> [i16; 4] {
        let mut buf = Blob::new(blob);
        let len = blob.len();
        // SAFETY: `buf` holds `len` writable bytes for the whole call.
        let desc = unsafe { relocate_and_register_core(buf.as_mut_ptr(), len, APP_VA, Abi::Ilp32, &mut kernel_cicon_alloc) }
            .expect("the ILP32 blob must relocate");
        assert_eq!(desc.obj_size, OBJECT_SIZE_ILP32);
        let off_obj = u32::from_le_bytes(blob[20..24].try_into().unwrap()) as usize;
        let mut out = [0i16; 4];
        for (k, o) in out.iter_mut().enumerate() {
            let at = off_obj + 16 + 2 * k;
            *o = i16::from_le_bytes(buf.bytes()[at..at + 2].try_into().unwrap());
        }
        out
    }

    /// `fix_objects` (`gemrslib.c:606-637`) on a v10 blob: the packed words
    /// become `chars × gl_wchar/gl_hchar + offset`, the 80-character width
    /// `gl_width` — at the ILP32 record offsets, not the LP64 ones.
    #[test]
    fn a_char_grid_ilp32_blob_is_fixed_at_load() {
        let _g = lock();
        let (wchar, hchar, width) =
            (super::super::graf::gl_wchar(), super::super::graf::gl_hchar(), super::super::graf::gl_width());
        let raw = [0x0405u16, 0xFF02, 0x0050, 0x0102];
        let got = ilp32_coords(&ilp32_blob(NRSC_VER_CHARGRID + 1, raw));
        let want = [
            fix_chpos(raw[0] as i16, wchar, false, width),
            fix_chpos(raw[1] as i16, hchar, false, width),
            fix_chpos(raw[2] as i16, wchar, true, width),
            fix_chpos(raw[3] as i16, hchar, false, width),
        ];
        assert_eq!(got, want, "rs_obfix of {raw:04x?} at cell {wchar}x{hchar}, gl_width {width}");
        assert_eq!(got[2], width, "the 80-character width is gl_width");
        assert_ne!(got, [raw[0] as i16, raw[1] as i16, raw[2] as i16, raw[3] as i16], "converted at all");
    }

    /// A v8 blob (pixels, the previous generation) is not converted.
    #[test]
    fn a_pixel_ilp32_blob_is_left_alone() {
        let _g = lock();
        let raw = [0x0405u16, 0xFF02, 0x0050, 0x0102];
        let got = ilp32_coords(&ilp32_blob(NRSC_VER_TRINDEX_PTR + 1, raw));
        assert_eq!(got, [raw[0] as i16, raw[1] as i16, raw[2] as i16, raw[3] as i16]);
    }
}

#[cfg(test)]
mod selector_tests {
    //! A39 stage 2 — **the caller's `ap_rscmem` selects the resource.**
    //!
    //! EmuTOS `rs_sglobe` (`aes/gemrslib.c:673-679`) re-reads `ap_rscmem` out
    //! of the caller's `AESGLOBAL` on every `rs_free`/`rs_gaddr`/`rs_saddr`,
    //! so an application that rewrites the word — TeraDesk's `load_icons`
    //! saves the `desktop.rsc` words, loads another blob, and restores them
    //! (`sdk/apps/teradesk/icon.c:2707-2708`, `:2762-2763`) — steers every
    //! later call back to the blob it names. The contract NewTOS implements
    //! is `docs/api/aes/resource-fileselector.md` § `rsrc_load`, A39 callout,
    //! items 3–5: the 4-byte selector is matched against the bases of the
    //! caller's **own** loaded blobs, and a value naming none of them (0,
    //! another process's blob, a foreign address, a null `global`) reads as
    //! "nothing loaded".
    //!
    //! These tests live in-crate for the same reason the A35 tests above do:
    //! a selector can only ever name a blob whose base fits four bytes, and
    //! on a 64-bit host only the access/target split of
    //! [`relocate_and_register_core`] — `load_into_user`'s shape — produces
    //! one. Each load here is exactly the dispatcher's `rsrc_load` tail after
    //! `read_file`: relocate at a user VA, [`register`] for the calling pid,
    //! [`report_load`] into the caller's `AESGLOBAL`. The calls under test
    //! (`rsrc_gaddr`, `rsrc_free`) go through the real dispatcher with the
    //! test's own persistent `AESGLOBAL`, as an application's would. Two
    //! processes are two claimed GEMDOS slots switched through `RUN`, as in
    //! `tests/aes_rsrc_per_process.rs`.

    use super::*;
    use crate::bdos::process::{self, PROCESS_TABLE};
    use std::sync::atomic::Ordering;
    use std::sync::{Mutex, MutexGuard};

    /// A genuine NRSC v7 blob (three trees).
    const FSEL: &[u8] = include_bytes!("gem_fsel.nrsc");

    /// User VAs two loads of one process get: the EL0 window base and a
    /// later pool block, both below `USER_TPA_MAX_VA` (measured for
    /// TeraDesk: `0x400000` for `desktop.rsc`, `0x427000` for the next
    /// `Malloc`; `TASK/A60-STATUS-DONE.MD` § 1).
    const VA_X: u64 = 0x0040_0000;
    const VA_Y: u64 = 0x0042_7000;
    /// The value TeraDesk left at byte 14 before A60: inside `desktop.rsc`'s
    /// range, but not its base (`TASK/A60-STATUS-DONE.MD` § 3).
    const VA_HYBRID: u64 = 0x0040_7000;

    /// `RUN`, the process table and the resource lists are process-wide
    /// statics; the tests of this module must not overlap.
    static LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A claimed GEMDOS slot with a basepage page of its own; its AES pid is
    /// `slot + 1`. Released on drop.
    struct Proc {
        slot: usize,
        page: *mut [u8; 4096],
    }

    impl Proc {
        fn new() -> Self {
            let slot = process::allocate_slot().expect("a free process slot");
            let page = Box::into_raw(Box::new([0u8; 4096]));
            PROCESS_TABLE[slot]
                .basepage_kva
                .store(page.cast::<BasePage>(), Ordering::Release);
            Proc { slot, page }
        }

        fn pid(&self) -> i16 {
            crate::aes::pid_of_slot(self.slot)
        }

        fn bp(&self) -> *mut BasePage {
            self.page.cast::<BasePage>()
        }

        /// Make this the running process: every AES call now carries its pid.
        fn run(&self) {
            process::set_current(self.bp());
        }
    }

    impl Drop for Proc {
        fn drop(&mut self) {
            // Leave no entry of this pid behind for the next test's tenant of
            // the slot — what `Pterm` does on the machine.
            on_process_exit(self.pid());
            if process::current() == self.bp() {
                process::set_current(std::ptr::null_mut());
            }
            process::free_slot(self.slot);
            // SAFETY: `page` came from `Box::into_raw` above and is not
            // aliased once the slot no longer names it.
            unsafe { drop(Box::from_raw(self.page)) };
        }
    }

    /// The caller's `AESGLOBAL`: gemlib's 16 WORDs, persistent across calls
    /// like an application's `aes_global[]`.
    type Global = [i16; 16];

    fn rscmem(g: &Global) -> u32 {
        // SAFETY: 32 bytes; byte 14..18 is `ap_rscmem`.
        let b = unsafe { std::slice::from_raw_parts(g.as_ptr() as *const u8, 32) };
        u32::from_le_bytes(b[AP_RSCMEM_OFF..AP_RSCMEM_OFF + 4].try_into().unwrap())
    }

    fn ptree(g: &Global) -> u32 {
        // SAFETY: as `rscmem`.
        let b = unsafe { std::slice::from_raw_parts(g.as_ptr() as *const u8, 32) };
        u32::from_le_bytes(b[AP_PTREE_OFF..AP_PTREE_OFF + 4].try_into().unwrap())
    }

    /// `_AESrscmem_set(v)` / `_AESrscfile_set(v)` — the application writing
    /// the two words (`sdk/gemlib/overlay/gem.h`, 4 bytes at 14 and 10).
    fn set_words(g: &mut Global, tree: u32, mem: u32) {
        let b = g.as_mut_ptr() as *mut u8;
        // SAFETY: 32 bytes; the two fields are only 2-byte aligned.
        unsafe {
            (b.add(AP_PTREE_OFF) as *mut u32).write_unaligned(tree);
            (b.add(AP_RSCMEM_OFF) as *mut u32).write_unaligned(mem);
        }
    }

    /// A blob loaded for the **current** pid at user VA `target`, reported
    /// through `global` — `rsrc_load`'s tail after `read_file`, without the
    /// pool. The access buffer must outlive every later call into it.
    struct Named {
        words: Vec<u64>,
        target: u64,
    }

    impl Named {
        fn load(target: u64, global: &mut Global) -> Self {
            let mut words = vec![0u64; FSEL.len().div_ceil(8)];
            let p = words.as_mut_ptr() as *mut u8;
            // SAFETY: `words` holds at least `FSEL.len()` bytes.
            unsafe { std::ptr::copy_nonoverlapping(FSEL.as_ptr(), p, FSEL.len()) };
            // SAFETY: `p` addresses `FSEL.len()` writable bytes that live as
            // long as this `Named`; single test thread under `LOCK`.
            let desc = unsafe { relocate_and_register_core(p, FSEL.len(), target, Abi::Lp64, &mut kernel_cicon_alloc) }
                .expect("the blob must relocate");
            assert!(unsafe { register(caller_pid(), 0, 0, desc) }, "room in the resource lists");
            // SAFETY: a 32-byte caller array.
            unsafe { report_load(global.as_mut_ptr(), &desc) };
            Named { words, target }
        }

        fn contains(&self, addr: u64) -> bool {
            addr >= self.target && addr < self.target + FSEL.len() as u64
        }
    }

    /// One AES call through the real dispatcher, carrying `global` (null
    /// allowed) → (`int_out[0]`, `addr_out[0]`).
    fn call(global: *mut i16, op: i16, n_intin: i16, intin: [i16; 4]) -> (i16, u64) {
        let mut control = [op, n_intin, 1, 0, 0];
        let mut intin = intin;
        let mut intout = [0i16; 8];
        let mut addrin = [0u64; 2];
        let mut addrout = [0u64; 2];
        let raw = [
            control.as_mut_ptr() as u64,
            global as u64,
            intin.as_mut_ptr() as u64,
            intout.as_mut_ptr() as u64,
            addrin.as_mut_ptr() as u64,
            addrout.as_mut_ptr() as u64,
        ];
        // SAFETY: a well-formed LP64 parameter block, alive for the call.
        unsafe { crate::aes::dispatch(raw.as_ptr() as u64, Abi::Lp64) };
        (intout[0], addrout[0])
    }

    fn gaddr(g: &mut Global, rtype: i16, index: i16) -> (i16, u64) {
        call(g.as_mut_ptr(), 112, 2, [rtype, index, 0, 0])
    }

    fn rsrc_free(g: &mut Global) -> i16 {
        call(g.as_mut_ptr(), 111, 0, [0; 4]).0
    }

    /// Where `rsrc_gaddr` resolves right now, for every selector family the
    /// blob has: `R_TREE`, `R_OBJECT` and `R_STRING` (the dereferenced free
    /// string, TeraDesk's info-line path).
    fn resolves_in(g: &mut Global, blob: &Named) -> bool {
        [(R_TREE, 0), (R_OBJECT, 0), (R_STRING, 0)].iter().all(|&(t, i)| {
            let (ok, a) = gaddr(g, t, i);
            ok == 1 && blob.contains(a)
        })
    }

    fn nothing_loaded(g: &mut Global) -> bool {
        [(R_TREE, 0), (R_OBJECT, 0), (R_STRING, 0)]
            .iter()
            .all(|&(t, i)| gaddr(g, t, i) == (0, 0))
    }

    // ── rsrc_load twice: the second is named, the first stays ──────────────

    /// `rs_readit` "init global" (`gemrslib.c:786-788`): the second load
    /// overwrites the words with its own base and tree table and frees
    /// nothing; `rsrc_gaddr` now resolves in the second blob.
    #[test]
    fn a_second_load_names_itself_and_is_selected() {
        let _g = lock();
        let app = Proc::new();
        app.run();
        let mut g = Global::default();
        let x = Named::load(VA_X, &mut g);
        assert_eq!(rscmem(&g), VA_X as u32);
        assert!(resolves_in(&mut g, &x));
        let y = Named::load(VA_Y, &mut g);
        assert_eq!(rscmem(&g), VA_Y as u32, "ap_rscmem names the second blob");
        assert_ne!(ptree(&g), 0);
        assert!(resolves_in(&mut g, &y), "rsrc_gaddr resolves in the second blob");
        assert!(!x.contains(gaddr(&mut g, R_TREE, 0).1), "…and not in the first");
    }

    // ── (a) restoring the words re-selects the earlier load ────────────────

    /// TeraDesk `load_icons` (`icon.c:2707-2708`, `:2762-2763`): save the
    /// `desktop.rsc` words, load another blob through the AES, restore the
    /// words — every later `rsrc_gaddr(R_STRING, …)` must resolve in
    /// `desktop.rsc` again (`rs_sglobe`, `gemrslib.c:677-678`; `get_addr`
    /// `R_TREE` from `ap_ptree[i]`, `:124`). Red in stage 1, which kept
    /// answering from the latest load.
    #[test]
    fn restoring_the_words_reselects_the_earlier_load() {
        let _g = lock();
        let app = Proc::new();
        app.run();
        let mut g = Global::default();
        let x = Named::load(VA_X, &mut g);
        let (sv_tree, sv_mem) = (ptree(&g), rscmem(&g)); // svtree / svrshdr
        let y = Named::load(VA_Y, &mut g);
        assert!(resolves_in(&mut g, &y));

        set_words(&mut g, sv_tree, sv_mem); // `_AESrscfile = svtree; _AESrscmem = svrshdr;`
        assert!(resolves_in(&mut g, &x), "after restoring the words rsrc_gaddr resolves in X");
        let (ok, tree0) = gaddr(&mut g, R_TREE, 0);
        assert_eq!(ok, 1);
        // R4: `R_TREE` is `ap_ptree[0]` of the selected blob (LP64 slot).
        // SAFETY: `x.words` holds the relocated blob; slot 0 of its tree table.
        let off_tri = u32::from_le_bytes(FSEL[16..20].try_into().unwrap()) as usize;
        let slot0 = unsafe { (x.words.as_ptr() as *const u8).add(off_tri).cast::<u64>().read_unaligned() };
        assert_eq!(tree0, slot0, "rsrc_gaddr(R_TREE, 0) == ap_ptree[0] of X");
        assert_eq!(sv_tree as u64, x.target + off_tri as u64, "the restored ap_ptree is X's table");
        assert!(!y.contains(tree0));
    }

    // ── (b) a foreign selector is "nothing loaded" and frees nothing ───────

    /// Item 4: a value naming no blob the AES loaded for this process — the
    /// pre-A60 hybrid `0x407000` (inside X's range, not its base), a plain
    /// foreign address, or 0 while X is named — reads as nothing loaded:
    /// `rsrc_gaddr` → 0/0, `rsrc_free` → 0 and **frees nothing**, so X is
    /// still there once the words name it again. Base comparison, not range
    /// (`TASK/A60-STATUS-DONE.MD` § 3).
    #[test]
    fn a_foreign_selector_reads_as_nothing_loaded_and_frees_nothing() {
        let _g = lock();
        let app = Proc::new();
        app.run();
        let mut g = Global::default();
        let x = Named::load(VA_X, &mut g);
        let (sv_tree, sv_mem) = (ptree(&g), rscmem(&g));
        for foreign in [VA_HYBRID as u32, 0x1234_5678, 0] {
            set_words(&mut g, sv_tree, foreign);
            assert!(nothing_loaded(&mut g), "selector {foreign:#x} must name nothing");
            assert_eq!(rsrc_free(&mut g), 0, "selector {foreign:#x}: nothing to free");
        }
        set_words(&mut g, sv_tree, sv_mem);
        assert!(resolves_in(&mut g, &x), "X was not freed by any of the foreign frees");
        assert_eq!(rsrc_free(&mut g), 1, "…and is freed once named");
        assert_eq!(rsrc_free(&mut g), 0, "gemrslib.c:702-704: a second free fails");
    }

    // ── (c) another process's blob is not this one's ───────────────────────

    /// Item 4, keyed by pid first: pid 1 naming the base of pid 2's blob
    /// reads as nothing loaded (the kernel never resolves in a blob it did
    /// not load for the caller), and pid 2's state is untouched.
    #[test]
    fn another_processs_blob_is_not_selectable() {
        let _g = lock();
        let p1 = Proc::new();
        let p2 = Proc::new();
        assert_ne!(p1.pid(), p2.pid());

        p1.run();
        let mut g1 = Global::default();
        let x = Named::load(VA_X, &mut g1);
        let (x_tree, x_mem) = (ptree(&g1), rscmem(&g1));

        p2.run();
        let mut g2 = Global::default();
        let z = Named::load(VA_Y, &mut g2);
        assert!(resolves_in(&mut g2, &z));

        p1.run();
        set_words(&mut g1, ptree(&g2), rscmem(&g2)); // pid 2's words in pid 1's array
        assert!(nothing_loaded(&mut g1), "pid 2's blob is not pid 1's");
        assert_eq!(rsrc_free(&mut g1), 0);

        p2.run();
        assert!(resolves_in(&mut g2, &z), "pid 2 keeps its blob");
        assert_eq!(rsrc_free(&mut g2), 1);

        p1.run();
        set_words(&mut g1, x_tree, x_mem);
        assert!(resolves_in(&mut g1, &x), "pid 1's own blob was never touched");
        assert_eq!(rsrc_free(&mut g1), 1);
    }

    // ── (d) rsrc_free frees the named blob only ────────────────────────────

    /// Item 6: `rs_free` frees the blob the words name (`dos_free
    /// (pglobal->ap_rscmem)`, `gemrslib.c:699`) and writes none of the
    /// words. With X named, X goes and Y stays loaded — reachable as soon as
    /// the application names Y again; the stale words naming freed X read as
    /// nothing loaded (NewTOS's safe reading of EmuTOS's freed-memory case).
    #[test]
    fn rsrc_free_frees_the_named_blob_and_leaves_the_other_loaded() {
        let _g = lock();
        let app = Proc::new();
        app.run();
        let mut g = Global::default();
        let x = Named::load(VA_X, &mut g);
        let (x_tree, x_mem) = (ptree(&g), rscmem(&g));
        let y = Named::load(VA_Y, &mut g);
        let (y_tree, y_mem) = (ptree(&g), rscmem(&g));

        set_words(&mut g, x_tree, x_mem);
        assert_eq!(rsrc_free(&mut g), 1, "frees X, the named blob");
        assert_eq!((ptree(&g), rscmem(&g)), (x_tree, x_mem), "rs_free writes none of the words");
        assert!(nothing_loaded(&mut g), "the words name a freed blob");
        assert_eq!(rsrc_free(&mut g), 0);

        set_words(&mut g, y_tree, y_mem);
        assert!(resolves_in(&mut g, &y), "Y stayed loaded and is reachable once named");
        assert!(!x.contains(gaddr(&mut g, R_TREE, 0).1));
        assert_eq!(rsrc_free(&mut g), 1, "frees Y");
        assert!(nothing_loaded(&mut g));
    }

    // ── a null `global` is an empty selector ───────────────────────────────

    /// Item 4: a null `global` in the AESPB reads as nothing loaded (EmuTOS
    /// would dereference `NULL->ap_rscmem`; XaAES falls back to the client's
    /// latest load — not adopted, see the analysis § 4.2 R3).
    #[test]
    fn a_null_global_reads_as_nothing_loaded() {
        let _g = lock();
        let app = Proc::new();
        app.run();
        let mut g = Global::default();
        let x = Named::load(VA_X, &mut g);
        assert_eq!(call(std::ptr::null_mut(), 112, 2, [R_TREE, 0, 0, 0]), (0, 0));
        assert_eq!(call(std::ptr::null_mut(), 111, 0, [0; 4]).0, 0, "nothing freed");
        assert!(resolves_in(&mut g, &x), "X is still loaded");
        assert_eq!(rsrc_free(&mut g), 1);
    }

    // ── a blob the four bytes cannot name ──────────────────────────────────

    /// A35 item 3 of the same callout: a load whose base does not fit the
    /// field — the in-place kernel path, where `report_load` writes nothing —
    /// "still succeeds and `rsrc_gaddr` still answers at full width". Its
    /// name is the 0 the caller was left with, so the zero selector reaches
    /// the process's latest such load and nothing else: a *named* blob is
    /// never reached by 0, and an unnamed one never by a non-zero value.
    #[test]
    fn a_blob_the_field_cannot_name_answers_to_the_zero_it_left() {
        let _g = lock();
        let app = Proc::new();
        app.run();
        let mut g = Global::default();
        let x = Named::load(VA_X, &mut g);
        let (x_tree, x_mem) = (ptree(&g), rscmem(&g));
        set_words(&mut g, 0, 0);
        assert!(nothing_loaded(&mut g), "0 does not reach a named blob");

        // The in-place load: a host buffer, above 4 GiB.
        let mut words = vec![0u64; FSEL.len().div_ceil(8)];
        let p = words.as_mut_ptr() as *mut u8;
        // SAFETY: `words` holds `FSEL.len()` bytes for the whole test.
        unsafe { std::ptr::copy_nonoverlapping(FSEL.as_ptr(), p, FSEL.len()) };
        assert!(unsafe { relocate_and_register(p, FSEL.len()) });
        let base = p as u64;
        assert!(base > u64::from(u32::MAX), "the harness's in-place blob sits above 4 GiB");
        let mut zero = Global::default();
        let (ok, tree) = gaddr(&mut zero, R_TREE, 0);
        assert_eq!(ok, 1, "the unnamed load answers to the zero it left");
        assert!(tree >= base && tree < base + FSEL.len() as u64, "at full width");
        set_words(&mut zero, 0, 0x1234_5678);
        assert!(nothing_loaded(&mut zero), "a non-zero foreign value reaches nothing");

        set_words(&mut g, x_tree, x_mem);
        assert!(resolves_in(&mut g, &x), "the named blob is unaffected");
        assert_eq!(rsrc_free(&mut g), 1);
        let mut zero = Global::default();
        assert_eq!(rsrc_free(&mut zero), 1, "the unnamed load is freed through its zero");
        assert_eq!(rsrc_free(&mut zero), 0);
    }
}

#[cfg(test)]
mod cicon_tests {
    //! A41 — the colour-icon load path: a hand-built NRSC v11 blob (two
    //! CICONBLKs, one with a 1-plane and a 4-plane variant, one with a single
    //! 8-plane variant) through the real relocation core and
    //! [`fix_cicons`], with the device's plane count varied so every branch
    //! of `best_match` (`gemrslib.c:267-282`) is taken, and the transformed
    //! pixels compared against the per-pixel rule of `expand_cicondata` +
    //! `transform_cicon` (`:284-343`, `:348-396`). Spec:
    //! `docs/api/aes/resource-fileselector.md` §5.6–§5.8.
    //!
    //! Like [`super::tests`], these use [`relocate_and_register_core`]'s
    //! access/target split the way `load_into_user` does: the blob is the
    //! test's buffer, relocated onto a user VA, and the device-form buffer is
    //! a second test buffer handed in through the allocator closure.

    use super::*;

    /// The user VA shape `load_into_user` hands the core (see `tests`).
    const APP_VA: u64 = 0x0040_0000;
    /// Where the device-form buffer is "mapped" for the app.
    const BUF_VA: u64 = 0x0050_0000;

    const G_BOX_T: u16 = G_BOX;
    const HDR: usize = size_of::<NrscHeader>(); // 84
    const W: i16 = 16;
    const H: i16 = 16;
    const MONO_WORDS: usize = (W as usize / 16) * H as usize; // 16

    /// Section offsets of the fixture, derived the way `mkrsc.py` derives
    /// them: header, an 8-byte tree slot padded to 8, three OBJECTs, no
    /// TEDINFO/BITBLK/ICONBLK, two CICONBLKs, three CICONs, no frstr/frimg,
    /// then the rasters.
    const OFF_TRI: usize = HDR; // 84
    const OFF_OBJ: usize = 96; // 84 + 8, padded to 8
    const N_OBJ: usize = 3;
    const OFF_CIB: usize = OFF_OBJ + N_OBJ * OBJECT_SIZE; // 192
    const N_CIB: usize = 2;
    const OFF_CIC: usize = OFF_CIB + N_CIB * CICONBLK_SIZE; // 304
    const N_CIC: usize = 3;
    const OFF_DATA: usize = OFF_CIC + N_CIC * CICON_SIZE; // 448

    /// Raster offsets inside the data section, in interning order.
    const R_MONO0_MASK: usize = OFF_DATA;
    const R_MONO0_DATA: usize = R_MONO0_MASK + MONO_WORDS * 2;
    const R_TEXT0: usize = R_MONO0_DATA + MONO_WORDS * 2; // 12 bytes
    const R_A_DATA: usize = R_TEXT0 + 12; // 1 plane
    const R_A_MASK: usize = R_A_DATA + MONO_WORDS * 2;
    const R_B_DATA: usize = R_A_MASK + MONO_WORDS * 2; // 4 planes
    const R_B_MASK: usize = R_B_DATA + MONO_WORDS * 2 * 4;
    const R_B_SEL: usize = R_B_MASK + MONO_WORDS * 2; // 4 planes
    const R_B_SELMASK: usize = R_B_SEL + MONO_WORDS * 2 * 4;
    const R_MONO1_MASK: usize = R_B_SELMASK + MONO_WORDS * 2;
    const R_MONO1_DATA: usize = R_MONO1_MASK + MONO_WORDS * 2;
    const R_TEXT1: usize = R_MONO1_DATA + MONO_WORDS * 2;
    const R_C_DATA: usize = R_TEXT1 + 12; // 8 planes
    const R_C_MASK: usize = R_C_DATA + MONO_WORDS * 2 * 8;
    const TOTAL: usize = R_C_MASK + MONO_WORDS * 2;

    fn put16(b: &mut [u8], off: usize, v: u16) {
        b[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn put32(b: &mut [u8], off: usize, v: u32) {
        b[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn put64(b: &mut [u8], off: usize, v: u64) {
        b[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }
    fn get64(b: &[u8], off: usize) -> u64 {
        u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
    }
    fn get16(b: &[u8], off: usize) -> u16 {
        u16::from_le_bytes(b[off..off + 2].try_into().unwrap())
    }

    /// Plane `p` of variant B / its SELECTED image (4 planes): plane 0 = the
    /// left 8 columns, plane 1 = the top 8 rows, planes 2/3 = every row for
    /// the SELECTED image only — so pixel indices 0, 1, 2, 3 (col) and 12+
    /// (sel) all occur, and the mask (rows 0..12) cuts the bottom off.
    fn plane_b(p: usize, sel: bool, r: usize) -> u16 {
        match p {
            0 => 0xFF00,
            1 => {
                if r < 8 {
                    0xFFFF
                } else {
                    0
                }
            }
            _ => {
                if sel {
                    0xFFFF
                } else {
                    0
                }
            }
        }
    }
    fn mask_b(r: usize) -> u16 {
        if r < 12 {
            0xFFFF
        } else {
            0
        }
    }
    /// Variant A (1 plane): a diagonal.
    fn plane_a(r: usize) -> u16 {
        0x8000 >> r
    }
    /// Variant C (8 planes, block 1): all planes set on every pixel — index
    /// 255 everywhere under a full mask.
    fn plane_c(_p: usize, _r: usize) -> u16 {
        0xFFFF
    }

    /// The fixture: an NRSC v11 (LP64) blob.
    fn blob() -> Vec<u8> {
        let mut b = vec![0u8; TOTAL];
        b[0..4].copy_from_slice(b"NRSC");
        let hdr = [
            NRSC_VER_CICON, // version
            1,              // n_trees
            N_OBJ as u32,
            OFF_TRI as u32,
            OFF_OBJ as u32,
            TOTAL as u32,
            OFF_DATA as u32, // off_frstr (empty)
            0,
            OFF_DATA as u32, // off_frimg (empty)
            0,
            OFF_CIB as u32, // off_ted (empty)
            0,
            OFF_CIB as u32, // off_ib (empty)
            0,
            OFF_CIB as u32, // off_bb (empty)
            0,
            OFF_CIB as u32,
            N_CIB as u32,
            OFF_CIC as u32,
            N_CIC as u32,
        ];
        for (i, v) in hdr.iter().enumerate() {
            put32(&mut b, 4 + i * 4, *v);
        }
        put64(&mut b, OFF_TRI, OFF_OBJ as u64); // tree 0 → object 0
        // OBJECTs: root G_BOX, two G_CICONs whose ob_spec is the CICONBLK's
        // byte offset (what the converter turns EmuTOS's table index into).
        let obj = |b: &mut [u8], i: usize, next: i16, head: i16, tail: i16, ty: u16, spec: u64| {
            let o = OFF_OBJ + i * OBJECT_SIZE;
            put16(b, o, next as u16);
            put16(b, o + 2, head as u16);
            put16(b, o + 4, tail as u16);
            put16(b, o + 6, ty);
            put64(b, o + 16, spec);
        };
        obj(&mut b, 0, -1, 1, 2, G_BOX_T, 0x0001_1100);
        obj(&mut b, 1, 2, -1, -1, G_CICON, OFF_CIB as u64);
        obj(&mut b, 2, 0, -1, -1, G_CICON, (OFF_CIB + CICONBLK_SIZE) as u64);
        // CICONBLK 0: 16×16, label "ICON0", mainlist → CICON A (→ B).
        let cib = |b: &mut [u8], i: usize, mask: usize, data: usize, text: usize, mainlist: u64| {
            let o = OFF_CIB + i * CICONBLK_SIZE;
            put64(b, o, mask as u64);
            put64(b, o + 8, data as u64);
            put64(b, o + 16, text as u64);
            let words: [i16; 11] = [0x1041, 0, 0, 0, 0, W, H, 0, H, 72, 8];
            for (k, w) in words.iter().enumerate() {
                put16(b, o + 24 + k * 2, *w as u16);
            }
            put64(b, o + ICONBLK_SIZE, mainlist);
        };
        cib(&mut b, 0, R_MONO0_MASK, R_MONO0_DATA, R_TEXT0, OFF_CIC as u64);
        cib(&mut b, 1, R_MONO1_MASK, R_MONO1_DATA, R_TEXT1, (OFF_CIC + 2 * CICON_SIZE) as u64);
        let cic = |b: &mut [u8], i: usize, planes: i16, cd: usize, cm: usize, sd: usize, sm: usize, next: u64| {
            let o = OFF_CIC + i * CICON_SIZE;
            put16(b, o, planes as u16);
            put64(b, o + 8, cd as u64);
            put64(b, o + 16, cm as u64);
            put64(b, o + 24, sd as u64);
            put64(b, o + 32, sm as u64);
            put64(b, o + 40, next);
        };
        cic(&mut b, 0, 1, R_A_DATA, R_A_MASK, 0, 0, (OFF_CIC + CICON_SIZE) as u64);
        cic(&mut b, 1, 4, R_B_DATA, R_B_MASK, R_B_SEL, R_B_SELMASK, 0);
        cic(&mut b, 2, 8, R_C_DATA, R_C_MASK, 0, 0, 0);
        // Rasters.
        for r in 0..H as usize {
            put16(&mut b, R_MONO0_MASK + r * 2, 0xFFFF);
            put16(&mut b, R_MONO0_DATA + r * 2, 0x0FF0);
            put16(&mut b, R_MONO1_MASK + r * 2, 0xFFFF);
            put16(&mut b, R_MONO1_DATA + r * 2, 0xF00F);
            put16(&mut b, R_A_DATA + r * 2, plane_a(r));
            put16(&mut b, R_A_MASK + r * 2, 0xFFFF);
            for p in 0..4 {
                put16(&mut b, R_B_DATA + (p * MONO_WORDS + r) * 2, plane_b(p, false, r));
                put16(&mut b, R_B_SEL + (p * MONO_WORDS + r) * 2, plane_b(p, true, r));
            }
            put16(&mut b, R_B_MASK + r * 2, mask_b(r));
            put16(&mut b, R_B_SELMASK + r * 2, mask_b(r));
            for p in 0..8 {
                put16(&mut b, R_C_DATA + (p * MONO_WORDS + r) * 2, plane_c(p, r));
            }
            put16(&mut b, R_C_MASK + r * 2, 0xFFFF);
        }
        b[R_TEXT0..R_TEXT0 + 5].copy_from_slice(b"ICON0");
        b[R_TEXT1..R_TEXT1 + 5].copy_from_slice(b"ICON1");
        b
    }

    /// A device whose palette makes every pen recognisable: pen `n` ↦
    /// `0x0100 + n` (bit 5 clear, as `set_color16` guarantees).
    fn device(planes: i16, num_colors: u16) -> CiconDevice {
        let mut palette = [0u16; 256];
        for (n, p) in palette.iter_mut().enumerate() {
            *p = 0x0100 | (n as u16 & 0x00DF) | ((n as u16 & 0x20) << 8);
        }
        CiconDevice { planes, num_colors, palette }
    }

    /// The colour EmuTOS ends up with for hardware index `n` on `dev`.
    fn colour_of(dev: &CiconDevice, n: usize) -> u16 {
        dev.palette[dev.rev_map_col()[n] as usize]
    }

    /// The relocated blob and its device-form buffer (or none).
    struct Loaded2 {
        blob: super::tests::Blob,
        buf: Vec<u16>,
        result: CiconBuf,
    }

    /// Relocate the fixture onto `APP_VA` at LP64 width, run `fix_cicons`
    /// for `dev` with a buffer of `cap` bytes on offer (`None` = the
    /// allocation fails), and return everything.
    fn load(dev: CiconDevice, cap: Option<usize>) -> Loaded2 {
        let mut blob = super::tests::Blob::new(&blob());
        let len = blob.len;
        let p = blob.as_mut_ptr();
        // The core itself asks `CiconDevice::live()` (no host here: 4 planes)
        // through `relocate_and_register_core`; to vary the device, drive the
        // two halves the core runs, in its order.
        assert!(unsafe { relocate_blob_core(p, len, APP_VA) }, "the fixture must relocate");
        let mut buf: Vec<u16> = Vec::new();
        let mut alloc = |bytes: usize| -> Option<(u64, u64)> {
            let cap = cap?;
            assert!(bytes <= cap, "asked for {bytes} bytes, the test offers {cap}");
            buf = vec![0u16; bytes / 2];
            Some((buf.as_mut_ptr() as u64, BUF_VA))
        };
        let result = unsafe { fix_cicons(p, APP_VA, Abi::Lp64, &mut || dev, &mut alloc) };
        Loaded2 { blob, buf, result }
    }

    fn mainlist(l: &Loaded2, block: usize) -> u64 {
        get64(l.blob.bytes(), OFF_CIB + block * CICONBLK_SIZE + ICONBLK_SIZE)
    }
    fn cicon_va(i: usize) -> u64 {
        APP_VA + (OFF_CIC + i * CICON_SIZE) as u64
    }
    fn cicon_field(l: &Loaded2, i: usize, k: usize) -> u64 {
        get64(l.blob.bytes(), OFF_CIC + i * CICON_SIZE + k * 8)
    }
    /// Device-form pixel `(c, r)` of the image at buffer offset `off`.
    fn px(l: &Loaded2, off: u64, c: usize, r: usize) -> u16 {
        let base = (off - BUF_VA) as usize / 2;
        l.buf[base + r * W as usize + c]
    }

    /// `best_match`, exact branch: on a 4-plane device block 0's 4-plane
    /// variant B wins over A although A comes first (`:274-275`), and block
    /// 1's 8-plane variant is "too many planes" (`:272`) → mono.
    #[test]
    fn best_match_takes_the_exact_variant_and_refuses_a_deeper_one() {
        let l = load(device(4, 16), Some(1 << 16));
        assert_eq!(mainlist(&l, 0), cicon_va(1), "block 0: the 4-plane variant B");
        assert_eq!(mainlist(&l, 1), 0, "block 1: an 8-plane variant on a 4-plane device → mono");
        assert_eq!(cicon_field(&l, 1, 5), 0, "next_res is cleared (`:471`)");
        assert_eq!(get16(l.blob.bytes(), OFF_CIC + CICON_SIZE), 4, "num_planes = gl_nplanes (`:470`)");
        // Both images of B (col + sel) in one buffer, sel right behind col
        // (`selbuf = colbuf + data_size`, `:455`).
        let bytes = (W as usize) * (H as usize) * 2;
        assert_eq!(l.result.len, 2 * bytes);
        assert_eq!(cicon_field(&l, 1, 1), BUF_VA, "col_data → the buffer");
        assert_eq!(cicon_field(&l, 1, 3), BUF_VA + bytes as u64, "sel_data → its second half");
        assert_eq!(cicon_field(&l, 1, 2), APP_VA + R_B_MASK as u64, "col_mask stays the file's");
        assert_eq!(cicon_field(&l, 1, 4), APP_VA + R_B_SELMASK as u64, "sel_mask stays the file's");
    }

    /// The exact-match transform: no expansion, no masking — each pixel's
    /// plane bits are its hardware index (`transform_cicon` without
    /// `expand_cicondata`), resolved through the palette by `REV_MAP_COL`.
    #[test]
    fn an_exact_match_is_transformed_without_expansion_or_masking() {
        let dev = device(4, 16);
        let l = load(dev, Some(1 << 16));
        let col = cicon_field(&l, 1, 1);
        let sel = cicon_field(&l, 1, 3);
        for r in 0..H as usize {
            for c in 0..W as usize {
                let bit = 0x8000u16 >> c;
                let mut idx = 0usize;
                for p in 0..4 {
                    if plane_b(p, false, r) & bit != 0 {
                        idx |= 1 << p;
                    }
                }
                // Outside the mask the index is still what the planes say
                // (no masking on an exact match).
                assert_eq!(px(&l, col, c, r), colour_of(&dev, idx), "col ({c},{r}) index {idx}");
                let mut sidx = 0usize;
                for p in 0..4 {
                    if plane_b(p, true, r) & bit != 0 {
                        sidx |= 1 << p;
                    }
                }
                assert_eq!(px(&l, sel, c, r), colour_of(&dev, sidx), "sel ({c},{r}) index {sidx}");
            }
        }
        // Index 15 → REV_MAP_COL[15] on a 16-colour device = pen 1 (MAP_COL[1]
        // = numcolors - 1 = 15): the classic "15 is black" of a 16-colour icon.
        assert_eq!(px(&l, sel, 0, 0), dev.palette[1]);
    }

    /// `best_match`, best-lower branch: on a 16-plane device (the live
    /// backend) block 0 keeps its deepest variant ≤ 16 (B, 4 planes) and
    /// block 1 its 8-plane one; both are expanded (`expand_cicondata`): the
    /// AND-of-all-planes fills the added planes, the mask is ANDed into
    /// every plane, and the index is planes 0..=7 — so a full-index pixel
    /// (15 of 4 planes, 255 of 8) reaches register 255 → pen 1, a masked-out
    /// pixel register 0 → pen 0 (white), everything else its own index.
    #[test]
    fn a_shallower_variant_is_expanded_to_the_device_depth() {
        let dev = device(16, 256);
        let l = load(dev, Some(1 << 16));
        assert_eq!(mainlist(&l, 0), cicon_va(1), "block 0: the deepest variant ≤ 16 is B");
        assert_eq!(mainlist(&l, 1), cicon_va(2), "block 1: its 8-plane variant");
        assert_eq!(get16(l.blob.bytes(), OFF_CIC + CICON_SIZE), 16, "num_planes = gl_nplanes");
        let rev = dev.rev_map_col();
        assert_eq!(rev[255], 1, "register 255 is pen 1 on a 256-colour device");
        assert_eq!(rev[0], 0);
        let col = cicon_field(&l, 1, 1);
        let sel = cicon_field(&l, 1, 3);
        for r in 0..H as usize {
            for c in 0..W as usize {
                let bit = 0x8000u16 >> c;
                let masked = mask_b(r) & bit != 0;
                let mut idx = 0usize;
                for p in 0..4 {
                    if plane_b(p, false, r) & bit != 0 {
                        idx |= 1 << p;
                    }
                }
                let want = if !masked {
                    colour_of(&dev, 0)
                } else if idx == 15 {
                    colour_of(&dev, 255)
                } else {
                    colour_of(&dev, idx)
                };
                assert_eq!(px(&l, col, c, r), want, "col ({c},{r}) index {idx} masked {masked}");
                let mut sidx = 0usize;
                for p in 0..4 {
                    if plane_b(p, true, r) & bit != 0 {
                        sidx |= 1 << p;
                    }
                }
                let want = if !masked {
                    colour_of(&dev, 0)
                } else if sidx == 15 {
                    colour_of(&dev, 255)
                } else {
                    colour_of(&dev, sidx)
                };
                assert_eq!(px(&l, sel, c, r), want, "sel ({c},{r}) index {sidx} masked {masked}");
            }
        }
        // Block 1: 8 planes all set under a full mask → index 255 everywhere.
        let c1 = cicon_field(&l, 2, 1);
        for r in 0..H as usize {
            for c in 0..W as usize {
                assert_eq!(px(&l, c1, c, r), colour_of(&dev, 255), "block 1 ({c},{r})");
            }
        }
        // The mask of the SELECTED pixel at (0,0): index 15 → black; the
        // pixel at (0,12) is outside the mask → white — the two ends of the
        // expansion.
        assert_eq!(px(&l, sel, 0, 0), dev.palette[1]);
        assert_eq!(px(&l, sel, 0, 12), dev.palette[0]);
    }

    /// `best_match`, the lower-only and none branches on a 2-plane device:
    /// block 0 keeps A (1 plane, the only one ≤ 2), block 1 has nothing ≤ 2
    /// → `mainlist` 0. Only A's image is transformed (the buffer is sized
    /// for it alone).
    #[test]
    fn with_no_exact_match_the_deepest_shallower_variant_wins_or_none() {
        let dev = device(2, 4);
        let l = load(dev, Some(1 << 16));
        assert_eq!(mainlist(&l, 0), cicon_va(0), "block 0: A, the only variant ≤ 2 planes");
        assert_eq!(mainlist(&l, 1), 0, "block 1: nothing ≤ 2 planes → mono");
        assert_eq!(l.result.len, (W as usize) * (H as usize) * 2, "one image, no SELECTED one");
        let col = cicon_field(&l, 0, 1);
        assert_eq!(col, BUF_VA);
        // 1 plane expanded to 2: the diagonal pixel has index 1 = all source
        // planes set → plane 1 is set too → index 3; the rest 0.
        for r in 0..H as usize {
            for c in 0..W as usize {
                let on = plane_a(r) & (0x8000u16 >> c) != 0;
                let want = colour_of(&dev, if on { 3 } else { 0 });
                assert_eq!(px(&l, col, c, r), want, "({c},{r})");
            }
        }
    }

    /// `transform_all_cicons`'s allocation failure (`:441-449`): no buffer,
    /// no colour — every `mainlist` is 0 and the CICON records keep their
    /// file data untouched, the load itself goes on.
    #[test]
    fn without_a_buffer_every_icon_falls_back_to_mono() {
        let l = load(device(16, 256), None);
        assert_eq!(mainlist(&l, 0), 0);
        assert_eq!(mainlist(&l, 1), 0);
        assert_eq!(l.result.len, 0);
        assert_eq!(l.result.access, 0);
        assert_eq!(cicon_field(&l, 1, 1), APP_VA + R_B_DATA as u64, "col_data untouched");
        assert_eq!(get16(l.blob.bytes(), OFF_CIC + CICON_SIZE), 4, "num_planes untouched");
    }

    /// Through the whole core, as `load_into_user` drives it: the descriptor
    /// resolves `R_OBJECT` of a G_CICON to an OBJECT whose relocated
    /// `ob_spec` addresses the CICONBLK — whose first bytes are the mono
    /// ICONBLK head (`obdefs.h:213-217`), what today's mono drawer and
    /// TeraDesk's `ob_spec.ciconblk->monoblk` read.
    #[test]
    fn r_object_of_a_g_cicon_leads_to_the_mono_iconblk_head() {
        let _g = super::tests::lock();
        let mut blob = super::tests::Blob::new(&blob());
        let len = blob.len;
        let p = blob.as_mut_ptr();
        let mut buf: Vec<u16> = Vec::new();
        let mut alloc = |bytes: usize| -> Option<(u64, u64)> {
            buf = vec![0u16; bytes / 2];
            Some((buf.as_mut_ptr() as u64, BUF_VA))
        };
        let desc = unsafe { relocate_and_register_core(p, len, APP_VA, Abi::Lp64, &mut alloc) }
            .expect("the fixture must load");
        let slot = unsafe { get_addr(&desc, R_OBJECT, 1) }.expect("object 1 exists");
        assert_eq!(slot.target, APP_VA + (OFF_OBJ + OBJECT_SIZE) as u64);
        // Read the OBJECT through the kernel alias, follow ob_spec.
        let spec = unsafe { (slot.access as *const u8).add(OB_SPEC_OFF_LP64).cast::<u64>().read_unaligned() };
        assert_eq!(spec, APP_VA + OFF_CIB as u64, "ob_spec = the CICONBLK's user VA");
        // The CICONBLK begins with the ICONBLK: three pointers, then the
        // eleven WORDs (`ib_char` first, `ib_wicon`/`ib_hicon` sixth/seventh).
        let head = (spec - APP_VA) as usize;
        assert_eq!(get64(blob.bytes(), head), APP_VA + R_MONO0_MASK as u64, "ib_pmask, relocated");
        assert_eq!(get64(blob.bytes(), head + 16), APP_VA + R_TEXT0 as u64, "ib_ptext, relocated");
        assert_eq!(get16(blob.bytes(), head + 24), 0x1041, "ib_char");
        assert_eq!(get16(blob.bytes(), head + 24 + 10), W as u16, "ib_wicon");
        assert_eq!(get16(blob.bytes(), head + 24 + 12), H as u16, "ib_hicon");
        let text = unsafe { core::slice::from_raw_parts(p.add(R_TEXT0), 5) };
        assert_eq!(text, b"ICON0");
        // Hostless: `CiconDevice::live()` is 4 planes → B chosen, one buffer.
        assert_eq!(get64(blob.bytes(), OFF_CIB + ICONBLK_SIZE), cicon_va(1));
        assert_eq!(desc.cicon.len, (W as usize) * (H as usize) * 4);
    }

    /// The short-read guard of A38, extended to the colour rasters: a blob
    /// whose CICON data would run past its declared size is **refused**
    /// (`rsrc_load` = 0), not relocated up to the point of failure — the
    /// OBJECTs still hold their file offsets afterwards.
    #[test]
    fn a_cicon_whose_raster_runs_past_the_blob_is_refused_untouched() {
        let mut b = blob();
        // Shorten the declared size so variant C's mask ends past it.
        put32(&mut b, 24, (TOTAL - 8) as u32);
        let mut short = super::tests::Blob::new(&b);
        let len = short.len;
        let p = short.as_mut_ptr();
        assert!(!unsafe { header_is_sound(p, len, Abi::Lp64) });
        assert!(!unsafe { relocate_blob_core(p, len, APP_VA) });
        assert_eq!(get64(short.bytes(), OFF_OBJ + OBJECT_SIZE + 16), OFF_CIB as u64, "ob_spec untouched");
        // A chain link that is not a CICON record, a cycle, and a SELECTED
        // image without its mask are refused the same way.
        let refused = |patch: &dyn Fn(&mut Vec<u8>), what: &str| {
            let mut b = blob();
            patch(&mut b);
            let mut bl = super::tests::Blob::new(&b);
            assert!(!unsafe { header_is_sound(bl.as_mut_ptr(), bl.len, Abi::Lp64) }, "{what}");
        };
        refused(&|b| put64(b, OFF_CIB + ICONBLK_SIZE, (OFF_CIC + 7) as u64), "misaligned link");
        refused(&|b| put64(b, OFF_CIC + 40, OFF_CIC as u64), "cycle A → A");
        refused(&|b| put64(b, OFF_CIC + CICON_SIZE + 32, 0), "sel_data without sel_mask");
        refused(&|b| put16(b, OFF_CIC + CICON_SIZE, 17), "17 planes");
        // And the unmodified fixture holds up.
        let mut ok = super::tests::Blob::new(&blob());
        assert!(unsafe { header_is_sound(ok.as_mut_ptr(), ok.len, Abi::Lp64) });
    }

    /// `REV_MAP_COL` as `init_colors` builds it (`vdi_col.c:736-747`), for
    /// the 256-colour device: `MAP_COL_ROM` inverted, pen 1 at register 255,
    /// pen 255 at register 15, the identity from 16 to 254.
    #[test]
    fn rev_map_col_matches_emutos_init_colors() {
        let rev = device(16, 256).rev_map_col();
        assert_eq!(rev[0], 0);
        assert_eq!(rev[255], 1);
        assert_eq!(rev[1], 2);
        assert_eq!(rev[2], 3);
        assert_eq!(rev[4], 4);
        assert_eq!(rev[6], 5);
        assert_eq!(rev[3], 6);
        assert_eq!(rev[5], 7);
        assert_eq!(rev[7], 8);
        assert_eq!(rev[8], 9);
        assert_eq!(rev[9], 10);
        assert_eq!(rev[10], 11);
        assert_eq!(rev[12], 12);
        assert_eq!(rev[14], 13);
        assert_eq!(rev[11], 14);
        assert_eq!(rev[13], 15);
        assert_eq!(rev[15], 255);
        for i in 16..255 {
            assert_eq!(rev[i], i as u8);
        }
        // 16 colours: pen 1 at register 15, nothing above.
        let rev16 = device(4, 16).rev_map_col();
        assert_eq!(rev16[15], 1);
        assert_eq!(rev16[255], 0);
    }
}
