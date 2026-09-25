//! GUI04 — `vex_wheelv` (VDI 134) and `vdi_mousex_handler`, the VDI half of
//! `CONF_WITH_EXTENDED_MOUSE`.
//!
//! Two things are pinned here:
//!
//! 1. **The exchange-vector pointer slots at the caller's ABI.** gemlib packs
//!    VDI pointer `n` at `CONTRL[7 + n * N_PTRINTS]` with
//!    `N_PTRINTS = sizeof(void *) / sizeof(short)`
//!    (`sdk/gemlib/gemlib-0.44.0/gem_vdiP.h:177,248`): 2 WORDs on m68k — whence
//!    EmuTOS' fixed `CONTRL[9]` for the outgoing pointer
//!    (`vdi/vdi_mouse.c:438-441`) — and **4** for an LP64 caller, which is what
//!    every NewTOS application is. `vex_wheelv(handle, new, &old)` therefore
//!    reads `new` at word 7 and writes `old` at word **11**; before GUI04 the
//!    old pointer went to word 9 and the application read back whatever was
//!    left in its `vdi_control[11..15]`.
//!
//! 2. **`vdi_mousex_handler`'s extra mouse buttons** (`vdi/vdi_mouse.c:525-
//!    545`): scancodes `0x37`/`0x5e`/`0x5f` set bits 2/3/4 of `MOUSE_BT`, their
//!    release codes clear them, and an ordinary relative-mouse packet must
//!    leave them standing — `mouse_int`'s "keep additional mouse button states"
//!    (`vdi/vdi_asm.S:270-277`).
//!
//! The wheel's own path (`0x59`/`0x5a` → `call_user_wheel`) is measured on the
//! AES side, where it becomes observable: `tests/aes_wheel_change.rs`.
//!
//! **No ILP32 case here.** `Vdipb::from_raw` reads the five *block* pointers at
//! the caller's width too, so an ILP32 call needs a parameter block of 4-byte
//! pointers — which a 64-bit host process cannot build, its heap being far
//! above 4 GiB. The ILP32 arm of the packing is the one EmuTOS itself uses
//! (`CONTRL[9]`) and is exercised on the target by every ported ILP32 app.

#[path = "vdi_reference/harness.rs"]
mod harness;

use tos_api::bdos::process::Abi;
use tos_api::bios::vectors::mousexvec;

/// `vex_wheelv`'s opcode (`vdi/vdi_main.c:110`).
const VEX_WHEELV: i16 = 134;
/// `vq_mouse`'s opcode — how the button mask is read back.
const VQ_MOUSE: i16 = 124;

/// A parameter block with room for gemlib's whole `vdi_control[VDI_CNTRLMAX]`
/// (15 WORDs, `mt_gem.h:3546`) — the shared reference harness' block stops at
/// 12, two WORDs short of an LP64 pointer slot 1.
struct WPb {
    contrl: Box<[i16; 16]>,
    intin: Box<[i16; 16]>,
    ptsin: Box<[i16; 16]>,
    intout: Box<[i16; 16]>,
    ptsout: Box<[i16; 16]>,
    block: Box<[u64; 5]>,
}

impl WPb {
    fn new() -> WPb {
        let mut pb = WPb {
            contrl: Box::new([0; 16]),
            intin: Box::new([0; 16]),
            ptsin: Box::new([0; 16]),
            intout: Box::new([0; 16]),
            ptsout: Box::new([0; 16]),
            block: Box::new([0; 5]),
        };
        pb.block[0] = pb.contrl.as_ptr() as u64;
        pb.block[1] = pb.intin.as_ptr() as u64;
        pb.block[2] = pb.ptsin.as_ptr() as u64;
        pb.block[3] = pb.intout.as_ptr() as u64;
        pb.block[4] = pb.ptsout.as_ptr() as u64;
        pb
    }

    fn call(&mut self, abi: Abi) {
        // SAFETY: the five arrays outlive the call and are large enough for
        // every opcode used here.
        unsafe {
            tos_api::vdi::dispatch(self.block.as_ptr() as u64, abi);
        }
    }

    /// Read VDI pointer slot `n` the way gemlib does, at `abi`'s width.
    fn ptr_slot(&self, n: usize, abi: Abi) -> u64 {
        let nptrints = match abi {
            Abi::Lp64 => 4,
            Abi::Ilp32 => 2,
        };
        let base = self.contrl[7 + n * nptrints..].as_ptr() as *const u8;
        // SAFETY: `7 + (n+1)*N_PTRINTS <= 15 < 16`; the slot is WORD-aligned only.
        unsafe {
            match abi {
                Abi::Lp64 => (base as *const u64).read_unaligned(),
                Abi::Ilp32 => (base as *const u32).read_unaligned() as u64,
            }
        }
    }

    /// Write VDI pointer slot `n` the way gemlib does, at `abi`'s width.
    fn set_ptr_slot(&mut self, n: usize, abi: Abi, v: u64) {
        let nptrints = match abi {
            Abi::Lp64 => 4,
            Abi::Ilp32 => 2,
        };
        let base = self.contrl[7 + n * nptrints..].as_mut_ptr() as *mut u8;
        // SAFETY: as in `ptr_slot`.
        unsafe {
            match abi {
                Abi::Lp64 => (base as *mut u64).write_unaligned(v),
                Abi::Ilp32 => (base as *mut u32).write_unaligned(v as u32),
            }
        }
    }

    /// Issue `vex_wheelv(handle, new, &old)` at `abi` and answer the `old`
    /// gemlib would read back.
    fn vex_wheelv(&mut self, handle: i16, new: u64, abi: Abi) -> u64 {
        self.contrl.fill(harness::POISON);
        self.contrl[0] = VEX_WHEELV;
        self.contrl[1] = 0;
        self.contrl[3] = 0;
        self.contrl[5] = 0;
        self.contrl[6] = handle;
        self.set_ptr_slot(0, abi, new);
        self.call(abi);
        self.ptr_slot(1, abi)
    }
}

/// The current `MOUSE_BT`, read the way an application reads it.
fn mouse_buttons(handle: i16) -> i16 {
    let mut pb = WPb::new();
    pb.contrl.fill(0);
    pb.contrl[0] = VQ_MOUSE;
    pb.contrl[6] = handle;
    pb.call(Abi::Lp64);
    pb.intout[0]
}

#[test]
fn vex_wheelv_returns_the_previous_vector_in_the_lp64_pointer_slot() {
    let _guard = harness::VDI_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let wk = harness::workstation();
    let mut pb = WPb::new();

    // Two distinct, plausible routine addresses. Their values never matter to
    // the VDI — `vdi_vex_wheelv` only swaps them (`vdi_mouse.c:438-441`).
    const A: u64 = 0x0000_0000_0001_3e40;
    const B: u64 = 0x0000_0000_0002_7180;

    // Install A. Whatever came before is not this test's business (the AES
    // installs its own handler at boot); install A a second time so the value
    // the *next* call must hand back is known.
    pb.vex_wheelv(wk.handle, A, Abi::Lp64);
    let old = pb.vex_wheelv(wk.handle, B, Abi::Lp64);
    assert_eq!(old, A, "vex_wheelv must return the previously installed vector");

    // ...and it must land in pointer slot **1**, words 11..14 at LP64. EmuTOS'
    // m68k position (word 9) cannot be asserted directly here: at LP64 word 9
    // is the third WORD of slot **0**, the pointer just passed in. What the
    // `old == A` above already rules out is the bug it replaced — writing the
    // 8 bytes at word 9 would have overwritten the tail of the incoming
    // pointer and left words 13/14 at their poison, so slot 1 would have read
    // back `0xDEADDEAD....` rather than `A`. The direct check is that the four
    // WORDs of slot 1 are no longer poison, and that the WORDs beyond it are.
    for w in 11..15 {
        assert_ne!(pb.contrl[w], harness::POISON, "CONTRL[{w}] is part of pointer slot 1");
    }
    assert_eq!(pb.contrl[15], harness::POISON, "nothing beyond slot 1 may be written");

    // Put A back so the module is left as this test found it, one swap on.
    pb.vex_wheelv(wk.handle, A, Abi::Lp64);
}

#[test]
fn mousex_handler_keeps_buttons_3_to_5_in_mouse_bt() {
    let _guard = harness::VDI_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let wk = harness::workstation();
    let backend = tos_api::host::host().unwrap().raster().unwrap();

    // Start from a known state: a packet with no buttons down.
    tos_api::vdi::feed_relative(0, 0, 0, backend);
    assert_eq!(mouse_buttons(wk.handle) & 0x1c, 0, "no extended button held yet");

    // `vdi_mousex_handler`, press codes (`vdi/vdi_mouse.c:529-540`).
    mousexvec(0x37); // button 3
    assert_eq!(mouse_buttons(wk.handle) & 0x1c, 0x04);
    mousexvec(0x5e); // button 4
    assert_eq!(mouse_buttons(wk.handle) & 0x1c, 0x0c);
    mousexvec(0x5f); // button 5
    assert_eq!(mouse_buttons(wk.handle) & 0x1c, 0x1c);

    // "keep additional mouse button states": an ordinary packet — here with
    // the *left* button down — must not disturb bits 2..4
    // (`vdi/vdi_asm.S:270-277`).
    tos_api::vdi::feed_relative(1, 1, 0x01, backend);
    assert_eq!(
        mouse_buttons(wk.handle) & 0x1f,
        0x1d,
        "a relative packet must keep buttons 3-5 and set button 1"
    );

    // Release codes (`vdi_mouse.c:531-541`), and the left button up again.
    mousexvec(0xb7);
    mousexvec(0xde);
    mousexvec(0xdf);
    tos_api::vdi::feed_relative(0, 0, 0, backend);
    assert_eq!(mouse_buttons(wk.handle) & 0x1f, 0, "every button released");
}

#[test]
fn a_wheel_scancode_does_not_touch_the_button_mask() {
    let _guard = harness::VDI_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let wk = harness::workstation();
    let backend = tos_api::host::host().unwrap().raster().unwrap();
    tos_api::vdi::feed_relative(0, 0, 0, backend);

    // `vdi_mousex_handler` runs the button arm and the wheel arm separately;
    // the four wheel codes match no button case, so `MOUSE_BT` is unchanged
    // and `call_user_but` is not reached (`vdi/vdi_mouse.c:529-553`).
    for code in [0x59u8, 0x5a, 0x5c, 0x5d] {
        mousexvec(code);
        mousexvec(code | 0x80);
        assert_eq!(
            mouse_buttons(wk.handle),
            0,
            "wheel scancode {code:#04x} must not change MOUSE_BT"
        );
    }
}
