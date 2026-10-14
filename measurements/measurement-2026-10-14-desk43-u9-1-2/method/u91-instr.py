#!/usr/bin/env python3
"""u91-instr.py <scratch-tree> -- DESK43 U9-1: apply the stack/latency instruments (u91_instr.rs) to a
SCRATCH copy of the tree (never the shared tree). Each anchor must occur exactly once (the usb::poll
anchor: twice, as tools/inject-arm.py expects; the first one gets the hook). Compatible with
tools/inject-arm.py applied before or after (the inject-arm anchors stay intact).

  kernel/src/u91_instr.rs           the module (copied)
  kernel/src/lib.rs                 `pub mod u91_instr;`
  kernel/src/shell_loop.rs          A at sh_ldapp entry; EL1 paint/return around desktop::run();
                                    U at the "[desk] desktop up" line; gem_call at the top of
                                    observe_desktop_gem_call
  kernel/src/launch.rs              B + EL0 paint before the child's slot is published
  kernel/src/arch/aarch64/exceptions.rs  X in finish_terminate's kernel-launch branch
  kernel/src/host_env.rs            tick() after the first usb::poll in pump_devices
"""
import pathlib
import shutil
import sys

MARK = "U91-INSTR-TEMP"
here = pathlib.Path(__file__).resolve().parent
tree = pathlib.Path(sys.argv[1])
k = tree / "kernel/src"


def patch(rel, old, new, count=1):
    p = k / rel
    s = p.read_text()
    if MARK in s and rel != "lib.rs":
        pass
    n = s.count(old)
    if n != count:
        sys.exit(f"u91-instr.py: {rel}: anchor found {n}x, expected {count}: {old!r}")
    s = s.replace(old, new, 1)
    p.write_text(s)


if (k / "u91_instr.rs").exists():
    sys.exit("u91-instr.py: tree already instrumented")
shutil.copy(here / "u91_instr.rs", k / "u91_instr.rs")
patch("lib.rs", "pub mod bootargs;\n", f"pub mod bootargs;\npub mod u91_instr; // {MARK}\n")
patch(
    "shell_loop.rs",
    "    // ── Arm 1 — DESKTOP_APP (`:600-612`) ─",
    f"    crate::u91_instr::ldapp(nextapp, &cmd); // {MARK}\n\n    // ── Arm 1 — DESKTOP_APP (`:600-612`) ─",
)
patch(
    "shell_loop.rs",
    "        desktop::run();\n",
    f"        crate::u91_instr::el1_paint(); // {MARK}\n        desktop::run();\n"
    f"        crate::u91_instr::el1_returned(); // {MARK}\n",
)
patch(
    "shell_loop.rs",
    '            kprintln!(\n                "[desk] desktop up ({} drives)",',
    f"            crate::u91_instr::desk_up(); // {MARK}\n"
    '            kprintln!(\n                "[desk] desktop up ({} drives)",',
)
patch(
    "shell_loop.rs",
    "pub fn observe_desktop_gem_call(args: &TrapArgs) {\n",
    f"pub fn observe_desktop_gem_call(args: &TrapArgs) {{\n    crate::u91_instr::gem_call(args); // {MARK}\n",
)
patch(
    "launch.rs",
    "    SHELL_CHILD_SLOT.store(child.slot, Ordering::Release);\n",
    f"    crate::u91_instr::el0_launch(child.slot, child.basepage_va); // {MARK}\n"
    "    SHELL_CHILD_SLOT.store(child.slot, Ordering::Release);\n",
)
patch(
    "arch/aarch64/exceptions.rs",
    "        crate::launch::record_kernel_child_exit(rc);\n",
    f"        crate::u91_instr::el0_exit(); // {MARK}\n        crate::launch::record_kernel_child_exit(rc);\n",
)
patch(
    "host_env.rs",
    "crate::drivers::usb::poll();\n",
    f"crate::drivers::usb::poll();\n        crate::u91_instr::tick(); // {MARK}\n",
    count=2,
)
print(f"u91-instr.py: {tree}: instrumented")
