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

/// The default arrow cursor (standard GEM shape), installed on first use.
/// 16 rows; bit 0x8000 is the leftmost pixel. `DATA` is the black arrow,
/// `MASK` the white outline (one pixel larger).
const ARROW_DATA: [u16; 16] = [
    0x0000, 0x4000, 0x6000, 0x7000, 0x7800, 0x7C00, 0x7E00, 0x7F00,
    0x7F80, 0x7E00, 0x6C00, 0x4600, 0x0600, 0x0300, 0x0300, 0x0000,
];
const ARROW_MASK: [u16; 16] = [
    0xC000, 0xE000, 0xF000, 0xF800, 0xFC00, 0xFE00, 0xFF00, 0xFF80,
    0xFFC0, 0xFFC0, 0xFF00, 0xEF00, 0x4F80, 0x0780, 0x0780, 0x0380,
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

/// The current cursor position (for the locator input device).
pub fn position() -> (i16, i16) {
    (GCURX.load(Ordering::Relaxed), GCURY.load(Ordering::Relaxed))
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
    for i in 0..16 {
        MASK[i].store(ARROW_MASK[i], Ordering::Relaxed);
        DATA[i].store(ARROW_DATA[i], Ordering::Relaxed);
    }
    // Park the cursor at the screen centre.
    GCURX.store(backend.width() as i16 / 2, Ordering::Relaxed);
    GCURY.store(backend.height() as i16 / 2, Ordering::Relaxed);
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
