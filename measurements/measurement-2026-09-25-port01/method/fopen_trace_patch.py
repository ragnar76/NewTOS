#!/usr/bin/env python3
"""PORT01-TEMP measurement patch (scratch copy only): serial trace of the file
calls a launched program makes.

After the crt0 fix QED receives its argument and tries to open it, but reports
a read error and shows a file name one character short. Two different things
could produce that picture -- QED asking for the wrong name, or the alert
being rendered short -- so this patch prints, over kprintln (UART only, never
fbcon, so the screendumps stay clean):

  1. every GEMDOS Fopen (0x3D) with the path string and the result,
  2. the alert string AES form_alert receives, verbatim with its length.

Applied to the scratch copy only, after the other patches.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70], s.count(a))
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


FILE = scr + '/tos-api/src/bdos/fs/file.rs'
FORM = scr + '/tos-api/src/aes/form.rs'

# 1) Fopen: path in, result out.
rep(FILE,
    """    let mode = (args.arg_u16(1) & 0x77) as u8;
""",
    """    let mode = (args.arg_u16(1) & 0x77) as u8;
    // PORT01-TEMP
    crate::aes::h02_log(format_args!(
        "[port01] Fopen len {} path {:?}", path.len(), path));
""")

# 2) the alert string form_alert is handed.
rep(FORM,
    """    let s = unsafe { cstr_slice(palstr) };
    pb.set_ret(do_alert(defbut, s, true));""",
    """    let s = unsafe { cstr_slice(palstr) };
    // PORT01-TEMP
    crate::aes::h02_log(format_args!(
        "[port01] form_alert len {} {:?}", s.len(), core::str::from_utf8(s)));
    pb.set_ret(do_alert(defbut, s, true));""")
