//! GEMDOS calls on a path with **no final component** — `"C:\"`, `"C:\SUB\"`,
//! `""` — answered the way EmuTOS answers them.
//!
//! **Where this comes from.** `tosdump/reference/TERADESK-GDTRACE-r1.BIN`, a
//! GEMDOS trace taken on a Pi 4 while TeraDesk opened `C:`, went into a folder
//! and closed the window, records
//!
//! ```text
//!  33  3522.899  606us  4e Fsfirst  "C:\" attr=0x37(RO|HID|SYS|DIR|ARC) -> -34 EPTHNF
//! ```
//!
//! three times, once per directory-window operation, each time with a DTA on
//! TeraDesk's stack that is restored immediately afterwards. That is
//! `x_attr()` asking for the attributes of the window's own path
//! (`sdk/apps/teradesk/xfilesys.c:1164-1176`: `Fgetdta` / `Fsetdta(&dta)` /
//! `Fsfirst(name, FA_ANY)` / `Fsetdta(olddta)`), and TeraDesk passes the
//! result through unchanged (`library/utility/xerror.c:26-32`), where `-33` is
//! its `ENOENT` and `-34` its `ENOTDIR` — two different answers, and it
//! branches on the difference elsewhere (`copy.c:531,582`, `dir.c:803`,
//! `error.c:77,82`).
//!
//! **What EmuTOS returns.** `EFILNF`. `ixsfirst` (`bdos/fsdir.c:452-474`) has
//! two failure paths and takes the second one:
//!
//! * `findit(name, &s, 0)` **succeeds**. `dcrack` eats `C:`, sees the
//!   `PATHSEP`, takes the root DND and steps past the separator
//!   (`fsdir.c:1717-1730`), leaving `n` on the terminating NUL. `getpath("")`
//!   then returns 0 — the remainder holds no separator and `dflag` is 0
//!   (`fsdir.c:1782-1793`) — which breaks `findit`'s loop with a valid DND
//!   and `*sp = ""` (`fsdir.c:1430,1495`).
//! * `scan(dn, "", att, &pos)` then **fails**: `builds("")` fills all eleven
//!   name bytes with blanks (`fsdir.c:737-795`), a name no directory entry can
//!   carry, so `match` never fires and `scan` returns NULL
//!   (`fsdir.c:1536,1556-1595`). `ixsfirst` answers `EFILNF`
//!   (`fsdir.c:469-470`).
//!
//! NewTOS took the *first* path instead: `resolve_parent` treated "no final
//! component" as a resolution failure and returned `EPTHNF`.
//!
//! `ixcreat` is the one function that does answer `EPTHNF` here, and it does
//! so with its own guard after `findit` succeeded
//! (`bdos/fsopnclo.c:117-118`) — `Dcreate` inherits it through `xmkdir`
//! (`bdos/fsdir.c:211`).

mod fatvol;

use fatvol::*;
// ── 1. a path with no final component ─────────────────────────────────────

#[test]
fn path_without_a_final_component_is_file_not_found_not_path_not_found() {
    let (_g, _h) = fs();

    let mut dta = Dta::new();
    assert_eq!(gemdos1(FSETDTA, dta.ptr()), E_OK);

    // The exact call the hardware trace records, three times.
    // EmuTOS: `findit` succeeds (root DND, `*sp == ""`), `scan` finds nothing
    // -> `EFILNF` (`bdos/fsdir.c:462-470`).
    let root = cpath("C:\\");
    assert_eq!(
        gemdos2(FSFIRST, root.as_ptr() as u64, FA_ANY),
        EFILNF,
        "Fsfirst(\"C:\\\") must answer EFILNF (ENOENT to the caller), not EPTHNF (ENOTDIR)"
    );

    // Same for a sub-directory path that ends in a separator: `findit` walks
    // *into* SUB and leaves an empty name behind, so the answer is again the
    // `scan` failure and not a path failure.
    let subslash = cpath("C:\\SUB\\");
    assert_eq!(gemdos2(FSFIRST, subslash.as_ptr() as u64, FA_ANY), EFILNF);

    // ... and for the empty path, where `dcrack` yields the current directory
    // rather than the root (`bdos/fsdir.c:1731-1736`).
    let map = gemdos1(DSETDRV, 2);
    assert_ne!(map & 0b100, 0, "C: must be in the drive map, got {map:#x}");
    let empty = cpath("");
    assert_eq!(gemdos2(FSFIRST, empty.as_ptr() as u64, FA_ANY), EFILNF);

    // A genuinely missing *directory* still reports EPTHNF — the first of
    // `ixsfirst`'s two failure paths (`findit` NULL, `fsdir.c:463`).
    let nodir = cpath("C:\\NOSUCH\\X.TXT");
    assert_eq!(gemdos2(FSFIRST, nodir.as_ptr() as u64, FA_ANY), EPTHNF);

    // And the wildcard search TeraDesk issues next still works, so the walk
    // above did not break ordinary resolution.
    let star = cpath("C:\\*.*");
    assert_eq!(gemdos2(FSFIRST, star.as_ptr() as u64, FA_ANY), E_OK);
    assert_eq!(dta.name(), "SUB");
    assert_eq!(gemdos1(FSNEXT, 0), E_OK);
    assert_eq!(dta.name(), "HELLO.TXT");
    assert_eq!(gemdos1(FSNEXT, 0), ENMFIL);

    let substar = cpath("C:\\SUB\\*.*");
    assert_eq!(gemdos2(FSFIRST, substar.as_ptr() as u64, FA_ANY), E_OK);
    assert_eq!(dta.name(), ".");
}

#[test]
fn the_other_path_calls_answer_the_empty_name_the_way_emutos_does() {
    let (_g, _h) = fs();

    // `ixopen` / `ixunlink`: `findit` succeeds, `scan` fails -> EFILNF
    // (`bdos/fsopnclo.c:246-257`, `:596-605`).
    let root = cpath("C:\\");
    assert_eq!(gemdos2(FOPEN, root.as_ptr() as u64, 0), EFILNF);
    assert_eq!(gemdos1(FDELETE, root.as_ptr() as u64), EFILNF);

    let subslash = cpath("C:\\SUB\\");
    assert_eq!(gemdos2(FOPEN, subslash.as_ptr() as u64, 0), EFILNF);

    // `ixcreat` is the exception: it guards the empty name itself and answers
    // EPTHNF (`bdos/fsopnclo.c:117-118`). `xmkdir` goes through `ixcreat`
    // (`bdos/fsdir.c:211`), so `Dcreate` answers the same.
    assert_eq!(gemdos2(FCREATE, root.as_ptr() as u64, 0), EPTHNF);
    assert_eq!(gemdos2(FCREATE, subslash.as_ptr() as u64, 0), EPTHNF);
    assert_eq!(gemdos1(DCREATE, root.as_ptr() as u64), EPTHNF);
    // Same guard, second half: a final component starting with a dot.
    let dot = cpath("C:\\.");
    assert_eq!(gemdos2(FCREATE, dot.as_ptr() as u64, 0), EPTHNF);
    let dotdot = cpath("C:\\SUB\\..");
    assert_eq!(gemdos1(DCREATE, dotdot.as_ptr() as u64), EPTHNF);

    // Ordinary opens are untouched.
    let hello = cpath("C:\\HELLO.TXT");
    let h = gemdos2(FOPEN, hello.as_ptr() as u64, 0);
    assert!(h >= 6, "Fopen(C:\\HELLO.TXT) -> {h}");
    assert_eq!(gemdos1(FCLOSE, h as u64), E_OK);

    let inner = cpath("C:\\SUB\\INNER.TXT");
    let h = gemdos2(FOPEN, inner.as_ptr() as u64, 0);
    assert!(h >= 6, "Fopen(C:\\SUB\\INNER.TXT) -> {h}");
    assert_eq!(gemdos1(FCLOSE, h as u64), E_OK);
}

