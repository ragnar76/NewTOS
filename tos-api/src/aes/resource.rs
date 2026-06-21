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

use super::object::Object;
use super::Aespb;
use core::cell::UnsafeCell;
use core::mem::size_of;

// Object types whose `ob_spec` is a string pointer (needs relocation).
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;
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
        let h = &*(base as *const NrscHeader);
        if &h.magic != b"NRSC" {
            return false;
        }
        let objs = base.add(h.off_objects as usize) as *mut Object;

        // Relocate each object's string ob_spec (offset → pointer).
        for i in 0..h.n_objects as usize {
            let o = &mut *objs.add(i);
            if o.ob_spec != 0 && matches!(o.ob_type, G_STRING | G_BUTTON | G_TITLE) {
                o.ob_spec += base as u64;
            }
        }

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

/// `rsrc_load` (110) — load `addr_in[0]` (a `.RSC` filename) from disk.
/// Batch 1: the file path (SD read + static buffer) lands in Batch 2;
/// for now this reports failure rather than faking success (no stub).
pub fn rsrc_load(pb: &Aespb) {
    // Batch 2 wires bdos::fs::read_file into a static buffer here, then
    // calls relocate_and_register. Until then, honestly report "not
    // loaded" so callers do not draw a phantom tree.
    pb.set_ret(0);
}

/// `rsrc_free` (111) — drop the loaded resource's tree table.
pub fn rsrc_free(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        *RSC.n_trees.get() = 0;
        *RSC.objects.get() = core::ptr::null_mut();
        *RSC.n_objects.get() = 0;
    }
    pb.set_ret(1);
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
