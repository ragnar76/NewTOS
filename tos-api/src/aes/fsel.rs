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
const F1NAME: i16 = 42;
const NM_NAMES: usize = 9; // F1NAME..F9NAME
const FSOK: i16 = 51;
const FSCANCEL: i16 = 52;

const SELECTED: u16 = 0x0001;
const MAX_DEPTH: i16 = 16;
const BLACK_TEXT: i16 = 1 << 8; // te_color giving black ink (bits 8..11)

const GEM_CAP: usize = 8 * 1024;

/// The AES's own GEM.RSC (file selector) + the dynamic text buffers its
/// TEDINFOs point at. Single-CPU, like the rest of the AES.
struct Sys {
    buf: UnsafeCell<[u8; GEM_CAP]>,
    root: UnsafeCell<*mut Object>, // FSELECTR root, null until loaded
    dir: UnsafeCell<[u8; 96]>,     // FSDIRECT path text
    sel: UnsafeCell<[u8; 16]>,     // FSSELECT filename text
    names: UnsafeCell<[[u8; 16]; NM_NAMES]>, // visible slot text
}
unsafe impl Sync for Sys {}

static SYS: Sys = Sys {
    buf: UnsafeCell::new([0u8; GEM_CAP]),
    root: UnsafeCell::new(core::ptr::null_mut()),
    dir: UnsafeCell::new([0u8; 96]),
    sel: UnsafeCell::new([0u8; 16]),
    names: UnsafeCell::new([[0u8; 16]; NM_NAMES]),
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

    // SAFETY: single-CPU AES.
    let dir = unsafe { &mut *SYS.dir.get() };
    let sel = unsafe { &mut *SYS.sel.get() };
    let names = unsafe { &mut *SYS.names.get() };
    copy_in(dir, path_p);
    copy_in(sel, sel_p);

    // Redirect the dialog's TEDINFOs at our buffers.
    set_ted(tree, FSDIRECT, dir.as_mut_ptr(), dir.len());
    set_ted(tree, FSSELECT, sel.as_mut_ptr(), sel.len());
    for (i, n) in names.iter_mut().enumerate() {
        set_ted(tree, F1NAME + i as i16, n.as_mut_ptr(), n.len());
    }

    // Read the directory (path holds the dir + wildcard, e.g. C:\*.*).
    let dlen = dir.iter().position(|&b| b == 0).unwrap_or(dir.len());
    let dpath = core::str::from_utf8(&dir[..dlen]).unwrap_or("C:\\*.*");
    let mut raw = [[0u8; 14]; NM_NAMES];
    let mut attrs = [0u8; NM_NAMES];
    let count = crate::bdos::fs::find::list_dir(dpath, &mut raw, &mut attrs);
    for (i, n) in names.iter_mut().enumerate() {
        if i < count {
            copy_name(n, &raw[i]);
        } else {
            *n = [0; 16];
        }
    }

    // Centre the dialog and clear any stale selection.
    center(tree, backend);
    for i in 0..NM_NAMES as i16 {
        object::set_state_bit(tree, F1NAME + i, SELECTED, false);
    }
    // SAFETY: valid tree; backend is the live screen.
    unsafe { object::draw_root(tree as *const Object, MAX_DEPTH, backend) };

    let button = modal(tree, backend);

    // Write the (possibly updated) path + selection back to the caller.
    write_out(path_p, dir);
    write_out(sel_p, sel);
    pb.set_intout(1, button);
    pb.set_ret(1);
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

/// Modal loop: a click selects a file slot (copied to the selection
/// field), `OK`/`Cancel` exit. Returns 1 (OK) or 0 (Cancel).
fn modal(tree: *mut Object, backend: &dyn RasterBackend) -> i16 {
    let Some(h) = host() else {
        return 0;
    };
    let tp = tree as *const Object;
    let mut prev = crate::vdi::mouse_buttons();
    let mut cur_sel: i16 = -1;
    loop {
        let cur = crate::vdi::mouse_buttons();
        if cur & 0x01 != 0 && prev & 0x01 == 0 {
            let (mx, my) = crate::vdi::mouse_position();
            // SAFETY: valid tree.
            let hit = unsafe { object::hit_test(tp, 0, MAX_DEPTH, mx, my) };
            if hit == FSOK {
                return 1;
            }
            if hit == FSCANCEL {
                return 0;
            }
            if (F1NAME..F1NAME + NM_NAMES as i16).contains(&hit) {
                let slot = (hit - F1NAME) as usize;
                if cur_sel >= 0 {
                    object::set_state_bit(tree, cur_sel, SELECTED, false);
                }
                object::set_state_bit(tree, hit, SELECTED, true);
                cur_sel = hit;
                // SAFETY: single-CPU AES.
                let names = unsafe { &*SYS.names.get() };
                let sel = unsafe { &mut *SYS.sel.get() };
                copy_name(sel, &names[slot]);
                object::draw_obj(tp, FSSELECT, 0, backend);
            }
        }
        prev = cur;
        h.pump_input();
    }
}
