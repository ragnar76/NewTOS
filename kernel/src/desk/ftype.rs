//! `desk::ftype` — filename → launch-behaviour classification (ADR-018).
//!
//! Native NewTOS programs are native AArch64 **ELF** binaries; the classic
//! Atari suffixes (`.PRG`/`.APP`/`.TOS`/`.TTP`/`.GTP`) stay reserved for the
//! original **m68k** binaries (run only under the future m68k emulator). To
//! keep the two architectures distinguishable by name, native ELF programs use
//! a new `.N_P` family whose middle letter encodes the launcher role:
//!
//! | Ext    | Flavour          | m68k analog     | Launcher behaviour                    |
//! |--------|------------------|-----------------|---------------------------------------|
//! | `.NAP` | [`Flavor::App`]      | `.PRG` / `.APP` | run; the app draws its own AES UI     |
//! | `.NCP` | [`Flavor::Console`]  | `.TOS`          | VT52 text screen, run, restore desktop|
//! | `.NTP` | [`Flavor::Params`]   | `.TTP` / `.GTP` | prompt for a command tail, then run   |
//!
//! The extension is a desktop/shell convention only — it is **not** part of the
//! TOS syscall ABI, so this classification touches no compatibility contract.
//! This mirrors EmuTOS `deskins.c` deciding a program's type from its name, but
//! for the native family rather than the classic one (the dead
//! `desktop.rs::name_is_program` is a reference only — not resurrected).

/// The launch flavour of a native NewTOS (`.N_P`) program — selects the
/// launcher behaviour once the file is known to be a native AArch64 ELF.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavor {
    /// `.NAP` — native GEM application; it draws its own AES UI over the
    /// desktop, so the launcher runs it with no special screen setup.
    App,
    /// `.NCP` — native console program; runs on a VT52 text screen
    /// (`fbcon::activate` before, desktop restore after).
    Console,
    /// `.NTP` — native program that takes a command tail (prompted for before
    /// launch; the tail dialog is a Stage-3 follow-on).
    Params,
}

/// What a filename's extension says about how the desktop should open it
/// (ADR-018).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileType {
    /// A native NewTOS AArch64 ELF program (`.NAP`/`.NCP`/`.NTP`).
    Native(Flavor),
    /// A classic m68k Atari binary (`.PRG`/`.APP`/`.TOS`/`.TTP`/`.GTP`) —
    /// runnable only under the (not-yet-available) m68k emulator.
    M68k,
    /// Anything else — treated as a document (file-type association to an
    /// application is future work).
    Doc,
}

/// Classify `name` (a bare filename, e.g. `HELLO.NCP`) by its 3-character
/// extension, **case-insensitively**. Matches the last four bytes (`.XYZ`);
/// a name shorter than four bytes, or one whose extension is unrecognised, is
/// a [`FileType::Doc`].
pub fn classify(name: &[u8]) -> FileType {
    let n = name.len();
    if n < 4 {
        return FileType::Doc;
    }
    // Uppercase the trailing 4 bytes (dot + 3 chars) for a case-blind compare.
    let mut ext = [0u8; 4];
    for (dst, &b) in ext.iter_mut().zip(&name[n - 4..]) {
        *dst = b.to_ascii_uppercase();
    }
    match &ext {
        b".NAP" => FileType::Native(Flavor::App),
        b".NCP" => FileType::Native(Flavor::Console),
        b".NTP" => FileType::Native(Flavor::Params),
        b".PRG" | b".APP" | b".TOS" | b".TTP" | b".GTP" => FileType::M68k,
        _ => FileType::Doc,
    }
}
