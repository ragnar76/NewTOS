//! EL0 (user-mode) virtual-address layout for Phase 06+ processes.
//!
//! Phase 05 D5 reserves the low-half VA window for per-process use
//! and defines the conventions Phase 06's `Pexec` will follow when
//! constructing a basepage / TPA / EL0 stack tuple. This module
//! does **not** allocate or map anything at boot; the actual
//! per-process page-table install happens in
//! [`crate::arch::aarch64::mmu`] / [`tos_api::bdos::proc`] later.
//!
//! ## VA window
//!
//! The kernel runs in the high-half (TTBR1) at
//! `0xFFFF_0000_0000_0000 + phys`. The mirror-image low-half window
//! `0x0000_0000_0000_0000 ..= 0x0000_FFFF_FFFF_FFFF` (the same
//! 48-bit span via `TTBR0_EL1` with `T0SZ = 16`) is reserved for
//! user-mode programs. Stage-1 boot tables currently identity-map
//! `0..4 GiB` of this range; Phase 06 replaces those with a
//! per-process L0 on each `Pexec`.
//!
//! ## Per-process layout (TOS convention, Compendium "BasePage")
//!
//! ```text
//!   USER_VA_BASE  ─┐ basepage (256 bytes, see `tos-types::Basepage`)
//!                  │ text
//!                  │ data
//!                  │ bss
//!                  │ free TPA
//!                  │ ⋮
//!   USER_STACK_TOP ┘ EL0 stack, growing **downward**
//! ```
//!
//! The TPA boundary (`p_hitpa` in the basepage) is set at process
//! load time; the stack top is fixed by this module so source-port
//! glue code can hand `__stack_top_inclusive_for(pid)` to the
//! C-runtime startup without a per-process linker tweak.
//!
//! All constants below are **pure VA values**; they do not imply any
//! particular physical mapping. Phase 06 wires them to physical
//! frames carved from the BDOS TT-RAM pool (per
//! `docs/api/structures/basepage.md` § "Allocation and lifetime").

/// Inclusive lower bound of the user-mode VA window.
///
/// `0x0000_0000_0000_0000` — the very first byte of the low-half
/// `TTBR0_EL1` translation regime. The basepage of the first
/// process loaded via `Pexec` lives here; subsequent processes
/// share the same VA layout via per-process TTBR0 swap.
pub const USER_VA_BASE: u64 = 0x0000_0000_0000_0000;

/// Exclusive upper bound of the user-mode VA window.
///
/// `0x0001_0000_0000_0000` — one past the last addressable byte
/// under `T0SZ = 16` (48-bit input VAs). Anything at or above this
/// value lies outside the low-half translation regime and would
/// fault unconditionally (in the "TBI"-equivalent zone for
/// `TCR_EL1.TBI0`).
pub const USER_VA_END_EXCLUSIVE: u64 = 0x0001_0000_0000_0000;

/// Top (exclusive) of the **default** EL0 stack for the first
/// process loaded by `Pexec`.
///
/// 4-KiB-aligned and located 4 KiB below `USER_VA_END_EXCLUSIVE` so
/// the topmost user page is reserved as a guard against `SP_EL0`
/// underflow when AAPCS64-required pre-call alignment forces a
/// `sub sp, sp, #16`.
///
/// Phase 06's process loader will populate the page immediately
/// below this address with the TOS-style `argv`/`env` blob
/// (Compendium "Pexec mode 0/3 entry conditions"), then point
/// `SP_EL0` at this value before the `eret` into user code.
pub const USER_STACK_TOP_EXCLUSIVE: u64 =
    USER_VA_END_EXCLUSIVE - 0x1000;

/// Default initial size of the EL0 stack reserved by `Pexec` when
/// the program header does not specify a stacksize. 64 KiB matches
/// the kernel main stack and is generous for typical TOS programs;
/// programs that request more (`PRGFLAGS` extension or `Pexec`
/// caller-supplied `stacksize`) get a larger reservation.
pub const USER_STACK_DEFAULT_SIZE: u64 = 64 * 1024;

/// Inclusive lower bound of the **default** EL0 stack region —
/// the address `Pexec` initialises `SP_EL0` against minus the
/// stack size. Useful for the basepage `p_hitpa` calculation.
pub const USER_STACK_BOTTOM_INCLUSIVE: u64 =
    USER_STACK_TOP_EXCLUSIVE - USER_STACK_DEFAULT_SIZE;

/// Sanity assertions — surface any constant drift at compile time.
const _: () = assert!(USER_VA_BASE < USER_VA_END_EXCLUSIVE);
const _: () = assert!(USER_STACK_TOP_EXCLUSIVE < USER_VA_END_EXCLUSIVE);
const _: () = assert!(USER_STACK_BOTTOM_INCLUSIVE < USER_STACK_TOP_EXCLUSIVE);
const _: () = assert!(USER_STACK_TOP_EXCLUSIVE & 0xFFF == 0);
const _: () = assert!(USER_STACK_DEFAULT_SIZE & 0xFFF == 0);
