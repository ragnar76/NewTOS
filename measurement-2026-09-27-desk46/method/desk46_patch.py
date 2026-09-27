#!/usr/bin/env python3
"""DESK46-TEMP (scratch copy only) -- the instrumentation the two DESK46 runs
need on top of the standard GUI04 injector scaffolding.

1) kernel/src/host_env.rs -- step the injector inside BOTH blocking console
   reads.  The injector normally only runs from `pump_input`, the AES event
   loop's idle pass; while EmuCON blocks in `Cnecin` the AES is not running at
   all, so without this hook no scripted keystroke could reach the console.
   Also: a once-a-second heartbeat, so a stalled system can be told apart from
   a system that is still polling for a key.

2) kernel/src/shell_loop.rs -- serial markers at every state transition of the
   shell loop, plus the CONSOLE HOLD: after a character-mode program has
   terminated, hold the screen for three seconds BEFORE `sh_chgrf` switches
   back to graphics, so a QMP screendump can capture the console screen the
   user sees only as a flash.  Finding 1 is exactly the question "what is on
   that screen", and it cannot be photographed without stopping time.

3) kernel/src/shell.rs -- markers around EmuCON's `Pexec`, and one per prompt,
   so the freeze of finding 2 can be located to a statement.

Applied to the scratch copy AFTER apply_injector.py and desk_arm_patch.py,
never to the shared tree.
"""
import os
import sys

scr = sys.argv[1]
# How many 200 Hz ticks the character screen is held after a console
# program terminates, so a screendump can see it.  0 = no hold, which is
# what measures how long the flash really lasts.
hold = os.environ.get('D46_HOLD', '600')
# 'noshell': leave kernel/src/shell.rs pristine, so what a run shows is the
# product's own console path and not our markers'.
noshell = len(sys.argv) > 2 and sys.argv[2] == 'noshell'


def rep(path, a, b, n=1):
    s = open(path).read()
    assert s.count(a) == n, (path, s.count(a), a[:70])
    open(path, 'w').write(s.replace(a, b, n))
    print("patched", path)


# ── 1) injector inside both blocking console reads ────────────────────────
p = scr + '/kernel/src/host_env.rs'
rep(p,
    "            crate::drivers::usb::poll();\n\n            if let Some(b) = crate::console::rx_pop() {",
    "            crate::drivers::usb::poll();\n"
    "            h02_inject::step(); // DESK46-TEMP: keys for EmuCON\n\n"
    "            if let Some(b) = crate::console::rx_pop() {")
rep(p,
    "            crate::drivers::usb::poll();\n\n            if let Some(w) = crate::console::rx_pop_key() {",
    "            crate::drivers::usb::poll();\n"
    "            h02_inject::step(); // DESK46-TEMP: keys for EmuCON\n\n"
    "            if let Some(w) = crate::console::rx_pop_key() {")

# heartbeat: once a second, whoever is stepping the injector
rep(p,
    "        let t0 = T0.load(Ordering::Relaxed);\n        let i = NEXT.load(Ordering::Relaxed);\n",
    "        let t0 = T0.load(Ordering::Relaxed);\n"
    "        // DESK46-TEMP heartbeat: proves the polling loop is still alive.\n"
    "        {\n"
    "            let sec = now.wrapping_sub(t0) / 200;\n"
    "            if HB.swap(sec, Ordering::Relaxed) != sec {\n"
    "                let mut b = [0u8; 64];\n"
    "                let mut w = W(&mut b, 0);\n"
    "                use core::fmt::Write as _;\n"
    "                let _ = write!(w, \"[d46] hb {}s t={}\\r\\n\", sec, now);\n"
    "                let n = w.1;\n"
    "                say(core::str::from_utf8(&b[..n]).unwrap_or(\"?\"));\n"
    "            }\n"
    "        }\n"
    "        let i = NEXT.load(Ordering::Relaxed);\n")
rep(p,
    "    static T0: AtomicU32 = AtomicU32::new(0);\n",
    "    static T0: AtomicU32 = AtomicU32::new(0);\n"
    "    static HB: AtomicU32 = AtomicU32::new(u32::MAX); // DESK46-TEMP\n")

# ── 2) shell-loop markers + the console hold ──────────────────────────────
p = scr + '/kernel/src/shell_loop.rs'
rep(p,
    "    loop {\n        shell::sh_chdef(); // :712\n        sh_chgrf(); // :713\n",
    "    loop {\n"
    "        crate::kprintln!(\"[d46] sh_main: loop top, rc={}, nextapp={}, isgem={}\","
    " rc, shell::sh_nextapp(), i32::from(shell::sh_isgem())); // DESK46-TEMP\n"
    "        shell::sh_chdef(); // :712\n        sh_chgrf(); // :713\n")
rep(p,
    "        rc = sh_ldapp(); // :725\n",
    "        rc = sh_ldapp(); // :725\n"
    "        crate::kprintln!(\"[d46] sh_main: sh_ldapp returned {}\", rc); // DESK46-TEMP\n")
rep(p,
    "fn sh_chgrf() {\n    let want = shell::sh_isgem() && have_graphics();\n    if want != shell::gl_shgem() {\n",
    "fn sh_chgrf() {\n    let want = shell::sh_isgem() && have_graphics();\n"
    "    crate::kprintln!(\"[d46] sh_chgrf: want={} gl_shgem={} t={}\","
    " i32::from(want), i32::from(shell::gl_shgem()),"
    " tos_api::bios::tick::hz_200()); // DESK46-TEMP\n"
    "    if want != shell::gl_shgem() {\n")

# The hold: `run_program_with_tail_rc` has just returned and the character
# screen still carries the program's output.  `sh_chgrf` would wipe it on the
# next pass of `sh_main`.
rep(p,
    "    let ret = crate::launch::run_program_with_tail_rc(&cmd, &tail[..tlen]);\n",
    "    let ret = crate::launch::run_program_with_tail_rc(&cmd, &tail[..tlen]);\n"
    "    if !shell::gl_shgem() {\n"
    "        // DESK46-TEMP: hold the character screen so a screendump can see it.\n"
    "        crate::kprintln!(\"[d46] CONSOLE-HOLD-BEGIN t={}\", tos_api::bios::tick::hz_200());\n"
    "        let t0 = tos_api::bios::tick::hz_200();\n"
    "        while tos_api::bios::tick::hz_200().wrapping_sub(t0) < " + hold + " {\n"
    "            core::hint::spin_loop();\n"
    "        }\n"
    "        crate::kprintln!(\"[d46] CONSOLE-HOLD-END t={}\", tos_api::bios::tick::hz_200());\n"
    "    }\n")

# ── 3) EmuCON markers ─────────────────────────────────────────────────────
if noshell:
    raise SystemExit(0)
p = scr + '/kernel/src/shell.rs'
rep(p,
    "    line[n] = 0; // NUL-terminate the path argument\n    let rc = pexec(0, line[start..].as_ptr() as u64);\n",
    "    line[n] = 0; // NUL-terminate the path argument\n"
    "    crate::kprintln!(\"[d46] EmuCON: Pexec enter\"); // DESK46-TEMP\n"
    # No `{}` in the marker below on purpose: run B showed the fault *inside*
    # `<i64 as Display>::fmt`, so a formatting call of our own would hide
    # whether the product path itself survives.  Plain string, no arguments.
    "    let rc = pexec(0, line[start..].as_ptr() as u64);\n"
    "    crate::kprintln!(\"[d46] EmuCON: back from Pexec\"); // DESK46-TEMP\n")
rep(p,
    "    loop {\n        prompt();\n        let n = read_line(&mut line);\n",
    "    loop {\n"
    "        crate::kprintln!(\"[d46] EmuCON: prompt\"); // DESK46-TEMP\n"
    "        prompt();\n"
    "        crate::kprintln!(\"[d46] EmuCON: read_line\"); // DESK46-TEMP\n"
    "        let n = read_line(&mut line);\n"
    "        crate::kprintln!(\"[d46] EmuCON: line len {}\", n); // DESK46-TEMP\n")
rep(p,
    "pub fn run_console() {\n    crate::fbcon::clear_and_home();\n",
    "pub fn run_console() {\n"
    "    crate::kprintln!(\"[d46] EmuCON: run_console enter\"); // DESK46-TEMP\n"
    "    crate::fbcon::clear_and_home();\n")
rep(p,
    "        if execute(&mut line, n) {\n            return;\n        }\n",
    "        if execute(&mut line, n) {\n"
    "            crate::kprintln!(\"[d46] EmuCON: leaving\"); // DESK46-TEMP\n"
    "            return;\n        }\n"
    "        crate::kprintln!(\"[d46] EmuCON: command done\"); // DESK46-TEMP\n")
