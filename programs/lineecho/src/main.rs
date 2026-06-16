//! `lineecho` — the first *real compiled* NewTOS program.
//!
//! A freestanding AArch64 program built against the TOS GEMDOS ABI
//! (not a hand-assembled byte array): it prompts, reads a line with
//! `Cconrs` (the line editor), echoes it back with `Cconws`, and exits
//! via `Pterm`. Doubles as the end-to-end test of `Cconrs` from EL0.
//!
//! ## Memory model
//!
//! The Phase-06 D3 loader maps a **single** 4 KiB page at user VA 0:
//! basepage at `0x000..0x100`, our text/rodata from `0x100` up. The
//! loader hands us `SP_EL0` pointing at an (unmapped) high stack, so
//! `_start` repoints `SP` to the **top of that one page** (`0x1000`)
//! and the stack grows down inside it — no separate stack mapping
//! needed while the program stays small. (A multi-page loader + real
//! TPA stack is the Phase-09 lift.)

#![no_std]
#![no_main]

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// ─── GEMDOS syscall shim (trap = svc #1, fn id in x8) ───────────────

#[inline(always)]
unsafe fn svc1(fnid: u64, a0: u64) -> i64 {
    let r: i64;
    // SAFETY: `svc #1` is the GEMDOS trap; the kernel reads x8 (fn id)
    // and x0 (arg). Return (sign-extended LONG) lands in x0.
    unsafe {
        asm!(
            "svc #1",
            in("x8") fnid,
            inout("x0") a0 => r,
            options(nostack),
        );
    }
    r
}

/// `Cconws` (0x09) — write a NUL-terminated string.
fn cconws(s: &[u8]) {
    unsafe { svc1(0x09, s.as_ptr() as u64) };
}

/// `Cconrs` (0x0A) — read an edited line into `p` (`p[0]`=max IN,
/// `p[1]`=len OUT, `p[2..]`=buffer).
fn cconrs(p: *mut u8) {
    unsafe { svc1(0x0A, p as u64) };
}

/// `Pterm` (0x4C) — terminate with exit code `rc`.
fn pterm(rc: i32) -> ! {
    unsafe { svc1(0x4C, rc as u64) };
    loop {
        core::hint::spin_loop();
    }
}

// ─── Program ─────────────────────────────────────────────────────────

static PROMPT: &[u8] = b"\r\n[lineecho] type a line, then Enter: \0";
static REPLY: &[u8] = b"\r\n[lineecho] you typed: \0";
static NEWLINE: &[u8] = b"\r\n\0";

/// Maximum line length we accept from `Cconrs`.
const MAX: usize = 32;

fn run() -> ! {
    cconws(PROMPT);

    // Line-input record on the stack: [max, len, buf...]. One extra
    // byte so we can NUL-terminate the line for `Cconws`.
    let mut rec = [0u8; 2 + MAX + 1];
    rec[0] = MAX as u8;
    cconrs(rec.as_mut_ptr());

    let len = (rec[1] as usize).min(MAX);
    rec[2 + len] = 0; // NUL-terminate for Cconws

    cconws(REPLY);
    cconws(&rec[2..]);
    cconws(NEWLINE);

    pterm(0);
}

/// ELF entry point (`e_entry = 0x100`). Naked so we control the
/// prologue: the loader's `SP_EL0` points at an unmapped high stack,
/// so we set `SP` to the top of our single mapped page before any
/// stack use, then jump into `run`.
#[no_mangle]
#[unsafe(naked)]
#[unsafe(link_section = ".text._start")]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "mov x9, #0x1000",   // top of the TPA page = initial SP
        "mov sp, x9",
        "b   {run}",
        run = sym run,
    )
}
