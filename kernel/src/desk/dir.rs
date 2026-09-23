//! `desk::dir` — the desktop's file-operation engine (`d_doop`/`dir_op`), the
//! drag-and-drop dispatch tree (`fun_drag`), and the menu/drag action glue
//! (`fun_op`/`fun_del`/`fun_selectall`/`fun_mkdir`/`fun_rename`), per
//! `docs/api/desktop/deskdir.md` + `deskfun.md`.
//!
//! Reimplements EmuTOS `deskdir.c` (the recursive copy/move/delete/rename
//! subtree walker) and the `deskfun.c` action layer in idiomatic `#![no_std]`
//! Rust on top of the `tos_api::bdos::fs` GEMDOS calls.
//!
//! ## What is contract, what is free (`docs/COMPATIBILITY.md`)
//!
//! The **observable filesystem effect** — which GEMDOS calls happen, in what
//! order, with what net result — is the contract (`deskdir.md`). The path
//! representation, the recursion strategy and the *interactive* error/overwrite
//! UI are explicitly free to replace. Accordingly:
//!
//! * EmuTOS mutates a single fixed on-stack path buffer per level
//!   (`add_path`/`sub_path`/`add_fname`); NewTOS uses per-level owned [`String`]s
//!   — the recursion-depth and path-length limits stay observably the same
//!   ([`MAX_LEVEL`]).
//! * EmuTOS's per-item skip/retry/abort and per-file overwrite dialogs
//!   (`d_dofdel`'s alert loop, `output_fname`) are replaced by the NewTOS
//!   default: overwrite existing files (GEMDOS `Fcreate` truncates), merge into
//!   an existing destination folder, and best-effort skip on a per-item error
//!   (surfaced to the caller as `false`, which shows one summary [`fun_alert`]).
//!   The *whole-disk-delete* and *delete/copy confirmation* prompts are
//!   preserved at the action layer ([`fun_del`]/[`fun_drag`]).
//!
//! The pure engine ([`d_doop`]/[`dir_op`]) issues **no dialogs**, so it is
//! directly driveable for the Task-#9 backend verification ([`selftest`]).

use alloc::string::String;
use alloc::vec::Vec;

use tos_api::bdos::fs::dirops::{dcreate, ddelete};
use tos_api::bdos::fs::file::{fclose, fcreate, fdatime, fdelete, fopen, fread, frename, fwrite};
use tos_api::bdos::fs::find;
use tos_api::trap::TrapArgs;
use tos_types::error::E_OK;

use super::global::{self, DeskState, DROOT, NIL};
use super::model::{AType, FNode, FA_SUBDIR};
use super::win::{DESKWH, OB_SELECTED};

// ── GEMDOS function ids ─────────────────────────────────────────────────────
const TRAP_GEMDOS: u16 = 1;
const FSETDTA: u16 = 0x1A;
const DCREATE: u16 = 0x39;
const DDELETE: u16 = 0x3A;
const FCREATE: u16 = 0x3C;
const FOPEN: u16 = 0x3D;
const FCLOSE: u16 = 0x3E;
const FREAD: u16 = 0x3F;
const FWRITE: u16 = 0x40;
const FDELETE: u16 = 0x41;
const FSFIRST: u16 = 0x4E;
const FSNEXT: u16 = 0x4F;
const FRENAME: u16 = 0x56;
const FDATIME: u16 = 0x57;

/// Attribute mask for the recursive scan (EmuTOS `ALLFILES` =
/// `FA_SUBDIR|FA_SYSTEM|FA_HIDDEN`): a subtree operation must see hidden and
/// system files too, unlike the display scan's `DISPATTR`.
const ALLFILES: u8 = 0x16;

/// Recursion-depth guard (EmuTOS `MAX_LEVEL`, `deskconf.h:44`). A folder tree
/// deeper than this aborts that subtree (the EmuTOS `STFO8DEE` alert path).
pub const MAX_LEVEL: i16 = 12;

/// Copy buffer size (heap). EmuTOS sizes this to available ST-RAM up to
/// `MAX_CLUS_SIZE` (32 KiB); NewTOS uses a fixed 32 KiB heap buffer — the size
/// only affects copy throughput, not the result.
const COPY_BUF: usize = 32 * 1024;

/// `MODE_CTRL` (`aesdefs.h`) — the Ctrl modifier toggles Copy↔Move on a drag.
const MODE_CTRL: u16 = 0x04;

/// A file/folder operation (EmuTOS `OP_*` + the internal `OP_RENAME`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    /// `OP_COUNT` — tally files/dirs/size without mutating anything.
    Count,
    /// `OP_DELETE` — delete files and (recursively) folders.
    Delete,
    /// `OP_COPY` — copy files/folders to a destination directory.
    Copy,
    /// `OP_MOVE` — copy then delete the source (cross-drive) or `Frename`
    /// (same drive, via the [`dir_op`] move→rename optimisation).
    Move,
    /// `OP_RENAME` — same-drive move via `Frename` (internal, not user-visible).
    Rename,
}

/// Running totals for a directory operation (EmuTOS `DIRCOUNT`).
#[derive(Clone, Copy, Default, Debug)]
pub struct DirCount {
    /// Number of files.
    pub files: u32,
    /// Number of folders.
    pub dirs: u32,
    /// Sum of file sizes in bytes.
    pub size: u64,
}

/// One selected source entry the top-level [`dir_op`] iterates (a resolved
/// snapshot of an [`FNode`], so the engine does not hold the desktop lock while
/// it does I/O).
#[derive(Clone)]
pub struct SelEntry {
    /// `NAME.EXT\0` FAT display name.
    pub name: [u8; 14],
    /// Whether this entry is a sub-directory.
    pub is_dir: bool,
    /// File length (0 for a folder).
    pub size: u32,
    /// Packed FAT time.
    pub time: u16,
    /// Packed FAT date.
    pub date: u16,
}

impl SelEntry {
    /// From a model [`FNode`].
    pub fn from_fnode(f: &FNode) -> Self {
        Self {
            name: f.f_name,
            is_dir: f.is_dir(),
            size: f.f_size,
            time: f.f_time,
            date: f.f_date,
        }
    }

    /// The display name as a `&str` (up to the NUL).
    fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

// ── GEMDOS wrappers ─────────────────────────────────────────────────────────

/// Build a native GEMDOS [`TrapArgs`].
fn gargs(func: u16, regs: [u64; 8]) -> TrapArgs {
    TrapArgs { regs, func: func as u64, svc_imm: TRAP_GEMDOS }
}

/// A NUL-terminated path buffer (`LEN_ZPATH`+ sized) built from `s`.
fn cpath(s: &str) -> [u8; 130] {
    let mut b = [0u8; 130];
    let src = s.as_bytes();
    let n = src.len().min(b.len() - 1);
    b[..n].copy_from_slice(&src[..n]);
    b
}

/// `Fdelete` — delete a file. Returns the GEMDOS result (0 = ok).
fn dos_delete(path: &str) -> i32 {
    let p = cpath(path);
    fdelete(&gargs(FDELETE, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]))
}

/// `Ddelete` — remove an (empty) directory.
fn dos_rmdir(path: &str) -> i32 {
    let p = cpath(path);
    ddelete(&gargs(DDELETE, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]))
}

/// `Dcreate` — create a directory.
fn dos_mkdir(path: &str) -> i32 {
    let p = cpath(path);
    dcreate(&gargs(DCREATE, [p.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]))
}

/// `Frename` — rename/move on the same drive. `x0` is the historical unused
/// word, `x1` = source, `x2` = destination.
fn dos_rename(old: &str, new: &str) -> i32 {
    let o = cpath(old);
    let n = cpath(new);
    frename(&gargs(FRENAME, [0, o.as_ptr() as u64, n.as_ptr() as u64, 0, 0, 0, 0, 0]))
}

/// `Fopen` — open a file for reading (`mode` 0 = read).
fn dos_open(path: &str, mode: u16) -> i32 {
    let p = cpath(path);
    fopen(&gargs(FOPEN, [p.as_ptr() as u64, mode as u64, 0, 0, 0, 0, 0, 0]))
}

/// `Fcreate` — create/truncate a file with attribute `attr`.
fn dos_create(path: &str, attr: u16) -> i32 {
    let p = cpath(path);
    fcreate(&gargs(FCREATE, [p.as_ptr() as u64, attr as u64, 0, 0, 0, 0, 0, 0]))
}

/// `Fread` — read up to `len` bytes into `buf`.
fn dos_read(handle: i16, len: i32, buf: *mut u8) -> i32 {
    fread(&gargs(FREAD, [handle as u64, len as u64, buf as u64, 0, 0, 0, 0, 0]))
}

/// `Fwrite` — write `len` bytes from `buf`.
fn dos_write(handle: i16, len: i32, buf: *const u8) -> i32 {
    fwrite(&gargs(FWRITE, [handle as u64, len as u64, buf as u64, 0, 0, 0, 0, 0]))
}

/// `Fclose` — close a handle.
fn dos_close(handle: i16) -> i32 {
    fclose(&gargs(FCLOSE, [handle as u64, 0, 0, 0, 0, 0, 0, 0]))
}

/// `Fdatime` (set) — stamp an open handle's time/date from the source entry.
fn dos_setdt(handle: i16, time: u16, date: u16) -> i32 {
    let t = time.to_le_bytes();
    let d = date.to_le_bytes();
    let buf = [t[0], t[1], d[0], d[1]];
    fdatime(&gargs(FDATIME, [buf.as_ptr() as u64, handle as u64, 1, 0, 0, 0, 0, 0]))
}

// ── path helpers (owned strings, no fixed-buffer mutation) ──────────────────

/// One raw directory entry from the recursive scan.
struct RawEntry {
    name: [u8; 14],
    attr: u8,
    size: u32,
    time: u16,
    date: u16,
}

impl RawEntry {
    fn is_dir(&self) -> bool {
        self.attr & FA_SUBDIR != 0
    }
    fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

/// Append `name` to a directory path that already ends with `\` (e.g.
/// `join_leaf("C:\\SUB\\", "F.TXT") == "C:\\SUB\\F.TXT"`).
fn join_leaf(dir: &str, name: &str) -> String {
    let mut s = String::from(dir);
    s.push_str(name);
    s
}

/// A leaf path → its directory form (`"C:\\SUB" -> "C:\\SUB\\"`).
fn as_dir(leaf: &str) -> String {
    let mut s = String::from(leaf);
    s.push('\\');
    s
}

/// A directory path ending with `\` → its leaf form (`"C:\\SUB\\" -> "C:\\SUB"`).
fn strip_dir(dir: &str) -> &str {
    dir.strip_suffix('\\').unwrap_or(dir)
}

/// Scan directory `dir` (must end with `\`) via GEMDOS `Fsfirst`/`Fsnext` with
/// the [`ALLFILES`] mask, collecting **every** entry (files, folders, hidden,
/// system) except the `.`/`..` pseudo-entries. Fully drains the search before
/// returning, so a recursive caller can safely start a fresh scan afterwards
/// (the current DTA is a single global — EmuTOS uses a per-level private DTA
/// for the same reason; collecting first is the clean equivalent).
fn scan_folder(dir: &str) -> Vec<RawEntry> {
    let mut spec = String::from(dir);
    spec.push_str("*.*");

    let mut dta = [0u8; 44];
    find::fsetdta(&gargs(FSETDTA, [dta.as_mut_ptr() as u64, 0, 0, 0, 0, 0, 0, 0]));

    let cp = cpath(&spec);
    let mut out = Vec::new();
    let mut rc = find::fsfirst(&gargs(FSFIRST, [cp.as_ptr() as u64, ALLFILES as u64, 0, 0, 0, 0, 0, 0]));
    while rc == E_OK {
        let mut name = [0u8; 14];
        name.copy_from_slice(&dta[30..44]);
        let is_dot = name[0] == b'.' && (name[1] == 0 || (name[1] == b'.' && name[2] == 0));
        if !is_dot {
            out.push(RawEntry {
                name,
                attr: dta[21],
                time: u16::from_le_bytes([dta[22], dta[23]]),
                date: u16::from_le_bytes([dta[24], dta[25]]),
                size: u32::from_le_bytes([dta[26], dta[27], dta[28], dta[29]]),
            });
        }
        rc = find::fsnext(&gargs(FSNEXT, [0; 8]));
    }
    out
}

/// Whether `src` is `dst` itself or a parent of it (EmuTOS `source_is_parent`):
/// copying/moving a folder into itself or a descendant. Both are fully-qualified
/// leaf paths (no wildcard).
fn source_is_parent(src: &str, dst: &str) -> bool {
    if !dst.starts_with(src) {
        return false;
    }
    // Equal (self) or the next byte is a separator (a descendant).
    dst.len() == src.len() || dst.as_bytes()[src.len()] == b'\\'
}

// ── per-item engine primitives ──────────────────────────────────────────────

/// `d_dofdel` — delete one file (EmuTOS `deskdir.c:237`). `true` on success.
fn d_dofdel(path: &str) -> bool {
    dos_delete(path) == E_OK
}

/// `d_dofoldel` — remove one now-empty folder (EmuTOS `deskdir.c:269`).
fn d_dofoldel(path: &str) -> bool {
    dos_rmdir(path) == E_OK
}

/// `d_dofileren`/`d_dofoldren` — rename one file/folder (EmuTOS
/// `deskdir.c:785`/`:831`). Same-drive move via `Frename`.
fn d_dofileren(old: &str, new: &str) -> bool {
    dos_rename(old, new) == E_OK
}

/// `d_dofcopy` — copy one file (EmuTOS `deskdir.c:394`): open source, create
/// destination (truncating any existing file — the NewTOS "always overwrite"
/// default), stream through `buf`, then stamp the destination's date/time from
/// the source. `true` on a complete copy.
fn d_dofcopy(src: &str, dst: &str, time: u16, date: u16, attr: u16, buf: &mut [u8]) -> bool {
    let srcfh = dos_open(src, 0);
    if srcfh < 0 {
        return false;
    }
    let srcfh = srcfh as i16;
    // Mask off the sub-directory bit — a file copy never carries FA_SUBDIR.
    let dstfh = dos_create(dst, attr & !(FA_SUBDIR as u16));
    if dstfh < 0 {
        dos_close(srcfh);
        return false;
    }
    let dstfh = dstfh as i16;

    let mut ok = true;
    loop {
        let n = dos_read(srcfh, buf.len() as i32, buf.as_mut_ptr());
        if n == 0 {
            dos_setdt(dstfh, time, date); // stamp target from source (deskdir.c:428)
            break;
        }
        if n < 0 {
            ok = false;
            break;
        }
        let w = dos_write(dstfh, n, buf.as_ptr());
        if w != n {
            ok = false; // short write = disk full (deskdir.c:467)
            break;
        }
    }

    dos_close(srcfh);
    dos_close(dstfh);
    if !ok {
        dos_delete(dst); // remove the partial destination (deskdir.c:497)
    }
    ok
}

// ── the recursive subtree walker ────────────────────────────────────────────

/// `d_doop` — process the entire contents of one folder recursively (EmuTOS
/// `deskdir.c:528`). `src_dir`/`dst_dir` end with `\`. Returns `true` if the
/// whole subtree completed without error (and without hitting [`MAX_LEVEL`]).
///
/// For `Copy`/`Move` a matching destination folder is created (merged if it
/// already exists); files are copied. For `Delete`/`Move` the folder is removed
/// once emptied (post-order, like EmuTOS). `Count` tallies files pre-order and
/// folders post-order (one `dirs++` per folder visited).
fn d_doop(level: i16, op: Op, src_dir: &str, dst_dir: &str, count: &mut DirCount, buf: &mut [u8]) -> bool {
    if level > MAX_LEVEL {
        return false; // STFO8DEE guard (deskdir.c:537)
    }

    let entries = scan_folder(src_dir);
    let mut ok = true;

    for e in &entries {
        let name = e.name_str();
        if e.is_dir() {
            let src_sub = join_leaf(src_dir, name);
            let src_sub_dir = as_dir(&src_sub);
            let dst_sub_dir = if matches!(op, Op::Copy | Op::Move) {
                let dst_sub = join_leaf(dst_dir, name);
                let _ = dos_mkdir(&dst_sub); // create or merge into existing
                as_dir(&dst_sub)
            } else {
                String::new()
            };
            if !d_doop(level + 1, op, &src_sub_dir, &dst_sub_dir, count, buf) {
                ok = false;
            }
        } else {
            let src_file = join_leaf(src_dir, name);
            match op {
                Op::Count => {
                    count.files += 1;
                    count.size += e.size as u64;
                }
                Op::Delete => {
                    if !d_dofdel(&src_file) {
                        ok = false;
                    }
                }
                Op::Copy | Op::Move => {
                    let dst_file = join_leaf(dst_dir, name);
                    let copied = d_dofcopy(&src_file, &dst_file, e.time, e.date, e.attr as u16, buf);
                    if op == Op::Move && copied {
                        if !d_dofdel(&src_file) {
                            ok = false;
                        }
                    } else if !copied {
                        ok = false;
                    }
                }
                Op::Rename => {} // rename never recurses (handled at dir_op level)
            }
        }
    }

    // End-of-folder handling (EmuTOS `deskdir.c:559-585`).
    match op {
        Op::Count => count.dirs += 1,
        // `&=` (not `&&`) so the folder is always removed, then the result
        // ANDed into `ok` — never short-circuit past the rmdir.
        Op::Delete | Op::Move => ok &= d_dofoldel(strip_dir(src_dir)),
        _ => {}
    }
    ok
}

// ── the top-level dispatch over the selected items ──────────────────────────

/// `dir_op` — perform `op` on the [`SelEntry`] set selected in `src_dir`
/// (EmuTOS `deskdir.c:871`). `src_dir`/`dst_dir` end with `\`. `dst_dir` is
/// `None` for `Delete`/`Count`. Returns `true` if every item completed.
///
/// The move→rename optimisation (`deskdir.c:885`): a `Move` whose source and
/// destination are on the **same drive letter** is silently a `Rename`
/// (`Frename`), never a copy+delete.
pub fn dir_op(
    op: Op,
    src_dir: &str,
    entries: &[SelEntry],
    dst_dir: Option<&str>,
    count: &mut DirCount,
    buf: &mut [u8],
) -> bool {
    let mut op = op;
    if op == Op::Move {
        if let Some(dst) = dst_dir {
            if src_dir.as_bytes().first() == dst.as_bytes().first() {
                op = Op::Rename;
            }
        }
    }
    if op == Op::Count {
        *count = DirCount::default();
    }

    let mut ok = true;
    for e in entries {
        let name = e.name_str();
        if e.is_dir {
            let src_sub = join_leaf(src_dir, name); // C:\SUB\CHILD
            match op {
                Op::Count => {
                    if !d_doop(0, Op::Count, &as_dir(&src_sub), "", count, buf) {
                        ok = false;
                    }
                }
                Op::Delete => {
                    if !d_doop(0, Op::Delete, &as_dir(&src_sub), "", count, buf) {
                        ok = false;
                    }
                }
                Op::Copy | Op::Move | Op::Rename => {
                    let dst = dst_dir.unwrap_or("");
                    let dst_sub = join_leaf(dst, name);
                    if source_is_parent(&src_sub, &dst_sub) {
                        ok = false; // STILLDIR — skip (deskdir.c:999)
                        continue;
                    }
                    if src_sub == dst_sub {
                        continue; // copy/move to self — no-op
                    }
                    let more = if op == Op::Rename {
                        d_dofileren(&src_sub, &dst_sub)
                    } else {
                        let _ = dos_mkdir(&dst_sub); // output_path (create/merge)
                        d_doop(0, op, &as_dir(&src_sub), &as_dir(&dst_sub), count, buf)
                    };
                    if !more {
                        ok = false;
                    }
                }
            }
        } else {
            let src_file = join_leaf(src_dir, name);
            match op {
                Op::Count => {
                    count.files += 1;
                    count.size += e.size as u64;
                }
                Op::Delete => {
                    if !d_dofdel(&src_file) {
                        ok = false;
                    }
                }
                Op::Copy | Op::Move | Op::Rename => {
                    let dst = dst_dir.unwrap_or("");
                    let dst_file = join_leaf(dst, name);
                    let more = if op == Op::Rename {
                        d_dofileren(&src_file, &dst_file)
                    } else {
                        let copied =
                            d_dofcopy(&src_file, &dst_file, e.time, e.date, e.attr(), buf);
                        if op == Op::Move && copied {
                            d_dofdel(&src_file)
                        } else {
                            copied
                        }
                    };
                    if !more {
                        ok = false;
                    }
                }
            }
        }
    }
    ok
}

impl SelEntry {
    /// The FAT attribute a file copy should carry (never `FA_SUBDIR`).
    fn attr(&self) -> u16 {
        if self.is_dir {
            FA_SUBDIR as u16
        } else {
            0
        }
    }
}

// ── fun_op / fun_del / fun_selectall (deskfun.c action layer) ───────────────

/// `fun_op` — the single choke point for Copy/Move/Delete from any caller
/// (EmuTOS `deskfun.c:782`). Runs an `OP_COUNT` pre-pass; a zero-item selection
/// is a silent no-op (`false`). Otherwise runs the real operation and returns
/// `true` (the operation was *attempted* — matching EmuTOS, per-item failures
/// are not reflected in the return value).
pub fn fun_op(op: Op, src_dir: &str, entries: &[SelEntry], dst_dir: Option<&str>) -> bool {
    let mut count = DirCount::default();
    let mut empty: [u8; 0] = [];
    if !dir_op(Op::Count, src_dir, entries, None, &mut count, &mut empty) {
        fun_alert(1, b"Operation not allowed.");
        return false;
    }
    if count.files == 0 && count.dirs == 0 {
        return false; // nothing selected/matched — silent (deskfun.c:796)
    }

    let mut buf: Vec<u8> = Vec::new();
    if matches!(op, Op::Copy | Op::Move) {
        buf.resize(COPY_BUF, 0);
    }
    let all_ok = dir_op(op, src_dir, entries, dst_dir, &mut count, &mut buf);
    if !all_ok {
        fun_alert(1, b"Some items could not be processed.");
    }
    true
}

/// `fun_del` — the Delete menu item / Delete-key handler (EmuTOS
/// `deskfun.c:1376`), window case: delete the current window's selection after
/// a confirmation alert. The desktop-icon case (whole-drive delete + the
/// Remove→`MN_SELECTED` redirect) is [`fun_del_desktop`].
///
/// Returns `true` if the delete ran (so the caller repaints/rebuilds).
pub fn fun_del() -> bool {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return fun_del_desktop();
    }
    let Some(src_dir) = window_dir(cwin) else {
        return false;
    };
    let entries = window_selected(cwin);
    if entries.is_empty() {
        return false;
    }
    // Confirm (EmuTOS gates this on G.g_cdelepref, default on).
    if !fun_confirm(b"Delete the selected item(s)?") {
        return false;
    }
    let done = fun_op(Op::Delete, &src_dir, &entries, None);
    if done {
        rebuild_window(cwin);
    }
    done
}

/// `fun_del`, desktop case (EmuTOS `deskfun.c:1391-1420`): each selected disk
/// icon dropped on Delete would wipe that whole drive — so it goes through
/// [`wants_to_delete_files`], whose "Remove" answer redirects (via a synthetic
/// `MN_SELECTED`) to the Options→Remove-desktop-icon handler instead of
/// touching the filesystem.
fn fun_del_desktop() -> bool {
    match wants_to_delete_files() {
        DeleteChoice::Cancel => false,
        DeleteChoice::Remove => {
            // Redirect to Options → Remove desktop icon (deskfun.c:1443). The
            // icon-removal handler is deskins (Task #14); the redirect itself is
            // implemented here so the wiring is faithful the moment it lands.
            super::main::fun_msg_mn_selected(
                super::menu::Title::Options,
                super::menu::Item::RemoveIcon,
            );
            false
        }
        DeleteChoice::Delete => {
            // Whole-disk / desktop-icon delete: gather each selected disk icon's
            // drive root and delete its full contents. Preserves the extra
            // whole-disk confirmation (already answered "Delete" above).
            let targets = desktop_selected_disks();
            let mut any = false;
            for root in targets {
                let entries = root_entries(&root);
                if !entries.is_empty() && fun_op(Op::Delete, &root, &entries, None) {
                    any = true;
                }
            }
            if any {
                super::redraw::redraw_desktop();
            }
            any
        }
    }
}

/// `wants_to_delete_files` — the three-choice delete alert (EmuTOS
/// `deskfun.c:1433`, `STRMVDEL`): Cancel / Remove (icon only) / Delete (files).
/// `form_alert` returns the 1-based button, left→right.
fn wants_to_delete_files() -> DeleteChoice {
    let text = b"[2][Remove the desktop icon,|or delete its files?][ Cancel | Remove | Delete ]\0";
    match super::form_alert(1, text.as_ptr()) {
        2 => DeleteChoice::Remove,
        3 => DeleteChoice::Delete,
        _ => DeleteChoice::Cancel,
    }
}

/// The answer to [`wants_to_delete_files`].
enum DeleteChoice {
    /// Do nothing.
    Cancel,
    /// Remove the desktop icon only (redirect to Options→Remove).
    Remove,
    /// Delete the underlying files.
    Delete,
}

/// Create directory `path` (`Dcreate`); `true` on success. The single-level
/// creation used by File → New folder ([`super::main`]).
pub fn mkdir(path: &str) -> bool {
    dos_mkdir(path) == E_OK
}

/// Rename/move `old` → `new` on the same drive (`Frename`); `true` on success.
/// The File → Rename action ([`super::main`]).
pub fn rename(old: &str, new: &str) -> bool {
    dos_rename(old, new) == E_OK
}

/// `fun_selectall` — select every item in the current window (EmuTOS
/// `deskfun.c:607`). Returns `true` if the current root is a real window (the
/// desktop has no "select all").
///
/// EmuTOS marks every `FNODE`/object `SELECTED` and sends a plain `WM_REDRAW`
/// — it does **not** re-scan the directory (a rescan would reset `f_selected`).
/// So this repaints the existing item objects (already flagged by
/// [`DeskState::select_all`]) rather than calling [`rebuild_window`].
pub fn fun_selectall() -> bool {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return false;
    }
    global::with(|g| g.select_all(cwin));
    // `win_sinfo(pw, TRUE)` before the redraw (`deskfun.c:627-631`): the info
    // line now counts the whole selection.
    super::main::win_sinfo(cwin, true);
    super::redraw::redraw_desktop();
    true
}

/// `fun_alert` — a one-line alert with a single OK button (EmuTOS
/// `deskfun.c:75`). `default` is the default button (1). Returns the clicked
/// button.
pub fn fun_alert(default: i16, msg: &[u8]) -> i16 {
    let mut text = Vec::with_capacity(msg.len() + 16);
    text.extend_from_slice(b"[1][");
    text.extend_from_slice(msg);
    text.extend_from_slice(b"][ OK ]\0");
    super::form_alert(default, text.as_ptr())
}

/// A yes/no confirmation alert; `true` if the user chose the affirmative
/// (second) button.
fn fun_confirm(msg: &[u8]) -> bool {
    let mut text = Vec::with_capacity(msg.len() + 24);
    text.extend_from_slice(b"[2][");
    text.extend_from_slice(msg);
    text.extend_from_slice(b"][ Cancel | OK ]\0");
    super::form_alert(2, text.as_ptr()) == 2
}

// ── the drag-and-drop dispatch tree (deskfun.c fun_drag) ────────────────────

/// `fun_drag` — dispatch a completed drag gesture (EmuTOS `deskfun.c:1302`).
/// `wh` = source window handle ([`DESKWH`] for the desktop), `dest_wh` =
/// destination window handle (`0`/[`DESKWH`] if dropped on the desktop),
/// `dobj` = destination object index, `keystate` = modifiers at drop.
/// Returns `exit_desktop` (always `false` in Task #9 — launching a dropped
/// program is Task #14).
pub fn fun_drag(wh: i16, dest_wh: i16, dobj: i16, keystate: u16) -> bool {
    if wh != DESKWH {
        // Dragging FROM a window.
        if dest_wh != DESKWH && dest_wh != NIL {
            fun_win2win(wh, dest_wh, dobj, keystate);
        } else if dobj == DROOT {
            // Drop onto empty desktop: making a desktop shortcut is Task #14.
            fun_alert(1, b"Can't drop items here.");
        } else {
            fun_win2desk(wh, dobj, keystate);
        }
    } else {
        // Dragging FROM the desktop.
        if dest_wh != DESKWH && dest_wh != NIL {
            fun_desk2win(dest_wh, dobj, keystate);
        } else {
            fun_desk2desk(dobj, keystate);
        }
    }
    false
}

/// The Copy/Move operation a drop with `keystate` requests.
fn drag_op(keystate: u16) -> Op {
    if keystate & MODE_CTRL != 0 {
        Op::Move
    } else {
        Op::Copy
    }
}

/// `fun_win2win` — copy/move between two windows (EmuTOS `deskfun.c:942`).
fn fun_win2win(src_wh: i16, dst_wh: i16, dst_obj: i16, keystate: u16) {
    let Some(src_dir) = window_dir(src_wh) else {
        return;
    };
    let entries = window_selected(src_wh);
    if entries.is_empty() {
        return;
    }
    // Destination: into the dropped-on folder if it is one, else the window dir.
    let dst_dir = dest_folder_dir(dst_wh, dst_obj).unwrap_or_else(|| window_dir(dst_wh).unwrap_or_default());
    let op = drag_op(keystate);
    if fun_op(op, &src_dir, &entries, Some(&dst_dir)) {
        if src_wh != dst_wh {
            super::main::desk_clear(src_wh);
        }
        if op == Op::Move {
            rebuild_window(src_wh);
        }
        rebuild_window(dst_wh);
        super::redraw::redraw_desktop();
    }
}

/// `fun_win2desk` — drop a window selection onto a desktop icon (EmuTOS
/// `deskfun.c:1117`): resolve the destination desktop icon and route through
/// [`fun_file2desk`], rebuilding the source window on success.
fn fun_win2desk(src_wh: i16, dobj: i16, keystate: u16) {
    let Some(src_dir) = window_dir(src_wh) else {
        return;
    };
    let entries = window_selected(src_wh);
    if entries.is_empty() {
        return;
    }
    if fun_file2desk(dobj, &src_dir, &entries, keystate) {
        rebuild_window(src_wh);
        super::redraw::redraw_desktop();
    }
}

/// `fun_file2desk` — drop a window file set onto a desktop icon (EmuTOS
/// `deskfun.c:1004`): a Disk icon copies/moves to that drive's root, the Trash
/// deletes (after confirmation). File/Folder-shortcut and Printer targets are
/// Task #14. Returns `true` if a filesystem operation ran.
fn fun_file2desk(dobj: i16, src_dir: &str, entries: &[SelEntry], keystate: u16) -> bool {
    let (atype, letter) = global::with(|g| {
        let info = &g.g_screeninfo[dobj as usize];
        (info.atype, (info.block.ib_char & 0xFF) as u8)
    })
    .unwrap_or((AType::None, 0));

    match atype {
        AType::Disk => {
            if !letter.is_ascii_uppercase() {
                return false;
            }
            let mut dst = String::new();
            dst.push(letter as char);
            dst.push_str(":\\");
            fun_op(drag_op(keystate), src_dir, entries, Some(&dst))
        }
        AType::Trash => {
            // EmuTOS confirms a trash drop via `wants_to_delete_files` (desktop
            // source) or `dir_op`'s delete-confirm preference (window source).
            // NewTOS's engine is dialog-free, so we always confirm here — no
            // accidental delete.
            if !fun_confirm(b"Delete the selected item(s)?") {
                return false;
            }
            fun_op(Op::Delete, src_dir, entries, None)
        }
        // File/Folder shortcuts and the printer are Task #14 (deskins/print).
        _ => {
            fun_alert(1, b"Operation not allowed.");
            false
        }
    }
}

/// `fun_desk2win` — drag from the desktop to a window (EmuTOS `deskfun.c:1198`):
/// copy each selected disk icon's contents into the window. Trash/printer icons
/// and non-existent drives are skipped.
fn fun_desk2win(dest_wh: i16, dobj: i16, _keystate: u16) {
    let dst_dir = dest_folder_dir(dest_wh, dobj).unwrap_or_else(|| window_dir(dest_wh).unwrap_or_default());
    let disks = desktop_selected_disks();
    let mut any = false;
    for root in disks {
        let entries = root_entries(&root);
        if !entries.is_empty() && fun_op(Op::Copy, &root, &entries, Some(&dst_dir)) {
            any = true;
        }
    }
    if any {
        rebuild_window(dest_wh);
        super::redraw::redraw_desktop();
    }
}

/// `fun_desk2desk` — drag between two desktop icons (EmuTOS `deskfun.c:1260`):
/// dropping one disk's contents onto another disk (Copy) or onto the trash
/// (Delete). Self-drop is rejected.
fn fun_desk2desk(dobj: i16, keystate: u16) {
    let (dtype, dletter) = global::with(|g| {
        let info = &g.g_screeninfo[dobj as usize];
        (info.atype, (info.block.ib_char & 0xFF) as u8)
    })
    .unwrap_or((AType::None, 0));

    let sources = desktop_selected_disks_with_obj();
    let mut any = false;
    for (sobj, src_root) in sources {
        if sobj == dobj {
            continue; // self-drop
        }
        let entries = root_entries(&src_root);
        if entries.is_empty() {
            continue;
        }
        let ran = match dtype {
            AType::Disk if dletter.is_ascii_uppercase() => {
                let mut dst = String::new();
                dst.push(dletter as char);
                dst.push_str(":\\");
                fun_op(drag_op(keystate), &src_root, &entries, Some(&dst))
            }
            AType::Trash => {
                if fun_confirm(b"Delete the selected item(s)?") {
                    fun_op(Op::Delete, &src_root, &entries, None)
                } else {
                    false
                }
            }
            _ => {
                fun_alert(1, b"Operation not allowed.");
                false
            }
        };
        any |= ran;
    }
    if any {
        super::redraw::redraw_desktop();
    }
}

// ── window rebuild bookkeeping (deskfun.c fun_rebld) ────────────────────────

/// `rebuild_window` — re-scan a window's directory, rebuild its item view and
/// repaint it (EmuTOS `deskfun.c:135` `rebuild_window` + `deskfun.c:173`
/// `fun_rebld`). Called after a filesystem-mutating operation touched a
/// window's directory.
pub fn rebuild_window(handle: i16) {
    if handle == DESKWH {
        return;
    }
    let work = super::wind_get_work(handle);
    let scanned = global::with(|g| {
        let sort = g.sort;
        let slot = g.win_slot(handle)?;
        let spec = g.windows.slots[slot].w_pnode.p_spec.clone();
        {
            let pn = &mut g.windows.slots[slot].w_pnode;
            pn.pn_open(&spec);
            pn.pn_active(true);
            pn.pn_sort(sort);
        }
        Some(slot)
    })
    .flatten();
    if let Some(slot) = scanned {
        if let Some(sliders) = global::with(|g| g.win_bldview(slot, work)) {
            super::main::set_sliders_pub(handle, sliders);
        }
    }
    super::redraw::redraw_desktop();
}

/// `fun_rebld` — rebuild every open window whose directory equals `path`
/// (EmuTOS `deskfun.c:173`). Used after a drop that changed a folder's
/// contents in a window other than the source/destination.
pub fn fun_rebld(path: &str) {
    let handles = global::with(|g| {
        let mut v: Vec<i16> = Vec::new();
        for w in &g.windows.slots {
            if w.w_id != DESKWH && window_dir_of(&w.w_pnode.p_spec).as_deref() == Some(path) {
                v.push(w.w_id);
            }
        }
        v
    })
    .unwrap_or_default();
    for h in handles {
        rebuild_window(h);
    }
}

// ── desktop-state helpers ───────────────────────────────────────────────────

/// The directory (ending with `\`) of window `handle`'s `p_spec`.
fn window_dir(handle: i16) -> Option<String> {
    global::with(|g| {
        let slot = g.win_slot(handle)?;
        window_dir_of(&g.windows.slots[slot].w_pnode.p_spec)
    })
    .flatten()
}

/// The directory of a `p_spec` (`"C:\\SUB\\*.*" -> "C:\\SUB\\"`).
fn window_dir_of(spec: &str) -> Option<String> {
    let i = spec.rfind('\\')?;
    Some(String::from(&spec[..=i]))
}

/// The selected [`SelEntry`]s of window `handle`.
fn window_selected(handle: i16) -> Vec<SelEntry> {
    global::with(|g| {
        let Some(slot) = g.win_slot(handle) else {
            return Vec::new();
        };
        g.windows.slots[slot]
            .w_pnode
            .p_flist
            .iter()
            .filter(|f| f.f_selected)
            .map(SelEntry::from_fnode)
            .collect()
    })
    .unwrap_or_default()
}

/// If `dobj` in window `dst_wh` is a folder item, its directory (`dir + name +
/// \`); otherwise `None` (drop into the window's own directory).
fn dest_folder_dir(dst_wh: i16, dobj: i16) -> Option<String> {
    if dobj < 0 {
        return None;
    }
    global::with(|g| {
        let slot = g.win_slot(dst_wh)?;
        if g.g_screeninfo.get(dobj as usize)?.atype != AType::Folder {
            return None;
        }
        let fnode = g.g_screeninfo[dobj as usize].fnode;
        let f = g.windows.slots[slot].w_pnode.p_flist.get(fnode as usize)?;
        let name = core::str::from_utf8(f.name_bytes()).ok()?;
        let dir = window_dir_of(&g.windows.slots[slot].w_pnode.p_spec)?;
        let mut d = join_leaf(&dir, name);
        d.push('\\');
        Some(d)
    })
    .flatten()
}

/// The drive-root spec (`X:\`) of every selected desktop **disk** icon.
fn desktop_selected_disks() -> Vec<String> {
    desktop_selected_disks_with_obj().into_iter().map(|(_, s)| s).collect()
}

/// Like [`desktop_selected_disks`] but also returns each icon's object index.
fn desktop_selected_disks_with_obj() -> Vec<(i16, String)> {
    global::with(|g| {
        let mut v: Vec<(i16, String)> = Vec::new();
        let mut child = g.g_screen[DROOT as usize].ob_head;
        while child != NIL && child != DROOT {
            let sel = g.g_screen[child as usize].ob_state & OB_SELECTED != 0;
            let info = &g.g_screeninfo[child as usize];
            if sel && info.atype == AType::Disk {
                let letter = (info.block.ib_char & 0xFF) as u8;
                if letter.is_ascii_uppercase() {
                    let mut root = String::new();
                    root.push(letter as char);
                    root.push_str(":\\");
                    v.push((child, root));
                }
            }
            child = g.g_screen[child as usize].ob_next;
        }
        v
    })
    .unwrap_or_default()
}

/// Scan a drive-root directory (`X:\`) into a full selected-entry list (every
/// top-level entry — dragging a disk icon means "all its contents").
fn root_entries(root: &str) -> Vec<SelEntry> {
    scan_folder(root)
        .into_iter()
        .map(|e| SelEntry {
            name: e.name,
            is_dir: e.is_dir(),
            size: e.size,
            time: e.time,
            date: e.date,
        })
        .collect()
}

impl DeskState {
    /// `fun_selectall` core — set every window-item child of `handle`'s root
    /// selected (both the `OBJECT` bit and the backing `f_selected`).
    pub fn select_all(&mut self, handle: i16) {
        let Some(slot) = self.win_slot(handle) else {
            return;
        };
        let root = self.windows.slots[slot].w_root;
        let mut child = self.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            self.g_screen[child as usize].ob_state |= OB_SELECTED;
            child = self.g_screen[child as usize].ob_next;
        }
        for f in &mut self.windows.slots[slot].w_pnode.p_flist {
            f.f_selected = true;
        }
    }
}

// ── Task-#9 backend verification (deskdir.md "Verification criteria") ────────

/// Direct, programmatic verification of the file-operation backend against a
/// live drive (the `xtask run --board rpi4 --sd /tmp/sd.img` C: volume). Not
/// wired into the normal boot — invoked from `lib.rs` only when proving the
/// backend, exactly like the other `selftest_*` modules. Prints one line per
/// check to the boot console.
///
/// Exercises: `Dcreate` (new folder), `Fcreate`+`Fwrite` (make a file), a
/// **copy** (`d_dofcopy`), a **rename** (`d_dofileren`, same-drive move), and a
/// recursive **delete** (`d_doop` empties + removes a folder), each confirmed by
/// a fresh directory rescan.
pub fn selftest() {
    use crate::kprintln;

    kprintln!("[desk-dir] backend self-test on C:\\ (mutates the SD image)");

    let base = "C:\\DESKT9\\"; // work directory, created + removed by the test
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut check = |name: &str, cond: bool| {
        if cond {
            pass += 1;
            kprintln!("[desk-dir]  PASS {}", name);
        } else {
            fail += 1;
            kprintln!("[desk-dir]  FAIL {}", name);
        }
    };

    // Clean any prior run.
    let _ = dos_delete("C:\\DESKT9\\A.TXT");
    let _ = dos_delete("C:\\DESKT9\\B.TXT");
    let _ = dos_delete("C:\\DESKT9\\SUB\\A.TXT");
    let _ = dos_rmdir("C:\\DESKT9\\SUB");
    let _ = dos_rmdir("C:\\DESKT9");

    // 1. Dcreate — new folder appears in a root rescan.
    check("Dcreate C:\\DESKT9", dos_mkdir("C:\\DESKT9") == E_OK);
    check("  rescan sees DESKT9", scan_folder("C:\\").iter().any(|e| e.name_str() == "DESKT9" && e.is_dir()));

    // 2. Fcreate + Fwrite — a file appears with the written length.
    let body = b"NewTOS desk file-op backend test.\r\n";
    let fh = dos_create("C:\\DESKT9\\A.TXT", 0);
    let mut wrote = false;
    if fh >= 0 {
        wrote = dos_write(fh as i16, body.len() as i32, body.as_ptr()) == body.len() as i32;
        dos_close(fh as i16);
    }
    check("Fcreate+Fwrite A.TXT", wrote);
    check(
        "  rescan sees A.TXT sized",
        scan_folder(base).iter().any(|e| e.name_str() == "A.TXT" && !e.is_dir() && e.size == body.len() as u32),
    );

    // 3. Copy (d_dofcopy) — B.TXT is a byte copy of A.TXT.
    let mut buf: Vec<u8> = alloc::vec![0u8; COPY_BUF];
    let copied = d_dofcopy(base_join("A.TXT").as_str(), base_join("B.TXT").as_str(), 0x2000, 0x5461, 0, &mut buf);
    check("Copy A.TXT -> B.TXT", copied);
    check(
        "  rescan sees B.TXT same size",
        scan_folder(base).iter().any(|e| e.name_str() == "B.TXT" && e.size == body.len() as u32),
    );

    // 4. Rename (same-drive move) — A.TXT -> R.TXT.
    let renamed = d_dofileren(base_join("A.TXT").as_str(), base_join("R.TXT").as_str());
    check("Rename A.TXT -> R.TXT", renamed);
    check(
        "  rescan: R.TXT present, A.TXT gone",
        scan_folder(base).iter().any(|e| e.name_str() == "R.TXT")
            && !scan_folder(base).iter().any(|e| e.name_str() == "A.TXT"),
    );

    // 5. Recursive delete via dir_op(Delete) on a whole sub-folder.
    let _ = dos_mkdir("C:\\DESKT9\\SUB");
    let fh2 = dos_create("C:\\DESKT9\\SUB\\N.TXT", 0);
    if fh2 >= 0 {
        dos_write(fh2 as i16, body.len() as i32, body.as_ptr());
        dos_close(fh2 as i16);
    }
    let mut count = DirCount::default();
    let sub_entry = [SelEntry { name: fat_name(b"SUB"), is_dir: true, size: 0, time: 0, date: 0 }];
    let del_ok = dir_op(Op::Delete, base, &sub_entry, None, &mut count, &mut buf);
    check("dir_op(Delete) SUB subtree", del_ok);
    check("  rescan: SUB gone", !scan_folder(base).iter().any(|e| e.name_str() == "SUB"));

    // 6. Count pre-pass reports the two remaining files (B.TXT, R.TXT).
    let mut cnt = DirCount::default();
    let all = [
        SelEntry { name: fat_name(b"B.TXT"), is_dir: false, size: body.len() as u32, time: 0, date: 0 },
        SelEntry { name: fat_name(b"R.TXT"), is_dir: false, size: body.len() as u32, time: 0, date: 0 },
    ];
    dir_op(Op::Count, base, &all, None, &mut cnt, &mut buf);
    check("Count reports 2 files", cnt.files == 2);

    // Tidy up (leave the image close to its original state).
    let _ = dos_delete("C:\\DESKT9\\B.TXT");
    let _ = dos_delete("C:\\DESKT9\\R.TXT");
    let _ = dos_rmdir("C:\\DESKT9");
    check("cleanup: DESKT9 removed", !scan_folder("C:\\").iter().any(|e| e.name_str() == "DESKT9"));

    kprintln!("[desk-dir] backend self-test: {} passed, {} failed", pass, fail);
}

/// `C:\DESKT9\<leaf>` — a helper for [`selftest`].
fn base_join(leaf: &str) -> String {
    join_leaf("C:\\DESKT9\\", leaf)
}

/// A `NAME.EXT\0`-padded 14-byte FAT display name from a byte slice.
fn fat_name(s: &[u8]) -> [u8; 14] {
    let mut n = [0u8; 14];
    let k = s.len().min(13);
    n[..k].copy_from_slice(&s[..k]);
    n
}
