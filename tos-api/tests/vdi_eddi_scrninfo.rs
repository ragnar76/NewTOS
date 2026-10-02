//! PORT06-EDDI — the `EdDI` cookie and `vq_scrninfo` (VDI 102, sub-opcode 1).
//!
//! Spec: `docs/api/vdi/eddi-vq_scrninfo.md` (approved 2026-10-02; E1–E9 as
//! recommended in §7, E3 = `0x02`). EmuTOS has no EdDI, so the reference for
//! the 273 words is the spec's §6.3 derivation plus what a real EdDI reported
//! on the Falcon truecolor workstation NewTOS models: the VDI Enhancer 1.00
//! on EmuTOS in Hatari
//! (`measurements/measurement-2026-10-02-scrninfo/hatari-falcon-tc-enhancer/
//! SCRNINFO.TXT`, section 3b). [`ENHANCER_FALCON_TC`] is that dump, copied
//! word for word; the expected NewTOS answer is it with the **one** word the
//! spec makes different (`[14]`, E3) patched — so a second difference shows
//! up as a test failure, not as a silently adopted value.
//!
//! **No ILP32 case.** `vq_scrninfo` moves WORD arrays only (no pointer in
//! CONTRL or in the answer — `[6/7]` is 0, E5), and a 64-bit host process
//! cannot build the 4-byte-pointer parameter block an ILP32 call needs (see
//! `vdi_vex_wheelv.rs`).

#[path = "vdi_reference/harness.rs"]
mod harness;

use std::sync::Mutex;

use tos_api::bdos::process::Abi;

const VQ_EXTND: i16 = 102;
const POISON: i16 = harness::POISON;

/// INTOUT room: well over the 273 words, as the probe and gemlib size it, so
/// a write past `INTOUT[272]` stays visible as a changed poison word.
const OUT_N: usize = 512;

/// The Enhancer 1.00's `work_out[0..=272]` on EmuTOS Falcon truecolor, as
/// `SCRNINFO.TXT` §3b lists it (rows of eight, index of the first word left).
#[rustfmt::skip]
const ENHANCER_FALCON_TC: [i16; 273] = {
    let mut w = [0i16; 273];
    let head: [i16; 16] = [2, 2, 16, 0, -32768, 0, 0, 0, 5, 5, 5, 0, 1, 0, 0, 0];
    let mut i = 0;
    while i < 16 { w[i] = head[i]; i += 1; }
    // [16..128]: -1 except the listed bit numbers.
    i = 16;
    while i < 128 { w[i] = -1; i += 1; }
    // red [16..20] = 11..15, green [32..36] = 6..10, blue [48..52] = 0..4
    i = 0;
    while i < 5 {
        w[16 + i] = 11 + i as i16;
        w[32 + i] = 6 + i as i16;
        w[48 + i] = i as i16;
        i += 1;
    }
    w[80] = 5; // genlock
    // [128..=272] = 0 (already).
    w
};

/// The NewTOS answer the spec defines: the Enhancer's, with `[14]` = `0x02`
/// (Falcon bit order, E3). The Enhancer is EdDI 1.00, which has no `[14]`
/// (spec §2: it arrives with 1.10) and leaves it 0.
fn expected() -> [i16; 273] {
    let mut w = ENHANCER_FALCON_TC;
    w[14] = 0x02;
    w
}

/// A parameter block with INTOUT room for 273+ words and gemlib's full
/// 15-WORD CONTRL. The shared harness block's INTOUT stops at 128.
struct SPb {
    contrl: Box<[i16; 16]>,
    intin: Box<[i16; 16]>,
    ptsin: Box<[i16; 16]>,
    intout: Box<[i16; OUT_N]>,
    ptsout: Box<[i16; 1024]>,
    block: Box<[u64; 5]>,
}

impl SPb {
    fn new() -> SPb {
        let mut pb = SPb {
            contrl: Box::new([0; 16]),
            intin: Box::new([0; 16]),
            ptsin: Box::new([0; 16]),
            intout: Box::new([0; OUT_N]),
            ptsout: Box::new([0; 1024]),
            block: Box::new([0; 5]),
        };
        pb.block[0] = pb.contrl.as_ptr() as u64;
        pb.block[1] = pb.intin.as_ptr() as u64;
        pb.block[2] = pb.ptsin.as_ptr() as u64;
        pb.block[3] = pb.intout.as_ptr() as u64;
        pb.block[4] = pb.ptsout.as_ptr() as u64;
        pb
    }

    /// Opcode 102 the way gemlib issues it: `CONTRL[0,1,3,5,6]` set,
    /// `CONTRL[2]/[4]` left to the driver (poisoned here), one INTIN word.
    /// Returns the trap's `d0`.
    fn op102(&mut self, sub: i16, mode: i16, handle: i16) -> i16 {
        self.contrl.fill(POISON);
        self.intin.fill(POISON);
        self.ptsin.fill(POISON);
        self.intout.fill(POISON);
        self.ptsout.fill(POISON);
        self.contrl[0] = VQ_EXTND;
        self.contrl[1] = 0;
        self.contrl[3] = 1;
        self.contrl[5] = sub;
        self.contrl[6] = handle;
        self.intin[0] = mode;
        // SAFETY: the five arrays outlive the call and are large enough for
        // both vq_extnd (45/12 words) and vq_scrninfo (273 words).
        unsafe { tos_api::vdi::dispatch(self.block.as_ptr() as u64, Abi::Lp64) }
    }
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    harness::VDI_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Spec §3.1 call form: `CONTRL = 102/0/-/1/-/1/handle`, `INTIN[0] = 2`.
/// The full 273-word answer, on the virtual **and** the physical handle (E6).
#[test]
fn vq_scrninfo_answers_the_falcon_hicolor_table_word_for_word() {
    let _g = lock();
    let wk = harness::workstation();

    for handle in [wk.handle, wk.phys] {
        let mut pb = SPb::new();
        let ret = pb.op102(1, 2, handle);

        let want = expected();
        for i in 0..273 {
            assert_eq!(
                pb.intout[i], want[i],
                "handle {handle}: work_out[{i}] = {} (Enhancer {})",
                pb.intout[i], ENHANCER_FALCON_TC[i]
            );
        }
        // Nothing beyond INTOUT[272], nothing into PTSOUT (spec §3.1; the
        // Enhancer writes neither).
        assert!(pb.intout[273..].iter().all(|&w| w == POISON), "INTOUT past 272 written");
        assert!(pb.ptsout.iter().all(|&w| w == POISON), "PTSOUT written");
        // Output counts: documented CONTRL[4] = 272, no PTSOUT.
        assert_eq!(pb.contrl[4], 272, "CONTRL[4]");
        assert_eq!(pb.contrl[2], 0, "CONTRL[2]");
        // The handle and the input arrays are the caller's.
        assert_eq!(pb.contrl[6], handle);
        assert_eq!(pb.intin[0], 2);
        // d0: no handler that reports magnitudes ran.
        assert_eq!(ret, 0, "d0 (flip_y)");
    }

    // The library's own table is the one dispatched.
    assert_eq!(tos_api::vdi::eddi::scrninfo_table(), expected());
}

/// The bit-number tables say where `vs_color`'s pseudo-palette word puts each
/// gun: rebuilding the word from `[16..63]` must give `pack_color16`'s word,
/// the one `vro_cpyfm` moves into a device-format MFDB (spec §5, "what
/// `vq_scrninfo` says must be exactly what `vro_cpyfm` reads").
#[test]
fn the_bit_tables_rebuild_the_device_word() {
    let t = tos_api::vdi::eddi::scrninfo_table();
    let build = |r5: u16, g5: u16, b5: u16| -> u16 {
        let mut px = 0u16;
        for (base, v) in [(16usize, r5), (32, g5), (48, b5)] {
            for bit in 0..16 {
                let n = t[base + bit];
                if n < 0 {
                    continue;
                }
                if v & (1 << bit) != 0 {
                    px |= 1 << n;
                }
            }
        }
        px
    };
    for &(rgb, r5, g5, b5) in &[
        ([1000, 1000, 1000], 31, 31, 31),
        ([1000, 0, 0], 31, 0, 0),
        ([0, 1000, 0], 0, 31, 0),
        ([0, 0, 1000], 0, 0, 31),
        ([0, 0, 0], 0, 0, 0),
    ] {
        assert_eq!(build(r5, g5, b5), tos_api::vdi::pack_color16(rgb), "rgb {rgb:?}");
    }
    // The genlock bit is the one bit no colour sets (E9).
    assert_eq!(t[80], 5);
    assert_eq!(tos_api::vdi::pack_color16([1000, 1000, 1000]) & (1 << 5), 0);
}

/// E7: `CONTRL[5] == 1` with `INTIN[0] != 2` is `vq_extnd` exactly as before
/// — the same words as the plain `CONTRL[5] == 0` call with that mode.
#[test]
fn sub_opcode_1_without_mode_2_stays_vq_extnd() {
    let _g = lock();
    let wk = harness::workstation();

    for mode in [0i16, 1, 3, -1, 0x0102] {
        let mut plain = SPb::new();
        let ret_plain = plain.op102(0, mode, wk.handle);
        let mut sub1 = SPb::new();
        let ret_sub1 = sub1.op102(1, mode, wk.handle);

        assert_eq!(sub1.intout[..], plain.intout[..], "mode {mode}: INTOUT");
        assert_eq!(sub1.ptsout[..], plain.ptsout[..], "mode {mode}: PTSOUT");
        assert_eq!(sub1.contrl[2], 6, "mode {mode}: CONTRL[2]");
        assert_eq!(sub1.contrl[4], 45, "mode {mode}: CONTRL[4]");
        assert_eq!(ret_sub1, ret_plain, "mode {mode}: d0");
        assert_eq!(ret_sub1, 1, "mode {mode}: vq_extnd raises flip_y");
        assert!(sub1.intout[45..].iter().all(|&w| w == POISON), "mode {mode}: past 45");
    }
}

/// `vq_extnd(0)` / `vq_extnd(1)` themselves are untouched, and `INTIN[0] == 2`
/// **without** the sub-opcode is still `vq_extnd` mode 1 (E7 needs both).
#[test]
fn vq_extnd_modes_are_unchanged() {
    let _g = lock();
    let wk = harness::workstation();

    // Mode 0: DEV_TAB / SIZ_TAB, the v_opnvwk tables.
    let mut m0 = SPb::new();
    assert_eq!(m0.op102(0, 0, wk.handle), 1);
    assert_eq!(m0.intout[0], harness::SCREEN_W as i16 - 1, "DEV_TAB[0]");
    assert_eq!(m0.intout[1], harness::SCREEN_H as i16 - 1, "DEV_TAB[1]");
    assert_eq!(m0.intout[13], harness::NUM_COLORS, "DEV_TAB[13]");
    assert_eq!(m0.ptsout[..12], wk.siz_tab[..], "SIZ_TAB");
    assert_eq!((m0.contrl[2], m0.contrl[4]), (6, 45));
    assert!(m0.intout[45..].iter().all(|&w| w == POISON));

    // Mode 1: INQ_TAB + clip rectangle (vdi_control.c:76-123, 210-213).
    let mut m1 = SPb::new();
    assert_eq!(m1.op102(0, 1, wk.handle), 1);
    assert_eq!(m1.intout[0], 4, "INQ_TAB[0]");
    assert_eq!(m1.intout[4], harness::PLANES, "INQ_TAB[4] = v_planes");
    assert_eq!(m1.intout[5], 0, "INQ_TAB[5]: truecolor, no lookup table");
    assert_eq!(m1.intout[14], 1024, "INQ_TAB[14] MAX_VERTICES");
    assert_eq!((m1.contrl[2], m1.contrl[4]), (6, 45));
    assert!(m1.intout[45..].iter().all(|&w| w == POISON));

    // INTIN[0] == 2 with CONTRL[5] == 0 → mode 1, word for word.
    let mut m2 = SPb::new();
    assert_eq!(m2.op102(0, 2, wk.handle), 1);
    assert_eq!(m2.intout[..], m1.intout[..]);
    assert_eq!(m2.ptsout[..], m1.ptsout[..]);
}

/// E6: an invalid handle gets what `vq_extnd` gets — nothing (the dispatcher's
/// handle check, EmuTOS `vdi_main.c:139-148`).
#[test]
fn vq_scrninfo_on_an_unopened_handle_writes_nothing() {
    let _g = lock();
    harness::workstation();
    let mut pb = SPb::new();
    let ret = pb.op102(1, 2, 0x7ff0);
    assert_eq!(ret, 0);
    assert!(pb.intout.iter().all(|&w| w == POISON), "INTOUT written");
    assert!(pb.ptsout.iter().all(|&w| w == POISON), "PTSOUT written");
}

static JAR: Mutex<()> = Mutex::new(());

/// The `EdDI` cookie is seeded at boot, with the E2 marker: present,
/// non-zero, odd — never a callable address, so no in-tree consumer
/// (`tosdump/core/scrninfo.c` skips zero/odd) calls into it.
#[test]
fn the_eddi_cookie_is_seeded_with_the_non_callable_marker() {
    let _g = JAR.lock().unwrap_or_else(|e| e.into_inner());
    tos_api::cookie::seed_defaults(0x0000_0100);

    assert_eq!(tos_api::cookie::EDDI, 0x4564_4449, "'EdDI', mintlib C_EdDI");
    let v = tos_api::cookie::get(tos_api::cookie::EDDI).expect("EdDI is seeded at boot");
    assert_eq!(v, tos_api::cookie::EDDI_MARKER);
    assert_eq!(v, 0x0000_0001);
    assert!(v != 0 && v & 1 == 1, "non-zero and odd: present, not callable");
}
