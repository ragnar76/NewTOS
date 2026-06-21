//! AES File Selector (`fsel_input` 90 / `fsel_exinput` 91) — Batch 1.
//!
//! Spec: [`docs/api/aes/resource-fileselector.md`]. The selector reuses
//! the real EmuTOS **FSELECTR** object tree, converted to native form
//! (`tools/mkrsc.py convert aes/gem.rsc`) and loaded from
//! `C:\GEMSYS\RSC\GEM.RSC`. We point its directory / selection / file-name
//! `TEDINFO`s at our own buffers, fill the visible slots from a directory
//! read ([`bdos::fs::list_dir`], `Fsfirst`/`Fsnext`-style), draw it, and
//! run a modal loop: a click selects a file, `OK`/`Cancel` exit.
//!
//! **Batch-1 scope:** list + click-to-select + OK/Cancel + write-back. The
//! scroll bar (>9 files), drive grid, and typing a path land in Batch 2.

use super::object::{self, Object, TedInfo};
use super::{resource, Aespb};
use crate::host::host;
use crate::vdi::backend::RasterBackend;
use core::cell::UnsafeCell;

// FSELECTR object indices (EmuTOS gem.rsc, verified against the converted
// resource).
const FSDIRECT: i16 = 3;
const FSSELECT: i16 = 5;
const FS1STDRV: i16 = 7; // first drive-letter gadget (A); A..Z = 7..32
const FSLSTDRV: i16 = 32; // last drive-letter gadget (Z)
const FUPAROW: i16 = 37;
const FDNAROW: i16 = 38;
const FSVSLID: i16 = 39; // slider track
const FSVELEV: i16 = 40; // slider thumb
const FILEBOX: i16 = 41;
const F1NAME: i16 = 42;
const NM_NAMES: usize = 9; // F1NAME..F9NAME
const FSOK: i16 = 51;
const FSCANCEL: i16 = 52;

const SELECTED: u16 = 0x0001;
const MAX_DEPTH: i16 = 16;
const BLACK_TEXT: i16 = 1 << 8; // te_color giving black ink (bits 8..11)

const GEM_CAP: usize = 8 * 1024;
/// Maximum directory entries held for scrolling.
const MAX_LIST: usize = 128;
/// Double-click window (milliseconds).
const DCLICK_MS: u64 = 400;

/// The AES's own GEM.RSC (file selector) + the dynamic text buffers its
/// TEDINFOs point at. Single-CPU, like the rest of the AES.
struct Sys {
    buf: UnsafeCell<[u8; GEM_CAP]>,
    root: UnsafeCell<*mut Object>, // FSELECTR root, null until loaded
    dir: UnsafeCell<[u8; 96]>,     // FSDIRECT path text
    sel: UnsafeCell<[u8; 16]>,     // FSSELECT filename text
    names: UnsafeCell<[[u8; 16]; NM_NAMES]>, // visible slot text
    all: UnsafeCell<[[u8; 16]; MAX_LIST]>,   // full directory listing
    count: UnsafeCell<usize>,      // entries in `all`
    top: UnsafeCell<usize>,        // index of the first visible entry
}
unsafe impl Sync for Sys {}

static SYS: Sys = Sys {
    buf: UnsafeCell::new([0u8; GEM_CAP]),
    root: UnsafeCell::new(core::ptr::null_mut()),
    dir: UnsafeCell::new([0u8; 96]),
    sel: UnsafeCell::new([0u8; 16]),
    names: UnsafeCell::new([[0u8; 16]; NM_NAMES]),
    all: UnsafeCell::new([[0u8; 16]; MAX_LIST]),
    count: UnsafeCell::new(0),
    top: UnsafeCell::new(0),
};

/// Load + relocate GEM.RSC once; return the FSELECTR tree (or null).
fn load_fselectr() -> *mut Object {
    // SAFETY: single-CPU AES.
    let cached = unsafe { *SYS.root.get() };
    if !cached.is_null() {
        return cached;
    }
    // SAFETY: single-CPU AES; exclusive owner of the buffer.
    let buf = unsafe { &mut *SYS.buf.get() };
    if let Some(len) = crate::bdos::fs::read_file("C:\\GEMSYS\\RSC\\GEM.RSC", buf) {
        // SAFETY: buf now holds GEM.RSC; relocate validates the header.
        if len > 0 && unsafe { resource::relocate_blob(buf.as_mut_ptr()) } {
            let root = unsafe { resource::tree_root(buf.as_mut_ptr(), 0) };
            unsafe { *SYS.root.get() = root };
            return root;
        }
    }
    core::ptr::null_mut()
}

/// Point object `idx`'s TEDINFO text at `ptr` (capacity `cap`), black ink,
/// left-justified.
fn set_ted(tree: *mut Object, idx: i16, ptr: *mut u8, cap: usize) {
    // SAFETY: idx is a valid TEDINFO-bearing object in the tree.
    unsafe {
        let spec = (*tree.add(idx as usize)).ob_spec;
        if spec == 0 {
            return;
        }
        let t = &mut *(spec as *mut TedInfo);
        t.te_ptext = ptr as u64;
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

/// `fsel_input` (90).
pub fn fsel_input(pb: &Aespb) {
    run(pb);
}

/// `fsel_exinput` (91) — same plus a custom title (drawn as the dialog
/// title in Batch 2; identical selection behaviour here).
pub fn fsel_exinput(pb: &Aespb) {
    run(pb);
}

fn run(pb: &Aespb) {
    let path_p = pb.addrin(0) as *mut u8;
    let sel_p = pb.addrin(1) as *mut u8;

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
        copy_in(&mut *SYS.sel.get(), sel_p);
        set_ted(tree, FSDIRECT, (*SYS.dir.get()).as_mut_ptr(), 96);
        set_ted(tree, FSSELECT, (*SYS.sel.get()).as_mut_ptr(), 16);
        let names = &mut *SYS.names.get();
        for i in 0..NM_NAMES {
            set_ted(tree, F1NAME + i as i16, names[i].as_mut_ptr(), 16);
        }
    }

    // Centre *before* listing so refill draws the box at its final spot.
    center(tree, backend);
    relist(tree); // read the directory, fill the slots + slider
    // SAFETY: valid tree; backend is the live screen.
    unsafe { object::draw_root(tree as *const Object, MAX_DEPTH, backend) };

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

/// Read the current directory (`SYS.dir`) into the full list, reset the
/// scroll position, and refill the visible slots + slider.
fn relist(tree: *mut Object) {
    let mut raw = [[0u8; 14]; MAX_LIST];
    let mut attrs = [0u8; MAX_LIST];
    // SAFETY: single-CPU AES.
    let n = unsafe {
        let dir = &*SYS.dir.get();
        let dlen = dir.iter().position(|&b| b == 0).unwrap_or(dir.len());
        let dpath = core::str::from_utf8(&dir[..dlen]).unwrap_or("C:\\*.*");
        crate::bdos::fs::find::list_dir(dpath, &mut raw, &mut attrs)
    };
    // SAFETY: single-CPU AES.
    unsafe {
        let all = &mut *SYS.all.get();
        for (i, slot) in all.iter_mut().enumerate() {
            *slot = if i < n { name16(&raw[i]) } else { [0; 16] };
        }
        *SYS.count.get() = n;
        *SYS.top.get() = 0;
    }
    refill(tree);
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
        object::draw_obj(tree as *const Object, FILEBOX, MAX_DEPTH, backend);
        object::draw_obj(tree as *const Object, FSVSLID, MAX_DEPTH, backend);
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

/// Set the directory buffer to `X:\*.*` for a clicked drive letter.
fn set_drive(ch: u8) {
    // SAFETY: single-CPU AES.
    unsafe {
        let dir = &mut *SYS.dir.get();
        *dir = [0; 96];
        dir[..6].copy_from_slice(&[ch, b':', b'\\', b'*', b'.', b'*']);
    }
}

fn name16(src: &[u8]) -> [u8; 16] {
    let mut out = [0u8; 16];
    let n = src.iter().position(|&b| b == 0).unwrap_or(src.len()).min(15);
    out[..n].copy_from_slice(&src[..n]);
    out
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
    center(tree, backend);
    relist(tree);
    // SAFETY: valid tree; backend is the live screen.
    unsafe { object::draw_root(tree as *const Object, MAX_DEPTH, backend) };
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

/// Modal loop: arrows scroll, a click selects a file (a double-click
/// confirms = OK), a drive letter re-lists that drive, `OK`/`Cancel` exit.
/// Returns 1 (OK) or 0 (Cancel).
fn modal(tree: *mut Object, backend: &dyn RasterBackend) -> i16 {
    let Some(h) = host() else {
        return 0;
    };
    let tp = tree as *const Object;
    let mut prev = crate::vdi::mouse_buttons();
    let mut cur_sel: i16 = -1;
    let mut last_hit: i16 = -1;
    let mut last_t: u64 = 0;
    loop {
        let cur = crate::vdi::mouse_buttons();
        if cur & 0x01 != 0 && prev & 0x01 == 0 {
            let (mx, my) = crate::vdi::mouse_position();
            // SAFETY: valid tree.
            let hit = unsafe { object::hit_test(tp, 0, MAX_DEPTH, mx, my) };
            let now = h.monotonic_millis();
            if hit == FSOK {
                return 1;
            } else if hit == FSCANCEL {
                return 0;
            } else if hit == FUPAROW {
                scroll(tree, -1);
            } else if hit == FDNAROW {
                scroll(tree, 1);
            } else if (F1NAME..F1NAME + NM_NAMES as i16).contains(&hit) {
                let slot = (hit - F1NAME) as usize;
                // SAFETY: single-CPU AES.
                let (idx, count) = unsafe { (*SYS.top.get() + slot, *SYS.count.get()) };
                if idx < count {
                    if cur_sel >= 0 {
                        object::set_state_bit(tree, cur_sel, SELECTED, false);
                    }
                    object::set_state_bit(tree, hit, SELECTED, true);
                    cur_sel = hit;
                    // SAFETY: single-CPU AES.
                    unsafe {
                        let all = &*SYS.all.get();
                        copy_name(&mut *SYS.sel.get(), &all[idx]);
                    }
                    object::draw_obj(tp, FSSELECT, 0, backend);
                    // Double-click on the same slot confirms.
                    if hit == last_hit && now.saturating_sub(last_t) < DCLICK_MS {
                        return 1;
                    }
                }
            } else if (FS1STDRV..=FSLSTDRV).contains(&hit) {
                // SAFETY: valid tree; BOXCHAR ob_spec packs the char at bits 24..31.
                let ch = unsafe { ((*tree.add(hit as usize)).ob_spec >> 24) as u8 };
                if ch.is_ascii_uppercase() {
                    set_drive(ch);
                    cur_sel = -1;
                    relist(tree);
                    object::draw_obj(tp, 0, MAX_DEPTH, backend); // dir field + list
                }
            }
            last_hit = hit;
            last_t = now;
        }
        prev = cur;
        h.pump_input();
    }
}
