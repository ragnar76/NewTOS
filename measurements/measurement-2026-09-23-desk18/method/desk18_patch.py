#!/usr/bin/env python3
"""DESK18-TEMP (scratch copy only), applied after the injector patches.

Two pieces of scaffolding, both serial-only (`kprintln!` writes to the UART,
not to `fbcon`, so the screendumps stay clean):

  1. `desk18_dump()` in kernel/src/desk/main.rs -- one line per open window
     (its AES handle and `w_pnode.p_spec`), one line per *visible* item in it
     (name + the absolute centre of the icon glyph, which is the point
     `act_chkobj` accepts as a hit), plus the AES top window, `G.g_appdir`
     and the GEMDOS current drive.
  2. a call to it once the desktop is up.

No click point in the DESK18 action lists is guessed; every one comes out of
this dump.
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
// DESK18-TEMP BEGIN (scratch copy only)
/// Serial dump of the desktop's window items with their click points.
pub(super) fn desk18_dump() {
    use crate::kprintln;
    let top = wind_get_top();
    let drv = dos_gdrv();
    let appdir = global::with(|g| g.g_appdir).unwrap_or(true);
    kprintln!(
        "[desk18] top={} drive={} g_appdir={}",
        top,
        (b'A' + drv as u8) as char,
        appdir
    );
    global::with(|g| {
        for slot in 0..NUM_WNODES {
            let wid = g.windows.slots[slot].w_id;
            if wid == 0 {
                continue;
            }
            let root = g.windows.slots[slot].w_root;
            let spec = g.windows.slots[slot].w_pnode.p_spec.clone();
            kprintln!("[desk18] win slot {} handle {} spec {}", slot, wid, spec);
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
                    "[desk18]   obj {} '{}' box ({},{}) {}x{} -> click ({},{})",
                    child,
                    core::str::from_utf8(&nm).unwrap_or("?").trim_end(),
                    ox,
                    oy,
                    g.g_screen[child as usize].ob_width,
                    g.g_screen[child as usize].ob_height,
                    cx,
                    cy
                );
                child = g.g_screen[child as usize].ob_next;
            }
        }
    });
}
// DESK18-TEMP END
'''

patch(scr + '/kernel/src/desk/main.rs', [
    ("// ── Event loop ─", DUMP + "\n// ── Event loop ─"),
])

# a79_patch_desk10.py has already put the A46 DESK_UP store in front of the
# "[desk] desktop up" kprintln; hang the dump off that line.
ANCHOR = ("    crate::host_env::h02_inject::DESK_UP.store(true, "
          "core::sync::atomic::Ordering::Release); // A46-TEMP\n")
patch(scr + '/kernel/src/desk/mod.rs', [
    (ANCHOR, ANCHOR + "    main::desk18_dump(); // DESK18-TEMP\n"),
])

# 3. `inf_conf`'s own rectangles: the ROOT position plus the absolute centre of
#    the two *Default dir* radio buttons and of OK/Cancel, printed once the
#    dialog is on screen. Every dialog click point in the DESK18 action lists
#    comes from here, none is computed by hand.
CONF_ANCHOR = "    let tree_ptr = tree.as_mut_ptr() as u64;\n    start_dialog(tree_ptr);\n"
CONF_DUMP = CONF_ANCHOR + """\
    {
        use crate::kprintln;
        let rx = tree[0].ob_x;
        let ry = tree[0].ob_y;
        let (gx, gy) = (tree[DCDEFDIR as usize].ob_x, tree[DCDEFDIR as usize].ob_y);
        kprintln!("[desk18] ADDESKCF ROOT ({},{}) {}x{}", rx, ry, tree[0].ob_width, tree[0].ob_height);
        for (nm, i, px, py) in [
            ("DCDEFAPP", DCDEFAPP as i16, gx, gy),
            ("DCDEFWIN", DCDEFAPP as i16 + 1, gx, gy),
            ("DCOK", DCOK, 0, 0),
            ("DC_CNCL", DC_CNCL, 0, 0),
        ] {
            let o = &tree[i as usize];
            let ax = rx + px + o.ob_x;
            let ay = ry + py + o.ob_y;
            kprintln!(
                "[desk18] {} abs ({},{}) {}x{} -> click ({},{}) sel={}",
                nm, ax, ay, o.ob_width, o.ob_height,
                ax + o.ob_width / 2, ay + o.ob_height / 2,
                o.ob_state & 1
            );
        }
    }
"""

patch(scr + '/kernel/src/desk/inf.rs', [(CONF_ANCHOR, CONF_DUMP)])
