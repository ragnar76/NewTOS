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
//! - Names are byte slices compared **exactly** (no case folding), and
//!   a name ends at its first NUL on both sides: a lookup key may be a
//!   bare `b"NAME"` (the `svc` path, whose string reader returns the
//!   length without the NUL) or a NUL-padded buffer such as the shell's
//!   128-byte `D.s_cmd` copy (`kernel/src/shell_loop.rs`). See
//!   [`lookup`].
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
/// `name` is matched by [`lookup`] byte-for-byte up to its first NUL:
/// the name ends at the first NUL on both sides, so `b"foo"` and
/// `b"foo\0"` register the same name. A name that is empty up to its
/// first NUL is never found.
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
///
/// The comparison is exact (case-sensitive, DESK43 decision REG-a) over
/// the bytes **before the first NUL** of `name` and of the entry's name
/// (`docs/analysis/desk43-u9-prep-2026-10-14.md` §4). This makes a
/// NUL-padded path buffer — the shell loop hands `pexec::load` its whole
/// `D.s_cmd` copy — find the same entry as the bare name the `svc` path
/// passes, the same way the FAT branch of `pexec::load` already ends the
/// path at its first NUL. An empty name (up to the NUL) matches nothing.
///
/// The registry is a NewTOS construction with no EmuTOS counterpart
/// (EmuTOS starts its ROM programs by entry point, not by name,
/// `aes/gemshlib.c:570-580`); its names are an internal, not a contract.
pub fn lookup(name: &[u8]) -> Option<&'static [u8]> {
    let name = until_nul(name);
    if name.is_empty() {
        return None;
    }
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
        if until_nul(entry_name) == name {
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

/// `s` up to (not including) its first NUL, or all of `s` without one.
fn until_nul(s: &[u8]) -> &[u8] {
    &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())]
}

#[cfg(test)]
mod tests {
    use super::{lookup, register, until_nul};

    // The registry is one process-global table of `MAX_PROGRAMS` entries
    // with no removal, shared by every test of this crate's test binary:
    // this module registers exactly one name, in one test, and uses names
    // no other test uses.
    static BLOB: [u8; 4] = *b"BLOB";

    #[test]
    fn until_nul_cuts_at_first_nul() {
        assert_eq!(until_nul(b"ABC"), b"ABC");
        assert_eq!(until_nul(b"ABC\0"), b"ABC");
        assert_eq!(until_nul(b"ABC\0X\0"), b"ABC");
        assert_eq!(until_nul(b"\0ABC"), b"");
        assert_eq!(until_nul(b""), b"");
    }

    /// `docs/analysis/desk43-u9-prep-2026-10-14.md` §4, test list item 1.
    #[test]
    fn lookup_ends_the_name_at_the_first_nul_and_compares_exactly() {
        assert!(register(b"U9REGTST", &BLOB));
        let hit = |n: &[u8]| lookup(n).map(|b| b.as_ptr()) == Some(BLOB.as_ptr());

        // The bare name (the `svc` path).
        assert!(hit(b"U9REGTST"));
        // The shell's whole 128-byte, NUL-padded `D.s_cmd` copy.
        let mut s_cmd = [0u8; 128];
        s_cmd[..8].copy_from_slice(b"U9REGTST");
        assert!(hit(&s_cmd));
        // The name ends at the first NUL: what follows it is not part of it.
        assert!(hit(b"U9REGTST\0X"));

        // Exact otherwise: no prefix, no extension, no case folding.
        assert!(!hit(b"U9REGTST2"));
        assert!(!hit(b"U9REGTS"));
        assert!(!hit(b"u9regtst"));
        assert!(!hit(b"C:\\U9REGTST"));
        // An empty name finds nothing.
        assert!(lookup(b"").is_none());
        assert!(lookup(&[0u8; 128]).is_none());
    }
}
