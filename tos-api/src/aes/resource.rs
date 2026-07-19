//! AES Resource Manager (`rsrc_*`, opcodes 110–114) — Batch 1.
//!
//! Spec: [`docs/api/aes/resource-fileselector.md`]. **NewTOS uses a native
//! resource format** (decision 2026-06-21): the on-disk `.RSC` stores our
//! own 64-bit `OBJECT` layout, little-endian, with internal pointers held
//! as **byte offsets from the blob base**. Loading is therefore a pure
//! offset→pointer relocation — no endianness swap and no 16→64-bit
//! expansion (unlike a classic Atari `.RSC`). Coordinates are already in
//! pixels, so `rsrc_obfix` is an identity.
//!
//! Native `.RSC` layout (all little-endian):
//! ```text
//!   NrscHeader (28 bytes): magic "NRSC", version, n_trees, n_objects,
//!                          off_trindex, off_objects, total_size
//!   [u32; n_trees]   tree index  @ off_trindex  (root OBJECT index/tree)
//!   [Object; n_objects]          @ off_objects  (ob_spec = offset for the
//!                                                 string object types, else
//!                                                 a literal colour/char word)
//!   string / data bytes          (referenced by the ob_spec offsets)
//! ```
//!
//! **Batch-1 scope:** the format, the relocation core
//! (`relocate_and_register`), `rsrc_gaddr`, `rsrc_obfix`, `rsrc_free`.
//! Loading from SD (`bdos::fs::read_file`), TEDINFO/ICONBLK/BITBLK
//! sub-pointer relocation, and `rsrc_saddr` are Batch 2.

use super::object::{BitBlk, IconBlk, Object, TedInfo};
use super::Aespb;
use crate::bdos::process::{arena, mmu as pmmu};
use core::cell::UnsafeCell;
use core::mem::size_of;
use core::sync::atomic::{AtomicI32, AtomicPtr, AtomicU64, Ordering};
use tos_types::{BasePage, Long, MX_PREFTTRAM};

// Object types whose `ob_spec` is a string pointer (needs relocation).
const G_TEXT: u16 = 21;
const G_BOXTEXT: u16 = 22;
const G_IMAGE: u16 = 23;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;
const G_FTEXT: u16 = 29;
const G_FBOXTEXT: u16 = 30;
const G_ICON: u16 = 31;
const G_TITLE: u16 = 32;

// rsrc_gaddr / rsrc_saddr R_* selectors.
const R_TREE: i16 = 0;
const R_OBJECT: i16 = 1;

/// Maximum number of object trees registered from one `.RSC`. A real GEM
/// app has one tree per dialog/menu: QED's `qed.rsc` declares 32
/// (`n_trees=32`), so a 16-cap left `rsrc_gaddr(R_TREE, idx>=16)` returning
/// 0 — the app then dereferenced a null tree base and faulted. Sized to 64
/// for headroom (512 B of pointer table); the header's `n_trees` still
/// bounds the actual count.
const MAX_TREES: usize = 64;

/// Native resource-file header (little-endian, 28 bytes).
#[repr(C)]
pub struct NrscHeader {
    /// `b"NRSC"`.
    pub magic: [u8; 4],
    /// Format version (1).
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
}

/// The loaded resource (single resource per app — NewTOS is single-app).
struct Loaded {
    trees: UnsafeCell<[*mut Object; MAX_TREES]>,
    n_trees: UnsafeCell<usize>,
    objects: UnsafeCell<*mut Object>,
    n_objects: UnsafeCell<usize>,
}
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for Loaded {}

static RSC: Loaded = Loaded {
    trees: UnsafeCell::new([core::ptr::null_mut(); MAX_TREES]),
    n_trees: UnsafeCell::new(0),
    objects: UnsafeCell::new(core::ptr::null_mut()),
    n_objects: UnsafeCell::new(0),
};

/// Relocate a native `.RSC` blob in place (offsets → absolute pointers)
/// and register its trees. `base` points at the start of the blob (which
/// must remain live until `rsrc_free`). Returns false on a bad header.
///
/// # Safety
/// `base` must point at a valid, writable native `.RSC` blob of at least
/// its declared `total_size` bytes.
pub unsafe fn relocate_and_register(base: *mut u8) -> bool {
    // In-place kernel path: access base == target base.
    // SAFETY: caller guarantees a valid blob.
    unsafe { relocate_and_register_core(base, base as u64) }
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
unsafe fn relocate_and_register_core(access_base: *mut u8, target_base: u64) -> bool {
    // SAFETY: caller guarantees a valid blob.
    unsafe {
        if !relocate_blob_core(access_base, target_base) {
            return false;
        }
        let h = &*(access_base as *const NrscHeader);
        // Target VA of the object array (what callers dereference).
        let objs_target = target_base + h.off_objects as u64;
        // The tree index is read through the kernel alias.
        let tri = access_base.add(h.off_trindex as usize) as *const u32;
        let n = (h.n_trees as usize).min(MAX_TREES);
        let trees = &mut *RSC.trees.get();
        for (t, slot) in trees.iter_mut().enumerate().take(n) {
            let root = *tri.add(t) as u64;
            *slot = (objs_target + root * OBJECT_SIZE as u64) as *mut Object;
        }
        *RSC.n_trees.get() = n;
        *RSC.objects.get() = objs_target as *mut Object;
        *RSC.n_objects.get() = h.n_objects as usize;
    }
    true
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
            match o.ob_type & 0x00ff {
                G_STRING | G_BUTTON | G_TITLE => o.ob_spec += tb,
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
                // G_BOX/G_IBOX/G_BOXCHAR: ob_spec is a literal word.
                _ => {}
            }
        }
    }
    true
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

// ─── Per-process EL0 resource window ────────────────────────────────
//
// A `svc`-dispatched (EL0) `rsrc_load` cannot use the kernel high-half
// RSC_BUF: an EL0 app faults reading a kernel VA. EmuTOS puts the RSC in
// the *application's* memory (`rs_readit` → `dos_alloc_anyram`,
// gemrslib.c:775) and returns app pointers via `rs_gaddr`. NewTOS mirrors
// that: the blob goes into a BDOS-pool block that is mapped into the
// calling process's spare L3 window above its TPA (see
// `crate::bdos::process::mmu`), and `rsrc_gaddr` hands back the user VA.

/// Kernel high-half offset (mirror of `process::{arena,loader,mmu}`): a
/// BDOS-pool physical address `pa` is reachable from the kernel at
/// `pa + VA_OFFSET`.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

/// User-page size (one 4-KiB physical frame).
const PAGE_SIZE: u64 = 4096;

/// Base user VA of the per-process AES resource window — the spare L3
/// above the TPA. An EL0 `.RSC` blob is mapped here.
const RSC_WINDOW_BASE: u64 = arena::TPA_SIZE;

/// Size of that window (all the mappable low-half VA above the TPA).
const RSC_WINDOW_SIZE: u64 = pmmu::USER_TPA_MAX_VA - arena::TPA_SIZE;

// The window must start page-aligned (`map_page` requires it) and be
// non-empty (the extra L3 provisioned in `process::mmu`).
const _: () = assert!(RSC_WINDOW_BASE.is_multiple_of(PAGE_SIZE));
const _: () = assert!(RSC_WINDOW_SIZE >= PAGE_SIZE);

/// Pool `m_start` of the EL0 process's currently-loaded `.RSC` block, for
/// `Mfree` at `rsrc_free`; `0` when nothing is loaded or the load used the
/// kernel-VA path. Holds a [`Long`] (Atari 32-bit pool address) so it feeds
/// `mfree` directly.
static USER_RSC_RAW: AtomicI32 = AtomicI32::new(0);

/// Next free byte offset within [`RSC_WINDOW_BASE`]. Advanced per load so a
/// reload maps at a **fresh** (never-cached) VA — avoiding any TLB
/// invalidate on the app's return path (which would risk the QUIRK-001
/// QEMU wedge). Reset when the owning process changes so every process gets
/// the whole window.
static RSC_WINDOW_CURSOR: AtomicU64 = AtomicU64::new(0);

/// Basepage of the process [`RSC_WINDOW_CURSOR`] belongs to (`null` = none).
static RSC_WINDOW_OWNER: AtomicPtr<BasePage> = AtomicPtr::new(core::ptr::null_mut());

/// `rsrc_load` (110) — load `addr_in[0]` (a `.RSC` filename) from disk.
/// NewTOS searches `C:\GEMSYS\RSC\<name>` first, then `C:\<name>`.
/// Returns 1 on success, 0 on failure.
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

    // SAFETY: single-CPU AES; exclusive logical owner of the buffer.
    let buf = unsafe { &mut *RSC_BUF.0.get() };
    for prefix in [b"C:\\GEMSYS\\RSC\\".as_slice(), b"C:\\".as_slice()] {
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
            // Kernel-side caller: relocate in place, kernel-VA pointers.
            unsafe { relocate_and_register(buf.as_mut_ptr()) }
        } else {
            // EL0 app caller: copy into per-process EL0-mapped memory and
            // relocate to user VAs.
            unsafe { load_into_user(run, buf, len) }
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
unsafe fn load_into_user(run: *mut BasePage, src: &[u8; RSC_CAP], len: usize) -> bool {
    // Cheap non-NRSC reject before any allocation.
    // SAFETY: `len >= size_of::<NrscHeader>()` guaranteed by the caller.
    if unsafe { &(*(src.as_ptr() as *const NrscHeader)).magic } != b"NRSC" {
        return false;
    }
    let Some(slot) = crate::bdos::process::slot_for(run) else {
        return false;
    };

    // Page-round the blob and carve a fresh window VA (see cursor doc).
    let pages = (len as u64 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    if RSC_WINDOW_OWNER.swap(run, Ordering::AcqRel) != run {
        RSC_WINDOW_CURSOR.store(0, Ordering::Release);
    }
    let cursor = RSC_WINDOW_CURSOR.load(Ordering::Acquire);
    if pages > RSC_WINDOW_SIZE - cursor {
        return false; // window exhausted → honest ENSMEM-style failure
    }
    let app_va = RSC_WINDOW_BASE + cursor;

    // Allocate a page-aligned pool block (over-allocate one page so the
    // rounded-up base fits, exactly as `arena::alloc_tpa`).
    // SAFETY: single-CPU trap context holds the implicit BDOS lock.
    let raw = unsafe {
        crate::bdos::mem::traps::mxalloc((pages + PAGE_SIZE) as Long, MX_PREFTTRAM)
    };
    if raw == 0 {
        return false;
    }
    let base_pa = (raw as u64 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let base_kva = base_pa.wrapping_add(VA_OFFSET) as *mut u8;

    // Copy the raw file bytes into the block via its kernel alias, then
    // relocate offsets → user VAs (accessed through the alias, resolved to
    // `app_va`).
    // SAFETY: `base_kva` aliases `pages` live RW pool bytes; `src` is valid.
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr(), base_kva, len);
    }
    // SAFETY: `base_kva` holds the copied blob; header re-checked inside.
    if !unsafe { relocate_and_register_core(base_kva, app_va) } {
        // SAFETY: `raw` is on the allocated list.
        unsafe { crate::bdos::mem::traps::mfree(raw) };
        return false;
    }

    // Map the block into the process's spare-L3 window (user RW, non-exec).
    let mut off = 0u64;
    while off < pages {
        // SAFETY: slot valid; VA in-window & page-aligned; PA page-aligned.
        if unsafe {
            pmmu::map_page(slot, app_va + off, base_pa + off, pmmu::PageFlags::UserRwNx)
        }
        .is_err()
        {
            // SAFETY: `raw` is on the allocated list.
            unsafe { crate::bdos::mem::traps::mfree(raw) };
            return false;
        }
        off += PAGE_SIZE;
    }
    // Publish the new PTEs + blob before the app reads them (no TLBI: the
    // window VAs were unmapped, so no stale TLB entry can exist).
    // SAFETY: barrier-only.
    unsafe { pmmu::publish_user_mappings() };

    // Own the block by the calling process so its `Pterm` reclaims it even
    // if the app exits without `rsrc_free` (EmuTOS frees `ap_rscmem` at
    // process teardown). `rsrc_free` additionally `Mfree`s it eagerly.
    // SAFETY: `raw` on the allocated list; `run` is a pointer-equality tag.
    unsafe { crate::bdos::mem::traps::set_owner(raw, run.cast()) };

    RSC_WINDOW_CURSOR.store(cursor + pages, Ordering::Release);
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
    // Return a per-process EL0 RSC block to the pool if one is tracked. The
    // window mapping is intentionally left in place: the app holds no live
    // pointers after the free, and the next load takes a fresh window VA, so
    // the stale mapping is never remapped (no TLBI) nor read. The block's
    // owner tag makes a later `Pterm` a no-op for it (it is now off the
    // allocated list, so `free_all_owned` / a second `Mfree` skip it).
    let raw = USER_RSC_RAW.swap(0, Ordering::AcqRel);
    if raw != 0 {
        // SAFETY: single-CPU trap context (implicit BDOS lock); `raw` came
        // from `mxalloc` in `load_into_user` and has not been freed.
        unsafe { crate::bdos::mem::traps::mfree(raw) };
    }

    // SAFETY: single-CPU AES. `objects` is non-null iff a resource is loaded
    // (set by `relocate_and_register`, cleared here).
    let was_loaded = unsafe { !(*RSC.objects.get()).is_null() };
    // SAFETY: single-CPU AES.
    unsafe {
        *RSC.n_trees.get() = 0;
        *RSC.objects.get() = core::ptr::null_mut();
        *RSC.n_objects.get() = 0;
    }
    pb.set_ret(if was_loaded { 1 } else { 0 });
}

/// `rsrc_gaddr` (112) — address of resource object `int_in[1]` of `R_*`
/// type `int_in[0]` → `addr_out[0]`.
pub fn rsrc_gaddr(pb: &Aespb) {
    let rtype = pb.intin(0);
    let index = pb.intin(1) as usize;
    // SAFETY: single-CPU AES; pointers were set by relocate_and_register.
    let addr = unsafe {
        match rtype {
            R_TREE => {
                if index < *RSC.n_trees.get() {
                    (*RSC.trees.get())[index] as u64
                } else {
                    0
                }
            }
            R_OBJECT => {
                let objs = *RSC.objects.get();
                if !objs.is_null() && index < *RSC.n_objects.get() {
                    objs.add(index) as u64
                } else {
                    0
                }
            }
            _ => 0,
        }
    };
    pb.set_addrout(0, addr);
    pb.set_ret(if addr != 0 { 1 } else { 0 });
}

/// `rsrc_saddr` (113) — store an address into the resource table. Batch 2.
pub fn rsrc_saddr(pb: &Aespb) {
    pb.set_ret(0);
}

/// `rsrc_obfix` (114) — convert object `int_in[1]`'s coordinates. Native
/// resources already store pixel coordinates, so this is an identity that
/// simply reports success.
pub fn rsrc_obfix(pb: &Aespb) {
    pb.set_ret(1);
}

/// Size of one native `Object` record (for `R_OBJECT` indexing / tools).
pub const OBJECT_SIZE: usize = size_of::<Object>();
