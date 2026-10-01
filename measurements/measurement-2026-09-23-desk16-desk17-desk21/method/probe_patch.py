#!/usr/bin/env python3
"""DESK16/DESK17/DESK21 probe (scratch copy only), applied after the injector
patches.

Serial-only scaffolding (`kprintln!` goes to the UART, never to `fbcon`, so the
screendumps stay clean):

  1. `probe_dump()` in kernel/src/desk/menu.rs -- the absolute rectangle of
     every menu title, every drop-down box and every drop-down entry, with the
     point that hovers/clicks it and the entry's DISABLED bit.  Printed from
     the end of `men_update`, but only when the DISABLED pattern actually
     changed, so the log stays readable.  EVERY click point in the action lists
     comes from this dump; none is guessed.
  2. `probe_icons()` in kernel/src/desk/main.rs -- one line per desktop icon
     and per visible window item with the centre of its icon glyph (the point
     `act_chkobj` accepts as a hit).  Copied from
     measurement-2026-09-23-desk15/method/desk15_patch.py (pieces 1, 2, 4).

The patch touches only files whose anchors exist both at HEAD a8d4b42 and
after the DESK16/17/21 change, so the same script builds the "before" and the
"after" kernel.
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


MENU_DUMP = '''
// PROBE-TEMP BEGIN (scratch copy only)
/// Serial dump of the menu-bar geometry and of every drop-down entry's
/// DISABLED bit.  Printed only when the DISABLED pattern changed.
pub(super) fn probe_dump() {
    use core::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    build_menu();
    // SAFETY: single-CPU desktop, tree built above.
    let o = unsafe { &*MENU.obj.get() };
    let labels = unsafe { &*MENU.labels.get() };
    let mut mask = 1u64;
    for (i, ob) in o.iter().enumerate().take(63) {
        if ob.ob_state & DISABLED != 0 {
            mask |= 1u64 << (i + 1);
        }
    }
    if LAST.swap(mask, Ordering::Relaxed) == mask {
        return;
    }
    let active = &o[2];
    crate::kprintln!(
        "[probe] active ({},{}) {}x{}",
        active.ob_x, active.ob_y, active.ob_width, active.ob_height
    );
    for ti in 0..MENUS.len() {
        let t = &o[3 + ti];
        let ax = active.ob_x + t.ob_x;
        crate::kprintln!(
            "[probe] title {} abs ({},{}) {}x{} hover ({},{}) st={:x}",
            ti, ax, t.ob_y, t.ob_width, t.ob_height,
            ax + t.ob_width / 2, t.ob_y + t.ob_height / 2, t.ob_state
        );
    }
    let themenus = (3 + MENUS.len()) as i16;
    let mut bi = o[themenus as usize].ob_head;
    let mut ti = 0usize;
    while bi != NIL && bi != themenus {
        let b = &o[bi as usize];
        crate::kprintln!(
            "[probe] box {} abs ({},{}) {}x{}",
            ti, b.ob_x, b.ob_y, b.ob_width, b.ob_height
        );
        let mut it = b.ob_head;
        let mut n = 0usize;
        while it != NIL && it != bi {
            let e = &o[it as usize];
            let ax = b.ob_x + e.ob_x;
            let ay = b.ob_y + e.ob_y;
            let lab = &labels[it as usize];
            let end = lab.iter().position(|&c| c == 0).unwrap_or(0);
            crate::kprintln!(
                "[probe]   {}.{} obj {} '{}' abs ({},{}) {}x{} click ({},{}) dis={}",
                ti, n, it,
                core::str::from_utf8(&lab[..end]).unwrap_or("?"),
                ax, ay, e.ob_width, e.ob_height,
                ax + e.ob_width / 2, ay + e.ob_height / 2,
                u8::from(e.ob_state & DISABLED != 0)
            );
            it = e.ob_next;
            n += 1;
        }
        bi = b.ob_next;
        ti += 1;
    }
}
// PROBE-TEMP END
'''

patch(scr + '/kernel/src/desk/menu.rs', [
    ("    set_checked(Item::SizeToFit, fit);\n}\n",
     "    set_checked(Item::SizeToFit, fit);\n    probe_dump(); // PROBE-TEMP\n}\n" + MENU_DUMP),
])

ICON_DUMP = '''
// PROBE-TEMP BEGIN (scratch copy only)
/// Serial dump of the desktop icons and the window items with their click
/// points (copied from measurement-2026-09-23-desk15/method/desk15_patch.py).
pub fn probe_icons() {
    use crate::kprintln;
    let top = wind_get_top();
    kprintln!("[probe] top={}", top);
    global::with(|g| {
        let (rx, ry) = (g.g_screen[DROOT as usize].ob_x, g.g_screen[DROOT as usize].ob_y);
        let mut child = g.g_screen[DROOT as usize].ob_head;
        while child != NIL && child != DROOT {
            let ib = &g.g_screeninfo[child as usize].block;
            let ox = rx + g.g_screen[child as usize].ob_x;
            let oy = ry + g.g_screen[child as usize].ob_y;
            kprintln!(
                "[probe] desk obj {} idx {} box ({},{}) {}x{} -> click ({},{})",
                child,
                g.g_screeninfo[child as usize].index,
                ox, oy,
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
            kprintln!("[probe] win slot {} handle {} spec {}", slot, wid, spec);
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
                    "[probe]   obj {} '{}' box ({},{}) {}x{} -> click ({},{})",
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
// PROBE-TEMP END
'''

patch(scr + '/kernel/src/desk/main.rs', [
    ("// ── Event loop ─", ICON_DUMP + "\n// ── Event loop ─"),
])

ANCHOR = ("    crate::host_env::h02_inject::DESK_UP.store(true, "
          "core::sync::atomic::Ordering::Release); // A46-TEMP\n")
patch(scr + '/kernel/src/desk/mod.rs', [
    (ANCHOR, ANCHOR + "    main::probe_icons(); // PROBE-TEMP\n"),
])

patch(scr + '/kernel/src/desk/redraw.rs', [(
    "    graf_mouse(M_OFF);\n    do_wredraw_nocursor(handle, clip);\n    graf_mouse(M_ON);\n}",
    "    graf_mouse(M_OFF);\n    do_wredraw_nocursor(handle, clip);\n    graf_mouse(M_ON);\n"
    "    crate::desk::main::probe_icons(); // PROBE-TEMP\n}")])

DSK_ANCHOR = ("    start_dialog(tree_ptr);\n"
              "    loop {\n"
              "        let mut retry = false;\n")
DSK_DUMP = ("    start_dialog(tree_ptr);\n"
            "    {\n"
            "        use crate::kprintln;\n"
            "        let (rx, ry) = (tree[0].ob_x, tree[0].ob_y);\n"
            "        kprintln!(\"[probe] ADINSDSK ROOT ({},{}) {}x{} curr_icon={} of {} ig={}\",\n"
            "            rx, ry, tree[0].ob_width, tree[0].ob_height,\n"
            "            curr_icon, list.len(), list[curr_icon].0);\n"
            "        for (nm, i, px, py) in [\n"
            "            (\"ID_ID\", ID_ID, 0i16, 0i16),\n"
            "            (\"ID_LABEL\", ID_LABEL, 0, 0),\n"
            "            (\"ID_DRIVE\", ID_DRIVE, MARGIN, y_radio),\n"
            "            (\"ID_TRASH\", ID_DRIVE + 1, MARGIN, y_radio),\n"
            "            (\"ID_PRINT\", ID_DRIVE + 2, MARGIN, y_radio),\n"
            "            (\"ID_UP\", ID_UP, gx, y_ibox),\n"
            "            (\"ID_DOWN\", ID_DOWN, gx, y_ibox),\n"
            "            (\"ID_OK\", ID_OK, 0, 0),\n"
            "            (\"ID_SKIP\", ID_SKIP, 0, 0),\n"
            "            (\"ID_CNCL\", ID_CNCL, 0, 0),\n"
            "        ] {\n"
            "            let o = &tree[i as usize];\n"
            "            let ax = rx + px + o.ob_x;\n"
            "            let ay = ry + py + o.ob_y;\n"
            "            kprintln!(\"[probe] {} abs ({},{}) {}x{} -> click ({},{}) st={:x}\",\n"
            "                nm, ax, ay, o.ob_width, o.ob_height,\n"
            "                ax + o.ob_width / 2, ay + o.ob_height / 2, o.ob_state);\n"
            "        }\n"
            "    }\n"
            "    loop {\n"
            "        let mut retry = false;\n")

patch(scr + '/kernel/src/desk/ins.rs', [(DSK_ANCHOR, DSK_DUMP)])
