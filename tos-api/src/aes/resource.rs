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
use core::cell::UnsafeCell;
use core::mem::size_of;

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

const MAX_TREES: usize = 16;

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
    // SAFETY: caller guarantees a valid blob.
    unsafe {
        if !relocate_blob(base) {
            return false;
        }
        let h = &*(base as *const NrscHeader);
        let objs = base.add(h.off_objects as usize) as *mut Object;
        // Build the tree table from the tree index.
        let tri = base.add(h.off_trindex as usize) as *const u32;
        let n = (h.n_trees as usize).min(MAX_TREES);
        let trees = &mut *RSC.trees.get();
        for (t, slot) in trees.iter_mut().enumerate().take(n) {
            let root = *tri.add(t) as usize;
            *slot = objs.add(root);
        }
        *RSC.n_trees.get() = n;
        *RSC.objects.get() = objs;
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
    // SAFETY: caller guarantees a valid blob.
    unsafe {
        let h = &*(base as *const NrscHeader);
        if &h.magic != b"NRSC" {
            return false;
        }
        let objs = base.add(h.off_objects as usize) as *mut Object;
        let b = base as u64;
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
                G_STRING | G_BUTTON | G_TITLE => o.ob_spec += b,
                G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => {
                    o.ob_spec += b;
                    // Relocate the shared TEDINFO's internal pointers once.
                    if !substruct_seen(objs, i, o.ob_spec) {
                        let t = &mut *(o.ob_spec as *mut TedInfo);
                        reloc(&mut t.te_ptext, b);
                        reloc(&mut t.te_ptmplt, b);
                        reloc(&mut t.te_pvalid, b);
                    }
                }
                G_IMAGE => {
                    o.ob_spec += b;
                    if !substruct_seen(objs, i, o.ob_spec) {
                        let bi = &mut *(o.ob_spec as *mut BitBlk);
                        reloc(&mut bi.bi_pdata, b);
                    }
                }
                G_ICON => {
                    o.ob_spec += b;
                    if !substruct_seen(objs, i, o.ob_spec) {
                        let ib = &mut *(o.ob_spec as *mut IconBlk);
                        reloc(&mut ib.ib_pmask, b);
                        reloc(&mut ib.ib_pdata, b);
                        reloc(&mut ib.ib_ptext, b);
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
        if let Some(len) = crate::bdos::fs::read_file(path_str, buf) {
            // SAFETY: buf now holds the file bytes; relocate validates the
            // header before touching anything.
            if len >= size_of::<NrscHeader>() && unsafe { relocate_and_register(buf.as_mut_ptr()) } {
                pb.set_ret(1);
                return;
            }
        }
    }
    pb.set_ret(0);
}

/// `rsrc_free` (111) — drop the loaded resource's tree table.
///
/// Returns 1 only if a resource was actually loaded; 0 when nothing was
/// loaded and on a double free, matching EmuTOS `rs_free` (returns 0 when
/// `ap_rscmem == NULL`, gemrslib.c:685-708; spec verification criterion
/// resource-fileselector.md:439-440). The tree table is reset either way.
pub fn rsrc_free(pb: &Aespb) {
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
