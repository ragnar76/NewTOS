//! The `Bconout(2, …)` screen device — VT52-style framebuffer text console.
//!
//! Renders BIOS/GEMDOS console output (`Bconout(2,…)`, `Cconout`, `Cconws`,
//! `Fwrite` to a CON handle …) to the framebuffer through the VDI's 8×16
//! system font. The kernel
//! [`Host::console_write_byte`](crate::host_env) forwards each byte here (in
//! addition to the UART).
//!
//! **This module owns no state.** The cursor position, the reverse-video flag
//! and the cell rendering live in the VDI's alpha layer
//! ([`tos_api::vdi::alpha_conout`]), which is the *same* state the VDI escape
//! sub-functions (`v_enter_cur`, `v_curtext`, `v_rvon`, …) drive — exactly as
//! in EmuTOS, where those escapes are literally `Bconout(2, ch)` into the one
//! BIOS VT-52 console (`vdi/vdi_esc.c:29-42,83,95,205`). That sharing is what
//! makes the classic TOS "GEM program runs a TOS program" sequence work:
//! `v_enter_cur` clears and homes, and the child's console output then starts
//! at the home position on the screen it just cleared.
//!
//! Console output is **not gated** on any mode flag, for the same reason it
//! is not in EmuTOS: `bconout2` writes into screen memory unconditionally,
//! whether the AES considers itself in graphics mode or not. A program that
//! prints while the GEM screen is up paints over it — as it does on TOS.
//!
//! Scope: printable glyphs, `CR`/`LF`/`BS`/`TAB`, line wrap and upward
//! scroll. `ESC` sequences in the byte stream are not parsed (they arrive
//! through `v_escape` instead).

use crate::vdi_backend::{self, VDI_SCREEN};

/// Clear the screen and home the cursor, i.e. hand the screen over to the
/// character console — the `ESC E` a workstation close (`v_clswk`, the AES'
/// `sh_toalpha`) and the desktop's own console-program launch leave behind.
pub fn clear_and_home() {
    if !vdi_backend::available() {
        return;
    }
    tos_api::vdi::alpha_clear_home(&VDI_SCREEN);
}

/// Write one byte to the console screen.
pub fn write_byte(b: u8) {
    if !vdi_backend::available() {
        return;
    }
    tos_api::vdi::alpha_conout(&VDI_SCREEN, b);
}
