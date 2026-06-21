//! Time-driven AES showcase.
//!
//! Cycles through the GEM managers' visual output, holding each screen for
//! a few seconds before moving on — **no keyboard or mouse required**, so
//! it runs unattended on real hardware (just boot and watch the HDMI
//! output; a running commentary also goes to the serial console).
//!
//! Only non-modal (draw-only) demos are used, so nothing waits for input:
//! the Form alert and File Selector use their `draw_alert` / `preview`
//! variants instead of the interactive paths.

use crate::arch::aarch64::delay_ms;
use crate::{kprintln, selftest_objc};

/// Time each screen is held (milliseconds).
const HOLD_MS: u64 = 5000;

/// Fill the whole screen white (the desktop background) between demos.
fn clear() {
    if let Some(b) = tos_api::host::host().and_then(|h| h.raster()) {
        let clip = tos_api::vdi::backend::ClipRect::full(b.width(), b.height());
        b.fill_rect(0, 0, b.width() as i16, b.height() as i16, 0, clip);
    }
}

/// Clear, log the label, draw, and hold.
fn show(label: &str, f: impl FnOnce()) {
    clear();
    kprintln!("[demo] {}", label);
    f();
    delay_ms(HOLD_MS);
}

/// Run the showcase loop forever.
pub fn showcase() -> ! {
    crate::fbcon::activate();
    kprintln!("[demo] NewTOS AES showcase — time-driven, ~5 s per screen");

    // Non-visual managers: round-trip once to the serial log.
    selftest_objc::run_d9();

    loop {
        show("Object Manager: dialog (objc_draw)", selftest_objc::run);
        show("Form Manager: alert (form_alert)", || {
            tos_api::aes::form::draw_alert(b"[2][NewTOS GEM AES|all managers running!][OK|Cancel]");
        });
        show("Menu Manager: bar + drop-down", selftest_objc::run_menu_open);
        show("Window Manager: framed window", selftest_objc::run_window);
        show("Window Manager: two windows (stacking)", selftest_objc::run_two_windows);
        show("Object Manager: icons + images", selftest_objc::run_icons);
        show("Object Manager: TEDINFO text", selftest_objc::run_tedinfo);
        show("Resource Manager: native .RSC dialog", selftest_objc::run_rsrc);
        show("File Selector: EmuTOS FSELECTR + SD listing", tos_api::aes::fsel::preview);
    }
}
