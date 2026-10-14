//! AES trap binding (`svc #2`, selector 200) — the calls a minimal GEM
//! window application makes.
//!
//! ABI (`docs/sdk/05-abi-pitfalls.md` §A1; `sdk/gemlib/overlay/_gc_aes.c`):
//! `x0` = 200, `x1` = the AES parameter block — six consecutive native
//! (64-bit) pointers: `control`, `global`, `intin`, `intout`, `addrin`,
//! `addrout`. Results come back in the arrays; the trap leaves `x0 = 0`.
//! `addrin`/`addrout` entries are native pointers on LP64.
//!
//! `global[]` keeps the m68k widths on every ABI (30 bytes,
//! `docs/api/aes/application.md` §`AESGLOBAL`, NewTOS layout decision
//! 2026-09-13); it is one per application and lives in [`GLOBAL`].
//!
//! The binding pattern (`control[0..4]` = opcode, `nintin`, `nintout`,
//! `naddrin`; `nintout` is what the dispatcher copies back,
//! `aes/gemsuper.c:443-444`) is the kernel desktop's `Aes::call`
//! (`kernel/src/desk/mod.rs`); each wrapper uses EmuTOS's own
//! `AES_CTRL_CODE` counts (`desk/gembind.c`, `include/aesdefs.h`).

use core::cell::UnsafeCell;

// ── opcodes (`include/aesdefs.h`) ────────────────────────────────────────
const APPL_INIT: i16 = 10;
const APPL_EXIT: i16 = 19;
const EVNT_MULTI: i16 = 25;
const FORM_ALERT: i16 = 52;
const GRAF_HANDLE: i16 = 77;
const GRAF_MOUSE: i16 = 78;
const WIND_CREATE: i16 = 100;
const WIND_OPEN: i16 = 101;
const WIND_CLOSE: i16 = 102;
const WIND_DELETE: i16 = 103;
const WIND_GET: i16 = 104;
const WIND_SET: i16 = 105;
const WIND_UPDATE: i16 = 107;
const WIND_CALC: i16 = 108;

// ── constants and `GRECT` ──────────────────────────────────────────────────
//
// The one public set lives in `tos-types` (`tos_types::aes`, DESK43 U5b),
// shared with the AES server and the desktop; re-exported here so a program
// keeps writing `aes::WM_REDRAW`. Widths are the fields': the `MU_*` mask and
// the window `kind` bits are `u16`, messages and `wind_get` fields `i16`.
pub use tos_types::aes::{
    Grect, ARROW, BEG_UPDATE, CLOSER, END_UPDATE, MOVER, MU_BUTTON, MU_KEYBD, MU_MESAG, MU_TIMER, M_OFF, M_ON,
    NAME, WC_BORDER, WC_WORK, WF_CXYWH, WF_FIRSTXYWH, WF_NAME, WF_NEXTXYWH, WF_TOP, WF_WXYWH, WM_CLOSED, WM_MOVED,
    WM_REDRAW, WM_TOPPED,
};

/// The application's `global[]` array (`AESGLOBAL`, 15 WORDs used).
struct Global(UnsafeCell<[i16; 16]>);
// SAFETY: a NewTOS program is single-threaded; the array is only touched
// by the AES through the trap, while this program's only thread waits in it.
unsafe impl Sync for Global {}
static GLOBAL: Global = Global(UnsafeCell::new([0; 16]));

/// One AES parameter-block call frame.
struct Aes {
    control: [i16; 8],
    intin: [i16; 16],
    intout: [i16; 16],
    addrin: [u64; 4],
    addrout: [u64; 4],
}

impl Aes {
    const fn new() -> Self {
        Self { control: [0; 8], intin: [0; 16], intout: [0; 16], addrin: [0; 4], addrout: [0; 4] }
    }

    /// Lay `control[0..4]` and trap.
    fn call(&mut self, op: i16, n_int: i16, n_out: i16, n_addr: i16) {
        self.control = [0; 8];
        self.control[0] = op;
        self.control[1] = n_int;
        self.control[2] = n_out;
        self.control[3] = n_addr;
        let pb: [u64; 6] = [
            self.control.as_mut_ptr() as u64,
            GLOBAL.0.get() as u64,
            self.intin.as_mut_ptr() as u64,
            self.intout.as_mut_ptr() as u64,
            self.addrin.as_mut_ptr() as u64,
            self.addrout.as_mut_ptr() as u64,
        ];
        // SAFETY: GEM trap, selector 200 = AES. The handler reads the block
        // and the arrays it names and writes the output arrays (and
        // `global[]`); every register but `x0` is preserved.
        unsafe {
            core::arch::asm!(
                "svc #2",
                inout("x0") 200u64 => _,
                in("x1") pb.as_ptr() as u64,
                options(nostack),
            );
        }
    }
}

/// `global[2]` (`AP_ID`) as `appl_init` filled it.
pub fn ap_id() -> i16 {
    // SAFETY: single-threaded; read after the AES wrote it.
    unsafe { (*GLOBAL.0.get())[2] }
}

/// `appl_init()` (10): register with the AES; the application id or −1.
pub fn appl_init() -> i16 {
    let mut a = Aes::new();
    a.call(APPL_INIT, 0, 1, 0);
    a.intout[0]
}

/// `appl_exit()` (19).
pub fn appl_exit() -> i16 {
    let mut a = Aes::new();
    a.call(APPL_EXIT, 0, 1, 0);
    a.intout[0]
}

/// The outputs of [`evnt_multi`] (`int_out[0..7]`, `docs/api/aes/event.md`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Events {
    /// `RET_CODE`: the `MU_*` bits that fired.
    pub which: u16,
    pub mx: i16,
    pub my: i16,
    pub mbutton: i16,
    pub kstate: i16,
    /// `EV_KRET`: scancode in the high byte, ASCII in the low byte.
    pub key: i16,
    /// `EV_BRET`: the click count.
    pub clicks: i16,
}

/// `evnt_multi` (25) with the button and timer parameters given and both
/// mouse rectangles unused; `msg` receives a message on `MU_MESAG`.
pub fn evnt_multi(
    flags: u16,
    bclicks: i16,
    bmask: i16,
    bstate: i16,
    timer_ms: u32,
    msg: &mut [i16; 8],
) -> Events {
    let mut a = Aes::new();
    a.intin[0] = flags as i16;
    a.intin[1] = bclicks;
    a.intin[2] = bmask;
    a.intin[3] = bstate;
    a.intin[14] = (timer_ms & 0xffff) as u16 as i16; // MT_LOCOUNT
    a.intin[15] = (timer_ms >> 16) as u16 as i16; // MT_HICOUNT
    a.addrin[0] = msg.as_mut_ptr() as u64;
    a.call(EVNT_MULTI, 16, 7, 1);
    Events {
        which: a.intout[0] as u16,
        mx: a.intout[1],
        my: a.intout[2],
        mbutton: a.intout[3],
        kstate: a.intout[4],
        key: a.intout[5],
        clicks: a.intout[6],
    }
}

/// `form_alert(default, string)` (52): the 1-based button chosen.
pub fn form_alert(default: i16, text: &core::ffi::CStr) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = default;
    a.addrin[0] = text.as_ptr() as u64;
    a.call(FORM_ALERT, 1, 1, 1);
    a.intout[0]
}

/// `graf_handle()` (77): the AES's physical workstation handle and the
/// system font's character cell (`gl_wchar`, `gl_hchar`) and box size.
#[derive(Clone, Copy, Debug, Default)]
pub struct GrafHandle {
    pub handle: i16,
    pub wchar: i16,
    pub hchar: i16,
    pub wbox: i16,
    pub hbox: i16,
}

pub fn graf_handle() -> GrafHandle {
    let mut a = Aes::new();
    a.call(GRAF_HANDLE, 0, 5, 0);
    GrafHandle {
        handle: a.intout[0],
        wchar: a.intout[1],
        hchar: a.intout[2],
        wbox: a.intout[3],
        hbox: a.intout[4],
    }
}

/// `graf_mouse(number, NULL)` (78).
pub fn graf_mouse(number: i16) {
    let mut a = Aes::new();
    a.intin[0] = number;
    a.call(GRAF_MOUSE, 1, 1, 1);
}

/// `wind_create(kind, max)` (100): the new window's handle, or a negative
/// value when none is free.
pub fn wind_create(kind: i16, max: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = kind;
    a.intin[1] = max.x;
    a.intin[2] = max.y;
    a.intin[3] = max.w;
    a.intin[4] = max.h;
    a.call(WIND_CREATE, 5, 1, 0);
    a.intout[0]
}

/// `wind_open(handle, r)` (101).
pub fn wind_open(handle: i16, r: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = r.x;
    a.intin[2] = r.y;
    a.intin[3] = r.w;
    a.intin[4] = r.h;
    a.call(WIND_OPEN, 5, 1, 0);
    a.intout[0]
}

/// `wind_close(handle)` (102).
pub fn wind_close(handle: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.call(WIND_CLOSE, 1, 1, 0);
    a.intout[0]
}

/// `wind_delete(handle)` (103).
pub fn wind_delete(handle: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.call(WIND_DELETE, 1, 1, 0);
    a.intout[0]
}

/// `wind_get(handle, field)` (104) for a rectangle field
/// (`WF_WXYWH`, `WF_CXYWH`, `WF_FIRSTXYWH`, `WF_NEXTXYWH`).
pub fn wind_get_grect(handle: i16, field: i16) -> Grect {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.call(WIND_GET, 2, 5, 0);
    Grect { x: a.intout[1], y: a.intout[2], w: a.intout[3], h: a.intout[4] }
}

/// `wind_set(handle, field, r)` (105) for a rectangle field
/// (`WF_CXYWH`).
pub fn wind_set_grect(handle: i16, field: i16, r: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.intin[2] = r.x;
    a.intin[3] = r.y;
    a.intin[4] = r.w;
    a.intin[5] = r.h;
    a.call(WIND_SET, 6, 1, 0);
    a.intout[0]
}

/// `wind_set(handle, WF_TOP)` (105).
pub fn wind_set_top(handle: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = WF_TOP;
    a.call(WIND_SET, 6, 1, 0);
    a.intout[0]
}

/// `wind_set(handle, WF_NAME, text)` (105). The pointer travels in
/// `addrin[0]` (the native packing the AES resolves next to the classic
/// `intin[2..4]` form, as the kernel desktop's `wind_set_ptr` passes it).
/// `text` must stay valid as long as the window shows it: the AES keeps the
/// pointer, not a copy (EmuTOS `w_strchg`, `aes/gemwmlib.c`).
pub fn wind_set_name(handle: i16, text: &'static core::ffi::CStr) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = WF_NAME;
    a.addrin[0] = text.as_ptr() as u64;
    a.call(WIND_SET, 2, 1, 1);
    a.intout[0]
}

/// `wind_update(mode)` (107).
pub fn wind_update(mode: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = mode;
    a.call(WIND_UPDATE, 1, 1, 0);
    a.intout[0]
}

/// `wind_calc(type, kind, r)` (108): the border rectangle around work area
/// `r` (`WC_BORDER`) or the work area inside border `r` (`WC_WORK`).
pub fn wind_calc(wtype: i16, kind: i16, r: Grect) -> Grect {
    let mut a = Aes::new();
    a.intin[0] = wtype;
    a.intin[1] = kind;
    a.intin[2] = r.x;
    a.intin[3] = r.y;
    a.intin[4] = r.w;
    a.intin[5] = r.h;
    a.call(WIND_CALC, 6, 5, 0);
    Grect { x: a.intout[1], y: a.intout[2], w: a.intout[3], h: a.intout[4] }
}
