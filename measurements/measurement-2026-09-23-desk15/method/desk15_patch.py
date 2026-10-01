#!/usr/bin/env python3
"""DESK15-TEMP (scratch copy only), applied after the injector patches.

Three pieces of scaffolding, all serial-only (`kprintln!` writes to the UART,
not to `fbcon`, so the screendumps stay clean):

  1. `desk15_dump()` in kernel/src/desk/main.rs -- one line per *desktop* icon
     and one line per visible item of every open window, each with the absolute
     centre of its icon glyph (the point `act_chkobj` accepts as a hit).  Every
     click point in the DESK15 action lists comes from this dump; none is
     guessed.
  2. a call to it once the desktop is up (and again whenever a window is
     rebuilt is *not* needed -- the action lists re-open the dialog instead).
  3. the `ADINSWIN` rectangles, printed once `install_window_icon` has the
     dialog on screen, plus the `ANODE` the dialog resolved for the selected
     file.  The first `[desk15] ADINSWIN` line doubles as the screendump
     marker for "the dialog is up".
"""
import sys

scr = sys.argv[1]


def patch(path, pairs):
    s = open(path).read()
    for a, b in pairs:
        assert s.count(a) == 1, (path, a[:70], s.count(a))
        s = s.replace(a, b, 1)
    open(path, 'w').write(s)
    print("patched", path)


DUMP = '''
// DESK15-TEMP BEGIN (scratch copy only)
/// Serial dump of the desktop icons and the window items with their click
/// points.
pub fn desk15_dump() {
    use crate::kprintln;
    let top = wind_get_top();
    kprintln!("[desk15] top={}", top);
    global::with(|g| {
        // Desktop icons (children of DROOT).
        let (rx, ry) = (g.g_screen[DROOT as usize].ob_x, g.g_screen[DROOT as usize].ob_y);
        let mut child = g.g_screen[DROOT as usize].ob_head;
        while child != NIL && child != DROOT {
            let ib = &g.g_screeninfo[child as usize].block;
            let ox = rx + g.g_screen[child as usize].ob_x;
            let oy = ry + g.g_screen[child as usize].ob_y;
            kprintln!(
                "[desk15] desk obj {} box ({},{}) {}x{} -> click ({},{})",
                child, ox, oy,
                g.g_screen[child as usize].ob_width,
                g.g_screen[child as usize].ob_height,
                ox + ib.ib_xicon + ib.ib_wicon / 2,
                oy + ib.ib_yicon + ib.ib_hicon / 2
            );
            child = g.g_screen[child as usize].ob_next;
        }
        for slot in 0..NUM_WNODES {
            let wid = g.windows.slots[slot].w_id;
            if wid == 0 {
                continue;
            }
            let root = g.windows.slots[slot].w_root;
            let spec = g.windows.slots[slot].w_pnode.p_spec.clone();
            kprintln!("[desk15] win slot {} handle {} spec {}", slot, wid, spec);
            let (rx, ry) = (g.g_screen[root as usize].ob_x, g.g_screen[root as usize].ob_y);
            let mut child = g.g_screen[root as usize].ob_head;
            while child != NIL && child != root {
                let fnode = g.g_screeninfo[child as usize].fnode;
                let ib = &g.g_screeninfo[child as usize].block;
                let ox = rx + g.g_screen[child as usize].ob_x;
                let oy = ry + g.g_screen[child as usize].ob_y;
                let cx = ox + ib.ib_xicon + ib.ib_wicon / 2;
                let cy = oy + ib.ib_yicon + ib.ib_hicon / 2;
                let mut nm = [b' '; 14];
                if fnode >= 0 {
                    if let Some(f) = g.windows.slots[slot].w_pnode.p_flist.get(fnode as usize) {
                        let b = f.name_bytes();
                        let n = b.len().min(14);
                        nm[..n].copy_from_slice(&b[..n]);
                    }
                }
                kprintln!(
                    "[desk15]   obj {} '{}' box ({},{}) {}x{} -> click ({},{})",
                    child,
                    core::str::from_utf8(&nm).unwrap_or("?").trim_end(),
                    ox, oy,
                    g.g_screen[child as usize].ob_width,
                    g.g_screen[child as usize].ob_height,
                    cx, cy
                );
                child = g.g_screen[child as usize].ob_next;
            }
        }
    });
}
// DESK15-TEMP END
'''

patch(scr + '/kernel/src/desk/main.rs', [
    ("// ── Event loop ─", DUMP + "\n// ── Event loop ─"),
])

# a79_patch_desk10.py has already put the A46 DESK_UP store in front of the
# "[desk] desktop up" kprintln; hang the dump off that line.
ANCHOR = ("    crate::host_env::h02_inject::DESK_UP.store(true, "
          "core::sync::atomic::Ordering::Release); // A46-TEMP\n")
patch(scr + '/kernel/src/desk/mod.rs', [
    (ANCHOR, ANCHOR + "    main::desk15_dump(); // DESK15-TEMP\n"),
])

# The ADINSWIN rectangles + the resolved ANODE, once the dialog is on screen.
WIN_ANCHOR = ("    start_dialog(tree_ptr);\n"
              "    loop {\n"
              "        let exitobj = super::form_do(tree_ptr, edit_start) & 0x7fff;\n")
WIN_DUMP = ("    start_dialog(tree_ptr);\n"
            "    {\n"
            "        use crate::kprintln;\n"
            "        let (rx, ry) = (tree[0].ob_x, tree[0].ob_y);\n"
            "        kprintln!(\"[desk15] ADINSWIN ROOT ({},{}) {}x{} pa={:?} curr_icon={}\",\n"
            "            rx, ry, tree[0].ob_width, tree[0].ob_height,\n"
            "            pa.as_ref().map(|a| (a.idx, a.isapp, a.aicon, a.dicon)), curr_icon);\n"
            "        for (nm, i, px, py) in [\n"
            "            (\"IW_NAME\", IW_NAME, 0i16, 0i16),\n"
            "            (\"IW_FOLD\", IW_FOLD, MARGIN, y_radio),\n"
            "            (\"IW_FILE\", IW_FILE, MARGIN, y_radio),\n"
            "            (\"IW_UP\", IW_UP, gx, y_ibox),\n"
            "            (\"IW_DOWN\", IW_DOWN, gx, y_ibox),\n"
            "            (\"IW_INST\", IW_INST, 0, 0),\n"
            "            (\"IW_REMV\", IW_REMV, 0, 0),\n"
            "            (\"IW_SKIP\", IW_SKIP, 0, 0),\n"
            "            (\"IW_CNCL\", IW_CNCL, 0, 0),\n"
            "        ] {\n"
            "            let o = &tree[i as usize];\n"
            "            let ax = rx + px + o.ob_x;\n"
            "            let ay = ry + py + o.ob_y;\n"
            "            kprintln!(\"[desk15] {} abs ({},{}) {}x{} -> click ({},{}) st={:x}\",\n"
            "                nm, ax, ay, o.ob_width, o.ob_height,\n"
            "                ax + o.ob_width / 2, ay + o.ob_height / 2, o.ob_state);\n"
            "        }\n"
            "    }\n"
            "    loop {\n"
            "        let exitobj = super::form_do(tree_ptr, edit_start) & 0x7fff;\n")

patch(scr + '/kernel/src/desk/ins.rs', [(WIN_ANCHOR, WIN_DUMP)])

# A second dump after every window repaint, so an action list that opens a
# window gets the item click points of that window too.  `do_wredraw` runs with
# the desktop lock released (it re-takes it per owner rectangle), which the H02
# injector's own call site does not -- `desk15_dump` takes `global::with`, so
# hanging it off the injector deadlocks on the spin mutex.
patch(scr + '/kernel/src/desk/redraw.rs', [(
    "    graf_mouse(M_OFF);\n    do_wredraw_nocursor(handle, clip);\n    graf_mouse(M_ON);\n}",
    "    graf_mouse(M_OFF);\n    do_wredraw_nocursor(handle, clip);\n    graf_mouse(M_ON);\n"
    "    crate::desk::main::desk15_dump(); // DESK15-TEMP\n}")])
