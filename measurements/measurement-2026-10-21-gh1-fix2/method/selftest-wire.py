#!/usr/bin/env python3
"""selftest-wire.py <tree> -- SCRATCH ONLY: insert the boot-path selftest calls after `tos_api::aes::init::init();` in <tree>/kernel/src/lib.rs (never the shared tree)."""
import sys
p = sys.argv[1] + "/kernel/src/lib.rs"; s = open(p).read()
anchor = "    tos_api::aes::init::init();\n"
assert s.count(anchor) == 1
calls = [("p02", "selftest::run_phase02_self_test()"), ("p04", "selftest::run_phase04_self_test()"),
         ("p07-time", "selftest_time::run_phase07_time_test()"), ("bios", "selftest_bios::run()"),
         ("xbios-audit", "selftest_xbios::run_audit()"), ("xbios", "selftest_xbios::run()"),
         ("cookie", "selftest_cookie::run()"), ("gemtrap", "{ let _ = selftest_gemtrap::run(); }"),
         ("p13-aes", "selftest_aes::run_phase13_aes_test()"),
         ("objc-window", "selftest_objc::run_window()"), ("objc-two-windows", "selftest_objc::run_two_windows()"),
         ("objc-p6-msgqueue", "selftest_objc::run_p6_msgqueue()")]
blk = "".join('    kprintln!("[ST] >> %s");\n    %s;\n    kprintln!("[ST] << %s");\n' % (n, c, n) for n, c in calls)
open(p, "w").write(s.replace(anchor, anchor + "    // SCRATCH SELFTEST WIRING (GH1-reg tester)\n" + blk))
