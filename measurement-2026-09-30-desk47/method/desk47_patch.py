#!/usr/bin/env python3
"""DESK47-TEMP (scratch copy only) -- dump the ADOPENAP tree object by object
at run time, the way measurement-2026-09-25-desk42/method/desk42_patch.py did
for ADDISKIN/ADFFINFO, plus one line with the tail `opn_appl` hands back.

The point is that the tree can be diffed against desk_rsc.c:877-919 from the
serial log alone, without reading our source.
"""
import sys
scr = sys.argv[1]

def rep(path, a, b, n=1):
    s = open(path).read()
    assert s.count(a) == n, (path, s.count(a), a[:70])
    open(path, 'w').write(s.replace(a, b, n))
    print("patched", path)

p = scr + '/kernel/src/desk/inf.rs'
rep(p,
    """    align_title(&mut tree, TITLE.len() as i16 - 1);

    // `inf_show(tree, APPLPARM);` — the **start field**, not `ROOT`""",
    """    align_title(&mut tree, TITLE.len() as i16 - 1);

    // DESK47-TEMP: dump ADOPENAP object by object, after form_center has run.
    {
        let t = super::form_center(tree.as_mut_ptr() as u64);
        crate::kprintln!("[dlg] ADOPENAP root ({},{}) {}x{}", t.x, t.y, t.w, t.h);
        for (i, o) in tree.iter().enumerate() {
            crate::kprintln!(
                "[dlg] obj {} ty {} fl 0x{:04x} st 0x{:04x} xywh {},{},{},{} next {} head {} tail {}",
                i, o.ob_type, o.ob_flags, o.ob_state,
                o.ob_x, o.ob_y, o.ob_width, o.ob_height,
                o.ob_next, o.ob_head, o.ob_tail);
        }
    }

    // `inf_show(tree, APPLPARM);` — the **start field**, not `ROOT`""")

rep(p,
    """    if inf_what(&mut tree, APPLOK) != 0 {
        ptail.clear();
        ptail.extend_from_slice(inf_sget(&b_parm));
        return true;
    }
    false""",
    """    let what = inf_what(&mut tree, APPLOK);
    crate::kprintln!("[dlg] opn_appl inf_what = {}", what);
    if what != 0 {
        ptail.clear();
        ptail.extend_from_slice(inf_sget(&b_parm));
        crate::kprintln!(
            "[dlg] opn_appl tail len {} = {:?}",
            ptail.len(),
            core::str::from_utf8(ptail).unwrap_or("<non-utf8>"));
        return true;
    }
    crate::kprintln!("[dlg] opn_appl cancelled");
    false""")
