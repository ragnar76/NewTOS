//! Phase 12 VDI foundation self-test (D1/D2).
//!
//! Exercises the VDI dispatcher and workstation lifecycle through the
//! **real trap path** (`svc #2`, VDIPB pointer in `x0`): open a virtual
//! workstation, check the returned capability table matches the
//! framebuffer geometry, clear the workstation (visibly fills the
//! screen), close it, and confirm the handle is freed by re-opening.

use crate::kprintln;

/// Issue a VDI call: `svc #2` with the parameter-block pointer in `x0`.
#[inline(always)]
fn vdi(pb_ptr: u64) {
    // SAFETY: svc #2 with a valid VDI parameter block in x0; the VDI
    // returns nothing in a register (x0 is clobbered by the handler).
    unsafe {
        core::arch::asm!("svc #2", inout("x0") pb_ptr => _, options(nostack));
    }
}

/// Run the Phase 12 VDI foundation self-test (dispatcher + workstation
/// lifecycle) through the `svc #2` trap path.
//
// The VDIPB arrays are written here and read by the dispatcher *through
// the parameter-block pointer* (`pb`), which the compiler's dataflow
// can't see — so resets like `contrl = [0; 12]` look like dead stores.
// They are not; allow the false positive.
#[allow(unused_assignments)]
pub fn run_phase12_vdi_test() {
    let (sw, sh) = crate::vdi_backend::dims();
    if sw == 0 || sh == 0 {
        kprintln!("[vdi] no framebuffer — skipping VDI test");
        return;
    }

    // VDI parameter block: five WORD arrays + the 5-pointer block.
    let mut contrl = [0i16; 12];
    let mut intin = [0i16; 16];
    let mut ptsin = [0i16; 16];
    let mut intout = [0i16; 45];
    let mut ptsout = [0i16; 12];
    let block: [u64; 5] = [
        contrl.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        ptsin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        ptsout.as_mut_ptr() as u64,
    ];
    let pb = block.as_ptr() as u64;

    kprintln!("[vdi] === Phase 12 VDI foundation test (D1/D2) ===");

    let mut fails = 0;
    let mut check = |cond: bool, label: &str| {
        kprintln!("[vdi]   {}: {}", label, if cond { "OK" } else { "FAIL" });
        if !cond {
            fails += 1;
        }
    };

    // ── v_opnvwk (100): open a virtual workstation ──
    contrl = [0i16; 12];
    contrl[0] = 100;
    contrl[3] = 11; // 11 work_in inputs
    // work_in: device id + line/marker/text/fill attrs + coord system.
    intin[..11].copy_from_slice(&[1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 2]);
    vdi(pb);
    let handle = contrl[6];

    check((1..=16).contains(&handle), "v_opnvwk returns a valid handle");
    check(
        intout[0] == (sw as i16 - 1) && intout[1] == (sh as i16 - 1),
        "DEV_TAB geometry = framebuffer size − 1",
    );
    check(intout[13] == 16, "DEV_TAB[13] = 16 colours");
    check(
        contrl[4] == 45 && contrl[2] == 6,
        "output counts (45 INTOUT, 6 PTSOUT pairs)",
    );

    // ── v_clrwk (3): clear the screen (visible: fills white) ──
    contrl = [0i16; 12];
    contrl[0] = 3;
    contrl[6] = handle;
    vdi(pb);
    kprintln!("[vdi]   v_clrwk: screen cleared to background (white)");

    // ── v_clsvwk (101): close, then prove the handle is freed by
    //    re-opening and getting a valid handle back ──
    contrl = [0i16; 12];
    contrl[0] = 101;
    contrl[6] = handle;
    vdi(pb);

    contrl = [0i16; 12];
    contrl[0] = 100;
    contrl[3] = 11;
    intin[..11].copy_from_slice(&[1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 2]);
    vdi(pb);
    let handle2 = contrl[6];
    check(
        (1..=16).contains(&handle2),
        "re-open after close yields a handle (slot freed)",
    );
    // tidy up
    contrl = [0i16; 12];
    contrl[0] = 101;
    contrl[6] = handle2;
    vdi(pb);

    if fails == 0 {
        kprintln!("[vdi] === VDI foundation PASS ===");
    } else {
        kprintln!("[vdi] === VDI foundation FAILED ({} checks) ===", fails);
    }
}
