//! AArch64 architecture support: boot stub, exception vectors,
//! register access, low-level halt.

pub mod boot;
pub mod dtb;
pub mod el0;
pub mod exceptions;
pub mod mmu;
pub mod user_entry;

/// Spin forever in the lowest-power state. Used by the panic handler and the
/// secondary-core parking loop in `boot::_start`.
#[inline(always)]
pub fn halt() -> ! {
    loop {
        // SAFETY: `wfe` has no side effects beyond entering low-power wait.
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) };
    }
}
