//! AES File Selector (`fsel_input` 90 / `fsel_exinput` 91) — Batch 1.
//!
//! Spec: [`docs/api/aes/resource-fileselector.md`]. The selector reuses
//! the real EmuTOS **FSELECTR** object tree, converted to native form
//! (`tools/mkrsc.py convert references/emutos/aes/gem.rsc`) and **embedded**
//! in the kernel as the built-in default — like EmuTOS links `gem.rsc` into
//! ROM (`erd.c`) — so the selector always works on a bare card; a
//! `C:\GEMSYS\RSC\GEM.RSC` on the drive overrides it when present, so the
//! resource can be replaced without a kernel rebuild. We point its directory /
//! selection / file-name
//! `TEDINFO`s at our own buffers, fill the visible slots from a directory
//! read ([`bdos::fs::list_dir`], `Fsfirst`/`Fsnext`-style), draw it, and
//! run a modal loop: a click selects a file, `OK`/`Cancel` exit.
//!
//! **Implemented:** directory list + scroll bar/arrows + drive grid +
//! click-to-select + double-click confirm + OK/Cancel + write-back, plus
//! (EmuTOS `fs_input`) keyboard editing of the `FSDIRECT` path and
//! `FSSELECT` name fields — typing a new path + Enter re-lists it — and
//! walking into a sub-directory by clicking it. The listing matches EmuTOS
//! `fs_active`: folders sort first, then files, both alphabetically, and
//! `.`/`..`/dot-files are filtered out (`gemfslib.c:298,312`). Up-navigation
//! is the `FCLSBOX` close-box gadget (EmuTOS up-one-level, `gemfslib.c:839`)
//! or editing the `FSDIRECT` path + Enter. `fsel_exinput` shows the caller's
//! custom dialog title (`FS_TITLE = addr_in[2]`), else the default
//! "ITEM SELECTOR", centred in the FSTITLE caption (`gemfslib.c:691-693`).

use super::object::{self, Object, ObjTree, TedInfo};
use super::{resource, Aespb};
use crate::host::host;
use crate::vdi::backend::RasterBackend;
use core::cell::UnsafeCell;

// FSELECTR object indices (EmuTOS gem.rsc, verified against the converted
// resource).
const FSTITLE: i16 = 1; // dialog title caption (G_STRING); default "ITEM SELECTOR"
const FSDIRECT: i16 = 3;
const FSSELECT: i16 = 5;
const FS1STDRV: i16 = 7; // first drive-letter gadget (A); A..Z = 7..32
const FSLSTDRV: i16 = 32; // last drive-letter gadget (Z)
const FCLSBOX: i16 = 34; // close-box gadget = up-one-level (EmuTOS fs_back)
const FUPAROW: i16 = 37;
const FDNAROW: i16 = 38;
const FSVSLID: i16 = 39; // slider track
const FSVELEV: i16 = 40; // slider thumb
const FILEBOX: i16 = 41;
const F1NAME: i16 = 42;
const NM_NAMES: usize = 9; // F1NAME..F9NAME
const FSOK: i16 = 51;
const FSCANCEL: i16 = 52;
/// Free-string index of the default file-selector title in GEM.RSC (EmuTOS
/// `rs_str(ITEMSLCT)`, `gemfslib.c:692`). Verified against `aes/gem.rsc`:
/// free-string 14 = "ITEM SELECTOR".
const ITEMSLCT: usize = 14;

const SELECTED: u16 = 0x0001;
const DISABLED: u16 = 0x0008;
const FL3DIND: u16 = 0x0200; // indicator (drive letters)
const FL3DBAK: u16 = 0x0400; // background panel (text fields)
const FL3DACT: u16 = 0x0600; // activator (buttons / arrows)
const MAX_DEPTH: i16 = 16;
const BLACK_TEXT: i16 = 1 << 8; // te_color giving black ink (bits 8..11)

const GEM_CAP: usize = 8 * 1024;

/// The AES's own file-selector resource (EmuTOS `gem.rsc`), converted to the
/// native LP64 NRSC format and **compiled into the kernel** — the same way
/// EmuTOS links `gem.rsc` into ROM (via `erd.c`). Unlike every app resource
/// (which `rsrc_load`s from disk), the file selector must always be available,
/// so its resource is embedded rather than read from `C:\GEMSYS\RSC\GEM.RSC`.
///
/// The blob is the **pre-relocation** NRSC (internal pointers stored as byte
/// offsets), byte-identical to what the disk file held; `load_fselectr` copies
/// it into `SYS.buf` and relocates it in place, exactly as the old disk path
/// did. Regenerate with:
///
/// ```text
/// python3 tools/mkrsc.py convert references/emutos/aes/gem.rsc \
///     tos-api/src/aes/gem_fsel.nrsc
/// ```
///
/// (also wired as `cargo xtask gen-fsel-rsc`).
const GEM_FSEL_RSC: &[u8] = include_bytes!("gem_fsel.nrsc");
/// Compile-time guarantee that `SYS.buf` can hold the embedded resource
/// (blob size + relocation is fully in-place). Raise [`GEM_CAP`] if the
/// converted `gem.rsc` ever outgrows the buffer.
const _: () = assert!(GEM_CAP >= GEM_FSEL_RSC.len());
/// Maximum directory entries held for scrolling. EmuTOS sizes this
/// dynamically from free RAM (`gemfslib.c:655`, "usually thousands"); we
/// use a large fixed cap in static BSS instead. Dynamic RAM sizing would
/// mean reworking the static `SYS` arrays (`all`/`all_attr`) into heap
/// allocations plus a `dos_avail_anyram()` equivalent — deferred (out of
/// scope for this package). 2048 covers a full FAT16 root directory (512
/// entries) with wide margin; the ~35 KB of BSS is negligible on RPi4.
/// Entries beyond the cap are silently ignored, exactly as Atari TOS /
/// EmuTOS do (`gemfslib.c:290-292`).
const MAX_LIST: usize = 2048;
/// Double-click window (milliseconds).
const DCLICK_MS: u64 = 400;

/// The AES's own GEM.RSC (file selector) + the dynamic text buffers its
/// TEDINFOs point at. Single-CPU, like the rest of the AES.
struct Sys {
    buf: UnsafeCell<[u8; GEM_CAP]>,
    root: UnsafeCell<*mut Object>, // FSELECTR root, null until loaded
    dir: UnsafeCell<[u8; 96]>,     // FSDIRECT path text
    sel: UnsafeCell<[u8; 16]>,     // FSSELECT filename text
    title: UnsafeCell<[u8; 64]>,   // FSTITLE custom-title text (fsel_exinput)
    names: UnsafeCell<[[u8; 16]; NM_NAMES]>, // visible slot text
    all: UnsafeCell<[[u8; 16]; MAX_LIST]>,   // full directory listing
    all_attr: UnsafeCell<[u8; MAX_LIST]>,    // per-entry attribute byte
    count: UnsafeCell<usize>,      // entries in `all`
    top: UnsafeCell<usize>,        // index of the first visible entry
}
unsafe impl Sync for Sys {}

static SYS: Sys = Sys {
    buf: UnsafeCell::new([0u8; GEM_CAP]),
    root: UnsafeCell::new(core::ptr::null_mut()),
    dir: UnsafeCell::new([0u8; 96]),
    sel: UnsafeCell::new([0u8; 16]),
    title: UnsafeCell::new([0u8; 64]),
    names: UnsafeCell::new([[0u8; 16]; NM_NAMES]),
    all: UnsafeCell::new([[0u8; 16]; MAX_LIST]),
    all_attr: UnsafeCell::new([0u8; MAX_LIST]),
    count: UnsafeCell::new(0),
    top: UnsafeCell::new(0),
};

/// FAT/TOS directory attribute bit (a sub-directory entry).
const FA_SUBDIR: u8 = 0x10;
/// `objc_edit` kinds (EmuTOS `EDINIT`/`EDCHAR`/`EDEND`).
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;
const ED_END: i16 = 3;

/// Relocate the NRSC blob already sitting in `SYS.buf`, tag its 3D flags, cache
/// and return the FSELECTR tree — or null if the header does not validate
/// (without caching). Shared by the disk-override and embedded-fallback paths.
fn relocate_and_cache(buf: &mut [u8; GEM_CAP]) -> *mut Object {
    // SAFETY: buf holds a pre-relocation NRSC blob; relocate validates the
    // header and rewrites the byte offsets to absolute pointers in place.
    if unsafe { resource::relocate_blob(buf.as_mut_ptr()) } {
        let root = unsafe { resource::tree_root(buf.as_mut_ptr(), 0) };
        set_3d_flags(root);
        // SAFETY: single-CPU AES.
        unsafe { *SYS.root.get() = root };
        return root;
    }
    core::ptr::null_mut()
}

/// Obtain the FSELECTR tree (or null). Tries a disk override first, then falls
/// back to the built-in resource:
///
/// 1. **Disk override** — if `C:\GEMSYS\RSC\GEM.RSC` exists (a native NRSC blob,
///    same format `mkrsc.py` emits), load and use it. This lets a user replace
///    the file-selector resource — a different design, more languages — without
///    rebuilding the kernel. Only accepted if it relocates to a valid tree.
/// 2. **Embedded fallback** — otherwise use [`GEM_FSEL_RSC`], the `gem.rsc`
///    compiled into the kernel (EmuTOS links its `gem.rsc` into ROM the same
///    way). The always-present baseline: the selector works on a bare card.
fn load_fselectr() -> *mut Object {
    // SAFETY: single-CPU AES.
    let cached = unsafe { *SYS.root.get() };
    if !cached.is_null() {
        return cached;
    }
    // SAFETY: single-CPU AES; exclusive owner of the buffer.
    let buf = unsafe { &mut *SYS.buf.get() };

    // 1. Disk override.
    if let Some(len) = crate::bdos::fs::read_file("C:\\GEMSYS\\RSC\\GEM.RSC", buf) {
        if len > 0 {
            let root = relocate_and_cache(buf);
            if !root.is_null() {
                return root;
            }
            // Present but invalid — fall through to the embedded blob below.
        }
    }

    // 2. Embedded fallback. The `const _` assert guarantees the fit.
    buf[..GEM_FSEL_RSC.len()].copy_from_slice(GEM_FSEL_RSC);
    relocate_and_cache(buf)
}

/// Tag the file-selector objects with their 3D flags (EmuTOS `fs_start`):
/// buttons + arrows raised, text fields + drive letters recessed.
fn set_3d_flags(tree: *mut Object) {
    // SAFETY: valid FSELECTR tree.
    unsafe {
        for b in [FSOK, FSCANCEL, FUPAROW, FDNAROW, FSVELEV] {
            (*tree.add(b as usize)).ob_flags |= FL3DACT;
        }
        for b in [FSDIRECT, FSSELECT] {
            (*tree.add(b as usize)).ob_flags |= FL3DBAK;
        }
        for d in FS1STDRV..=FSLSTDRV {
            (*tree.add(d as usize)).ob_flags |= FL3DIND;
        }
    }
}

/// Disable drive-letter gadgets for drives that are not present, and mark
/// the current drive `cur` (`A`..`Z`) selected — like the EmuTOS selector.
fn set_drive_states(tree: *mut Object, cur: u8) {
    let bits = crate::bios::drive::drvbits();
    let cur_idx = if cur.is_ascii_uppercase() { (cur - b'A') as i16 } else { 2 };
    // SAFETY: valid FSELECTR tree.
    unsafe {
        for d in FS1STDRV..=FSLSTDRV {
            let idx = d - FS1STDRV; // 0 = A … 25 = Z
            let o = &mut *tree.add(d as usize);
            o.ob_state &= !(DISABLED | SELECTED);
            if bits & (1 << idx) == 0 {
                o.ob_state |= DISABLED;
            } else if idx == cur_idx {
                o.ob_state |= SELECTED;
            }
        }
    }
}

/// Object types (`obdefs.h`).
const G_TEXT: u16 = 21;
const G_BOXTEXT: u16 = 22;
const G_FTEXT: u16 = 29;
const G_FBOXTEXT: u16 = 30;

/// An empty (identity) template, so `edit_field`'s `find_pos` is the
/// identity on the plain fields below.
static EMPTY_TMPL: [u8; 1] = [0];

/// Point object `idx`'s TEDINFO text at `ptr` (capacity `cap`), black ink,
/// left-justified, and make the field a **plain** box.
///
/// The GEM.RSC file-selector fields are `G_FBOXTEXT` with formatted
/// templates that embed literals and the 8.3 filename slots (e.g.
/// `Selection: ________.___`). NewTOS stores **plain** names/paths in its
/// own buffers, not the template-raw form those templates expect, so we
/// down-convert the repointed fields to `G_BOXTEXT`/`G_TEXT` and clear the
/// template — `just_draw` then draws the stored string directly (and
/// `edit_field` keeps an identity caret). Real RSC dialogs, which we do not
/// repoint, keep full `ob_format` template merging.
fn set_ted(tree: *mut Object, idx: i16, ptr: *mut u8, cap: usize) {
    // SAFETY: idx is a valid TEDINFO-bearing object in the tree.
    unsafe {
        let ob = &mut *tree.add(idx as usize);
        match ob.ob_type {
            G_FBOXTEXT => ob.ob_type = G_BOXTEXT,
            G_FTEXT => ob.ob_type = G_TEXT,
            _ => {}
        }
        let spec = ob.ob_spec;
        if spec == 0 {
            return;
        }
        let t = &mut *(spec as *mut TedInfo);
        t.te_ptext = ptr as u64;
        t.te_ptmplt = EMPTY_TMPL.as_ptr() as u64;
        // Clear validation too: NewTOS stores the *plain* name/path, so the
        // RSC's raw-form `te_pvalid` codes are wrong here (e.g. the filename
        // set 'F' excludes '.', which a plain "NAME.EXT" needs). An empty
        // pvalid makes `edit_field` accept any character.
        t.te_pvalid = EMPTY_TMPL.as_ptr() as u64;
        t.te_txtlen = cap as i16;
        t.te_color = BLACK_TEXT;
        t.te_just = 0;
    }
}

fn copy_in(dst: &mut [u8], src: *const u8) {
    for b in dst.iter_mut() {
        *b = 0;
    }
    if src.is_null() {
        return;
    }
    // SAFETY: trusted GEM pointer to a NUL-terminated string.
    unsafe {
        let mut i = 0;
        while i < dst.len() - 1 {
            let c = *src.add(i);
            dst[i] = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
}

fn copy_name(dst: &mut [u8; 16], src: &[u8]) {
    *dst = [0; 16];
    let n = src.iter().position(|&b| b == 0).unwrap_or(src.len()).min(15);
    dst[..n].copy_from_slice(&src[..n]);
}

fn write_out(dst: *mut u8, src: &[u8]) {
    if dst.is_null() {
        return;
    }
    let n = src.iter().position(|&b| b == 0).unwrap_or(src.len());
    // SAFETY: caller buffer is large enough for the in-out string (GEM).
    unsafe {
        for i in 0..n {
            *dst.add(i) = src[i];
        }
        *dst.add(n) = 0;
    }
}

/// `fsel_input` (90) — no caller title, so FSTITLE shows the default
/// "ITEM SELECTOR" (EmuTOS `fs_input(..., NULL)`, gemsuper.c:323).
pub fn fsel_input(pb: &Aespb) {
    run(pb, core::ptr::null());
}

/// `fsel_exinput` (91) — `fsel_input` plus a caller-supplied dialog title in
/// `addr_in[2]` (`FS_TITLE`, gemsuper.c:326 / aesdefs.h:464), shown centred in
/// the FSTITLE caption. A null title falls back to the default "ITEM SELECTOR".
pub fn fsel_exinput(pb: &Aespb) {
    run(pb, pb.addrin(2) as *const u8);
}

fn run(pb: &Aespb, label: *const u8) {
    let path_p = pb.addrin(0) as *mut u8;
    let sel_p = pb.addrin(1) as *mut u8;

    // EmuTOS `fs_input`: bail out immediately on a NULL path pointer
    // (gemfslib.c:630-631; spec resource-fileselector.md:328,411,447). The
    // dialog never opens: int_out[0] = 0 (did not run), int_out[1] = 0.
    if path_p.is_null() {
        pb.set_intout(1, 0);
        pb.set_ret(0);
        return;
    }

    let tree = load_fselectr();
    let Some(backend) = host().and_then(|h| h.raster()) else {
        pb.set_intout(1, 0);
        pb.set_ret(0);
        return;
    };
    if tree.is_null() {
        pb.set_intout(1, 0);
        pb.set_ret(0);
        return;
    }

    // Copy inputs into our buffers and point the dialog's TEDINFOs at them
    // (short, non-overlapping borrows for the single-CPU AES).
    // SAFETY: single-CPU AES; the resource TEDINFOs are distinct memory.
    unsafe {
        copy_in(&mut *SYS.dir.get(), path_p);
        // EmuTOS `fs_input`: an empty path defaults to "<curdrive>:\*.*"
        // (`strcpy(pipath,"A:\\*.*"); *pipath += dos_gdrv()`,
        // gemfslib.c:634-638; spec resource-fileselector.md:446). The default
        // is written back to the caller on exit (via `write_out` below).
        {
            let dir = &mut *SYS.dir.get();
            if dir[0] == 0 {
                let drv = b'A' + crate::bdos::fs::drive::current_drive() as u8;
                dir[..6].copy_from_slice(&[drv, b':', b'\\', b'*', b'.', b'*']);
            }
        }
        copy_in(&mut *SYS.sel.get(), sel_p);
        set_ted(tree, FSDIRECT, (*SYS.dir.get()).as_mut_ptr(), 96);
        set_ted(tree, FSSELECT, (*SYS.sel.get()).as_mut_ptr(), 16);
        let names = &mut *SYS.names.get();
        for i in 0..NM_NAMES {
            set_ted(tree, F1NAME + i as i16, names[i].as_mut_ptr(), 16);
        }
    }

    // Centre *before* listing so refill draws the box at its final spot.
    let cur = unsafe { (*SYS.dir.get())[0] };
    set_drive_states(tree, cur);
    center(tree, backend);
    // EmuTOS `fs_input` sets the dialog title — the caller's custom label or the
    // default "ITEM SELECTOR" — and centres it (gemfslib.c:691-693).
    apply_title(tree, label);
    relist(tree); // read the directory, fill the slots + slider
    // The FSELECTR tree is loaded from GEM.RSC as native (LP64) `Object`s, so it
    // must be read at the LP64 width even when an ILP32 app is current — force
    // the accessor's ABI here rather than reading `current_abi()`.
    object::draw_root(ObjTree::lp64(tree as *const Object), MAX_DEPTH, backend);

    let button = modal(tree, backend);

    // Write the (possibly updated) path + selection back to the caller.
    // SAFETY: single-CPU AES.
    unsafe {
        write_out(path_p, &*SYS.dir.get());
        write_out(sel_p, &*SYS.sel.get());
    }
    pb.set_intout(1, button);
    pb.set_ret(1);
}

/// Read the current directory (`SYS.dir`) into the full list, sort it
/// (EmuTOS `fs_active`: folders first, then files, both alphabetical),
/// reset the scroll position, and refill the visible slots + slider.
///
/// `list_dir` writes directly into the static `SYS` arrays (no large stack
/// buffer), so `MAX_LIST` can be raised without risking the kernel stack.
fn relist(tree: *mut Object) {
    // SAFETY: single-CPU AES; `dir` and `all`/`all_attr` are distinct cells.
    let n = unsafe {
        let dir = &*SYS.dir.get();
        let dlen = dir.iter().position(|&b| b == 0).unwrap_or(dir.len());
        let dpath = core::str::from_utf8(&dir[..dlen]).unwrap_or("C:\\*.*");
        let all = &mut *SYS.all.get();
        let all_attr = &mut *SYS.all_attr.get();
        crate::bdos::fs::find::list_dir(dpath, all, all_attr)
    };
    sort_entries(n);
    // SAFETY: single-CPU AES.
    unsafe {
        *SYS.count.get() = n;
        *SYS.top.get() = 0;
    }
    refill(tree);
}

/// Sort the first `n` directory entries in place — folders before files,
/// then alphabetically. This is EmuTOS `fs_active`'s shell sort
/// (`gemfslib.c:312-326`) driven by the folder/file prefix key from
/// `fs_add` (`gemfslib.c:254`) and `strcmp` from `fs_comp`
/// (`gemfslib.c:243`), applied to our two parallel `SYS` arrays.
fn sort_entries(n: usize) {
    // SAFETY: single-CPU AES.
    unsafe {
        let all = &mut *SYS.all.get();
        let attr = &mut *SYS.all_attr.get();
        let mut gap = n / 2;
        while gap > 0 {
            let mut i = gap;
            while i < n {
                let mut j = i - gap;
                loop {
                    if entry_cmp(&all[j], attr[j], &all[j + gap], attr[j + gap])
                        != core::cmp::Ordering::Greater
                    {
                        break;
                    }
                    all.swap(j, j + gap);
                    attr.swap(j, j + gap);
                    if j < gap {
                        break;
                    }
                    j -= gap;
                }
                i += 1;
            }
            gap /= 2;
        }
    }
}

/// Compare two list entries by EmuTOS's sort key: the folder/file prefix
/// (`\007` for a sub-directory, `' '` for a file, so folders sort lowest —
/// `fs_add`, `gemfslib.c:254`) then the display name via `strcmp`
/// (`fs_comp`, `gemfslib.c:243`). Names are NUL-padded, so a plain array
/// compare is `strcmp`-equivalent.
fn entry_cmp(na: &[u8; 16], aa: u8, nb: &[u8; 16], ab: u8) -> core::cmp::Ordering {
    let pa = if aa & FA_SUBDIR != 0 { 0x07u8 } else { b' ' };
    let pb = if ab & FA_SUBDIR != 0 { 0x07u8 } else { b' ' };
    pa.cmp(&pb).then_with(|| na.cmp(nb))
}

/// Copy the visible window into the slot buffers, update the slider, and
/// redraw the file box.
fn refill(tree: *mut Object) {
    // SAFETY: single-CPU AES.
    unsafe {
        let all = &*SYS.all.get();
        let top = *SYS.top.get();
        let count = *SYS.count.get();
        let names = &mut *SYS.names.get();
        for (i, n) in names.iter_mut().enumerate() {
            *n = if top + i < count { all[top + i] } else { [0; 16] };
        }
        // Clear slot selection state (no per-slot redraw; the box redraw
        // below repaints them).
        for i in 0..NM_NAMES as i16 {
            (*tree.add((F1NAME + i) as usize)).ob_state &= !SELECTED;
        }
    }
    update_slider(tree);
    if let Some(backend) = host().and_then(|h| h.raster()) {
        // LP64 accessor for the kernel-built FSELECTR tree (see `run`).
        let t = ObjTree::lp64(tree as *const Object);
        object::draw_obj_t(t, FILEBOX, MAX_DEPTH, backend);
        object::draw_obj_t(t, FSVSLID, MAX_DEPTH, backend);
    }
}

/// Position/size the slider thumb (FSVELEV) for the current scroll state.
fn update_slider(tree: *mut Object) {
    // SAFETY: single-CPU AES; valid tree.
    unsafe {
        let count = *SYS.count.get();
        let track_h = (*tree.add(FSVSLID as usize)).ob_height;
        let thumb = &mut *tree.add(FSVELEV as usize);
        if count <= NM_NAMES {
            thumb.ob_y = 0;
            thumb.ob_height = track_h;
        } else {
            let th = (track_h * NM_NAMES as i16 / count as i16).max(8);
            let maxtop = (count - NM_NAMES) as i16;
            let cur = *SYS.top.get() as i16;
            thumb.ob_y = ((track_h - th) * cur / maxtop).max(0);
            thumb.ob_height = th;
        }
    }
}

/// Scroll the list by `delta` rows (clamped); refill if it moved.
fn scroll(tree: *mut Object, delta: i32) {
    // SAFETY: single-CPU AES.
    let moved = unsafe {
        let count = *SYS.count.get();
        if count <= NM_NAMES {
            false
        } else {
            let maxtop = (count - NM_NAMES) as i32;
            let old = *SYS.top.get();
            let new = (old as i32 + delta).clamp(0, maxtop) as usize;
            *SYS.top.get() = new;
            new != old
        }
    };
    if moved {
        refill(tree);
    }
}

/// Navigate the directory path into a sub-directory `name` from the list
/// (EmuTOS fsel folder handling): insert the folder name before the mask.
/// `.` is a no-op; `..` walks one level up (never past the drive root).
fn nav_into_folder(name: &[u8]) {
    let nlen = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    let nm = &name[..nlen];
    if nm == b"." {
        return;
    }
    // SAFETY: single-CPU AES.
    unsafe {
        let dir = &mut *SYS.dir.get();
        let dlen = dir.iter().position(|&b| b == 0).unwrap_or(dir.len());
        // Split into the directory prefix (up to & incl. the last '\') and
        // the mask (the final path component).
        let mask_start = match dir[..dlen].iter().rposition(|&b| b == b'\\') {
            Some(p) => p + 1,
            None => dlen,
        };
        let mut mask = [0u8; 16];
        let mk = (dlen - mask_start).min(15);
        mask[..mk].copy_from_slice(&dir[mask_start..mask_start + mk]);

        let mut prefix = [0u8; 96];
        prefix[..mask_start].copy_from_slice(&dir[..mask_start]);
        let mut plen = mask_start;

        if nm == b".." {
            // Walk up: drop the trailing '\', then back to the previous '\',
            // but never before the drive root ("X:\").
            if plen >= 1 && prefix[plen - 1] == b'\\' {
                plen -= 1;
            }
            let root = if plen >= 2 && prefix[1] == b':' { 3 } else { 0 };
            while plen > root && prefix[plen - 1] != b'\\' {
                plen -= 1;
            }
        } else {
            // Walk down: append the folder name + '\'.
            for &c in nm {
                if plen < 95 {
                    prefix[plen] = c;
                    plen += 1;
                }
            }
            if plen < 95 {
                prefix[plen] = b'\\';
                plen += 1;
            }
        }

        // Rebuild dir = prefix + mask.
        *dir = [0; 96];
        dir[..plen].copy_from_slice(&prefix[..plen]);
        let mut w = plen;
        for i in 0..mk {
            if mask[i] == 0 {
                break;
            }
            if w < 95 {
                dir[w] = mask[i];
                w += 1;
            }
        }
    }
}

/// Whether the directory field (`SYS.dir`, edited in place) now differs
/// from the path last listed (EmuTOS `path_changed` — a `strncmp`, i.e. the
/// logical NUL-terminated strings, not the raw buffers, so a shortened path
/// with stale tail bytes past its NUL still compares equal).
fn path_differs(listed: &[u8; 96]) -> bool {
    // SAFETY: single-CPU AES.
    let dir = unsafe { &*SYS.dir.get() };
    let dlen = dir.iter().position(|&b| b == 0).unwrap_or(dir.len());
    let llen = listed.iter().position(|&b| b == 0).unwrap_or(listed.len());
    dlen != llen || dir[..dlen] != listed[..llen]
}

/// Set the directory buffer to `X:\*.*` for a clicked drive letter.
fn set_drive(ch: u8) {
    // SAFETY: single-CPU AES.
    unsafe {
        let dir = &mut *SYS.dir.get();
        *dir = [0; 96];
        dir[..6].copy_from_slice(&[ch, b':', b'\\', b'*', b'.', b'*']);
    }
}

/// Draw the file selector on `C:\*.*` **without** the modal loop — for a
/// non-interactive showcase. Returns once the populated dialog is drawn.
pub fn preview() {
    let tree = load_fselectr();
    let Some(backend) = host().and_then(|h| h.raster()) else {
        return;
    };
    if tree.is_null() {
        return;
    }
    // SAFETY: single-CPU AES.
    unsafe {
        let dir = &mut *SYS.dir.get();
        *dir = [0; 96];
        dir[..6].copy_from_slice(b"C:\\*.*");
        set_ted(tree, FSDIRECT, dir.as_mut_ptr(), 96);
        set_ted(tree, FSSELECT, (*SYS.sel.get()).as_mut_ptr(), 16);
        let names = &mut *SYS.names.get();
        for i in 0..NM_NAMES {
            set_ted(tree, F1NAME + i as i16, names[i].as_mut_ptr(), 16);
        }
    }
    set_drive_states(tree, b'C');
    center(tree, backend);
    relist(tree);
    // LP64 accessor for the kernel-built FSELECTR tree (see `run`).
    object::draw_root(ObjTree::lp64(tree as *const Object), MAX_DEPTH, backend);
}

/// Centre the FSELECTR root on screen.
fn center(tree: *mut Object, backend: &dyn RasterBackend) {
    // SAFETY: valid tree.
    unsafe {
        let r = &mut *tree;
        r.ob_x = ((backend.width() as i16 - r.ob_width) / 2).max(0);
        r.ob_y = ((backend.height() as i16 - r.ob_height) / 2).max(0);
    }
}

/// Set the FSTITLE caption and centre it — EmuTOS `fs_input`
/// (`gemfslib.c:691-693`): `obj->ob_spec = pilabel ? pilabel : rs_str(ITEMSLCT)`
/// then `centre_title(tree, FSTITLE)`. The caller's custom `label` (copied into
/// our own buffer so the tree never holds a raw EL0 pointer) takes precedence;
/// otherwise the default is the `ITEMSLCT` free string read from the loaded
/// GEM.RSC blob (the `rs_str` analog). The tree is cached across calls, so
/// ob_spec is rewritten every call — a prior custom title never leaks into a
/// later default-title dialog.
fn apply_title(tree: *mut Object, label: *const u8) {
    // SAFETY: single-CPU AES; valid FSELECTR tree; SYS.buf holds the blob.
    unsafe {
        let title_ptr: *const u8 = if !label.is_null() {
            copy_in(&mut *SYS.title.get(), label);
            (*SYS.title.get()).as_ptr()
        } else {
            resource::blob_frstr(SYS.buf.get() as *const u8, ITEMSLCT)
        };
        (*tree.add(FSTITLE as usize)).ob_spec = title_ptr as u64;
        centre_title(tree, FSTITLE);
    }
}

/// Centre the `objnum` G_STRING caption horizontally in the root — EmuTOS
/// `centre_title` (`gemfslib.c:179-185`): `ob_x = (root.ob_width -
/// strlen(ob_spec)*gl_wchar) / 2`. Unlike EmuTOS (whose G_STRING draw is not
/// width-clipped) NewTOS clips a string's glyphs to `ob_width`
/// (`object::draw_text`, the icon-label behaviour), so we also widen the object
/// to the title's pixel width; the visible result — a fully drawn, centred
/// caption — matches EmuTOS.
fn centre_title(tree: *mut Object, objnum: i16) {
    // SAFETY: single-CPU AES; valid tree; `objnum` is a G_STRING child of ROOT.
    unsafe {
        let root_w = (*tree).ob_width;
        let ob = &mut *tree.add(objnum as usize);
        // strlen(ob_spec): the title is our own buffer or a GEM.RSC frstr
        // string, both NUL-terminated (capped like `object::read_cstr` at 255).
        let tw = if ob.ob_spec == 0 {
            0
        } else {
            let p = ob.ob_spec as *const u8;
            let mut n = 0i16;
            while n < 255 && *p.add(n as usize) != 0 {
                n += 1;
            }
            n * crate::vdi::font::CELL_W
        };
        ob.ob_x = (root_w - tw) / 2;
        ob.ob_width = tw;
    }
}

/// Whether the directory path (`SYS.dir`, path+mask) is at the drive root, so
/// FCLSBOX up-navigation must do nothing — EmuTOS `fs_back`/FCLSBOX
/// (`gemfslib.c:840-844`): stop if there is no path separator (`pstr == locstr`)
/// or the character before the last separator is the drive colon (`X:\`).
fn at_drive_root() -> bool {
    // SAFETY: single-CPU AES.
    let dir = unsafe { &*SYS.dir.get() };
    let dlen = dir.iter().position(|&b| b == 0).unwrap_or(dir.len());
    match dir[..dlen].iter().rposition(|&b| b == b'\\') {
        None => true,                  // no separator (locstr like "*.*")
        Some(0) => true,               // separator at the very start
        Some(p) => dir[p - 1] == b':', // char before last separator is ':'
    }
}

/// Modal loop (EmuTOS `fs_input`): the `FSDIRECT` path field and `FSSELECT`
/// selection field are keyboard-editable; typing a new path + Enter re-lists
/// it, a click on a folder walks into it, arrows scroll, a click selects a
/// file (double-click confirms), a drive letter re-lists that drive,
/// `OK`/`Cancel` exit. Returns 1 (OK) or 0 (Cancel).
fn modal(tree: *mut Object, backend: &dyn RasterBackend) -> i16 {
    let Some(h) = host() else {
        return 0;
    };
    let tp = tree as *const Object;
    // The FSELECTR tree is native (LP64) `Object`s from GEM.RSC; read it at the
    // LP64 width even while an ILP32 app is current (see `run`). Thread this one
    // accessor through every edit / hit-test / draw / state helper below.
    let t = ObjTree::lp64(tp);
    // Snapshot the listed path so a manual edit of FSDIRECT is detected.
    // SAFETY: single-CPU AES.
    let mut listed: [u8; 96] = unsafe { *SYS.dir.get() };
    // Focus the selection field for editing (EmuTOS `fm_do(tree, FSSELECT)`).
    let mut focus = FSSELECT;
    let mut idx = object::edit_field_t(t, focus, 0, 0, ED_INIT);

    let mut prev = crate::vdi::mouse_buttons();
    let mut cur_sel: i16 = -1;
    let mut last_hit: i16 = -1;
    let mut last_t: u64 = 0;

    // Re-list the (manually edited) directory path and re-arm editing.
    let apply_path = |tree: *mut Object, focus: &mut i16, idx: &mut i16, listed: &mut [u8; 96]| {
        object::edit_field_t(t, *focus, 0, *idx, ED_END);
        relist(tree);
        // SAFETY: single-CPU AES.
        *listed = unsafe { *SYS.dir.get() };
        object::draw_obj_t(t, FSDIRECT, 0, backend);
        *focus = FSSELECT;
        *idx = object::edit_field_t(t, *focus, 0, 0, ED_INIT);
    };

    loop {
        // Keyboard: route to the focused editable field.
        if let Some(w) = h.console_try_read_key() {
            match (w & 0xff) as u8 {
                b'\r' | b'\n' => {
                    // Enter: re-list if the path was edited, else confirm.
                    if path_differs(&listed) {
                        cur_sel = -1;
                        apply_path(tree, &mut focus, &mut idx, &mut listed);
                    } else {
                        return 1;
                    }
                }
                b'\t' => {
                    object::edit_field_t(t, focus, 0, idx, ED_END);
                    focus = if focus == FSSELECT { FSDIRECT } else { FSSELECT };
                    idx = object::edit_field_t(t, focus, 0, 0, ED_INIT);
                }
                _ => {
                    // Whole key word (scancode << 8 | ascii) → arrows move the caret.
                    idx = object::edit_field_t(t, focus, w as i16, idx, ED_CHAR);
                }
            }
        }

        let cur = crate::vdi::mouse_buttons();
        if cur & 0x01 != 0 && prev & 0x01 == 0 {
            let (mx, my) = crate::vdi::mouse_position();
            let hit = object::hit_test_t(t, 0, MAX_DEPTH, mx, my);
            let now = h.monotonic_millis();
            if hit == FSOK {
                // Like TOS: if the mask was edited, OK re-lists, not exits.
                if path_differs(&listed) {
                    cur_sel = -1;
                    apply_path(tree, &mut focus, &mut idx, &mut listed);
                } else {
                    return 1;
                }
            } else if hit == FSCANCEL {
                return 0;
            } else if hit == FUPAROW {
                scroll(tree, -1);
            } else if hit == FDNAROW {
                scroll(tree, 1);
            } else if hit == FCLSBOX {
                // EmuTOS FCLSBOX (gemfslib.c:839-848): walk one directory level
                // up, unless already at the drive root ("X:\"). This is the
                // up-navigation control that replaces the (WP7a-filtered) ".."
                // list entry; reuse the existing ".." path-shortening + relist
                // (`nav_into_folder`/`apply_path`), guarded so we never shorten
                // past the drive root.
                if !at_drive_root() {
                    nav_into_folder(b"..");
                    cur_sel = -1;
                    apply_path(tree, &mut focus, &mut idx, &mut listed);
                }
            } else if hit == FSDIRECT || hit == FSSELECT {
                // Click into an editable field → focus it (caret to its end).
                if hit != focus {
                    object::edit_field_t(t, focus, 0, idx, ED_END);
                    focus = hit;
                }
                idx = object::edit_field_t(t, focus, 0, 0, ED_INIT);
            } else if (F1NAME..F1NAME + NM_NAMES as i16).contains(&hit) {
                let slot = (hit - F1NAME) as usize;
                // SAFETY: single-CPU AES.
                let (entry, count) = unsafe { (*SYS.top.get() + slot, *SYS.count.get()) };
                if entry < count {
                    // SAFETY: single-CPU AES; entry < count.
                    let (name, is_dir) = unsafe {
                        ((*SYS.all.get())[entry], (*SYS.all_attr.get())[entry] & FA_SUBDIR != 0)
                    };
                    if is_dir {
                        // Folder: walk into it and re-list (EmuTOS single click).
                        nav_into_folder(&name);
                        cur_sel = -1;
                        apply_path(tree, &mut focus, &mut idx, &mut listed);
                    } else {
                        // File: select it; a double-click confirms (= OK).
                        if cur_sel >= 0 {
                            object::set_state_bit_t(t, cur_sel, SELECTED, false);
                        }
                        object::set_state_bit_t(t, hit, SELECTED, true);
                        cur_sel = hit;
                        // SAFETY: single-CPU AES.
                        unsafe { copy_name(&mut *SYS.sel.get(), &name) };
                        // Re-arm editing on the selection field (caret at end).
                        focus = FSSELECT;
                        idx = object::edit_field_t(t, focus, 0, 0, ED_INIT);
                        if hit == last_hit && now.saturating_sub(last_t) < DCLICK_MS {
                            return 1;
                        }
                    }
                }
            } else if (FS1STDRV..=FSLSTDRV).contains(&hit) {
                // SAFETY: valid tree; BOXCHAR ob_spec packs the char at bits 24..31.
                let ch = unsafe { ((*tree.add(hit as usize)).ob_spec >> 24) as u8 };
                if ch.is_ascii_uppercase() {
                    set_drive(ch);
                    set_drive_states(tree, ch);
                    cur_sel = -1;
                    object::edit_field_t(t, focus, 0, idx, ED_END);
                    relist(tree);
                    // SAFETY: single-CPU AES.
                    listed = unsafe { *SYS.dir.get() };
                    object::draw_obj_t(t, 0, MAX_DEPTH, backend); // drive states + dir + list
                    focus = FSSELECT;
                    idx = object::edit_field_t(t, focus, 0, 0, ED_INIT); // caret last
                }
            }
            last_hit = hit;
            last_t = now;
        }
        prev = cur;
        h.pump_input();
    }
}
