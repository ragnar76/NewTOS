//! PROC06 — **every terminate path hands the `Pexec` caller `rc & 0xFFFF`,
//! zero-extended.**
//!
//! EmuTOS:
//!
//! ```c
//! void xterm(UWORD rc) { … run->p_dreg[0] = rc; … termuser(); }  /* bdos/proc.c:601,616 */
//! #define terminate() xterm(-32)                                 /* bdos/console.c:101 */
//! ```
//!
//! `p_dreg[0]` is a LONG and `termuser` reloads the parent's `d0` from it
//! (`bdos/rwa.S:253,265-267`), so the parent's `Pexec` (modes 0/4/6) sees the
//! low 16 bits of the child's exit code with the high word clear — never a
//! negative value. Spec: `docs/api/gemdos/process/4c-pterm.md`, "Exit-code
//! matrix" and its host-test list; `4b-pexec.md`, native ABI (UWORD
//! zero-extended into `x0`, `0..=0xFFFF`).
//!
//! The kernel's shared terminate tail (`finish_terminate` in
//! `kernel/src/arch/aarch64/exceptions.rs`) writes
//! [`exit_code_for_caller`] into the `Pexec` caller's `x0`; the kernel crate
//! is `aarch64-unknown-none-softfloat` only, so the decision is tested here.
//! Before PROC06 the tail wrote `rc as i64 as u64`: the console Ctrl-C's `-32`
//! reached the caller as `0xFFFF_FFFF_FFFF_FFE0` (a "load error"), the
//! upcall-nesting overflow's `-998` likewise.

use tos_api::bdos::process::pterm::{exit_code_for_caller, parent_exit_code};
use tos_api::gemdos::console::CTRLC_RC;

/// Spec host test 1: the exit-code matrix at the pure-function level.
#[test]
fn parent_exit_code_matrix() {
    let table: &[(i32, i32)] = &[
        (0, 0),
        (1, 1),
        (0x7FFF, 0x7FFF),
        (0x8000, 32768),
        (-1, 65535),
        (-2, 65534),
        (-32, 65504),
        (-998, 64538),
        (0x12345, 0x2345),
        (i32::MIN, 0),
        (i32::MAX, 0xFFFF),
    ];
    for &(rc, want) in table {
        assert_eq!(parent_exit_code(rc), want, "parent_exit_code({rc})");
    }
}

/// Spec host test 1, last clause: the result is always in `0..=0xFFFF`.
#[test]
fn parent_exit_code_is_never_negative() {
    let mut rc = i32::MIN;
    loop {
        let v = parent_exit_code(rc);
        assert!((0..=0xFFFF).contains(&v), "parent_exit_code({rc}) = {v}");
        match rc.checked_add(0x0001_0003) {
            Some(n) => rc = n,
            None => break,
        }
    }
}

/// Spec host test 2: the console Ctrl-C (`terminate()` = `xterm(-32)`)
/// reaches the parent as 65504.
#[test]
fn ctrl_c_exit_code_reaches_the_parent_as_65504() {
    assert_eq!(CTRLC_RC, -32);
    assert_eq!(parent_exit_code(CTRLC_RC), 65504);
    assert_eq!(exit_code_for_caller(CTRLC_RC), 65504u64);
}

/// Spec host test 3: the value the terminate tail writes into the `Pexec`
/// caller's 64-bit `x0`.
#[test]
fn exit_code_for_caller_is_zero_extended_into_x0() {
    assert_eq!(exit_code_for_caller(-32), 65504u64);
    assert_eq!(exit_code_for_caller(-1), 65535u64);
    assert_eq!(exit_code_for_caller(-998), 64538u64);
    assert_eq!(exit_code_for_caller(0), 0u64);
    assert_eq!(exit_code_for_caller(0x8000), 0x8000u64);
    assert_eq!(exit_code_for_caller(0x12345), 0x2345u64);
    for rc in [i32::MIN, -0x8000, -2, -1, 0, 1, 0x7FFF, 0xFFFF, 0x10000, i32::MAX] {
        let x0 = exit_code_for_caller(rc);
        assert_eq!(x0 >> 16, 0, "upper 48 bits of x0 for rc {rc}: {x0:#x}");
        assert_eq!(x0, u64::from(rc as u16), "x0 for rc {rc}");
        // A Pexec caller reading x0 as a LONG never sees a load error.
        assert!((x0 as i64) >= 0 && (x0 as u32 as i32) >= 0);
    }
}
