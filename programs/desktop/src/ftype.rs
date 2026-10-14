//! `desktop::ftype` — filename → launch-behaviour classification (ADR-018).
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
//! | `.NTP` | [`Flavor::Params`]   | `.TTP`          | VT52 text screen, prompt for a command tail, then run |
//! | `.NAC` | [`Flavor::Accessory`]| `.ACC`          | **not** launched — loaded at AES start |
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
    /// (`fbcon::clear_and_home` before, desktop restore after).
    Console,
    /// `.NTP` — native **console** program that takes a command tail. The tail
    /// is asked for before the launch, in EmuTOS's own `ADOPENAP` box
    /// ([`super::inf::opn_appl`], `deskinf.c:1250-1271`) — this is the flavour
    /// that answers `true` to [`super::main`]'s `isparm`, i.e. the one bit
    /// `do_aopen` tests before it puts the box up (`desksupp.c:877,978-985`).
    ///
    /// The classic analogue is `.TTP`, **not** `.GTP` (user decision
    /// 2026-09-26, DESK44): the ADR-018 middle letter names the launcher role,
    /// and `.NCP`/`.NTP` are the console pair exactly as `.TOS`/`.TTP` are.
    /// EmuTOS separates the two by one flag — `#P *.TTP` is `AF_ISPARM`,
    /// `#Y *.GTP` is `AF_ISPARM | AF_ISCRYS` (`deskapp.c:181-188`, parsed at
    /// `:310-322`) — so this choice is what [`super::main`]'s `isgraf` answers
    /// and what puts a `.NTP` on the alpha screen (`gemshlib.c:513-523`).
    Params,
    /// `.NAC` — native **desk accessory** (ADR-018, ACC05). Unlike the three
    /// above it is *not* launched on demand: the AES loads it once at session
    /// start from `C:\ACC\` or the boot drive's root and it runs for the whole
    /// session (`crate::accs`, `docs/api/aes/accessories.md` §1).
    ///
    /// A double-click on a `.NAC` therefore does **nothing** — EmuTOS's
    /// desktop has no accessory launch path either: an `.ACC` is only ever
    /// auto-loaded by `count_accs`/`load_accs` (`aes/geminit.c:264-315`), and
    /// its own desktop `open_item` has no case for the extension
    /// (`desk/deskact.c`/`deskins.c` classify `.PRG`/`.APP`/`.TOS`/`.TTP`
    /// only). Running a second copy of an accessory as an ordinary program
    /// would give it a second `menu_register` slot and a second message loop
    /// with no way out, so the desktop reports it instead of launching it.
    Accessory,
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
        b".NAC" => FileType::Native(Flavor::Accessory),
        b".PRG" | b".APP" | b".TOS" | b".TTP" | b".GTP" => FileType::M68k,
        _ => FileType::Doc,
    }
}
