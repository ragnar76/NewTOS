//! VDI mouse cursor, button/position query, and exchange vectors. Spec:
//! `docs/api/vdi/input/_mouse.md`.
//!
//! The cursor **sprite** machinery (form, hide-count, save/restore draw)
//! is complete here. The cursor **position** and **button** state come
//! from the input subsystem (the IKBD mouse packets), which is not yet
//! wired; per the spec this means a *parked* cursor and zero buttons —
//! a documented limitation, not fabricated motion. When the mouse driver
//! lands it calls [`set_state`] and the same sprite code animates.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI16, AtomicU16, AtomicU64, AtomicU8, Ordering};

use super::backend::{ClipRect, RasterBackend};
use super::vwk::validate_color_index;
use super::Vdipb;

const TICKCAL_MS: i16 = 20; // ms per timer-C tick (matches BIOS Tickcal)

// ── live mouse state (fed by the input subsystem when present) ──
static GCURX: AtomicI16 = AtomicI16::new(0);
static GCURY: AtomicI16 = AtomicI16::new(0);
static MOUSE_BT: AtomicU8 = AtomicU8::new(0);
/// EmuTOS `cur_ms_stat` (`vdi_defs.h:294`): the mouse status bitmap since the
/// last event, maintained by [`feed_relative`] exactly as `mouse_int`
/// (`vdi_asm.S:240-328`) and consumed by `gloc_key` (the locator input,
/// `vdi_mouse.c:195-231`). Bit 0/1 = current left/right button state, bit 5 =
/// moved, bit 6/7 = left/right button-change flag. Only `v_locator` reads it;
/// the desktop/AES mouse path (`evnt_multi` → `mouse_buttons`) never does, so
/// maintaining it does not perturb the normal cursor/desktop behaviour.
static CUR_MS_STAT: AtomicU8 = AtomicU8::new(0);
/// Cursor hide depth: 0 = shown, ≥1 = hidden. GEM powers up hidden.
static HIDE_COUNT: AtomicI16 = AtomicI16::new(1);
static INIT_DONE: AtomicU8 = AtomicU8::new(0);

// ── cursor form (MFORM): hotspot, colours, 16 mask + 16 data rows ──
static HOT_X: AtomicI16 = AtomicI16::new(0);
static HOT_Y: AtomicI16 = AtomicI16::new(0);
static BG_COL: AtomicU8 = AtomicU8::new(0); // mask colour (white)
static FG_COL: AtomicU8 = AtomicU8::new(1); // data colour (black)
static MASK: [AtomicU16; 16] = [const { AtomicU16::new(0) }; 16];
static DATA: [AtomicU16; 16] = [const { AtomicU16::new(0) }; 16];

// ── AES-tracked cursor forms (EmuTOS gl_prevmouse / rlr->p_mouse) ──
//
// `graf_mouse(M_SAVE/M_RESTORE/M_PREVIOUS)` (EmuTOS `gemgrlib.c:439-451`) must
// snapshot and re-install cursor forms. The *current* form is the live sprite
// state (`HOT_*`/`*_COL`/`MASK`/`DATA` above, = EmuTOS `gl_mouse`).
// [`PREV_FORM`] mirrors `gl_prevmouse` — the form installed immediately before
// the current one, updated by every [`install_form`] (the `gsx_mfset`
// equivalent, `gemgsxif.c:407-420`). [`SAVED_FORM`] mirrors `rlr->p_mouse`;
// single-tasking makes one slot the 1:1 equivalent of the per-process field
// (the per-process `p_mouse` belongs to the cooperative kernel, Phase 2b).

/// A cursor form snapshot (`MFORM`): hotspot, mask/data colours, and the 16
/// mask + 16 data rows.
#[derive(Clone, Copy)]
struct MForm {
    hot_x: i16,
    hot_y: i16,
    bg_col: u8,
    fg_col: u8,
    mask: [u16; 16],
    data: [u16; 16],
}

/// The default arrow form (mask=white / data=black): the power-up current form,
/// and the initial value of both tracked slots so a stray `M_RESTORE` /
/// `M_PREVIOUS` before any save/set installs the arrow, never a blank sprite.
const ARROW_FORM: MForm = MForm {
    hot_x: PRESET_FORMS[0].0,
    hot_y: PRESET_FORMS[0].1,
    bg_col: 0,
    fg_col: 1,
    mask: PRESET_FORMS[0].2,
    data: PRESET_FORMS[0].3,
};

/// One `MFORM` slot, touched only from the single-CPU trap path (like [`Backing`]).
struct FormSlot {
    form: UnsafeCell<MForm>,
}
// SAFETY: touched only from the single-CPU, IRQ-off VDI trap path.
unsafe impl Sync for FormSlot {}

/// EmuTOS `gl_prevmouse` — the form installed before the current one.
static PREV_FORM: FormSlot = FormSlot { form: UnsafeCell::new(ARROW_FORM) };
/// EmuTOS `rlr->p_mouse` — the `M_SAVE` snapshot (one slot; single-tasking).
static SAVED_FORM: FormSlot = FormSlot { form: UnsafeCell::new(ARROW_FORM) };

// ── exchange-vector slots (function pointers) ──
static USER_BUT: AtomicU64 = AtomicU64::new(0);
static USER_MOT: AtomicU64 = AtomicU64::new(0);
static USER_CUR: AtomicU64 = AtomicU64::new(0);
static USER_WHEEL: AtomicU64 = AtomicU64::new(0);
static TIM_ADDR: AtomicU64 = AtomicU64::new(0);

// ── 16×16 backing store for the pixels under a shown cursor ──
//
// Holds the raw device pixel words (XRGB8888) saved by `draw`, restored
// verbatim by `erase` — the bit-exact save-under EmuTOS' `cur_display`/
// `cur_replace` perform on the screen words (AUD-V6-001). A PEL index would
// lose any colour that is not a palette entry (Falcon truecolor content).
struct Backing {
    px: UnsafeCell<[u32; 256]>,
    ox: AtomicI16,
    oy: AtomicI16,
    valid: AtomicU8,
}
// SAFETY: touched only from the single-CPU, IRQ-off VDI trap path.
unsafe impl Sync for Backing {}
static BACK: Backing = Backing {
    px: UnsafeCell::new([0; 256]),
    ox: AtomicI16::new(0),
    oy: AtomicI16::new(0),
    valid: AtomicU8::new(0),
};

/// The eight AES preset cursor forms, `(xhot, yhot, mask[16], data[16])`,
/// extracted verbatim from EmuTOS `aes/mform.rsc`. The index is the
/// `graf_mouse` / VDI shape number: ARROW 0, TEXT_CRSR 1, HOURGLASS 2,
/// POINT_HAND 3, FLAT_HAND 4, THIN_CROSS 5, THICK_CROSS 6, OUTLN_CROSS 7.
/// 16 rows, bit 0x8000 = leftmost pixel; `data` is the black shape, `mask`
/// the white outline. ARROW (0) is also the default installed on first use.
pub const PRESET_FORMS: [(i16, i16, [u16; 16], [u16; 16]); 8] = [
    // ARROW
    (0, 0,
     [0xC000, 0xE000, 0xF000, 0xF800, 0xFC00, 0xFE00, 0xFF00, 0xFF80, 0xFFC0, 0xFFE0, 0xFE00, 0xEF00, 0xCF00, 0x8780, 0x0780, 0x0380],
     [0x0000, 0x4000, 0x6000, 0x7000, 0x7800, 0x7C00, 0x7E00, 0x7F00, 0x7F80, 0x7C00, 0x6C00, 0x4600, 0x0600, 0x0300, 0x0300, 0x0000]),
    // TEXT_CRSR (I-beam)
    (7, 7,
     [0x1FF8, 0x1FF8, 0x1FF8, 0x07E0, 0x03C0, 0x03C0, 0x03C0, 0x03C0, 0x03C0, 0x03C0, 0x03C0, 0x03C0, 0x07E0, 0x1FF8, 0x1FF8, 0x1FF8],
     [0x0000, 0x0E70, 0x03C0, 0x0180, 0x0180, 0x0180, 0x0180, 0x0180, 0x0180, 0x0180, 0x0180, 0x0180, 0x0180, 0x03C0, 0x0E70, 0x0000]),
    // HOURGLASS (busy)
    (8, 8,
     [0xFFFF, 0xFFFF, 0xFFFF, 0x7FFE, 0x7FFE, 0x3FFC, 0x1FF8, 0x0FF0, 0x0FF0, 0x1FF8, 0x3FFC, 0x7FFE, 0x7FFE, 0xFFFF, 0xFFFF, 0xFFFF],
     [0x0000, 0x7FFE, 0x2004, 0x1008, 0x1448, 0x0AB0, 0x0560, 0x02C0, 0x0340, 0x04A0, 0x0910, 0x1088, 0x12A8, 0x3554, 0x7FFE, 0x0000]),
    // POINT_HAND (pointing finger)
    (0, 0,
     [0x3000, 0x7800, 0x7C00, 0x3E07, 0x1F0F, 0x0F9E, 0x07DE, 0x07FE, 0x1FFE, 0x3FFF, 0x7FFF, 0x7FFE, 0x3FFE, 0x1FFE, 0x0FFF, 0x01FF],
     [0x3000, 0x4800, 0x4400, 0x2207, 0x1109, 0x0892, 0x0452, 0x0632, 0x1912, 0x2481, 0x5241, 0x4982, 0x2602, 0x1802, 0x0E01, 0x0180]),
    // FLAT_HAND (open hand)
    (8, 8,
     [0x0000, 0x0180, 0x0FF0, 0x1FFE, 0x1FFF, 0x1FFF, 0x1FFF, 0xFFFF, 0xFFFF, 0xFFFF, 0x7FFF, 0x3FFF, 0x1FFE, 0x0FFC, 0x0FFC, 0x1FFE],
     [0x0000, 0x0D80, 0x1270, 0x124E, 0x1249, 0x1249, 0x1249, 0xF249, 0x9001, 0x9801, 0x4401, 0x4001, 0x3002, 0x0804, 0x0804, 0x1FFE]),
    // THIN_CROSS
    (7, 7,
     [0x0000, 0x0380, 0x0380, 0x0380, 0x0380, 0x0380, 0x0380, 0x7FFC, 0x7FFC, 0x7FFC, 0x0380, 0x0380, 0x0380, 0x0380, 0x0380, 0x0380],
     [0x0000, 0x0000, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x3FF8, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0100, 0x0000]),
    // THICK_CROSS
    (8, 8,
     [0x0000, 0x07C0, 0x07C0, 0x07C0, 0x07C0, 0x07C0, 0xFFFE, 0xFFFE, 0xFFFE, 0xFFFE, 0xFFFE, 0x07C0, 0x07C0, 0x07C0, 0x07C0, 0x07C0],
     [0x0000, 0x0000, 0x0380, 0x0380, 0x0380, 0x0380, 0x0380, 0x7FFC, 0x7FFC, 0x7FFC, 0x0380, 0x0380, 0x0380, 0x0380, 0x0380, 0x0000]),
    // OUTLN_CROSS
    (8, 8,
     [0x07E0, 0x07E0, 0x0660, 0x0660, 0x0660, 0xFE7F, 0xFE7F, 0xC003, 0xC003, 0xFE7F, 0xFE7F, 0x0660, 0x0660, 0x0660, 0x07E0, 0x07E0],
     [0x0000, 0x03C0, 0x0240, 0x0240, 0x0240, 0x0240, 0x7E7E, 0x4002, 0x4002, 0x7E7E, 0x0240, 0x0240, 0x0240, 0x0240, 0x03C0, 0x0000]),
];

/// Update the mouse position + button state from the input subsystem.
/// Redraws the cursor at the new position if it is currently shown.
///
/// The integration point for the IKBD mouse driver (relative-mouse
/// packets); unused until that driver is wired — see the module note.
#[allow(dead_code)]
pub fn set_state(x: i16, y: i16, buttons: u8, backend: &dyn RasterBackend) {
    if shown() {
        erase(backend);
    }
    GCURX.store(x, Ordering::Relaxed);
    GCURY.store(y, Ordering::Relaxed);
    MOUSE_BT.store(buttons, Ordering::Relaxed);
    if shown() {
        draw(backend);
    }
}

/// Feed a relative-mouse event from the input subsystem (the kernel's
/// IKBD mouse-packet decoder). `dx`/`dy` are signed pixel deltas;
/// `buttons` is already in VDI convention (bit 0 = left, bit 1 = right).
/// Accumulates onto the current cursor position, clamps it to the
/// screen, and updates the cursor + the `vq_mouse`/AES-event state.
///
/// This is the API-level half of the mouse driver: it owns the cursor
/// semantics. The hardware packet decode lives in the kernel — see
/// `docs/api/vdi/input/_mouse-packet.md`.
pub fn feed_relative(dx: i16, dy: i16, buttons: u8, backend: &dyn RasterBackend) {
    ensure_init(backend);
    // Maintain `cur_ms_stat` for the locator input device (`v_locator` /
    // `gloc_key`), byte-for-byte as EmuTOS `mouse_int` (`vdi_asm.S:240-328`):
    // the raw deltas decide the motion bit, the button transition sets the
    // change bits. This touches only `cur_ms_stat`; the cursor draw, `MOUSE_BT`
    // and `GCURX`/`GCURY` (via `set_state` below) are unchanged, so the normal
    // cursor/desktop path stays byte-identical.
    update_ms_stat(dx, dy, buttons);
    let max_x = backend.width() as i16 - 1;
    let max_y = backend.height() as i16 - 1;
    let (cx, cy) = position();
    let nx = (cx + dx).clamp(0, max_x);
    let ny = (cy + dy).clamp(0, max_y);
    set_state(nx, ny, buttons, backend);
}

/// Update [`CUR_MS_STAT`] from one relative-mouse event, reproducing EmuTOS
/// `mouse_int` (`vdi_asm.S:240-328`) bit-for-bit. On a button-state change the
/// byte is rewritten to the new button bits (0/1) with the changed buttons
/// flagged in bits 6 (left) / 7 (right) — the `eor`/`ror.b #2`/`or` sequence —
/// which also clears the motion bit; with no button change the byte keeps its
/// button/change bits. The motion bit (5) is then set if the packet carried
/// movement (raw `dx`/`dy`, before clipping), else cleared. `buttons` is in VDI
/// convention (bit 0 = left, bit 1 = right); only bits 0/1 are used, so EmuTOS'
/// "keep the extended-button bits in MOUSE_BT" reduces to a no-op here.
fn update_ms_stat(dx: i16, dy: i16, buttons: u8) {
    let new_btn = buttons & 0x03;
    let moved = dx != 0 || dy != 0;
    // Atomic RMW: the feed can race a `gloc_key` consume if it runs from an
    // IRQ during a `pump_input`.
    let _ = CUR_MS_STAT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |prev| {
        let old_btn = prev & 0x03;
        let mut stat = if new_btn != old_btn {
            let delta = (old_btn ^ new_btn) & 0x03;
            new_btn | delta.rotate_right(2)
        } else {
            prev
        };
        if moved {
            stat |= 0x20;
        } else {
            stat &= !0x20;
        }
        Some(stat)
    });
}

/// The current cursor position (for the locator input device).
pub fn position() -> (i16, i16) {
    (GCURX.load(Ordering::Relaxed), GCURY.load(Ordering::Relaxed))
}

/// The current cursor position, materialising the parked start position
/// (screen centre) on first use. `graf_mouse(M_ON)` shows the cursor "where
/// it is"; before any mouse motion that must be the initialised park
/// position, never the power-up `(0, 0)`. Reading the raw [`position`]
/// before [`ensure_init`] yields `(0, 0)`, so the very first `M_ON` would
/// pin the cursor over the menu bar (opening an AES hover-menu at boot).
/// Idempotent — `ensure_init` runs at most once.
pub fn position_init(backend: &dyn RasterBackend) -> (i16, i16) {
    ensure_init(backend);
    position()
}

/// The current button bitmask (for the locator/choice input devices).
#[allow(dead_code)]
pub fn buttons() -> u8 {
    MOUSE_BT.load(Ordering::Relaxed)
}

/// EmuTOS `gloc_key` button branch (`vdi_mouse.c:203-209`): if a mouse-button
/// change is pending (bit 6 or 7 of `cur_ms_stat`), consume it and return the
/// terminator code — 0x20 for the left button (bit 6), 0x21 for the right
/// (bit 7), left taking precedence. Clears the change bits, keeping the button
/// and motion bits (`&= 0x23`). `None` when no button change is pending. Used
/// only by `v_locator`.
pub fn take_button_terminator() -> Option<i16> {
    // Test-and-clear bits 6/7 atomically (see `update_ms_stat` on the race);
    // masking to 0x23 keeps bits 0,1 (buttons) and 5 (motion). When no change
    // bit is set this matches EmuTOS leaving `cur_ms_stat` untouched, because
    // the reserved bits 2/3/4 are always zero here.
    let prev = CUR_MS_STAT.fetch_and(0x23, Ordering::Relaxed);
    if prev & 0xc0 != 0 {
        Some(if prev & 0x40 != 0 { 0x20 } else { 0x21 })
    } else {
        None
    }
}

/// EmuTOS `gloc_key` motion branch (`vdi_mouse.c:221-227`): if the motion bit
/// (5) of `cur_ms_stat` is set, consume it and report movement — the caller
/// reads the live [`position`] for the point. Used only by `v_locator`.
pub fn take_motion() -> bool {
    (CUR_MS_STAT.fetch_and(!0x20, Ordering::Relaxed) & 0x20) != 0
}

/// EmuTOS `vdi_v_locator` entry (`vdi_mouse.c:286-287`): set the locator / VDI
/// cursor position `GCURX`/`GCURY` from the input point, unconditionally in
/// both request and sample modes. This is the logical position `dis_cur` draws
/// at and `gloc_key` / `PTSOUT` report; the request-mode `dis_cur`/`hide_cur`
/// bracket performs the drawing, so this does not itself erase/redraw a shown
/// sprite (the next `erase` still restores from the last drawn origin).
pub fn set_locator_pos(x: i16, y: i16) {
    GCURX.store(x, Ordering::Relaxed);
    GCURY.store(y, Ordering::Relaxed);
}

fn shown() -> bool {
    HIDE_COUNT.load(Ordering::Relaxed) <= 0
}

fn ensure_init(backend: &dyn RasterBackend) {
    if INIT_DONE.swap(1, Ordering::Relaxed) != 0 {
        return;
    }
    let (_, _, mask, data) = &PRESET_FORMS[0]; // ARROW
    for i in 0..16 {
        MASK[i].store(mask[i], Ordering::Relaxed);
        DATA[i].store(data[i], Ordering::Relaxed);
    }
    // Park the cursor at the screen centre.
    GCURX.store(backend.width() as i16 / 2, Ordering::Relaxed);
    GCURY.store(backend.height() as i16 / 2, Ordering::Relaxed);
    // Install the default exchange-vector slots (EmuTOS `vdimouse_init`,
    // `vdi_mouse.c:585-589`; the `vex_timv` slot `tim_addr`, `vdi_misc.c:128`):
    // the button / motion / wheel / timer vectors default to the `just_rts`
    // no-op, the cursor-draw vector to `mov_cur`. NewTOS never *invokes* these
    // from the cooperative feed path — the default behaviour is already inlined
    // (`feed_relative` redraws the cursor; buttons/motion are no-ops) — but the
    // slots must hold a real address so an app that installs its own vector via
    // `vex_*` gets the genuine previous address back to save/restore, instead
    // of a spurious NULL (AUD-V6-002).
    // Cast the fn items through a raw pointer (not a direct fn→int cast) to
    // take their address as a `u64` sentinel.
    USER_BUT.store(just_rts as *const () as u64, Ordering::Relaxed);
    USER_MOT.store(just_rts as *const () as u64, Ordering::Relaxed);
    USER_CUR.store(mov_cur as *const () as u64, Ordering::Relaxed);
    USER_WHEEL.store(just_rts as *const () as u64, Ordering::Relaxed);
    TIM_ADDR.store(just_rts as *const () as u64, Ordering::Relaxed);
}

/// EmuTOS `just_rts` (`vdi_asm.S`): the default no-op mouse vector shared by
/// the button / motion / wheel / timer slots. Only its **address** matters —
/// it is the sentinel the `vex_*` swap returns so an app can save and restore
/// the default; it is never called from NewTOS' cooperative feed path
/// (AUD-V6-002, the STOP-split: invoking an app-installed vector would be an
/// EL1→EL0 upcall, out of scope).
fn just_rts() {}

/// EmuTOS `mov_cur` (`vdi_asm.S:496`): the default cursor-draw vector. EmuTOS'
/// `mouse_int` calls it to redraw the sprite after a move; NewTOS redraws the
/// cursor inline in [`feed_relative`], so this is a pure address sentinel for
/// the `vex_curv` save/restore contract — never invoked.
fn mov_cur() {}

/// Initialise the cursor subsystem at workstation-open time (EmuTOS
/// `vdimouse_init`, `vdi_mouse.c:582-583`): load the default arrow form and
/// park the cursor at the screen centre. This materialises `GCURX`/`GCURY`
/// **before** the first cursor op, so `vq_mouse`/`v_locator` report the parked
/// centre rather than the power-up `(0, 0)`. Idempotent (guarded by
/// `INIT_DONE`), so it never clobbers a live cursor position moved by the HID
/// mouse.
pub fn init(backend: &dyn RasterBackend) {
    ensure_init(backend);
}

/// Install an arbitrary cursor form (16-row mask + data, hotspot), drawn
/// with the standard mask=white / data=black colours. Erases and redraws
/// the sprite if it is currently shown. Shared by the AES presets and
/// `graf_mouse`'s USER_DEF path.
pub fn set_form(hot_x: i16, hot_y: i16, mask: &[u16; 16], data: &[u16; 16], backend: &dyn RasterBackend) {
    // Standard mask=white / data=black colours (the AES presets and the
    // `graf_mouse(USER_DEF)` path). The install itself lives in [`install_form`]
    // (the `gsx_mfset` equivalent), which also records the outgoing form as the
    // "previous" one so `graf_mouse(M_PREVIOUS)` can revert to it.
    install_form(
        &MForm { hot_x, hot_y, bg_col: 0, fg_col: 1, mask: *mask, data: *data },
        backend,
    );
}

/// Install one of the eight preset cursor forms by its shape number
/// (0..=7); out-of-range numbers are ignored (EmuTOS `gsx_mfset`).
pub fn set_preset(n: i16, backend: &dyn RasterBackend) {
    if let Some((hx, hy, mask, data)) = PRESET_FORMS.get(n as usize) {
        set_form(*hx, *hy, mask, data, backend);
    }
}

/// Read the live sprite state into an [`MForm`] (EmuTOS `gl_mouse`).
fn current_form() -> MForm {
    let mut mask = [0u16; 16];
    let mut data = [0u16; 16];
    for i in 0..16 {
        mask[i] = MASK[i].load(Ordering::Relaxed);
        data[i] = DATA[i].load(Ordering::Relaxed);
    }
    MForm {
        hot_x: HOT_X.load(Ordering::Relaxed),
        hot_y: HOT_Y.load(Ordering::Relaxed),
        bg_col: BG_COL.load(Ordering::Relaxed),
        fg_col: FG_COL.load(Ordering::Relaxed),
        mask,
        data,
    }
}

/// Install `form` as the current cursor — the `gsx_mfset` equivalent (EmuTOS
/// `gemgsxif.c:407-420`). Record the outgoing form as [`PREV_FORM`]
/// (`gl_prevmouse = gl_mouse`), then set the new one, erasing and redrawing the
/// sprite if it is currently shown. The hotspot is clamped to the 16×16 grid,
/// exactly as [`set_form`] did.
fn install_form(form: &MForm, backend: &dyn RasterBackend) {
    ensure_init(backend);
    // EmuTOS `gsx_mfset`: gl_prevmouse = gl_mouse before installing the new form.
    // SAFETY: single trap path.
    unsafe { *PREV_FORM.form.get() = current_form() };
    let was_shown = shown();
    if was_shown {
        erase(backend);
    }
    HOT_X.store(form.hot_x.clamp(0, 15), Ordering::Relaxed);
    HOT_Y.store(form.hot_y.clamp(0, 15), Ordering::Relaxed);
    BG_COL.store(form.bg_col, Ordering::Relaxed);
    FG_COL.store(form.fg_col, Ordering::Relaxed);
    for i in 0..16 {
        MASK[i].store(form.mask[i], Ordering::Relaxed);
        DATA[i].store(form.data[i], Ordering::Relaxed);
    }
    if was_shown {
        draw(backend);
    }
}

/// EmuTOS `gr_mouse(M_SAVE)` (`gemgrlib.c:441`): snapshot the current cursor
/// form into the saved slot (`rlr->p_mouse`). No draw, no form change.
pub fn save_form() {
    // SAFETY: single trap path.
    unsafe { *SAVED_FORM.form.get() = current_form() };
}

/// EmuTOS `gr_mouse(M_RESTORE)` (`gemgrlib.c:443-445`): reinstall the form
/// stored by [`save_form`] (`rlr->p_mouse`).
pub fn restore_form(backend: &dyn RasterBackend) {
    // SAFETY: single trap path; `MForm` is `Copy`.
    let form = unsafe { *SAVED_FORM.form.get() };
    install_form(&form, backend);
}

/// EmuTOS `gr_mouse(M_PREVIOUS)` (`gemgrlib.c:446-450`): reinstall the form
/// installed immediately before the current one (`gl_prevmouse`). EmuTOS copies
/// it into a temporary first because `gsx_mfset` overwrites `gl_prevmouse`; we
/// read the slot into a local before [`install_form`] rewrites it, so the two
/// forms swap (current ↔ previous) exactly as EmuTOS does.
pub fn previous_form(backend: &dyn RasterBackend) {
    // SAFETY: single trap path; `MForm` is `Copy`.
    let form = unsafe { *PREV_FORM.form.get() };
    install_form(&form, backend);
}

fn full_clip(backend: &dyn RasterBackend) -> ClipRect {
    ClipRect::full(backend.width(), backend.height())
}

/// Save the 16×16 area under the cursor bit-exact, then draw the sprite.
///
/// EmuTOS `cur_display`/`cur_display16` save the raw screen words under the
/// sprite (`*save++ = *dst`, `vdi_mouse.c:983-985`) *before* overwriting
/// them. We do the chunky-per-pixel equivalent: store the exact device
/// pixel word via [`get_device_pixel`](RasterBackend::get_device_pixel),
/// **not** the nearest-PEL round-trip `read_pixel` gives — the latter
/// corrupts every pixel whose colour is not a palette entry (Falcon
/// truecolor content), AUD-V6-001. The sprite paint itself keeps the PEL
/// `fill_rect` path (mask → bg, data → fg), which is exact for the
/// two-colour sprite. Order (save → draw) is unchanged.
fn draw(backend: &dyn RasterBackend) {
    let ox = GCURX.load(Ordering::Relaxed) - HOT_X.load(Ordering::Relaxed);
    let oy = GCURY.load(Ordering::Relaxed) - HOT_Y.load(Ordering::Relaxed);
    let clip = full_clip(backend);
    let fg = FG_COL.load(Ordering::Relaxed);
    let bg = BG_COL.load(Ordering::Relaxed);
    // SAFETY: single trap path.
    let store = unsafe { &mut *BACK.px.get() };
    for dy in 0..16i16 {
        let mrow = MASK[dy as usize].load(Ordering::Relaxed);
        let drow = DATA[dy as usize].load(Ordering::Relaxed);
        for dx in 0..16i16 {
            let (px, py) = (ox + dx, oy + dy);
            store[(dy * 16 + dx) as usize] = backend.get_device_pixel(px, py);
            let bit = 0x8000u16 >> dx;
            if drow & bit != 0 {
                backend.fill_rect(px, py, 1, 1, fg, clip);
            } else if mrow & bit != 0 {
                backend.fill_rect(px, py, 1, 1, bg, clip);
            }
        }
    }
    BACK.ox.store(ox, Ordering::Relaxed);
    BACK.oy.store(oy, Ordering::Relaxed);
    BACK.valid.store(1, Ordering::Relaxed);
}

/// Restore the pixels saved under the cursor bit-exact.
///
/// EmuTOS `cur_replace`/`cur_replace16` write the saved screen words
/// straight back (`*dst = *src++`, `vdi_mouse.c:1099`/`:1046`). We do the
/// chunky-per-pixel equivalent: put each saved device pixel word back via
/// [`put_device_pixel`](RasterBackend::put_device_pixel) — restoring the
/// exact original colour, not the palette approximation `fill_rect` would
/// (AUD-V6-001). `put_device_pixel` does not flush on its own, so the whole
/// 16×16 box is flushed to the point of coherency once at the end. Cells
/// off the screen edge are ignored by the backend, matching the clipping in
/// `cur_replace`. Order (erase → restore) and the `BACK.valid` swap
/// (guarding against a double restore) are unchanged.
fn erase(backend: &dyn RasterBackend) {
    if BACK.valid.swap(0, Ordering::Relaxed) == 0 {
        return;
    }
    let ox = BACK.ox.load(Ordering::Relaxed);
    let oy = BACK.oy.load(Ordering::Relaxed);
    // SAFETY: single trap path.
    let store = unsafe { &*BACK.px.get() };
    for dy in 0..16i16 {
        for dx in 0..16i16 {
            backend.put_device_pixel(ox + dx, oy + dy, store[(dy * 16 + dx) as usize]);
        }
    }
    backend.flush_rect(ox, oy, ox + 15, oy + 15);
}

/// Opcode 111 — `vsc_form`: install the 37-WORD cursor definition from
/// `INTIN` (hotspot x/y, reserved, mask colour, data colour, then 16
/// mask rows and 16 data rows).
pub fn vsc_form(pb: &Vdipb, backend: &dyn RasterBackend) {
    ensure_init(backend);
    let was_shown = shown();
    if was_shown {
        erase(backend);
    }
    // Hotspot constrained to 0..15 by masking (EmuTOS `set_mouse_form`
    // `& 0x0f`): values above 15 wrap, they never overflow the 16×16 sprite.
    HOT_X.store(pb.intin(0) & 0x0f, Ordering::Relaxed);
    HOT_Y.store(pb.intin(1) & 0x0f, Ordering::Relaxed);
    // intin(2) = reserved (planes, must be 1) — ignored, like `set_mouse_form`.
    // Mask/data colour indices run through `validate_color_index`: an
    // out-of-range pen (< 0 or ≥ numcolors) silently becomes pen 1.
    let ncol = backend.num_colors();
    BG_COL.store(validate_color_index(pb.intin(3), ncol), Ordering::Relaxed);
    FG_COL.store(validate_color_index(pb.intin(4), ncol), Ordering::Relaxed);
    for i in 0..16 {
        MASK[i].store(pb.intin(5 + i) as u16, Ordering::Relaxed);
        DATA[i].store(pb.intin(21 + i) as u16, Ordering::Relaxed);
    }
    if was_shown {
        draw(backend);
    }
}

/// EmuTOS `dis_cur`: the counted-show core. Decrement the hide depth,
/// underflow-clamped to 0, and physically draw the cursor on the 1 → 0
/// transition (`vdi_mouse.c:127-141`).
fn dis_cur(backend: &dyn RasterBackend) {
    let prev = HIDE_COUNT.load(Ordering::Relaxed);
    let new = (prev - 1).max(0);
    HIDE_COUNT.store(new, Ordering::Relaxed);
    if prev > 0 && new == 0 {
        draw(backend);
    }
}

/// Opcode 122 — `v_show_c`: counted show. `INTIN[0] == 0` is a **force-show**
/// (EmuTOS `linea_show_mouse`, `vdi_mouse.c:1125-1131`): if the cursor is
/// currently hidden (`HIDE_COUNT != 0`), collapse the hide depth to a single
/// level so the following `dis_cur` uncovers it immediately. A **non-zero**
/// `INTIN[0]` performs an ordinary counted show — this is the reverse of the
/// naive reading of the flag.
pub fn v_show_c(pb: &Vdipb, backend: &dyn RasterBackend) {
    ensure_init(backend);
    if pb.intin(0) == 0 && HIDE_COUNT.load(Ordering::Relaxed) != 0 {
        HIDE_COUNT.store(1, Ordering::Relaxed);
    }
    dis_cur(backend);
}

/// Force-show the cursor at its **current** position (VDI `v_dspcur` escape,
/// EmuTOS escfn18 → `v_show_c` with `INTIN[0]` forced to 0): collapse any hide
/// depth to a single level, then display. The Atari-documented `x,y`
/// coordinates are ignored, matching EmuTOS/Atari TOS.
pub fn force_show(backend: &dyn RasterBackend) {
    ensure_init(backend);
    if HIDE_COUNT.load(Ordering::Relaxed) != 0 {
        HIDE_COUNT.store(1, Ordering::Relaxed);
    }
    dis_cur(backend);
}

/// Opcode 123 — `v_hide_c`: erase the cursor (if shown) and increment the
/// hide depth.
pub fn v_hide_c(backend: &dyn RasterBackend) {
    if shown() {
        erase(backend);
    }
    let c = HIDE_COUNT.load(Ordering::Relaxed);
    HIDE_COUNT.store(c + 1, Ordering::Relaxed);
}

// ── Direct (non-trap) cursor control for the kernel desktop ──
//
// The desktop is kernel-side and repaints the framebuffer directly, so it
// brackets each repaint with `hide` (erase the sprite) + `show` (redraw it on
// top), and calls `show` once at start-up to take the cursor out of GEM's
// power-up hidden state. Same machinery as `v_hide_c`/`v_show_c`, without a
// `Vdipb`.

/// Erase the cursor and increment the hide depth (kernel `v_hide_c`).
pub fn hide(backend: &dyn RasterBackend) {
    if shown() {
        erase(backend);
    }
    HIDE_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Decrement the hide depth; draw the cursor when it becomes shown (kernel
/// `v_show_c`). Balances a [`hide`]. Same counted-show core as the trap path.
pub fn show(backend: &dyn RasterBackend) {
    ensure_init(backend);
    dis_cur(backend);
}

/// Force the cursor visible (hide depth 0) and draw it — the initial unhide.
pub fn show_reset(backend: &dyn RasterBackend) {
    ensure_init(backend);
    let prev = HIDE_COUNT.swap(0, Ordering::Relaxed);
    if prev > 0 {
        draw(backend);
    }
}

/// Show the graphics cursor at `(x, y)` (VDI `v_dspcur` escape).
pub fn display_at(x: i16, y: i16, backend: &dyn RasterBackend) {
    ensure_init(backend);
    if shown() {
        erase(backend);
    }
    GCURX.store(x, Ordering::Relaxed);
    GCURY.store(y, Ordering::Relaxed);
    HIDE_COUNT.store(0, Ordering::Relaxed);
    draw(backend);
}

/// Remove the graphics cursor (VDI `v_rmcur` escape).
pub fn remove(backend: &dyn RasterBackend) {
    if shown() {
        erase(backend);
    }
    HIDE_COUNT.store(1, Ordering::Relaxed);
}

/// Opcode 124 — `vq_mouse`: report the button bitmask + cursor position.
pub fn vq_mouse(pb: &Vdipb) {
    pb.set_intout(0, MOUSE_BT.load(Ordering::Relaxed) as i16);
    pb.set_ptsout(0, GCURX.load(Ordering::Relaxed));
    pb.set_ptsout(1, GCURY.load(Ordering::Relaxed));
    pb.set_contrl(4, 1);
    pb.set_contrl(2, 1);
}

/// Swap one exchange vector: install `CONTRL[7..10]` (native 64-bit
/// pointer), return the previous in `CONTRL[9..12]`.
fn swap_vector(pb: &Vdipb, slot: &AtomicU64) {
    let new = pb.contrl_u64(7);
    let prev = slot.swap(new, Ordering::Relaxed);
    pb.set_contrl_u64(9, prev);
}

/// Opcode 125 — `vex_butv`: install the button-change vector.
pub fn vex_butv(pb: &Vdipb) {
    swap_vector(pb, &USER_BUT);
}
/// Opcode 126 — `vex_motv`: install the mouse-move vector.
pub fn vex_motv(pb: &Vdipb) {
    swap_vector(pb, &USER_MOT);
}
/// Opcode 127 — `vex_curv`: install the cursor-draw vector.
pub fn vex_curv(pb: &Vdipb) {
    swap_vector(pb, &USER_CUR);
}
/// Opcode 134 — `vex_wheelv`: install the mouse-wheel vector.
pub fn vex_wheelv(pb: &Vdipb) {
    swap_vector(pb, &USER_WHEEL);
}

/// Opcode 118 — `vex_timv`: install the timer-tick vector and return the
/// tick period in ms (`Tickcal`).
pub fn vex_timv(pb: &Vdipb) {
    swap_vector(pb, &TIM_ADDR);
    pb.set_intout(0, TICKCAL_MS);
    pb.set_contrl(4, 1);
}
