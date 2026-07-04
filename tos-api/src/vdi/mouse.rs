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

use super::backend::{ClipRect, Pel, RasterBackend};
use super::Vdipb;

const TICKCAL_MS: i16 = 20; // ms per timer-C tick (matches BIOS Tickcal)

// ── live mouse state (fed by the input subsystem when present) ──
static GCURX: AtomicI16 = AtomicI16::new(0);
static GCURY: AtomicI16 = AtomicI16::new(0);
static MOUSE_BT: AtomicU8 = AtomicU8::new(0);
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

// ── exchange-vector slots (function pointers) ──
static USER_BUT: AtomicU64 = AtomicU64::new(0);
static USER_MOT: AtomicU64 = AtomicU64::new(0);
static USER_CUR: AtomicU64 = AtomicU64::new(0);
static USER_WHEEL: AtomicU64 = AtomicU64::new(0);
static TIM_ADDR: AtomicU64 = AtomicU64::new(0);

// ── 16×16 backing store for the pixels under a shown cursor ──
struct Backing {
    px: UnsafeCell<[Pel; 256]>,
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
    let max_x = backend.width() as i16 - 1;
    let max_y = backend.height() as i16 - 1;
    let (cx, cy) = position();
    let nx = (cx + dx).clamp(0, max_x);
    let ny = (cy + dy).clamp(0, max_y);
    set_state(nx, ny, buttons, backend);
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
}

/// Install an arbitrary cursor form (16-row mask + data, hotspot), drawn
/// with the standard mask=white / data=black colours. Erases and redraws
/// the sprite if it is currently shown. Shared by the AES presets and
/// `graf_mouse`'s USER_DEF path.
pub fn set_form(hot_x: i16, hot_y: i16, mask: &[u16; 16], data: &[u16; 16], backend: &dyn RasterBackend) {
    ensure_init(backend);
    let was_shown = shown();
    if was_shown {
        erase(backend);
    }
    HOT_X.store(hot_x.clamp(0, 15), Ordering::Relaxed);
    HOT_Y.store(hot_y.clamp(0, 15), Ordering::Relaxed);
    BG_COL.store(0, Ordering::Relaxed); // mask = white
    FG_COL.store(1, Ordering::Relaxed); // data = black
    for i in 0..16 {
        MASK[i].store(mask[i], Ordering::Relaxed);
        DATA[i].store(data[i], Ordering::Relaxed);
    }
    if was_shown {
        draw(backend);
    }
}

/// Install one of the eight preset cursor forms by its shape number
/// (0..=7); out-of-range numbers are ignored (EmuTOS `gsx_mfset`).
pub fn set_preset(n: i16, backend: &dyn RasterBackend) {
    if let Some((hx, hy, mask, data)) = PRESET_FORMS.get(n as usize) {
        set_form(*hx, *hy, mask, data, backend);
    }
}

fn full_clip(backend: &dyn RasterBackend) -> ClipRect {
    ClipRect::full(backend.width(), backend.height())
}

/// Save the 16×16 area under the cursor and draw the sprite.
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
            store[(dy * 16 + dx) as usize] = backend.read_pixel(px, py);
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

/// Restore the pixels saved under the cursor.
fn erase(backend: &dyn RasterBackend) {
    if BACK.valid.swap(0, Ordering::Relaxed) == 0 {
        return;
    }
    let ox = BACK.ox.load(Ordering::Relaxed);
    let oy = BACK.oy.load(Ordering::Relaxed);
    let clip = full_clip(backend);
    // SAFETY: single trap path.
    let store = unsafe { &*BACK.px.get() };
    for dy in 0..16i16 {
        for dx in 0..16i16 {
            backend.fill_rect(ox + dx, oy + dy, 1, 1, store[(dy * 16 + dx) as usize], clip);
        }
    }
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
    HOT_X.store(pb.intin(0).clamp(0, 15), Ordering::Relaxed);
    HOT_Y.store(pb.intin(1).clamp(0, 15), Ordering::Relaxed);
    // intin(2) = reserved (planes, must be 1)
    BG_COL.store(pb.intin(3) as u8, Ordering::Relaxed);
    FG_COL.store(pb.intin(4) as u8, Ordering::Relaxed);
    for i in 0..16 {
        MASK[i].store(pb.intin(5 + i) as u16, Ordering::Relaxed);
        DATA[i].store(pb.intin(21 + i) as u16, Ordering::Relaxed);
    }
    if was_shown {
        draw(backend);
    }
}

/// Opcode 122 — `v_show_c`: decrement the hide depth (or force it to
/// shown when `INTIN[0]` ≠ 0) and draw the cursor when it becomes shown.
pub fn v_show_c(pb: &Vdipb, backend: &dyn RasterBackend) {
    ensure_init(backend);
    let reset = pb.intin(0) != 0;
    let prev = HIDE_COUNT.load(Ordering::Relaxed);
    let new = if reset { 0 } else { (prev - 1).max(0) };
    HIDE_COUNT.store(new, Ordering::Relaxed);
    if prev > 0 && new == 0 {
        draw(backend);
    }
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
/// `v_show_c`). Balances a [`hide`].
pub fn show(backend: &dyn RasterBackend) {
    ensure_init(backend);
    let prev = HIDE_COUNT.load(Ordering::Relaxed);
    let new = (prev - 1).max(0);
    HIDE_COUNT.store(new, Ordering::Relaxed);
    if prev > 0 && new == 0 {
        draw(backend);
    }
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
