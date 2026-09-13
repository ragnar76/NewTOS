//! GEM-trap selector self-test: the `d0` query/probe values of the VDI
//! dispatcher front end.
//!
//! Spec: [`docs/api/vdi/_dispatcher.md`], section "`d0` special values
//! (`vdi_asm.S:40-52`)" — the table of what the trap returns for a `d0`
//! that is *not* a VDI/AES call:
//!
//! | `d0` on entry | Effect |
//! |---------------|--------|
//! | `-1` (`0xffff`) | return the address of the VDI dispatcher (`_GSX_ENTRY`) |
//! | `-2` | GDOS-absence probe: `d0` **left unchanged**, `rte` |
//! | anything else | `rte`, no action — `d0` left unchanged |
//!
//! EmuTOS reference: `references/emutos/vdi/vdi_asm.S:40-49` (pinned
//! `1a7484ab`), whose comment is explicit: *"Just like Atari TOS, we leave
//! d0 unchanged, indicating the absence of GDOS if this is called with -2
//! in the low-order word of d0."*
//!
//! The probe is what `vq_gdos()` / `vq_vgdos()` are built out of, so this
//! test drives the two library functions' arithmetic verbatim (see
//! [`vq_gdos`] / [`vq_vgdos`]) over the **real** trap path rather than
//! asserting on an internal.
//!
//! Like every other `selftest_*` module this one is not part of the normal
//! boot; call [`run`] from `lib.rs` when verifying this path.

use crate::kprintln;

/// Issue the GEM trap (`svc #2`) with `selector` in `x0` and return the
/// `x0` the kernel came back with.
///
/// `x1` (the parameter-block register) is zeroed: none of the selectors
/// this test uses consults it — the VDI (115) and AES (200) selectors,
/// which do, are covered by `selftest_vdi` / `selftest_aes`.
#[inline(always)]
fn gem_trap(selector: i64) -> i64 {
    let out: i64;
    // SAFETY: the GEM trap is a plain synchronous exception; for a
    // non-VDI/non-AES selector the handler reads no caller memory at all.
    unsafe {
        core::arch::asm!(
            "svc #2",
            inout("x0") selector => out,
            in("x1") 0u64,
            options(nostack),
        );
    }
    out
}

/// `vq_gdos()` — "is GDOS present?", byte for byte the arithmetic of
/// `sdk/gemlib/overlay/_gc_vq_gdos.c:30-39` (itself the transliteration of
/// the m68k `moveq #-2,d0 / trap #2 / cmp.w #-2,d0 / sne / ext.w`).
///
/// Returns 1 when GDOS is present, 0 when it is not.
fn vq_gdos() -> i16 {
    let x0 = gem_trap(-2);
    i16::from(x0 as i16 != -2)
}

/// `vq_vgdos()` — the GDOS id/version probe, the arithmetic of
/// `sdk/gemlib/overlay/_gc_vq_vgdos.c`: the GDOS id, or `-2` when there is
/// no GDOS.
fn vq_vgdos() -> i64 {
    let x0 = gem_trap(-2);
    if x0 as i16 == -2 {
        -2
    } else {
        x0
    }
}

/// Run the GEM-trap selector self-test. Prints one line per check and a
/// summary; returns the number of failed checks.
pub fn run() -> u32 {
    kprintln!("[gemtrap] === GEM trap selector contract (V16) ===");

    let mut fails = 0u32;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[gemtrap]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // ── The contract itself: an unhandled selector leaves x0 alone ──
    let probe = gem_trap(-2);
    check(probe == -2, "selector -2 leaves x0 unchanged (-2)");
    let other = gem_trap(999);
    check(other == 999, "selector 999 leaves x0 unchanged (999)");

    // ── The two library functions built on it ──
    check(vq_gdos() == 0, "vq_gdos() reports no GDOS (0)");
    check(vq_vgdos() == -2, "vq_vgdos() reports no GDOS (-2)");

    // ── Selector -1, the "jsr-able dispatcher address" query ──
    //
    // Documented NewTOS deviation (docs/COMPATIBILITY.md, "GEM trap
    // selector -1"): EmuTOS returns `_GSX_ENTRY`, an m68k address the
    // caller may `jsr` to. NewTOS has no such entry — the VDI is reachable
    // only through `svc #2`, and the dispatcher is EL1 code an EL0 caller
    // can neither call nor even read — so the query answers 0, "no
    // jsr-able address", which is the value a conforming caller tests for
    // before using it.
    check(
        gem_trap(-1) == 0,
        "selector -1 answers 0 (no jsr-able address; documented deviation)",
    );

    kprintln!(
        "[gemtrap] === {} ===",
        if fails == 0 {
            "all checks passed"
        } else {
            "FAILURES — see above"
        }
    );
    fails
}
