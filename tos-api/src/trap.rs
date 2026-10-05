//! Trap argument marshalling.
//!
//! Native (AArch64) callers follow AAPCS64:
//!   * function ID in `x8`,
//!   * up to eight argument registers `x0..x7`,
//!   * return value in `x0` (sign-extended `i64`).
//!
//! The exception entry stub captures the eight argument registers and
//! the function ID into a [`TrapArgs`] which is then passed by
//! reference into the per-class dispatcher. Larger argument sets (none
//! occur in NewTOS's GEMDOS surface) would need a stack tail; this is
//! deliberately not supported in PHASE-02.
//!
//! The m68k user emulator (PHASE-10) translates legacy stack-passed
//! parameters into this same `TrapArgs` shape before re-entering the
//! dispatcher, so the TOS-API layer never sees m68k specifics.

use tos_types::Long;

pub use crate::uaccess::Space;

/// Trap class as identified by the SVC immediate. The classes mirror
/// the m68k trap numbers for source-level compatibility.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum TrapClass {
    /// Trap #1 — GEMDOS.
    Gemdos = 1,
    /// Trap #2 — the **GEM trap**, shared by the VDI and the AES. The
    /// selector in `x0` (115 = VDI, 200 = AES) chooses which; `x1` holds
    /// the parameter block. See [`crate::vdi`] / [`crate::aes`].
    Gem = 2,
    /// Trap #13 — BIOS.
    Bios = 13,
    /// Trap #14 — XBIOS.
    Xbios = 14,
}

impl TrapClass {
    /// Decode the SVC immediate into a `TrapClass`. Returns `None` for
    /// unknown immediates so the dispatcher can fail with `EINVFN`.
    pub const fn from_imm(imm: u16) -> Option<Self> {
        match imm {
            1 => Some(Self::Gemdos),
            2 => Some(Self::Gem),
            13 => Some(Self::Bios),
            14 => Some(Self::Xbios),
            _ => None,
        }
    }
}

/// Captured native register state at trap entry.
///
/// The layout is mirrored bit-for-bit by `exception_vectors.S`; do not
/// reorder fields without updating the assembly.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct TrapArgs {
    /// Argument registers `x0..x7` at trap entry.
    pub regs: [u64; 8],
    /// Function ID (`x8` at trap entry, low 16 bits significant).
    pub func: u64,
    /// SVC immediate (= TOS trap number).
    pub svc_imm: u16,
    /// Whose address space the pointer arguments live in (ARCH03 stage 3a):
    /// [`Space::User`] for a trap from an EL0 program — its pointers are
    /// accessed with its rights through [`crate::uaccess`] — and
    /// [`Space::Kernel`] for an in-kernel caller (the built-in desktop's
    /// `svc` from EL1, and every `TrapArgs` the kernel builds to call a leaf
    /// function directly), whose pointers are kernel addresses. Set by the
    /// trap handler from the caller's `SPSR_EL1.M` ([`Space::from_spsr`]).
    pub space: Space,
}

impl TrapArgs {
    /// A `TrapArgs` built by the kernel for its own call of a leaf function:
    /// the pointer arguments are kernel addresses ([`Space::Kernel`]).
    #[inline]
    pub const fn kernel(regs: [u64; 8], func: u64, svc_imm: u16) -> Self {
        Self { regs, func, svc_imm, space: Space::Kernel }
    }

    /// Function ID truncated to its low 16 bits, matching TOS WORD width.
    #[inline]
    pub const fn func_id(&self) -> u16 {
        self.func as u16
    }

    /// Read argument `i` (zero-based) as a 16-bit signed value.
    #[inline]
    pub const fn arg_i16(&self, i: usize) -> i16 {
        self.regs[i] as i16
    }

    /// Read argument `i` as a 16-bit unsigned value.
    #[inline]
    pub const fn arg_u16(&self, i: usize) -> u16 {
        self.regs[i] as u16
    }

    /// Read argument `i` as a 32-bit signed value.
    #[inline]
    pub const fn arg_i32(&self, i: usize) -> i32 {
        self.regs[i] as i32
    }

    /// Read argument `i` as a raw 64-bit value (pointer payload).
    #[inline]
    pub const fn arg_u64(&self, i: usize) -> u64 {
        self.regs[i]
    }

    /// Read argument `i` as a `*const u8` pointer.
    #[inline]
    pub const fn arg_ptr(&self, i: usize) -> *const u8 {
        self.regs[i] as *const u8
    }
}

/// Convenience: build the canonical "invalid function" return value.
#[inline]
pub const fn einvfn() -> Long {
    tos_types::error::EINVFN
}
