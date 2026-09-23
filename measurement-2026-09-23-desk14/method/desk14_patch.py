#!/usr/bin/env python3
"""DESK14-TEMP (scratch copy only): dump the file selector's clickable
rectangles to the serial line, so the injector's click points are taken from
the running build instead of guessed.

Logs, right after the selector's first `objc_draw`:
  * FSOK / FSCANCEL absolute rect,
  * F1NAME..F9NAME absolute rect + the listed name each slot carries.

Everything goes through `tos_api::aes::h02_log` (the a79 serial-only sink), so
nothing reaches the framebuffer and the screendumps stay clean.
"""
import sys

scr = sys.argv[1]


def patch(path, pairs):
    s = open(path).read()
    for a, b in pairs:
        assert s.count(a) == 1, (path, a[:70])
        s = s.replace(a, b, 1)
    open(path, 'w').write(s)
    print("patched", path)


# 1) abs_offset is private; the dump needs it.
patch(scr + '/tos-api/src/aes/object.rs', [
    ("fn abs_offset(t: ObjTree, obj: i16) -> (i16, i16) {",
     "pub(crate) fn abs_offset(t: ObjTree, obj: i16) -> (i16, i16) {"),
])

# 2) the dump itself, between the selector's first draw and the modal loop.
patch(scr + '/tos-api/src/aes/fsel.rs', [
    ("    let button = modal(tree, backend);",
     "    // DESK14-TEMP: click geometry, straight out of the drawn tree.\n"
     "    {\n"
     "        let dt = ObjTree::lp64(tree as *const Object);\n"
     "        for (obj, label) in [(FSOK, \"FSOK\"), (FSCANCEL, \"FSCANCEL\")] {\n"
     "            let (x, y) = object::abs_offset(dt, obj);\n"
     "            crate::aes::h02_log(format_args!(\n"
     "                \"[desk14] {} obj {} abs ({},{}) {}x{}\",\n"
     "                label, obj, x, y, dt.width(obj), dt.height(obj)));\n"
     "        }\n"
     "        for i in 0..NM_NAMES {\n"
     "            let obj = F1NAME + i as i16;\n"
     "            let (x, y) = object::abs_offset(dt, obj);\n"
     "            // SAFETY: single-CPU AES; the slot texts are live.\n"
     "            let nm = unsafe { &(*SYS.names.get())[i] };\n"
     "            let end = nm.iter().position(|&b| b == 0).unwrap_or(nm.len());\n"
     "            crate::aes::h02_log(format_args!(\n"
     "                \"[desk14] F{}NAME obj {} abs ({},{}) {}x{} = '{}'\",\n"
     "                i + 1, obj, x, y, dt.width(obj), dt.height(obj),\n"
     "                core::str::from_utf8(&nm[..end]).unwrap_or(\"?\")));\n"
     "        }\n"
     "    }\n"
     "    let button = modal(tree, backend);"),
])
