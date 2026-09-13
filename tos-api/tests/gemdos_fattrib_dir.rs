//! `Fattrib` (GEMDOS 0x43) GET on a directory written **with a trailing
//! separator** — the spelling every cflib application passes.
//!
//! **Where this comes from.** cflib's `make_normalpath` appends a backslash to
//! the path before it probes it (`sdk/cflib/cflib-21/fsnorm.c:85-92`), and
//! `path_exists` then asks `Fattrib(pathname, 0, 0) >= 0 && (attr & FA_DIR)`
//! (`sdk/cflib/cflib-21/fsexists.c:41`). An EL0 syscall trace of QED on QEMU
//! `raspi4b` records the two calls this breaks:
//!
//! ```text
//! GEMDOS fn=0x43 Fattrib  path="C:\CLIPBRD\"   -> -33     (clipbrd.c:281)
//! GEMDOS fn=0x43 Fattrib  path="C:\Apps\QED\"  -> -33     (appinit.c:104)
//! ```
//!
//! The first sends QED into `Dcreate("C:\CLIPBRD")` on the folder it created
//! itself in the previous run — `EACCDN`, and the NOSCRAP alert
//! (`sdk/apps/qed/src/clipbrd.c:288-294`). The second makes cflib's `app_init`
//! throw away `gl_appdir` and fall back to the current directory
//! (`sdk/cflib/cflib-21/appinit.c:104-107`).
//!
//! **Why this is the completion of a decision, not a new one.**
//! `docs/COMPATIBILITY.md` already sanctions `Fattrib` GET on folders, citing
//! this exact caller; the contract only overlooked that the path cflib hands
//! over ends in a separator. The reference for the sanctioned behaviour is
//! FreeMiNT: `sys_f_attrib` returns `xattr.attr` for files *and* folders
//! (`references/freemint/sys/dosdir.c:1001`), `relpath2cookie` peels an empty
//! last component (`sys/filesys.c:986-1010`) and `fatfs_lookup` resolves the
//! empty name **as `"."`** (`sys/fatfs.c:5753-5764`).
//!
//! **What deliberately does not change.** The empty final component is
//! EmuTOS-faithful for every other caller (`Fsfirst`/`Fopen` → `EFILNF` via
//! `scan("")`, `Dcreate` → `EPTHNF` via `ixcreat`'s guard; see
//! `gemdos_fs_paths.rs`), and it stays that way. Inside `Fattrib` GET only the
//! *directory* answer is added: a trailing separator on a **file**
//! (`ENOTDIR`/`EPTHNF` — FreeMiNT agrees, `sys/filesys.c:970-976`), on a
//! **missing** name and on the bare **drive root** all keep today's code.

mod fatvol;

use fatvol::*;

/// `Fattrib` (0x43) with all three arguments.
fn fattrib(path: &[u8], wrt: u64, mode: u64) -> i32 {
    call(0x43, [path.as_ptr() as u64, wrt, mode, 0, 0, 0, 0, 0])
}

const FA_SUBDIR: i32 = 0x10;
const EACCDN: i32 = -36;

// ── 1. the case QED and every cflib app hits ──────────────────────────────

#[test]
fn fattrib_get_resolves_a_directory_written_with_a_trailing_separator() {
    let (_g, _h) = fs();

    // The spelling cflib produces. Must answer the folder's attribute byte
    // with FA_SUBDIR set, exactly as the form without the separator does.
    let with_sep = cpath("C:\\SUB\\");
    let without = cpath("C:\\SUB");
    let a = fattrib(&with_sep, 0, 0);
    let b = fattrib(&without, 0, 0);
    assert!(
        a >= 0 && (a & FA_SUBDIR) != 0,
        "Fattrib(\"C:\\SUB\\\", 0, 0) -> {a}, expected a folder attribute >= 0 with 0x10 set \
         (cflib path_exists probes exactly this spelling)"
    );
    assert_eq!(
        a, b,
        "both spellings must resolve to the same directory entry"
    );

    // A forward slash is a separator too (`bdos/fsdir.c` `PATHSEP` handling;
    // our `resolve_parent` splits on both).
    let fwd = cpath("C:/SUB/");
    assert_eq!(fattrib(&fwd, 0, 0), b);

    // More than one trailing separator collapses the same way.
    let twice = cpath("C:\\SUB\\\\");
    assert_eq!(fattrib(&twice, 0, 0), b);

    // Relative to the current directory (the root here) — cflib's `gl_appdir`
    // is not always drive-qualified.
    let map = gemdos1(DSETDRV, 2);
    assert_ne!(map & 0b100, 0, "C: must be in the drive map, got {map:#x}");
    let rel = cpath("SUB\\");
    assert_eq!(fattrib(&rel, 0, 0), b);
}

// ── 2. what the fix must NOT change ───────────────────────────────────────

#[test]
fn only_the_directory_answer_is_added_every_other_spelling_is_untouched() {
    let (_g, _h) = fs();

    // A **file** with a trailing separator stays "not a directory". EmuTOS
    // walks into HELLO.TXT, finds it is no directory and answers `EPTHNF`
    // (`bdos/fsdir.c:1407-1497`); FreeMiNT answers ENOTDIR, which *is* -34
    // (`sys/mint/errno.h:277`). Same answer, so nothing to lift here.
    let file_sep = cpath("C:\\HELLO.TXT\\");
    assert_eq!(fattrib(&file_sep, 0, 0), EPTHNF);
    // ... and the file itself still reports its own attribute byte.
    let file = cpath("C:\\HELLO.TXT");
    let fa = fattrib(&file, 0, 0);
    assert!(fa >= 0 && (fa & FA_SUBDIR) == 0, "Fattrib(file) -> {fa}");

    // A **missing** name with a trailing separator keeps EmuTOS' `EPTHNF`
    // (the intermediate component is what fails). FreeMiNT would say ENOENT
    // here; we do not follow it — that divergence is not part of this fix.
    let missing = cpath("C:\\NOSUCH\\");
    assert_eq!(fattrib(&missing, 0, 0), EPTHNF);

    // The bare **drive root** keeps `EFILNF`, as EmuTOS and TOS 3.06 answer
    // it. Stripping the separator would leave no component at all, and there
    // is no directory entry whose attribute byte could be read; FreeMiNT
    // synthesizes FA_DIR here, we deliberately do not. Pinned so the decision
    // cannot be made silently by a later edit.
    let root = cpath("C:\\");
    assert_eq!(
        fattrib(&root, 0, 0),
        EFILNF,
        "Fattrib(\"C:\\\", 0, 0) stays EFILNF — deciding otherwise is a separate decision"
    );
    let root_fwd = cpath("C:/");
    assert_eq!(fattrib(&root_fwd, 0, 0), EFILNF);

    // The **SET** branch is not touched: a trailing separator there still
    // finds nothing (eleven blanks) and reports `EFILNF`, and the folder
    // itself still refuses to be written (`EACCDN`, the #38 corruption
    // guard).
    let with_sep = cpath("C:\\SUB\\");
    assert_eq!(fattrib(&with_sep, 1, 0), EFILNF);
    let without = cpath("C:\\SUB");
    assert_eq!(fattrib(&without, 1, 0), EACCDN);
}

// ── 3. the neighbours of the empty final component stay EmuTOS-faithful ───

#[test]
fn the_other_calls_on_a_path_ending_in_a_separator_are_unchanged() {
    let (_g, _h) = fs();

    // Measured identical on EmuTOS 1.4 and Atari TOS 3.06 (Hatari, real FAT12
    // floppy): only `Fattrib` GET is lifted, these three are not.
    let with_sep = cpath("C:\\SUB\\");

    let mut dta = Dta::new();
    assert_eq!(gemdos1(FSETDTA, dta.ptr()), E_OK);
    // `ixsfirst`: `findit` succeeds, `scan("")` cannot match eleven blanks
    // (`bdos/fsdir.c:462-470`).
    assert_eq!(gemdos2(FSFIRST, with_sep.as_ptr() as u64, FA_ANY), EFILNF);
    // `ixopen`: same shape (`bdos/fsopnclo.c:246-257`).
    assert_eq!(gemdos2(FOPEN, with_sep.as_ptr() as u64, 0), EFILNF);
    // `ixcreat`'s own empty-name guard, inherited by `xmkdir`
    // (`bdos/fsopnclo.c:117-118`, `bdos/fsdir.c:211`).
    assert_eq!(gemdos1(DCREATE, with_sep.as_ptr() as u64), EPTHNF);
    // And `Dcreate` on the existing folder keeps `EACCDN` — identical on
    // NewTOS, EmuTOS and TOS 3.06, so QED's second call is not the defect.
    let without = cpath("C:\\SUB");
    assert_eq!(gemdos1(DCREATE, without.as_ptr() as u64), EACCDN);

    // The directory listing that follows still works, so resolution as such
    // is intact.
    let star = cpath("C:\\SUB\\*.*");
    assert_eq!(gemdos2(FSFIRST, star.as_ptr() as u64, FA_ANY), E_OK);
    assert_eq!(dta.name(), ".");
}
