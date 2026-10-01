#!/usr/bin/env python3
"""DESK24/DESK25/DESK26-TEMP (scratch copy only): the serial scaffolding these
three board runs need on top of the standard injector + probe patches.

Every dump goes through `tos_api::aes::h02_log` (the a79 serial-only sink) or
`kprintln!` (UART only), so nothing reaches the framebuffer and the screendumps
stay clean.

1) kernel/src/desk/dir.rs   -- `ADCPYDEL` geometry + the `confirm` decision
                               (copied from measurement-2026-09-23-desk13).
2) kernel/src/desk/dir.rs   -- `ADCPALER` geometry + the two names it was
                               pre-filled with + the button that came back
                               (same source).  This is the dialog DESK24 makes
                               the *folder* conflict use, so the two names tell
                               file case from folder case apart in the log.
3) tos-api/src/aes/form.rs  -- every `form_alert` box + button rectangle with
                               its click point.  DESK25's two alerts
                               (`STRMVDEL`, `STDELDIS`) are plain alerts, so
                               this is where their click points are measured.
4) kernel/src/desk/main.rs  -- every `win_sinfo` call: the handle, the
                               `check_selected` argument and the info line it
                               produced.  DESK26 is exactly a missing call with
                               `check_selected == false`, so the log shows the
                               defect and the fix without reading pixels.
5) kernel/src/desk/dir.rs   -- [after only] what `output_path` was asked and
                               what it decided (DESK24), and the `STDELDIS`
                               question with its answer (DESK25).

Applied to the scratch copy AFTER a79_patch_desk10.py / apply_injector.py /
a78_patch2.py / key_step_patch.py / probe_patch.py, never to the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b, optional=False):
    s = open(path).read()
    n = s.count(a)
    if n == 0 and optional:
        print("skipped (anchor absent, 'before' tree):", a[:50].replace("\n", " "))
        return
    assert n == 1, (path, a[:70], n)
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


DIR = scr + '/kernel/src/desk/dir.rs'

# 1) ADCPYDEL geometry + the confirm decision (desk13 piece 1)
rep(DIR,
    """        ctx.tree = b.ptr();
        start_dialog(ctx.tree);""",
    """        ctx.tree = b.ptr();
        start_dialog(ctx.tree);
        {
            // DESK24-TEMP
            let (rx, ry) = super::objc_offset(ctx.tree, 0);
            let (ox, oy) = super::objc_offset(ctx.tree, CDOK);
            let (cx, cy) = super::objc_offset(ctx.tree, CDOK + 1);
            tos_api::aes::h02_log(format_args!(
                "[d24] CPYDEL root=({},{},{},{}) CDOK=({},{}) CDCNCL=({},{}) files={} dirs={} confirm={}\\r\\n",
                rx, ry, b.tree[0].ob_width, b.tree[0].ob_height,
                ox, oy, cx, cy, ctx.files, ctx.dirs, confirm));
        }""")

# 2) ADCPALER geometry + the names + the answer (desk13 piece 2)
rep(DIR,
    """    super::form_do(tree_ptr, 0);

    // "trigger a redraw for the copy alert dialog""",
    """    {
        // DESK24-TEMP
        let (rx, ry) = super::objc_offset(tree_ptr, 0);
        let (ox, oy) = super::objc_offset(tree_ptr, CAOK);
        let (kx, ky) = super::objc_offset(tree_ptr, CASKIP);
        let (sx, sy) = super::objc_offset(tree_ptr, CASTOP);
        tos_api::aes::h02_log(format_args!(
            "[d24] CPALER root=({},{},{},{}) CAOK=({},{}) CASKIP=({},{}) CASTOP=({},{}) curr={:?} copy={:?}\\r\\n",
            rx, ry, tree[0].ob_width, tree[0].ob_height, ox, oy, kx, ky, sx, sy,
            core::str::from_utf8(&src_copy[..src_copy.iter().position(|&c| c == 0).unwrap_or(0)]).unwrap_or("?"),
            core::str::from_utf8(&dst[..dst.iter().position(|&c| c == 0).unwrap_or(0)]).unwrap_or("?")));
    }
    super::form_do(tree_ptr, 0);

    // "trigger a redraw for the copy alert dialog\"""")

rep(DIR,
    """    inf_gindex(&tree, CAOK as usize, 3) + CAOK
}""",
    """    let ob = inf_gindex(&tree, CAOK as usize, 3) + CAOK;
    // DESK24-TEMP
    tos_api::aes::h02_log(format_args!(
        "[d24] CPALER answer ob={} (CAOK={} CASKIP={} CASTOP={}) copy now {:?}\\r\\n",
        ob, CAOK, CASKIP, CASTOP,
        core::str::from_utf8(&dst[..dst.iter().position(|&c| c == 0).unwrap_or(0)]).unwrap_or("?")));
    ob
}""")

# 5a) what output_path was asked and what it decided  (DESK24, after only)
rep(DIR,
    """                    match output_path(op, &mut dst_sub, &mut ctx) {""",
    """                    let __op_in = dst_sub.clone(); // DESK24-TEMP
                    let __op_r = output_path(op, &mut dst_sub, &mut ctx);
                    tos_api::aes::h02_log(format_args!(
                        "[d24] output_path({:?}, {:?}) -> {:?}, dst now {:?}\\r\\n",
                        op, __op_in.as_str(), __op_r, dst_sub.as_str()));
                    match __op_r {""",
    optional=True)

# 5b) the STDELDIS question and its answer  (DESK25, after only)
rep(DIR,
    """        let letter = src_dir.as_bytes().first().copied().unwrap_or(b'?');
        if !delete_disk_alert(letter) {
            ctx.stop = true;
        }""",
    """        let letter = src_dir.as_bytes().first().copied().unwrap_or(b'?');
        let __yes = delete_disk_alert(letter); // DESK25-TEMP
        tos_api::aes::h02_log(format_args!(
            "[d25] STDELDIS drive {:?} icontype={:?} -> yes={}\\r\\n",
            letter as char, icontype, __yes));
        if !__yes {
            ctx.stop = true;
        }""",
    optional=True)

# 5c) `dir_op` entry: op + icontype, so the "before" log shows the parameter is
#     simply absent ("icontype=<absent>") and the "after" log shows what
#     reached it.  The argument only exists in the "after" tree.
HAS_ICONTYPE = 'icontype: Option<AType>' in open(DIR).read()
ICON_ARG = 'icontype' if HAS_ICONTYPE else '"<absent>"'
rep(DIR,
    """    let mut ok = true;
    for e in entries {
        if ctx.stop {
            break;
        }
        let name = e.name_str();
        if e.is_dir {""",
    """    tos_api::aes::h02_log(format_args!(
        "[d25] dir_op op={:?} icontype={:?} src={:?} n={} stop={}\\r\\n",
        op, """ + ICON_ARG + """, src_dir, entries.len(), ctx.stop)); // DESK25-TEMP
    let mut ok = true;
    for e in entries {
        if ctx.stop {
            break;
        }
        let name = e.name_str();
        if e.is_dir {""")

# 3) every form_alert: box + buttons with their click points
rep(scr + '/tos-api/src/aes/form.rs',
    """    // Hide the cursor while drawing the alert so its save-under does not""",
    """    {
        // DESK24-TEMP: the alert's geometry, so DESK25's two alert click
        // points are measured and not guessed.
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

# 4) every win_sinfo call with its argument and the line it produced (DESK26)
rep(scr + '/kernel/src/desk/main.rs',
    """    if let Some((w_id, ptr)) = info {
        super::wind_set_ptr(w_id, WF_INFO, ptr);
    }
}""",
    """    if let Some((w_id, ptr)) = info {
        {
            // DESK26-TEMP
            // SAFETY: `ptr` is the node's own NUL-terminated `w_info` buffer.
            let s = unsafe { core::slice::from_raw_parts(ptr as *const u8, 80) };
            let n = s.iter().position(|&c| c == 0).unwrap_or(0);
            tos_api::aes::h02_log(format_args!(
                "[d26] win_sinfo(h={}, check_selected={}) -> {:?}\\r\\n",
                handle, check_selected, core::str::from_utf8(&s[..n]).unwrap_or("?")));
        }
        super::wind_set_ptr(w_id, WF_INFO, ptr);
    }
}""")
