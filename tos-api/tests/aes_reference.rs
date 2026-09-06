//! AES conformance against measured TOS behaviour.
//!
//! See `aes_reference/harness.rs` for what these tests cover, which `global`
//! fields are deliberately not compared and why. The case module is generated
//! by `tosdump/host/mkrusttests`; do not edit it by hand.
//!
//! Both files live in the `aes_reference/` subdirectory so cargo does not pick
//! them up as integration-test targets of their own.

#[path = "aes_reference/harness.rs"]
mod harness;

#[path = "aes_reference/cases_r1.rs"]
mod cases_r1;

/// The AES leaves the VDI's **string** input device in SAMPLE mode.
///
/// EmuTOS' scheduler polls the keyboard on every dispatcher pass — `chkkbd()`
/// (`aes/gemdisp.c:157-177`) — and reads the key through `gsx_char()`, which
/// opens with `intin = {4, 2}` + `gsx_ncode(SET_INPUT_MODE, 0, 2)`, i.e.
/// `vsin_mode(4, 2)` (`aes/gemgsxif.c:459-463`). The four device modes are
/// VDI globals (`vdi/vdi_defs.h:258`), not per-workstation fields, so that
/// write is observable to any application: `vqin_mode(4)` reports 1 instead
/// of the `vdimouse_init` default 0 (`vdi/vdi_mouse.c:575-578`).
///
/// That is what the reference TT reported, and this test compares against the
/// measurement rather than against a literal: it decodes `vqin_mode`'s
/// `INTOUT[0]` out of the two VDI dumps taken on that machine —
/// `EMUTOS14-VDI-r2.BIN` record 16 (case 51) and `EMUTOS14-VDI-all.BIN`
/// record 40 (case 77), the same query in two independent runs.
///
/// It has to live in the **AES** binary: `tests/vdi_reference` deliberately
/// runs no AES and opens the physical workstation itself, and `v_opnwk` puts
/// all four devices back to REQUEST (`vdimouse_init`,
/// `vdi/vdi_control.c:563`), so 0 is the right answer there. This binary sets
/// up what the dumper had — `appl_init`, `graf_handle`, `v_opnvwk` on the
/// AES's physical handle — and `v_opnvwk` performs no such reset.
#[test]
fn aes_dispatch_leaves_the_string_device_in_sample_mode() {
    /// `vqin_mode`'s `INTOUT[0]` for `intin[0] = dev`, out of a VDI reference
    /// record: 32-byte header, 1592-byte records, `INTOUT` at offset 800,
    /// `INTIN` at 32, opcode at 2 (`tests/vdi_reference/harness.rs`).
    fn tt_vqin_mode(dump: &[u8], rec: usize, dev: i16) -> i16 {
        let r = &dump[32 + rec * 1592..][..1592];
        let be16 = |o: usize| (((r[o] as u16) << 8) | r[o + 1] as u16) as i16;
        assert_eq!(be16(2), 115, "record {rec} is not a vqin_mode call");
        assert_eq!(be16(32), dev, "record {rec} queried another device");
        be16(800)
    }

    static DUMP_R2: &[u8] = include_bytes!("../../tosdump/reference/EMUTOS14-VDI-r2.BIN");
    static DUMP_ALL: &[u8] = include_bytes!("../../tosdump/reference/EMUTOS14-VDI-all.BIN");

    let _guard = harness::AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // `appl_init` + `graf_handle` + `v_opnvwk`, the context the dumper ran in.
    // The `appl_init` trap alone already ran a dispatch.
    let ctx = harness::context();

    // Both runs of the reference TT, and the three devices the AES does not
    // touch: locator/valuator/choice keep the `vdimouse_init` default.
    assert_eq!(tt_vqin_mode(DUMP_R2, 16, 4), 1);
    assert_eq!(tt_vqin_mode(DUMP_ALL, 40, 4), 1);
    assert_eq!(tt_vqin_mode(DUMP_ALL, 6, 1), 0);
    assert_eq!(tt_vqin_mode(DUMP_ALL, 38, 2), 0);
    assert_eq!(tt_vqin_mode(DUMP_ALL, 39, 3), 0);

    for (dev, want) in [(1i16, 0i16), (2, 0), (3, 0), (4, 1)] {
        assert_eq!(
            vqin_mode(ctx.handle, dev),
            want,
            "vqin_mode({dev}) after an AES dispatch"
        );
    }
}

/// One raw `vqin_mode` (opcode 115) VDI call on `handle`, returning
/// `INTOUT[0]`. The five arrays and the pointer block are laid out the way
/// `harness::open_workstation` lays them for `v_opnvwk`.
fn vqin_mode(handle: i16, dev: i16) -> i16 {
    let mut contrl = Box::new([0i16; 12]);
    let mut intin = Box::new([0i16; 128]);
    let mut ptsin = Box::new([0i16; 256]);
    let mut intout = Box::new([0i16; 128]);
    let mut ptsout = Box::new([0i16; 256]);

    contrl[0] = 115; // vqin_mode
    contrl[3] = 1; // one intin word
    contrl[6] = handle;
    intin[0] = dev;

    let block = [
        contrl.as_mut_ptr() as u64,
        intin.as_mut_ptr() as u64,
        ptsin.as_mut_ptr() as u64,
        intout.as_mut_ptr() as u64,
        ptsout.as_mut_ptr() as u64,
    ];

    // SAFETY: `block` holds five live pointers to arrays owned here, each
    // longer than the one input and one output word `vqin_mode` touches, and
    // all of them outlive the call.
    unsafe { tos_api::vdi::dispatch(block.as_ptr() as u64, harness::ABI) };
    intout[0]
}
