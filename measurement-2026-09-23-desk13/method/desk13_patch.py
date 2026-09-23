#!/usr/bin/env python3
"""DESK13-TEMP (scratch copy only): the serial scaffolding the DESK13 board run
needs on top of the standard injector patches.

Every dump goes through `tos_api::aes::h02_log` (the a79 serial-only sink), so
nothing is written to the framebuffer and the screendumps stay clean.

1) kernel/src/desk/dir.rs — when the `ADCPYDEL` copy/delete box goes up, print
   the absolute rectangles of its two exit buttons, so the click points are
   measured rather than guessed; and print the `confirm` decision with the
   three preference values behind it.
2) kernel/src/desk/dir.rs — the same for the `ADCPALER` name-conflict box's
   three exit buttons, plus the two names it was pre-filled with and the button
   that came back.
3) kernel/src/desk/inf.rs — the `ADSETPRE` box size and its OK/Cancel
   rectangle (the dialog is put up inside the blocking `inf_show`, so the
   position is derived from `form_center`'s rule and checked against the
   picture).
4) kernel/src/desk/mod.rs — the `_IDT` word the desktop read at start, i.e.
   what `DateFmt::Idt`/`TimeFmt::Idt` resolve against.

Applied to the scratch copy AFTER a79_patch_desk10.py / apply_injector.py /
a78_patch2.py / key_step_patch.py, never to the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70])
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


# 1) ADCPYDEL geometry + the confirm decision
rep(scr + '/kernel/src/desk/dir.rs',
    """        ctx.tree = b.ptr();
        start_dialog(ctx.tree);""",
    """        ctx.tree = b.ptr();
        start_dialog(ctx.tree);
        {
            // DESK13-TEMP
            let (rx, ry) = super::objc_offset(ctx.tree, 0);
            let (ox, oy) = super::objc_offset(ctx.tree, CDOK);
            let (cx, cy) = super::objc_offset(ctx.tree, CDOK + 1);
            tos_api::aes::h02_log(format_args!(
                "[desk13] CPYDEL root=({},{},{},{}) CDOK=({},{}) CDCNCL=({},{}) files={} dirs={} confirm={} (cdel={} ccpy={} covwr={})\\r\\n",
                rx, ry, b.tree[0].ob_width, b.tree[0].ob_height,
                ox, oy, cx, cy, ctx.files, ctx.dirs, confirm, confdel, confcpy, covwr));
        }""")

# 2) ADCPALER geometry + the names + the answer
rep(scr + '/kernel/src/desk/dir.rs',
    """    super::form_do(tree_ptr, 0);

    // "trigger a redraw for the copy alert dialog""",
    """    {
        // DESK13-TEMP
        let (rx, ry) = super::objc_offset(tree_ptr, 0);
        let (ox, oy) = super::objc_offset(tree_ptr, CAOK);
        let (kx, ky) = super::objc_offset(tree_ptr, CASKIP);
        let (sx, sy) = super::objc_offset(tree_ptr, CASTOP);
        tos_api::aes::h02_log(format_args!(
            "[desk13] CPALER root=({},{},{},{}) CAOK=({},{}) CASKIP=({},{}) CASTOP=({},{}) curr={:?} copy={:?}\\r\\n",
            rx, ry, tree[0].ob_width, tree[0].ob_height, ox, oy, kx, ky, sx, sy,
            core::str::from_utf8(&src_copy[..src_copy.iter().position(|&c| c == 0).unwrap_or(0)]).unwrap_or("?"),
            core::str::from_utf8(&dst[..dst.iter().position(|&c| c == 0).unwrap_or(0)]).unwrap_or("?")));
    }
    super::form_do(tree_ptr, 0);

    // "trigger a redraw for the copy alert dialog\"""")

rep(scr + '/kernel/src/desk/dir.rs',
    """    inf_gindex(&tree, CAOK as usize, 3) + CAOK
}""",
    """    let ob = inf_gindex(&tree, CAOK as usize, 3) + CAOK;
    // DESK13-TEMP
    tos_api::aes::h02_log(format_args!("[desk13] CPALER answer ob={} (CAOK={} CASKIP={} CASTOP={})\\r\\n", ob, CAOK, CASKIP, CASTOP));
    ob
}""")

# 2b) what output_fname decided, and what d_dofcopy did with it
rep(scr + '/kernel/src/desk/dir.rs',
    """    match output_fname(src, dst, ctx) {
        More::Ok => {}""",
    """    let __of = output_fname(src, dst, ctx);
    tos_api::aes::h02_log(format_args!("[desk13] output_fname({:?}) -> {:?}, dst now {:?}\\r\\n", src, __of, dst.as_str())); // DESK13-TEMP
    match __of {
        More::Ok => {}""")

# 3) ADSETPRE size + OK/Cancel rectangle
rep(scr + '/kernel/src/desk/inf.rs',
    """    // ── run it ──────────────────────────────────────────────────────────
    inf_show(tree.as_mut_ptr() as u64, ROOT);""",
    """    // DESK13-TEMP
    tos_api::aes::h02_log(format_args!(
        "[desk13] ADSETPRE box=({},{}) SPOK=({},{},{},{}) SPCNCL=({},{}) pre: cdel={} ccpy={} covwr={} dclick={} tf={:?} df={:?}\\r\\n",
        dw, dh, tree[SPOK].ob_x, tree[SPOK].ob_y, tree[SPOK].ob_width, tree[SPOK].ob_height,
        tree[SPOK + 1].ob_x, tree[SPOK + 1].ob_y, confdel, confcpy, confovwr, dclick, timefmt, datefmt));
    // ── run it ──────────────────────────────────────────────────────────
    inf_show(tree.as_mut_ptr() as u64, ROOT);""")

# 4) the _IDT word the desktop read
rep(scr + '/kernel/src/desk/redraw.rs',
    """    if let Some(idt) = dos_getcookie(COOKIE_IDT) {
        global::with(|g| g.g_idt = idt);
    }""",
    """    if let Some(idt) = dos_getcookie(COOKIE_IDT) {
        global::with(|g| g.g_idt = idt);
        // DESK13-TEMP: once, so the serial log carries the word the
        // `Default` buttons resolve against.
        use core::sync::atomic::{AtomicBool, Ordering};
        static ONCE: AtomicBool = AtomicBool::new(false);
        if !ONCE.swap(true, Ordering::Relaxed) {
            tos_api::aes::h02_log(format_args!("[desk13] _IDT = {:#06x} via Ssystem(S_GETCOOKIE)\\r\\n", idt));
        }
    } else {
        tos_api::aes::h02_log(format_args!("[desk13] _IDT read FAILED\\r\\n"));
    }""")
