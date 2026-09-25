#!/usr/bin/env python3
"""DESK42 probe (scratch copy only), applied after deskrest_patch.py.

deskrest_patch.py's `[dlg]` dump prints only the EXIT objects of a desktop
dialog.  DESK42 also has to click a *radio button* (`FFRONLY`), so this widens
the dump to every object of the tree: type, flags, state and the absolute
rectangle with its centre point.  That is where the Read-Only click point in
d42c.actions comes from -- measured, not derived from the source by hand.

Serial only (`kprintln!` goes to the UART), so the screendumps stay clean.
"""
import sys

p = sys.argv[1] + '/kernel/src/desk/inf.rs'
s = open(p).read()

old = """            if ob.ob_flags & 0x0004 != 0 {
                crate::kprintln!(
                    "[dlg]   exit obj {} abs ({},{}) {}x{} -> click ({},{})",
                    i, o[0].ob_x + ob.ob_x, o[0].ob_y + ob.ob_y, ob.ob_width, ob.ob_height,
                    o[0].ob_x + ob.ob_x + ob.ob_width / 2, o[0].ob_y + ob.ob_y + ob.ob_height / 2);
            }"""

new = """            if ob.ob_flags & 0x0004 != 0 {
                crate::kprintln!(
                    "[dlg]   exit obj {} abs ({},{}) {}x{} -> click ({},{})",
                    i, o[0].ob_x + ob.ob_x, o[0].ob_y + ob.ob_y, ob.ob_width, ob.ob_height,
                    o[0].ob_x + ob.ob_x + ob.ob_width / 2, o[0].ob_y + ob.ob_y + ob.ob_height / 2);
            }
            {
                // DESK42-TEMP: every object, so a non-EXIT gadget can be
                // clicked from a measured point too.  A child's ob_x/ob_y are
                // relative to its parent; both trees DESK42 builds have exactly
                // one nesting level (the radio pair's G_IBOX), so the parent
                // offset is added by hand here rather than by walking up.
                let (px, py) = if i >= 9 && i <= 10 && o[8].ob_type == 25 {
                    (o[8].ob_x, o[8].ob_y)
                } else {
                    (0, 0)
                };
                crate::kprintln!(
                    "[dlg]   obj {} ty {} fl {:#06x} st {:#06x} abs ({},{}) {}x{} -> click ({},{})",
                    i, ob.ob_type, ob.ob_flags, ob.ob_state,
                    o[0].ob_x + px + ob.ob_x, o[0].ob_y + py + ob.ob_y, ob.ob_width, ob.ob_height,
                    o[0].ob_x + px + ob.ob_x + ob.ob_width / 2,
                    o[0].ob_y + py + ob.ob_y + ob.ob_height / 2);
            }"""

assert s.count(old) == 1, s.count(old)
open(p, 'w').write(s.replace(old, new, 1))
print("patched", p)
