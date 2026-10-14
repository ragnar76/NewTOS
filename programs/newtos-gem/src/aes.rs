//! AES trap binding (`svc #2`, selector 200) — EmuTOS's `desk/gembind.c`.
//!
//! DESK43 U6b: this is the *aesbind* the kernel desktop used to carry in
//! `kernel/src/desk/mod.rs`, merged with the calls the U6a runtime had here.
//! The kernel desktop (still EL1) and every native program call the AES
//! through these functions; a binding is a parameter block and a trap, never
//! a Rust call into the server.
//!
//! ABI (`docs/sdk/05-abi-pitfalls.md` §A1; `sdk/gemlib/overlay/_gc_aes.c`):
//! `x0` = 200, `x1` = the AES parameter block — six consecutive native
//! (64-bit) pointers: `control`, `global`, `intin`, `intout`, `addrin`,
//! `addrout`. Results come back in the arrays; the trap leaves `x0 = 0`.
//! `addrin`/`addrout` entries are native pointers on LP64. The trap is the
//! same from EL1 and EL0; the dispatcher reads a kernel caller's arrays
//! directly and an EL0 caller's with its rights (`tos-api/src/aes/mod.rs`
//! `Aespb::from_raw`, ARCH03 stage 3a).
//!
//! **Opcodes** carry their EmuTOS names (`include/aesdefs.h`), once; the
//! constants the calls take (`WF_*`, `WM_*`, `MU_*`, `FMD_*`, …) and `GRECT`
//! are `tos_types::aes` (DESK43 U5b), re-exported here.
//!
//! **Counts.** [`Aes::call`] lays `control[0..4]` = opcode, `nintin`,
//! `nintout`, `naddrin` (`AES_CTRL_CODE`, `desk/gembind.c:46-50`). Each
//! binding passes the counts EmuTOS's own binding of that function uses
//! (cited per function); where NewTOS uses a different shape of the same call
//! (the native pointer packing of `wind_set`, `evnt_multi`'s full `intin`), the
//! binding says so.
//!
//! `global[]` keeps the m68k widths on every ABI (30 bytes,
//! `docs/api/aes/application.md` §`AESGLOBAL`, NewTOS layout decision
//! 2026-09-13); there is one per application, in [`GLOBAL`]. The AES writes
//! it only in `appl_init` and the `rsrc_*` calls (`tos-api/src/aes/appl.rs`,
//! `resource.rs`).

use core::cell::UnsafeCell;

// ── opcodes (`include/aesdefs.h:17-115`) ──────────────────────────────────
/// `appl_init` (`aesdefs.h:17`).
pub const APPL_INIT: i16 = 10;
/// `appl_exit` (`aesdefs.h:24`).
pub const APPL_EXIT: i16 = 19;
/// `evnt_button` (`aesdefs.h:28`).
pub const EVNT_BUTTON: i16 = 21;
/// `evnt_multi` (`aesdefs.h:32`).
pub const EVNT_MULTI: i16 = 25;
/// `evnt_dclick` (`aesdefs.h:33`).
pub const EVNT_DCLICK: i16 = 26;
/// `menu_bar` (`aesdefs.h:36`).
pub const MENU_BAR: i16 = 30;
/// `menu_icheck` (`aesdefs.h:37`).
pub const MENU_ICHECK: i16 = 31;
/// `menu_ienable` (`aesdefs.h:38`).
pub const MENU_IENABLE: i16 = 32;
/// `menu_tnormal` (`aesdefs.h:39`).
pub const MENU_TNORMAL: i16 = 33;
/// `objc_add` (`aesdefs.h:48`).
pub const OBJC_ADD: i16 = 40;
/// `objc_draw` (`aesdefs.h:50`).
pub const OBJC_DRAW: i16 = 42;
/// `objc_find` (`aesdefs.h:51`).
pub const OBJC_FIND: i16 = 43;
/// `objc_offset` (`aesdefs.h:52`).
pub const OBJC_OFFSET: i16 = 44;
/// `form_do` (`aesdefs.h:59`).
pub const FORM_DO: i16 = 50;
/// `form_dial` (`aesdefs.h:60`).
pub const FORM_DIAL: i16 = 51;
/// `form_alert` (`aesdefs.h:61`).
pub const FORM_ALERT: i16 = 52;
/// `form_center` (`aesdefs.h:63`).
pub const FORM_CENTER: i16 = 54;
/// `graf_rubberbox` (`aesdefs.h:68`).
pub const GRAF_RUBBOX: i16 = 70;
/// `graf_growbox` (`aesdefs.h:71`).
pub const GRAF_GROWBOX: i16 = 73;
/// `graf_handle` (`aesdefs.h:75`).
pub const GRAF_HANDLE: i16 = 77;
/// `graf_mouse` (`aesdefs.h:76`).
pub const GRAF_MOUSE: i16 = 78;
/// `graf_mkstate` (`aesdefs.h:77`).
pub const GRAF_MKSTATE: i16 = 79;
/// `fsel_exinput` (`aesdefs.h:86`).
pub const FSEL_EXINPUT: i16 = 91;
/// `wind_create` (`aesdefs.h:89`).
pub const WIND_CREATE: i16 = 100;
/// `wind_open` (`aesdefs.h:90`).
pub const WIND_OPEN: i16 = 101;
/// `wind_close` (`aesdefs.h:91`).
pub const WIND_CLOSE: i16 = 102;
/// `wind_delete` (`aesdefs.h:92`).
pub const WIND_DELETE: i16 = 103;
/// `wind_get` (`aesdefs.h:93`).
pub const WIND_GET: i16 = 104;
/// `wind_set` (`aesdefs.h:94`).
pub const WIND_SET: i16 = 105;
/// `wind_find` (`aesdefs.h:95`).
pub const WIND_FIND: i16 = 106;
/// `wind_update` (`aesdefs.h:96`).
pub const WIND_UPDATE: i16 = 107;
/// `wind_calc` (`aesdefs.h:97`).
pub const WIND_CALC: i16 = 108;
/// `wind_new` (`aesdefs.h:98`).
pub const WIND_NEW: i16 = 109;
/// `shel_write` (`aesdefs.h:109`).
pub const SHEL_WRITE: i16 = 121;
/// `shel_get` (`aesdefs.h:110`).
pub const SHEL_GET: i16 = 122;
/// `shel_put` (`aesdefs.h:111`).
pub const SHEL_PUT: i16 = 123;
/// `shel_wdef` (`aesdefs.h:115`, PC-GEM).
pub const SHEL_WDEF: i16 = 127;

// ── constants and `GRECT` ──────────────────────────────────────────────────
//
// The one public set lives in `tos-types` (`tos_types::aes`, DESK43 U5b),
// shared with the AES server and the desktop; re-exported here so a program
// keeps writing `aes::WM_REDRAW`. Widths are the fields': the `MU_*` mask and
// the window `kind` bits are `u16`, messages and `wind_get` fields `i16`.
pub use tos_types::aes::{
    Grect, ARROW, BEG_UPDATE, CLOSER, DESKWH, END_UPDATE, FMD_FINISH, FMD_START, MOVER, MU_BUTTON, MU_KEYBD,
    MU_M1, MU_MESAG, MU_TIMER, M_OFF, M_ON, NAME, WC_BORDER, WC_WORK, WF_CXYWH, WF_FIRSTXYWH, WF_NAME,
    WF_NEWDESK, WF_NEXTXYWH, WF_TOP, WF_WXYWH, WM_CLOSED, WM_MOVED, WM_REDRAW, WM_TOPPED,
};

/// The application's `global[]` array (`AESGLOBAL`, 15 WORDs used).
struct Global(UnsafeCell<[i16; 16]>);
// SAFETY: a NewTOS program is single-threaded, and so is the kernel desktop
// (one control flow, `kernel/src/desk/global.rs` `DeskStatic`); the array is
// only touched by the AES through the trap, while that one flow waits in it,
// and read by [`ap_id`] afterwards.
unsafe impl Sync for Global {}
static GLOBAL: Global = Global(UnsafeCell::new([0; 16]));

/// One AES parameter-block call frame. [`Aes::call`] zeroes `control`, sets
/// the opcode and the array lengths, and traps; results land in
/// `intout`/`addrout`.
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

    /// Trap opcode `op` with `n_int` int-ins, `n_out` int-outs and `n_addr`
    /// addr-ins — the four `control[]` words every AES binding lays
    /// (`AES_CTRL_CODE(op, nintin, nintout, naddrin)`, EmuTOS
    /// `desk/gembind.c:46-50`).
    ///
    /// `n_out` is `OUT_LEN`, and it is not decoration: the dispatcher copies
    /// **exactly** `control[2]` words of its result back
    /// (`aes/gemsuper.c:443-444`), so a call that under-declares gets nothing
    /// — with `n_out == 0` not even `RET_CODE`.
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
        // `global[]`); every register but `x0` is preserved. An exception
        // from EL1 (`SPSel = 0`) or EL0 runs the handler on `SP_EL1`, so the
        // caller's stack is untouched (`nostack`).
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

/// `global[2]` (`AP_ID`, `aesdefs.h:149`) as `appl_init` filled it.
// NEWTOS: a read of this crate's `global[]`, not an AES call.
pub fn ap_id() -> i16 {
    // SAFETY: single-threaded; read after the AES wrote it.
    unsafe { (*GLOBAL.0.get())[2] }
}

// ── application manager ────────────────────────────────────────────────────

/// `appl_init()` (10, `desk/gembind.c:111-119`,
/// `AES_CTRL_CODE(APPL_INIT, 0, 1, 0)`) — register with the AES; the
/// application id, or −1.
pub fn appl_init() -> i16 {
    let mut a = Aes::new();
    a.call(APPL_INIT, 0, 1, 0);
    a.intout[0]
}

/// `appl_exit()` (19, `desk/gembind.c:123-125`) — deregister from the AES,
/// closing the process descriptor and draining its message queue
/// (`gemaplib.c:251-261`).
pub fn appl_exit() -> i16 {
    let mut a = Aes::new();
    a.call(APPL_EXIT, 0, 1, 0);
    a.intout[0]
}

// ── event manager ──────────────────────────────────────────────────────────

/// `evnt_button(clicks, mask, state)` (21, `desk/gembind.c:188-194`,
/// `AES_CTRL_CODE(EVNT_BUTTON, 3, 5, 0)`) — wait until the button reaches
/// `state` (masked). Returns the click count (`RET_CODE`).
pub fn evnt_button(clicks: i16, mask: i16, state: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = clicks;
    a.intin[1] = mask;
    a.intin[2] = state;
    a.call(EVNT_BUTTON, 3, 5, 0);
    a.intout[0]
}

/// The inputs of [`evnt_multi`] (`int_in[0..16]`, `docs/api/aes/event.md`):
/// the `MU_*` mask, the button wait, the two mouse rectangles and the timer.
/// [`Default`] is "nothing": every field zero.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvntMulti {
    /// `MU_*` event mask (`int_in[0]`).
    pub flags: u16,
    /// Button: clicks to wait for (`int_in[1]`).
    pub bclicks: i16,
    /// Button: mask (`int_in[2]`).
    pub bmask: i16,
    /// Button: state (`int_in[3]`).
    pub bstate: i16,
    /// `MU_M1`: 0 = wait for entry, 1 = for exit (`int_in[4]`).
    pub m1flags: i16,
    /// `MU_M1` rectangle (`int_in[5..9]`).
    pub m1: Grect,
    /// `MU_M2`: 0 = entry, 1 = exit (`int_in[9]`).
    pub m2flags: i16,
    /// `MU_M2` rectangle (`int_in[10..14]`).
    pub m2: Grect,
    /// `MU_TIMER` interval in ms (`int_in[14]` low, `int_in[15]` high word).
    pub timer_ms: u32,
}

/// The outputs of [`evnt_multi`] (`int_out[0..7]`, `docs/api/aes/event.md`;
/// EmuTOS's out-parameters `pmx, pmy, pmb, pks, pkr, pbr`,
/// `desk/gembind.c:239-268`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Events {
    /// `RET_CODE`: the `MU_*` bits that fired.
    pub which: u16,
    /// `EV_MX`: mouse x.
    pub mx: i16,
    /// `EV_MY`: mouse y.
    pub my: i16,
    /// `EV_MB`: mouse button state.
    pub button: i16,
    /// `EV_KS`: keyboard shift state (`K_*` bits).
    pub kstate: i16,
    /// `EV_KRET`: scancode in the high byte, ASCII in the low byte.
    pub kret: i16,
    /// `EV_BRET`: the click count.
    pub bret: i16,
}

/// `evnt_multi` (25, `desk/gembind.c:239-268`) — block until any requested
/// event fires. `msg` receives a message on `MU_MESAG` (`addr_in[0]`); null
/// when the mask has no `MU_MESAG`.
///
/// All sixteen `int_in` words are declared (`control[1] = 16`): EmuTOS's own
/// binding passes `AES_CTRL_CODE(EVNT_MULTI, 0, 7, 1)` and hands the AES its
/// `int_in` array by reference (`gem_if0`), which a trap-based binding
/// cannot; declaring the full block is the trap form of the same call.
pub fn evnt_multi(p: &EvntMulti, msg: *mut i16) -> Events {
    let mut a = Aes::new();
    a.addrin[0] = msg as u64;
    a.intin[0] = p.flags as i16;
    a.intin[1] = p.bclicks;
    a.intin[2] = p.bmask;
    a.intin[3] = p.bstate;
    a.intin[4] = p.m1flags;
    a.intin[5] = p.m1.x;
    a.intin[6] = p.m1.y;
    a.intin[7] = p.m1.w;
    a.intin[8] = p.m1.h;
    a.intin[9] = p.m2flags;
    a.intin[10] = p.m2.x;
    a.intin[11] = p.m2.y;
    a.intin[12] = p.m2.w;
    a.intin[13] = p.m2.h;
    a.intin[14] = (p.timer_ms & 0xffff) as u16 as i16; // MT_LOCOUNT
    a.intin[15] = (p.timer_ms >> 16) as u16 as i16; // MT_HICOUNT
    a.call(EVNT_MULTI, 16, 7, 1);
    Events {
        which: a.intout[0] as u16,
        mx: a.intout[1],
        my: a.intout[2],
        button: a.intout[3],
        kstate: a.intout[4],
        kret: a.intout[5],
        bret: a.intout[6],
    }
}

/// `evnt_dclick(rate, setit)` (26, `desk/gembind.c:282-286`) — the AES's
/// double-click speed index (0 = slowest … 4 = fastest). With `setit == false`
/// `rate` is ignored and the current index is returned; with `setit == true`
/// the index is stored and returned.
pub fn evnt_dclick(rate: i16, setit: bool) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = rate;
    a.intin[1] = setit as i16;
    a.call(EVNT_DCLICK, 2, 1, 0);
    a.intout[0]
}

// ── menu manager ───────────────────────────────────────────────────────────

/// `menu_bar(tree, show)` (30, `desk/gembind.c:293-297`) — install
/// (`show = 1`) or remove the menu bar.
pub fn menu_bar(tree: u64, show: i16) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = show;
    a.call(MENU_BAR, 1, 1, 1);
    a.intout[0]
}

/// `menu_icheck(tree, item, checked)` (31, `desk/gembind.c:301-306`) —
/// set/clear the `CHECKED` `ob_state` bit on menu item `item`.
pub fn menu_icheck(tree: u64, item: i16, checked: bool) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = item;
    a.intin[1] = checked as i16;
    a.call(MENU_ICHECK, 2, 1, 1);
    a.intout[0]
}

/// `menu_ienable(tree, item, enable)` (32, `desk/gembind.c:310-315`) —
/// set/clear the `DISABLED` `ob_state` bit on menu item `item`. Bit 15 of the
/// item word is the AES's redraw-now flag (`aes/gemsuper.c:175-176`).
pub fn menu_ienable(tree: u64, item: i16, enable: bool) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = item;
    a.intin[1] = enable as i16;
    a.call(MENU_IENABLE, 2, 1, 1);
    a.intout[0]
}

/// `menu_tnormal(tree, title, normal)` (33, `desk/gembind.c:319-324`) — draw
/// `title` normal (`normal = 1`) or highlighted (`0`).
pub fn menu_tnormal(tree: u64, title: i16, normal: bool) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = title;
    a.intin[1] = normal as i16;
    a.call(MENU_TNORMAL, 2, 1, 1);
    a.intout[0]
}

// ── object manager ─────────────────────────────────────────────────────────

/// `objc_add(tree, parent, child)` (40, `desk/gembind.c:352-357`) — link
/// `child` as the last child of `parent`.
pub fn objc_add(tree: u64, parent: i16, child: i16) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = parent;
    a.intin[1] = child;
    a.call(OBJC_ADD, 2, 1, 1);
    a.intout[0]
}

/// `objc_draw(tree, start, depth, clip)` (42, `desk/gembind.c:371-381`) —
/// draw the subtree rooted at `start`, `depth` levels deep, clipped to `clip`.
pub fn objc_draw(tree: u64, start: i16, depth: i16, clip: Grect) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = start;
    a.intin[1] = depth;
    a.intin[2] = clip.x;
    a.intin[3] = clip.y;
    a.intin[4] = clip.w;
    a.intin[5] = clip.h;
    a.call(OBJC_DRAW, 6, 1, 1);
    a.intout[0]
}

/// `objc_find(tree, start, depth, mx, my)` (43, `desk/gembind.c:385-392`) —
/// the deepest object of `tree` under `(mx, my)` within `depth` levels of
/// `start`; `NIL` (−1) if none.
pub fn objc_find(tree: u64, start: i16, depth: i16, mx: i16, my: i16) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = start;
    a.intin[1] = depth;
    a.intin[2] = mx;
    a.intin[3] = my;
    a.call(OBJC_FIND, 4, 1, 1);
    a.intout[0]
}

/// `objc_offset(tree, obj, &x, &y)` (44, `desk/gembind.c:396-400`) — the
/// absolute screen position of `obj` (the sum of its ancestors' `ob_x`/`ob_y`).
pub fn objc_offset(tree: u64, obj: i16) -> (i16, i16) {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = obj;
    a.call(OBJC_OFFSET, 1, 3, 1);
    (a.intout[1], a.intout[2])
}

// ── form manager ───────────────────────────────────────────────────────────

/// `form_do(tree, start)` (50, `desk/gembind.c:452-456`) — run a modal dialog
/// until an `EXIT`/`TOUCHEXIT` object (or Enter on the default button) ends
/// it; the object index that did. `tree` must stay valid across the call.
pub fn form_do(tree: u64, start: i16) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = start;
    a.call(FORM_DO, 1, 1, 1);
    a.intout[0]
}

/// `form_dial(fmd, little, big)` (51, `desk/gembind.c:460-472`,
/// `AES_CTRL_CODE(FORM_DIAL, 9, 1, 0)`): `int_in[1..5]` the small rectangle,
/// `int_in[5..9]` the big one.
pub fn form_dial(fmd: i16, little: Grect, big: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = fmd;
    a.intin[1] = little.x;
    a.intin[2] = little.y;
    a.intin[3] = little.w;
    a.intin[4] = little.h;
    a.intin[5] = big.x;
    a.intin[6] = big.y;
    a.intin[7] = big.w;
    a.intin[8] = big.h;
    a.call(FORM_DIAL, 9, 1, 0);
    a.intout[0]
}

/// `form_alert(default, text)` (52, `desk/gembind.c:476-480`) — modal alert;
/// `text` is a NUL-terminated `[icon][message][buttons]` string that must stay
/// readable for the duration of the call. Returns the 1-based button clicked.
pub fn form_alert(default: i16, text: *const u8) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = text as u64;
    a.intin[0] = default;
    a.call(FORM_ALERT, 1, 1, 1);
    a.intout[0]
}

/// `form_center(tree, &x, &y, &w, &h)` (54, `desk/gembind.c:497-500`) — centre
/// `tree`'s ROOT on screen (the AES writes the new `ob_x`/`ob_y` into the ROOT
/// record) and return the tree's **outer** rectangle, i.e. the ROOT box grown
/// by any `OUTLINED` ring and `SHADOWED` drop shadow (EmuTOS `ob_center`,
/// `gemobed.c:67-106`).
pub fn form_center(tree: u64) -> Grect {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.call(FORM_CENTER, 0, 5, 1);
    Grect { x: a.intout[1], y: a.intout[2], w: a.intout[3], h: a.intout[4] }
}

// ── graphics manager ───────────────────────────────────────────────────────

/// `graf_rubberbox(x, y, minw, minh, &w, &h)` (70, EmuTOS `graf_rubbox`,
/// `desk/gembind.c:541-548`) — anchor a rubber-band box at `(x, y)`; the opposite corner
/// tracks the mouse until the button is released. Returns the final `(w, h)`.
pub fn graf_rubbox(x: i16, y: i16, minw: i16, minh: i16) -> (i16, i16) {
    let mut a = Aes::new();
    a.intin[0] = x;
    a.intin[1] = y;
    a.intin[2] = minw;
    a.intin[3] = minh;
    a.call(GRAF_RUBBOX, 4, 3, 0);
    (a.intout[1], a.intout[2])
}

/// `graf_growbox_grect(from, to)` (73, `desk/gembind.c:589-599`:
/// `GR_I1..GR_I8` = from x/y/w/h, to x/y/w/h,
/// `AES_CTRL_CODE(GRAF_GROWBOX, 8, 1, 0)`) — the zoom outline from the small
/// rectangle `from` out to the big rectangle `to`.
pub fn graf_growbox(from: Grect, to: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = from.x;
    a.intin[1] = from.y;
    a.intin[2] = from.w;
    a.intin[3] = from.h;
    a.intin[4] = to.x;
    a.intin[5] = to.y;
    a.intin[6] = to.w;
    a.intin[7] = to.h;
    a.call(GRAF_GROWBOX, 8, 1, 0);
    a.intout[0]
}

/// What [`graf_handle`] reports (`intout[0..5]`, `aes/gemsuper.c:287-293`).
#[derive(Clone, Copy, Debug, Default)]
pub struct GrafHandle {
    /// `gl_handle`: the AES's physical VDI workstation.
    pub handle: i16,
    /// `gl_wchar`: the system font's character width.
    pub wchar: i16,
    /// `gl_hchar`: the system font's character height.
    pub hchar: i16,
    /// `gl_wbox`: the character box width.
    pub wbox: i16,
    /// `gl_hbox`: the character box height.
    pub hbox: i16,
}

/// `graf_handle(&gl_wchar, &gl_hchar, &gl_wbox, &gl_hbox)` (77,
/// `desk/gembind.c:639-641`) — the AES's VDI workstation handle and the
/// system font's metrics.
pub fn graf_handle() -> GrafHandle {
    let mut a = Aes::new();
    a.call(GRAF_HANDLE, 0, 5, 0);
    GrafHandle { handle: a.intout[0], wchar: a.intout[1], hchar: a.intout[2], wbox: a.intout[3], hbox: a.intout[4] }
}

/// `graf_mouse(number, NULL)` (78, `desk/gembind.c:650-654`,
/// `AES_CTRL_CODE(GRAF_MOUSE, 1, 1, 1)`) — select a built-in mouse form, or
/// hide ([`M_OFF`]) / show ([`M_ON`]) the pointer. The form pointer
/// (`addr_in[0]`) is NULL: only `USER_DEF` (255) reads it
/// (`tos-api/src/aes/graf.rs` `graf_mouse`), and there is no binding for a
/// user-defined form here.
pub fn graf_mouse(number: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = number;
    a.call(GRAF_MOUSE, 1, 1, 1);
    a.intout[0]
}

/// The instantaneous mouse/keyboard state from [`graf_mkstate`].
#[derive(Clone, Copy, Debug, Default)]
pub struct MouseState {
    /// Mouse x.
    pub mx: i16,
    /// Mouse y.
    pub my: i16,
    /// Button mask (bit 0 = left).
    pub buttons: i16,
    /// Keyboard shift/ctrl/alt state (`K_*` bits).
    pub kstate: i16,
}

/// `graf_mkstate(&mx, &my, &mb, &ks)` (79, `desk/gembind.c:658-660`) — poll the
/// mouse position, button mask and keyboard shift state without waiting.
pub fn graf_mkstate() -> MouseState {
    let mut a = Aes::new();
    a.call(GRAF_MKSTATE, 0, 5, 0);
    MouseState { mx: a.intout[1], my: a.intout[2], buttons: a.intout[3], kstate: a.intout[4] }
}

// ── file selector ──────────────────────────────────────────────────────────

/// `fsel_exinput(pipath, pisel, &button, title)` (91, `desk/gembind.c:702-707`,
/// `AES_CTRL_CODE(FSEL_EXINPUT, 0, 2, 3)`) — the modal file selector with a
/// caller-supplied caption.
///
/// `path` carries the initial directory-plus-wildcard in and the chosen one
/// out; `sel` the initial file name in and the chosen one out; `title` is the
/// NUL-terminated caption. Returns `(rc, button)` — `RET_CODE` and
/// `FS_BUTTON`: `rc == 0` means the selector never ran, `button` is 1 for OK
/// and 0 for Cancel. Both buffers are written back by the AES and must be large
/// enough for what it returns: up to 96 path bytes plus the NUL and up to 16
/// name bytes plus the NUL (`tos-api/src/aes/fsel.rs:135-136`).
pub fn fsel_exinput(path: &mut [u8], sel: &mut [u8], title: &[u8]) -> (i16, i16) {
    let mut a = Aes::new();
    a.addrin[0] = path.as_mut_ptr() as u64;
    a.addrin[1] = sel.as_mut_ptr() as u64;
    a.addrin[2] = title.as_ptr() as u64;
    a.call(FSEL_EXINPUT, 0, 2, 3);
    (a.intout[0], a.intout[1])
}

// ── window manager ─────────────────────────────────────────────────────────

/// `wind_create(kind, max)` (100, EmuTOS `wind_create_grect`,
/// `desk/gembind.c:717-724`) — reserve a window
/// of border `kind` (the `NAME`/`CLOSER`/… bits) and maximum extent `max`;
/// the handle (≥ 1) or a negative value when none is free.
pub fn wind_create(kind: u16, max: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = kind as i16;
    a.intin[1] = max.x;
    a.intin[2] = max.y;
    a.intin[3] = max.w;
    a.intin[4] = max.h;
    a.call(WIND_CREATE, 5, 1, 0);
    a.intout[0]
}

/// `wind_open(handle, r)` (101, EmuTOS `wind_open_grect`,
/// `desk/gembind.c:728-735`) — display window
/// `handle` at `r`, topmost.
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

/// `wind_close(handle)` (102, `desk/gembind.c:739-742`) — hide window
/// `handle` (the handle stays valid).
pub fn wind_close(handle: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.call(WIND_CLOSE, 1, 1, 0);
    a.intout[0]
}

/// `wind_delete(handle)` (103, `desk/gembind.c:746-749`) — free the window
/// handle.
pub fn wind_delete(handle: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.call(WIND_DELETE, 1, 1, 0);
    a.intout[0]
}

/// `wind_get_grect(handle, field, &r)` (104, EmuTOS `wind_get`,
/// `desk/gembind.c:753-757`) for a
/// rectangle field (`WF_WXYWH`, `WF_CXYWH`, `WF_FIRSTXYWH`, `WF_NEXTXYWH`, …).
/// For `WF_FIRSTXYWH`/`WF_NEXTXYWH` a returned rectangle with `w == 0 ||
/// h == 0` ends the window's rectangle list.
pub fn wind_get_grect(handle: i16, field: i16) -> Grect {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.call(WIND_GET, 2, 5, 0);
    Grect { x: a.intout[1], y: a.intout[2], w: a.intout[3], h: a.intout[4] }
}

/// `wind_get(handle, field, &w1)` (104, `desk/gembind.c:753-757`) for a field
/// that answers one word in `int_out[1]` (`WF_TOP`: the top-most window's
/// handle).
pub fn wind_get_int(handle: i16, field: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.call(WIND_GET, 2, 5, 0);
    a.intout[1]
}

/// `wind_set(handle, field, r)` (105, `desk/gembind.c:772-785`,
/// `AES_CTRL_CODE(WIND_SET, 6, 1, 0)`) for a rectangle field (`WF_CXYWH`).
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

/// `wind_set(handle, field, value)` (105) for a scalar field (`WF_TOP` with
/// no value, the slider size/position fields). Declares the five words the
/// scalar form carries (`int_in[0..5]`); the AES reads none beyond
/// `int_in[2]` for these fields, and an undeclared word reaches it as 0, as
/// the zeroed caller array would deliver it (`tos-api/src/aes/mod.rs`
/// `Aespb::from_raw`).
pub fn wind_set_int(handle: i16, field: i16, value: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.intin[2] = value;
    a.call(WIND_SET, 5, 1, 0);
    a.intout[0]
}

/// `wind_set(handle, field, ptr)` (105) for a pointer field
/// (`WF_NAME`/`WF_INFO`): the native packing, the pointer in `addr_in[0]` with
/// `naddrin = 1`, which the AES tells from the classic `int_in[2..]` form by
/// `control[AIN_LEN]` (`tos-api/src/aes/mod.rs` `Aespb::ain_len`). The AES
/// keeps the pointer, not a copy (EmuTOS `w_strchg`, `aes/gemwmlib.c`): the
/// string must stay valid as long as the window shows it.
pub fn wind_set_ptr(handle: i16, field: i16, ptr: u64) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = ptr;
    a.intin[0] = handle;
    a.intin[1] = field;
    a.call(WIND_SET, 2, 1, 1);
    a.intout[0]
}

/// `wind_set(DESKWH, WF_NEWDESK, tree, root)` (105, EmuTOS `deskmain.c:2049`/
/// `:2163`) — register (`tree != 0`) or un-register (`tree == 0`) the
/// desktop's background object tree with the AES. The tree pointer travels in
/// `addr_in[0]`, the root in `int_in[2]` (the native packing; the AES resolves
/// it or the classic `int_in[2..4]` form, `tos-api/src/aes/window.rs`). `tree`
/// must stay valid for as long as it is registered.
pub fn wind_set_newdesk(tree: u64, root: i16) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree;
    a.intin[0] = DESKWH;
    a.intin[1] = WF_NEWDESK;
    a.intin[2] = root;
    a.call(WIND_SET, 3, 1, 1);
    a.intout[0]
}

/// `wind_find(mx, my)` (106, `desk/gembind.c:795-799`) — the handle of the
/// window under screen point `(mx, my)`; [`DESKWH`] for the desktop.
pub fn wind_find(mx: i16, my: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = mx;
    a.intin[1] = my;
    a.call(WIND_FIND, 2, 1, 0);
    a.intout[0]
}

/// `wind_update(mode)` (107, `desk/gembind.c:803-806`) — take
/// ([`BEG_UPDATE`]) or release ([`END_UPDATE`]) the AES window-tree lock.
pub fn wind_update(mode: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = mode;
    a.call(WIND_UPDATE, 1, 1, 0);
    a.intout[0]
}

/// `wind_calc(type, kind, r, &out)` (108, EmuTOS `wind_calc_grect`,
/// `desk/gembind.c:810-818`) — the
/// border rectangle around work area `r` ([`WC_BORDER`]) or the work area
/// inside border `r` ([`WC_WORK`]) of a window with border `kind`.
pub fn wind_calc(wtype: i16, kind: u16, r: Grect) -> Grect {
    let mut a = Aes::new();
    a.intin[0] = wtype;
    a.intin[1] = kind as i16;
    a.intin[2] = r.x;
    a.intin[3] = r.y;
    a.intin[4] = r.w;
    a.intin[5] = r.h;
    a.call(WIND_CALC, 6, 5, 0);
    Grect { x: a.intout[1], y: a.intout[2], w: a.intout[3], h: a.intout[4] }
}

/// `wind_new()` (109, `desk/gembind.c:828-830`) — close and delete **every**
/// window this application still has open and drop its `wind_update` locks
/// (AES side `wm_new`, `gemwmlib.c:1875-1893`).
pub fn wind_new() -> i16 {
    let mut a = Aes::new();
    a.call(WIND_NEW, 0, 1, 0);
    a.intout[0]
}

// ── shell manager ──────────────────────────────────────────────────────────

/// `shel_write(doex, isgr, iscr, pcmd, ptail)` (121, `desk/gembind.c:907-914`,
/// `AES_CTRL_CODE(SHEL_WRITE, 3, 1, 2)`) — tell the AES shell which program is
/// to run after this one. `pcmd` and `ptail` are NUL-terminated /
/// length-prefixed byte strings the AES copies out of the caller's memory;
/// they must stay readable for the duration of the trap.
pub fn shel_write(doex: i16, isgr: i16, iscr: i16, pcmd: *const u8, ptail: *const u8) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = doex;
    a.intin[1] = isgr;
    a.intin[2] = iscr;
    a.addrin[0] = pcmd as u64;
    a.addrin[1] = ptail as u64;
    a.call(SHEL_WRITE, 3, 1, 2);
    a.intout[0]
}

/// `shel_get(pbuffer, len)` (122, `desk/gembind.c:918-922`,
/// `AES_CTRL_CODE(SHEL_GET, 1, 1, 1)`; AES side `sh_get`,
/// `aes/gemshlib.c:203-206`) — copy the first `pbuffer.len()` bytes of the AES
/// shell buffer into `pbuffer`. EmuTOS's `len` is a `WORD`, so a longer slice
/// is bounded to `i16::MAX`.
pub fn shel_get(pbuffer: &mut [u8]) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = pbuffer.len().min(i16::MAX as usize) as i16;
    a.addrin[0] = pbuffer.as_mut_ptr() as u64;
    a.call(SHEL_GET, 1, 1, 1);
    a.intout[0]
}

/// `shel_put(pdata, len)` (123, `desk/gembind.c:926-930`,
/// `AES_CTRL_CODE(SHEL_PUT, 1, 1, 1)`; AES side `sh_put`,
/// `aes/gemshlib.c:213-216`) — write `pdata` into the front of the AES shell
/// buffer.
pub fn shel_put(pdata: &[u8]) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = pdata.len().min(i16::MAX as usize) as i16;
    a.addrin[0] = pdata.as_ptr() as u64;
    a.call(SHEL_PUT, 1, 1, 1);
    a.intout[0]
}

/// `shel_wdef(lpcmd, lpdir)` (127, `desk/gembind.c:963-967`,
/// `AES_CTRL_CODE(SHEL_WDEF, 0, 1, 2)`, `#if CONF_WITH_PCGEM`) — set the
/// shell's default startup (program, directory) pair. Both strings must stay
/// readable for the duration of the trap.
pub fn shel_wdef(lpcmd: *const u8, lpdir: *const u8) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = lpcmd as u64;
    a.addrin[1] = lpdir as u64;
    a.call(SHEL_WDEF, 0, 1, 2);
    a.intout[0]
}
