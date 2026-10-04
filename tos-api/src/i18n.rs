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
    /// The crash-report prompt (EmuTOS `dopanic`,
    /// `bios/kprint.c:583`: `_("\n*** Press any key to continue ***")`).
    PressAnyKey,
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
        Sys::PressAnyKey => b"*** Press any key to continue ***",
    }
}

/// A `form_error` alert class (EmuTOS `fm_error` switch, gemfmlib.c:443-467 →
/// the gem.rsc AL*ERR resources). `form_error`'s positive DOS error number is
/// mapped to one of these classes and the full alert string is drawn.
#[derive(Clone, Copy)]
pub enum ErrAlert {
    /// AL18ERR — file/folder not found (DOS 2, 3, 18).
    NotFound,
    /// AL04ERR — no room to open another document (DOS 4).
    NoRoom,
    /// AL05ERR — item exists / read-only (DOS 5).
    Exists,
    /// AL15ERR — the specified drive does not exist (DOS 15).
    NoDrive,
    /// AL08ERR — not enough memory (DOS 8, 10, 11).
    NoMem,
    /// ALXXERR — generic "TOS error #%u." (the `switch` default). The caller
    /// substitutes the error number for the `%u` placeholder.
    Generic,
}

/// The full alert string (`[icon][message][buttons]`) for a `form_error`
/// class in the active language — byte-for-byte the EmuTOS gem.rsc AL*ERR
/// resource (`aes/gem.rsc`). `Generic` still carries the `%u` placeholder for
/// the caller to fill with the error number (EmuTOS `fm_show` / `sprintf`).
pub fn err_alert(class: ErrAlert) -> &'static [u8] {
    match current_lang() {
        Lang::En => en_err_alert(class),
    }
}

/// English AL*ERR alert strings, verbatim from `aes/gem.rsc`.
fn en_err_alert(class: ErrAlert) -> &'static [u8] {
    match class {
        // AL18ERR
        ErrAlert::NotFound => {
            b"[2][This application cannot|find the folder or file|you just tried to access.][  OK  ]"
        }
        // AL04ERR
        ErrAlert::NoRoom => {
            b"[1][This application does not|have room to open another|document.  To make room,|close any document that|you do not need.][  OK  ]"
        }
        // AL05ERR
        ErrAlert::Exists => {
            b"[1][An item with this name|already exists in the|directory, or this item|is set to Read Only status.][  OK  ]"
        }
        // AL15ERR
        ErrAlert::NoDrive => b"[1][The drive you specified|does not exist.][Cancel]",
        // AL08ERR
        ErrAlert::NoMem => {
            b"[1][There is not enough memory|in your computer for the|application you just tried|to run.][  OK  ]"
        }
        // ALXXERR
        ErrAlert::Generic => b"[3][TOS error #%u.][Cancel]",
    }
}
