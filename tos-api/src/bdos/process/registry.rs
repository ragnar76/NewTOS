//! RAM-resident program registry — Phase 06 D3 stand-in for the
//! filesystem-backed `Pexec` loader.
//!
//! Classic GEMDOS `Pexec(PE_LOADGO, path, …)` opens `path` via
//! `Fopen`, reads the program header, copies the body. NewTOS does
//! not have a filesystem until Phase 09, so until then `Pexec`
//! resolves `path` against this in-kernel registry of named ELF
//! blobs. Selftests / boot code call [`register`] at startup with
//! a fixed name → blob mapping; [`lookup`] is what [`super::pterm`]
//! / `Pexec` consult during dispatch.
//!
//! ## Scope
//!
//! - Static-sized table: [`MAX_PROGRAMS`] entries. Plenty for the
//!   D3 + later phase selftests (each typically registers ≤ 2
//!   programs).
//! - Names are byte slices (NUL-terminated `c"NAME"` literals work
//!   directly via the AAPCS64 ABI; the registry only compares
//!   bytes).
//! - No removal — the registry is build-once at boot.
//!
//! Phase 09 will replace this with the real filesystem-backed
//! loader and [`register`] will become a debug-only fallback.

use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// Maximum number of programs the registry can hold. Increase by
/// editing the constant — no runtime growth.
pub const MAX_PROGRAMS: usize = 8;

/// A single registry entry. All four fields are atomic so
/// `register` / `lookup` can run lock-free across the BDOS-lock
/// boundary the way the rest of [`super`] does.
struct Entry {
    name_ptr: AtomicPtr<u8>,
    name_len: AtomicUsize,
    blob_ptr: AtomicPtr<u8>,
    blob_len: AtomicUsize,
}

impl Entry {
    const fn empty() -> Self {
        Self {
            name_ptr: AtomicPtr::new(ptr::null_mut()),
            name_len: AtomicUsize::new(0),
            blob_ptr: AtomicPtr::new(ptr::null_mut()),
            blob_len: AtomicUsize::new(0),
        }
    }
}

static REGISTRY: [Entry; MAX_PROGRAMS] = [const { Entry::empty() }; MAX_PROGRAMS];

/// Register a program. Returns `true` on success, `false` if the
/// registry is full (caller-side bug — bump [`MAX_PROGRAMS`]).
///
/// `name` is matched verbatim by [`lookup`], byte-for-byte. Trailing
/// NULs in the supplied slice **are** part of the name; pass
/// `b"foo"` not `b"foo\0"` unless callers will look up the latter.
///
/// Both `name` and `blob` must outlive the registry entry — typical
/// callers pass `&'static [u8]` literals or `static` arrays.
pub fn register(name: &'static [u8], blob: &'static [u8]) -> bool {
    for entry in REGISTRY.iter() {
        // Atomic claim: blob_ptr is the "occupied" indicator. CAS
        // from null → non-null wins exactly one writer per slot.
        if entry
            .blob_ptr
            .compare_exchange(
                ptr::null_mut(),
                blob.as_ptr() as *mut u8,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            entry.blob_len.store(blob.len(), Ordering::Release);
            entry.name_ptr.store(name.as_ptr() as *mut u8, Ordering::Release);
            entry.name_len.store(name.len(), Ordering::Release);
            return true;
        }
    }
    false
}

/// Find a program by name. Returns `Some(&'static [u8])` referencing
/// the blob registered with [`register`], or `None` if no entry
/// matches.
pub fn lookup(name: &[u8]) -> Option<&'static [u8]> {
    for entry in REGISTRY.iter() {
        let name_ptr = entry.name_ptr.load(Ordering::Acquire);
        if name_ptr.is_null() {
            continue;
        }
        let name_len = entry.name_len.load(Ordering::Acquire);
        // SAFETY: name_ptr / name_len were stored together by the
        // CAS-protected `register` path; both are 'static.
        let entry_name = unsafe {
            core::slice::from_raw_parts(name_ptr, name_len)
        };
        if entry_name == name {
            let blob_ptr = entry.blob_ptr.load(Ordering::Acquire);
            let blob_len = entry.blob_len.load(Ordering::Acquire);
            // SAFETY: same — 'static blob.
            let blob = unsafe {
                core::slice::from_raw_parts(blob_ptr, blob_len)
            };
            return Some(blob);
        }
    }
    None
}
