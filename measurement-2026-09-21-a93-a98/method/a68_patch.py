#!/usr/bin/env python3
"""A68-A70-TEMP (scratch copy only): H02 serial AES trace hooks + A46 desktop-up
flag, as measurement-2026-09-20-a78/method/a78_patch.py minus the pd.rs
post_msg trace (anchor no longer present, not needed here). Applied before
apply_injector.py."""
import sys
scr = sys.argv[1]
def patch(path, pairs):
    s = open(path).read()
    for a, b in pairs:
        assert s.count(a) == 1, (path, a[:60])
        s = s.replace(a, b, 1)
    open(path, 'w').write(s)
    print("patched", path)
# 1) Host::debug_write default (serial-only sink)
patch(scr + '/tos-api/src/host.rs', [(
    "    /// Non-blocking read from the console input device.",
    "    /// H02-TEMP (scratch copy only): serial-only debug write.\n    fn debug_write(&self, s: &[u8]) {\n        self.console_write_bytes(s)\n    }\n\n    /// Non-blocking read from the console input device.")])
# 2) H02_TRACE + h02_log (named by the injector's Step::Arm)
patch(scr + '/tos-api/src/aes/mod.rs', [
    ("pub fn user_mot(x: i16, y: i16) {\n    input::mchange(x, y);\n}\n",
     "pub fn user_mot(x: i16, y: i16) {\n    input::mchange(x, y);\n}\n\n"
     "// H02-TEMP BEGIN\n"
     "pub static H02_TRACE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);\n"
     "pub fn h02_log(args: core::fmt::Arguments) {\n"
     "    struct W;\n"
     "    impl core::fmt::Write for W {\n"
     "        fn write_str(&mut self, s: &str) -> core::fmt::Result {\n"
     "            if let Some(h) = crate::host::host() { h.debug_write(s.as_bytes()); }\n"
     "            Ok(())\n"
     "        }\n"
     "    }\n"
     "    use core::fmt::Write as _;\n"
     "    let mut w = W;\n"
     "    let _ = w.write_fmt(args);\n"
     "    let _ = w.write_str(\"\\r\\n\");\n"
     "}\n"
     "// H02-TEMP END\n"),
])
# 3) desktop-up flag for the dual arming (A46)
patch(scr + '/kernel/src/desk/mod.rs', [(
    '    kprintln!(\n        "[desk] desktop up ({} drives)",',
    '    crate::host_env::h02_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // A46-TEMP\n'
    '    kprintln!(\n        "[desk] desktop up ({} drives)",')])
