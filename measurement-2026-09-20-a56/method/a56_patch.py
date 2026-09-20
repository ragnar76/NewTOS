#!/usr/bin/env python3
"""A78-TEMP (scratch copy only): H02 serial AES trace + A46 desktop-up flag.
Same patches as measurement-2026-09-18-h05/method (scratch-*.diff) and
measurement-2026-09-19-a46/method/make_scratch.sh, applied before apply_injector.py."""
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
# 2) H02_TRACE + h02_log + h02! macro, and the [aes] op line in dispatch
patch(scr + '/tos-api/src/aes/mod.rs', [
    ("    let sw = crysbind(&pb);\n",
     "    let sw = crysbind(&pb);\n\n"
     "    // H02-TEMP: opcode trace to the serial line once the injector arms it.\n"
     "    if H02_TRACE.load(core::sync::atomic::Ordering::Relaxed) {\n"
     "        let op = pb.control(C_OPCODE);\n"
     "        let o = unsafe { core::slice::from_raw_parts(pb.intout, O_SIZE) };\n"
     "        h02_log(format_args!(\"[aes] pid={} idx={} op {} in=({},{},{},{},{},{}) out=({},{},{},{},{},{},{}) yield={}\",\n"
     "            appl::current_pid(), appl::current_identity(), op, pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3), pb.intin(4), pb.intin(5),\n"
     "            o[0], o[1], o[2], o[3], o[4], o[5], o[6], sw.is_some()));\n"
     "    }\n"),
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
     "#[macro_export]\n"
     "macro_rules! h02 {\n"
     "    ($($t:tt)*) => {\n"
     "        if $crate::aes::H02_TRACE.load(core::sync::atomic::Ordering::Relaxed) {\n"
     "            $crate::aes::h02_log(format_args!($($t)*));\n"
     "        }\n"
     "    };\n"
     "}\n"
     "// H02-TEMP END\n"),
])
# 3) message posts to serial
patch(scr + '/tos-api/src/aes/pd.rs', [(
    "    super::trace::msg_post(super::appl::reported_pid(idx), &msg, MSG_WORDS, posted);\n",
    "    super::trace::msg_post(super::appl::reported_pid(idx), &msg, MSG_WORDS, posted);\n"
    "    crate::h02!(\"[h02] post_msg idx={} {:?} posted={}\", idx, &msg[..8], posted);\n")])
# 4) desktop-up flag for the dual arming (A46)
patch(scr + '/kernel/src/desk/mod.rs', [(
    '    kprintln!(\n        "[desk] desktop up ({} drives)",',
    '    crate::host_env::h02_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // A46-TEMP\n'
    '    kprintln!(\n        "[desk] desktop up ({} drives)",')])
