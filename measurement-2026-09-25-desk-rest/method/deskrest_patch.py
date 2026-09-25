#!/usr/bin/env python3
"""DESK31/32/33/38 probe (scratch copy only), applied after probe_patch.py.

Serial-only scaffolding — `h02_log`/`kprintln!` go to the UART, never to
`fbcon`, so the screendumps stay clean:

  1. tos-api/src/aes/form.rs — every `form_alert` box + button rectangle with
     its click point (copied verbatim from
     measurement-2026-09-23-desk24-desk25-desk26/method/desk24_patch.py piece 3).
     That is where the click points for DESK31's `STRMVDEL`/`STILLDIR` and for
     DESK38's `STTRINFO`/`STPRINFO` alerts come from.

  2. kernel/src/desk/inf.rs — `start_dialog` dumps the centred tree's ROOT
     rectangle and every EXIT button in it, so the OK/Cancel points of the
     `ADDISKIN` (DESK38) and `ADFFINFO` boxes are measured too.

  3. kernel/src/desk/main.rs — one line per `do_info` call with the icon type it
     resolved, and one per `process_funkey` (DESK33), so the serial log says
     which arm ran even where the picture cannot.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70], s.count(a))
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


# 1) every form_alert: box + buttons with their click points
rep(scr + '/tos-api/src/aes/form.rs',
    """    // Hide the cursor while drawing the alert so its save-under does not""",
    """    {
        // DESKREST-TEMP: the alert's geometry, so the alert click points are
        // measured and not guessed.
        crate::aes::h02_log(format_args!(
            "[alert] box ({},{}) {}x{} nbut={} defbut={}\\r\\n",
            box_x, box_y, box_w, box_h, numbut, defbut));
        for j in 0..numbut {
            let o = &tree[first_btn + j];
            crate::aes::h02_log(format_args!(
                "[alert]   but {} abs ({},{}) {}x{} -> click ({},{})\\r\\n",
                j + 1, box_x + o.ob_x, box_y + o.ob_y, o.ob_width, o.ob_height,
                box_x + o.ob_x + o.ob_width / 2, box_y + o.ob_y + o.ob_height / 2));
        }
    }
    // Hide the cursor while drawing the alert so its save-under does not""")

# 2) every desktop dialog: the centred ROOT rect + every EXIT button in it
rep(scr + '/kernel/src/desk/inf.rs',
    """    super::form_dial_start(t);
    super::objc_draw_dialog(tree, t);
    t
}""",
    """    super::form_dial_start(t);
    super::objc_draw_dialog(tree, t);
    {
        // DESKREST-TEMP: the dialog's geometry, so its button click points are
        // measured and not guessed.
        // SAFETY: `tree` is the caller's live object array; the walk stops at
        // LASTOB, which every desktop dialog sets.
        let o = unsafe { core::slice::from_raw_parts(tree as *const Object, 64) };
        crate::kprintln!("[dlg] root ({},{}) {}x{}", o[0].ob_x, o[0].ob_y, o[0].ob_width, o[0].ob_height);
        for (i, ob) in o.iter().enumerate().take(64) {
            if ob.ob_flags & 0x0004 != 0 {
                crate::kprintln!(
                    "[dlg]   exit obj {} abs ({},{}) {}x{} -> click ({},{})",
                    i, o[0].ob_x + ob.ob_x, o[0].ob_y + ob.ob_y, ob.ob_width, ob.ob_height,
                    o[0].ob_x + ob.ob_x + ob.ob_width / 2, o[0].ob_y + ob.ob_y + ob.ob_height / 2);
            }
            if ob.ob_flags & 0x0020 != 0 {
                break;
            }
        }
    }
    t
}""")

# 3) do_info / process_funkey trace
rep(scr + '/kernel/src/desk/main.rs',
    """    let Some((atype, _isapp)) = dir::i_find_atype(cwin, curr) else {
        return 1; // `if (!pa) return ret;`
    };""",
    """    let Some((atype, _isapp)) = dir::i_find_atype(cwin, curr) else {
        crate::kprintln!("[d38] do_info(curr={}) cwin={} -> no ANODE", curr, cwin);
        return 1; // `if (!pa) return ret;`
    };
    crate::kprintln!("[d38] do_info(curr={}) cwin={} atype={:?}", curr, cwin, atype);""")

rep(scr + '/kernel/src/desk/main.rs',
    """    let pa = super::inf::app_by_funkey(funkey as u8)?;""",
    """    let pa = match super::inf::app_by_funkey(funkey as u8) {
        Some(pa) => pa,
        None => {
            crate::kprintln!("[d33] process_funkey({}) -> no application holds it", funkey);
            return None;
        }
    };
    crate::kprintln!("[d33] process_funkey({}) -> {} appdir={}", funkey, pa.app_path, pa.app_dir);""")

# `AType` already derives Debug (kernel/src/desk/model.rs), so the do_info
# trace needs no further patch.
