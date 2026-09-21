#!/usr/bin/env python3
"""A73-TEMP (scratch copy only), after a68_patch.py: serial trace of every
evnt_multi completion and of every menu-bar track result, named by the
injector's Step::Arm (H02_TRACE). Works on both the fixed and the HEAD
event.rs (the anchors exist in both)."""
import sys
scr = sys.argv[1]
def patch(path, pairs):
    s = open(path).read()
    for a, b in pairs:
        assert s.count(a) == 1, (path, a[:70])
        s = s.replace(a, b, 1)
    open(path, 'w').write(s)
    print("patched", path)
patch(scr + '/tos-api/src/aes/event.rs', [
    ("            if fired != 0 {\n                break;\n            }\n",
     "            if fired != 0 {\n"
     "                if super::H02_TRACE.load(Ordering::Relaxed) { // A73-TEMP\n"
     "                    let (tx, ty) = crate::vdi::mouse_position();\n"
     "                    super::h02_log(format_args!(\"[a73] evnt_multi fired={:#06x} clicks={} mouse=({},{}) mb={:#04x} pass={}\", fired, clicks, tx, ty, crate::vdi::mouse_buttons(), pass));\n"
     "                }\n"
     "                break;\n            }\n"),
    ("    match super::menu::track(tree, backend) {\n        Some((title, item)) => {\n",
     "    let a73_r = super::menu::track(tree, backend); // A73-TEMP\n"
     "    if super::H02_TRACE.load(Ordering::Relaxed) {\n"
     "        let (tx, ty) = crate::vdi::mouse_position();\n"
     "        super::h02_log(format_args!(\"[a73] track -> {:?} at ({},{}) mb={:#04x}\", a73_r, tx, ty, crate::vdi::mouse_buttons()));\n"
     "    }\n"
     "    match a73_r {\n        Some((title, item)) => {\n"),
])
