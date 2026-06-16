//! Per-process user-memory arena (Phase 06 D3 / interim).
//!
//! Source-port programs need page-aligned memory for their code +
//! data + bss + stack. The BDOS pool's `Mxalloc` returns 16-byte-
//! aligned addresses — fine for `malloc` but not for the user-space
//! page-table mappings the loader installs. Until Phase 09 (or
//! earlier, if a real app needs it) introduces an aligned allocator
//! on top of the BDOS pool, we provide one statically-allocated
//! 4-KiB page per process slot. That's enough for the EC4-class
//! "tiny test program" payloads the D3 selftest exercises and for
//! the smallest real source-ports.
//!
//! ## Sizing
//!
//! [`super::MAX_PROCESSES`] (16) × 4 KiB = 64 KiB BSS. Combined with
//! the per-slot page-table arena from
//! [`super::mmu`] (16 × 16 KiB = 256 KiB) the total static budget
//! for process state is 320 KiB — well under any DRAM-pressure
//! threshold on supported boards.
//!
//! Multi-page programs and proper basepage / text / data / bss
//! splits are a Phase 09 concern; that step will introduce a
//! page-aligned BDOS-pool allocator and retire this module.

use super::MAX_PROCESSES;

/// One 4-KiB-aligned page of user memory.
#[repr(C, align(4096))]
struct UserPage([u8; 4096]);

/// Per-slot user-memory arena. Each slot owns exactly one 4-KiB
/// page that holds the loaded program's basepage + text + data
/// (combined for the tiny D3 layout).
#[unsafe(no_mangle)]
static mut __PROCESS_USER_PAGES: [UserPage; MAX_PROCESSES] =
    [const { UserPage([0u8; 4096]) }; MAX_PROCESSES];

const PAGE_SIZE: u64 = 4096;

/// Phase-05 high-half offset. Mirror of the constant in
/// `kernel::arch::aarch64::mmu`; user-page statics live at high-half
/// kernel VAs and `map_page` needs their phys.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

#[inline(always)]
const fn va_to_pa(va: u64) -> u64 {
    va.wrapping_sub(VA_OFFSET)
}

/// Kernel-side mutable byte slice over `slot`'s user page. Used by
/// the loader to copy ELF segment bytes in. Returns `None` for an
/// out-of-range slot.
///
/// # Safety
///
/// Caller holds the BDOS giant lock for the duration of the borrow.
/// Two concurrent borrows of the same slot are not detected by the
/// type system; the BDOS-lock contract is what makes them safe.
#[allow(clippy::deref_addrof)] // see `super::mmu::init_slot_tables`
pub unsafe fn page_bytes_mut(slot: usize) -> Option<&'static mut [u8; 4096]> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    // SAFETY: caller contract.
    let page = unsafe { &mut (*&raw mut __PROCESS_USER_PAGES)[slot] };
    Some(&mut page.0)
}

/// Phys address of `slot`'s user page — the value
/// [`super::mmu::map_page`] consumes for the L3 entry.
///
/// Returns `None` for an out-of-range slot.
#[allow(clippy::deref_addrof)] // see `super::mmu::init_slot_tables`
pub fn page_phys(slot: usize) -> Option<u64> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    // SAFETY: read-only address-of a `static mut`.
    let page = unsafe { &(*&raw const __PROCESS_USER_PAGES)[slot] };
    Some(va_to_pa((&raw const *page) as u64))
}

/// Size of a user page in bytes (= one 4-KiB physical frame).
pub const fn page_size() -> u64 {
    PAGE_SIZE
}
