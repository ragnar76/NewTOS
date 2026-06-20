//! System-string localisation (i18n) for **OS-owned** text only —
//! default dialog buttons, `form_error` messages, system alerts, etc.
//!
//! These strings are accessed by **ID** through [`sys_string`], so every
//! call site is language-agnostic from the start. Only English is wired
//! up now (per the project decision: prepare the abstraction, one
//! language for now); adding a language is just another table plus
//! teaching [`current_lang`] to pick it — eventually from the `_AKP`
//! cookie's low word ([`crate::cookie`]), or from a loaded resource file
//! once the RSC loader (Phase-13 D7) exists.
//!
//! **Application** text is *not* handled here: GEM programs supply their
//! own strings, normally from their own `.RSC` resource.

/// Identifier of an OS-owned system string.
#[derive(Clone, Copy)]
pub enum Sys {
    /// Default dialog button when an alert/dialog specifies none.
    Ok,
    /// Common secondary button label.
    Cancel,
}

/// UI language. Only English exists today; the variant set grows as
/// tables are added.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English.
    En,
}

/// The active UI language. Currently always English; the upgrade path is
/// to derive it from the `_AKP` cookie (low word = language id) or a
/// loaded resource, without touching any call site.
pub fn current_lang() -> Lang {
    Lang::En
}

/// The system string for `id` in the active language, as a NUL-free byte
/// slice (the AES draws bytes, one 8×16 glyph each).
pub fn sys_string(id: Sys) -> &'static [u8] {
    match current_lang() {
        Lang::En => en(id),
    }
}

/// English table.
fn en(id: Sys) -> &'static [u8] {
    match id {
        Sys::Ok => b"OK",
        Sys::Cancel => b"Cancel",
    }
}
