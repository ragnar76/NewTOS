//! K1 (zView M4) — the AES owns a real, open physical workstation, and
//! `graf_handle` reports it.
//!
//! EmuTOS (pinned tree `1a7484aba1bfba23e2c0899e243b6d8177fe41df`): at the
//! start of every AES session `run_accs_and_desktop` calls `gsx_init`
//! (`aes/geminit.c:675`), whose `gsx_wsopen` opens the physical workstation
//! with `v_opnwk(intin, &gl_handle, &gl_ws)` (`aes/gemgsxif.c:169-186`) and
//! whose `gsx_start` sets it up (`aes/gemgraf.c:383-428`); `graf_handle`
//! returns `gl_handle` (`aes/gemsuper.c:287-293`); the session ends with
//! `gsx_wsclose` → `v_clswk` (`geminit.c:749`, `gemgsxif.c:191-194`), which
//! frees every virtual workstation and keeps the physical one
//! (`vdi/vdi_control.c:573-590`). The first `v_opnvwk` scans from
//! `VDI_PHYS_HANDLE + 1` (`vdi_control.c:420`).
//!
//! The defect this pins: `graf_handle` answered 1 while nothing had ever
//! opened handle 1, so `vq_extnd(graf_handle, …)` wrote nothing and WinDom's
//! `ApplInit` (`appl_init.c:42-46`) read `app.nplanes = app.color = 0`
//! (`measurements/measurement-2026-10-06-zview-m4/checks/k1-evidence.txt`).
//! Spec: `docs/api/aes/window-graphics.md` § `graf_handle`.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{host_only, Pb, AES_LOCK, POISON, SCREEN_COLORS, SCREEN_H, SCREEN_PLANES, SCREEN_W};
use tos_api::bdos::process::Abi;

const OP_APPL_INIT: i16 = 10;
const OP_GRAF_HANDLE: i16 = 77;

const V_OPNVWK: i16 = 100;
const VQ_EXTND: i16 = 102;
const VQL_ATTRIBUTES: i16 = 35;
const VQT_ATTRIBUTES: i16 = 38;

fn lock() -> std::sync::MutexGuard<'static, ()> {
    AES_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// A VDI parameter block (kernel space, LP64), every array poisoned before
/// each call so an unanswered call is visible as untouched poison.
struct Vdi {
    contrl: Box<[i16; 16]>,
    intin: Box<[i16; 16]>,
    ptsin: Box<[i16; 16]>,
    intout: Box<[i16; 64]>,
    ptsout: Box<[i16; 16]>,
    block: Box<[u64; 5]>,
}

impl Vdi {
    fn new() -> Vdi {
        let mut v = Vdi {
            contrl: Box::new([0; 16]),
            intin: Box::new([0; 16]),
            ptsin: Box::new([0; 16]),
            intout: Box::new([0; 64]),
            ptsout: Box::new([0; 16]),
            block: Box::new([0; 5]),
        };
        v.block[0] = v.contrl.as_ptr() as u64;
        v.block[1] = v.intin.as_ptr() as u64;
        v.block[2] = v.ptsin.as_ptr() as u64;
        v.block[3] = v.intout.as_ptr() as u64;
        v.block[4] = v.ptsout.as_ptr() as u64;
        v
    }

    /// Opcode `op` on `handle` with `intin` as the INTIN words.
    fn call(&mut self, op: i16, handle: i16, intin: &[i16]) {
        self.contrl.fill(POISON);
        self.intout.fill(POISON);
        self.ptsout.fill(POISON);
        self.contrl[0] = op;
        self.contrl[1] = 0;
        self.contrl[3] = intin.len() as i16;
        self.contrl[5] = 0;
        self.contrl[6] = handle;
        self.intin[..intin.len()].copy_from_slice(intin);
        // SAFETY: the five arrays outlive the call and hold every count the
        // VDI writes for these opcodes (≤ 45 INTOUT, 12 PTSOUT words).
        unsafe { tos_api::vdi::dispatch(self.block.as_ptr() as u64, Abi::Lp64) };
    }

    /// `v_opnvwk(work_in = 1 × 10, 2)` on `handle`; the new handle.
    fn v_opnvwk(&mut self, handle: i16) -> i16 {
        self.call(V_OPNVWK, handle, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2]);
        self.contrl[6]
    }

    /// `vq_extnd(handle, mode)`.
    fn vq_extnd(&mut self, handle: i16, mode: i16) {
        self.call(VQ_EXTND, handle, &[mode]);
    }
}

/// A fresh AES session on the harness host (640×480, 4 planes): close the
/// previous one as `geminit.c:749` does, open the next as `:675` does.
fn new_session() {
    host_only();
    tos_api::aes::gsxif::gsx_wsclose();
    tos_api::aes::gsxif::gsx_init();
}

/// `graf_handle()` → `int_out[0]`.
fn graf_handle() -> i16 {
    let mut pb = Pb::new();
    pb.poison();
    pb.call(OP_GRAF_HANDLE, 0, 5, 0, 0);
    pb.intout[0]
}

/// WinDom's `ApplInit` (`appl_init.c:42-46`), call for call: `appl_init`,
/// `graf_handle`, then `vq_extnd(aeshdl, 1)` → `work_out[4]` = planes and
/// `vq_extnd(aeshdl, 0)` → `work_out[13]` = colours. Both answer, and the
/// answer is the screen's.
#[test]
fn graf_handle_names_an_open_workstation_that_answers_vq_extnd() {
    let _g = lock();
    new_session();

    let mut pb = Pb::new();
    pb.poison();
    pb.call(OP_APPL_INIT, 0, 1, 0, 0);
    assert!(pb.intout[0] >= 0, "appl_init failed ({})", pb.intout[0]);

    let aeshdl = graf_handle();
    assert_eq!(aeshdl, 1, "gl_handle is VDI_PHYS_HANDLE (vdi_control.c:544)");

    let mut v = Vdi::new();
    v.vq_extnd(aeshdl, 1);
    assert_eq!(v.contrl[4], 45, "vq_extnd(aeshdl, 1) must answer 45 INTOUT words");
    assert_eq!(v.intout[4], SCREEN_PLANES as i16, "INQ_TAB[4] = v_planes (app.nplanes)");

    v.vq_extnd(aeshdl, 0);
    assert_eq!(v.contrl[4], 45, "vq_extnd(aeshdl, 0) must answer 45 INTOUT words");
    assert_eq!(v.intout[13], SCREEN_COLORS as i16, "DEV_TAB[13] = colours (app.color)");
    assert_eq!(v.intout[0], SCREEN_W as i16 - 1, "DEV_TAB[0] = xres");
    assert_eq!(v.intout[1], SCREEN_H as i16 - 1, "DEV_TAB[1] = yres");
}

/// `gsx_start`'s set-up of the AES's workstation (`gemgraf.c:405-422`):
/// `vsl_type(7)`, `vsl_width(1)`, and the text height restored to the
/// current one after the `ws_chminh` excursion — i.e. the height a freshly
/// opened workstation has.
#[test]
fn gsx_start_leaves_the_aes_line_and_text_setup_on_its_workstation() {
    let _g = lock();
    new_session();
    let h = graf_handle();

    let mut v = Vdi::new();
    v.call(VQL_ATTRIBUTES, h, &[]);
    assert_eq!(v.intout[0], 7, "vsl_type(7) (gemgraf.c:420)");
    assert_eq!(v.ptsout[0], 1, "vsl_width(1) (gemgraf.c:421)");

    v.call(VQT_ATTRIBUTES, h, &[]);
    let aes_height = (v.ptsout[0], v.ptsout[1], v.ptsout[2], v.ptsout[3]);
    let fresh = v.v_opnvwk(h);
    assert!(fresh > 0);
    v.call(VQT_ATTRIBUTES, fresh, &[]);
    let fresh_height = (v.ptsout[0], v.ptsout[1], v.ptsout[2], v.ptsout[3]);
    assert_eq!(aes_height, fresh_height, "vst_height(ws_chmaxh) restores the current height (:409-410)");
}

/// Handle numbering: the first `v_opnvwk` of a session is 2, the next 3;
/// the session end's `v_clswk` frees them and keeps the physical one, and
/// the next session hands out 2 again.
#[test]
fn the_first_v_opnvwk_of_a_session_is_handle_2_and_the_session_end_frees_it() {
    let _g = lock();
    new_session();
    let h = graf_handle();

    let mut v = Vdi::new();
    assert_eq!(v.v_opnvwk(h), 2, "the free-slot scan starts at VDI_PHYS_HANDLE + 1 (vdi_control.c:420)");
    assert_eq!(v.v_opnvwk(h), 3);

    tos_api::aes::gsxif::gsx_wsclose();
    v.vq_extnd(2, 1);
    assert_eq!(v.intout[4], POISON, "v_clswk closed virtual handle 2 (vdi_control.c:579-584)");
    v.vq_extnd(h, 1);
    assert_eq!(v.intout[4], SCREEN_PLANES as i16, "v_clswk keeps the physical workstation");

    tos_api::aes::gsxif::gsx_init();
    assert_eq!(graf_handle(), 1);
    assert_eq!(v.v_opnvwk(h), 2, "the next session starts numbering at 2 again");
}
