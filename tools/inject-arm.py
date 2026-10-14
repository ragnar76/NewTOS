#!/usr/bin/env python3
r"""inject-arm.py -- arm the scripted IKBD injector on a *program identity* (T10).

usage:
  tools/inject-arm.py <scratch-tree> [--tag TAG] ARM [ARM ...]

  ARM = <trigger>=<script-file>

  trigger         the script's clock starts at the first injector tick after ...
  -------------   ---------------------------------------------------------------
  desktop         ... the kernel's "[desk] desktop up" event: the first
                  `evnt_multi` of a `deskmain` run of the desktop the shell's
                  DESKTOP arm started (kernel/src/shell_loop.rs,
                  observe_desktop_gem_call -- its key is the program identity:
                  the launched shell child, or the built-in desktop while there
                  is none). Only the first such event arms it.
  launch:NAME     ... the launcher published a shell child (kernel/src/launch.rs,
                  `launch::shell_child_slot()`, set right before the drop to
                  EL0) whose name is NAME. Only the first such child arms it.
  up:NAME         ... the shell child named NAME reached its first `evnt_multi`
                  after its `appl_init` (the same criterion as "desktop up", for
                  an application). Only the first time.

  NAME is the child's program name as EmuTOS `p_nameit` forms it from the
  launch command `D.s_cmd` (aes/gempd.c:77-86): the part after the last `\`
  or `:`, up to the first `.`, at most 8 characters, compared exactly
  (`C:\HELLOGEM.NAP` -> `HELLOGEM`, the embedded `NEWDESK` -> `NEWDESK`). The
  name is read from `D.s_cmd` (`tos_api::aes::shell::s_cmd`) when the injector
  first sees the child's slot -- the launcher writes it just before it
  publishes the slot (`set_launch_info`).

  Script file: one step per line, `<ticks> <byte> [<byte> ...]` -- the bytes
  (hex `0x..` or decimal) are fed to `crate::ikbd::feed` in order once
  `hz_200 - T0 >= ticks` (200 Hz). `#` starts a comment. IKBD make/break
  scancodes, relative mouse packets `0xF8|buttons dx dy`, ... -- the
  `kernel/src/ikbd.rs` decoder's input.

  --tag TAG       the serial prefix of the injector's own lines (default `inj`):
                    [TAG] <trigger> armed at hz_200 <n> (slot <s> name <NAME>)
                    [TAG] <trigger> feed 0x.. at hz_200 <n> (+<d>)

WHY (TASK/T10.md, docs/analysis/desk43-u9-prep-2026-10-14.md §3). The older
measurement harnesses (measurement-*/method/apply_injector.py,
a78_patch2.py) arm on "an EL0 program is the running process"
(`RUN.is_null()`, chosen at the first tick). Since the desk accessories that
is an accessory, and once the desktop is an EL0 program (DESK43 U9) every
candidate is EL0 -- the test cannot tell programs apart. This tool arms on the
kernel's own identity events instead. It replaces that heuristic **for new
runs**; the existing measurement scripts are immutable and stay as they are.

It patches a SCRATCH copy of the tree only (never the shared tree):
  kernel/src/host_env.rs    + module `inj_arm` (appended) and a `step()` call
                              after the USB poll in `pump_devices` -- the site
                              and order of the earlier injectors, so a run
                              armed the same way feeds at the same ticks;
  kernel/src/shell_loop.rs  `desktop`: one line before the desktop-up
                              `kprintln!` (the anchor the U8a/U8b injectors
                              assert); `up:`: one line at the top of
                              `observe_desktop_gem_call` (every GEM trap).
Each anchor must occur exactly once, else the tool stops with an error.
"""
import pathlib
import re
import sys

MARK = "INJ-ARM-TEMP"


def die(msg):
    sys.exit(f"inject-arm.py: {msg}")


def parse_script(path):
    steps = []
    for n, line in enumerate(pathlib.Path(path).read_text().splitlines(), 1):
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) < 2:
            die(f"{path}:{n}: need '<ticks> <byte> ...'")
        try:
            at = int(parts[0], 0)
            bs = [int(b, 0) for b in parts[1:]]
        except ValueError:
            die(f"{path}:{n}: not a number")
        if at < 0 or any(b < 0 or b > 255 for b in bs):
            die(f"{path}:{n}: out of range")
        steps += [(at, b) for b in bs]
    if not steps:
        die(f"{path}: empty script")
    if any(a > b for (a, _), (b, _) in zip(steps, steps[1:])):
        die(f"{path}: tick offsets must not decrease")
    return steps


def parse_arm(arg):
    if "=" not in arg:
        die(f"ARM '{arg}': expected <trigger>=<script-file>")
    trig, script = arg.split("=", 1)
    if trig == "desktop":
        kind, name = 0, ""
    elif trig.startswith("launch:"):
        kind, name = 1, trig[len("launch:"):]
    elif trig.startswith("up:"):
        kind, name = 2, trig[len("up:"):]
    else:
        die(f"trigger '{trig}': desktop | launch:NAME | up:NAME")
    if kind and not re.fullmatch(r"[\x21-\x7e]{1,8}", name) or "." in name:
        die(f"trigger '{trig}': NAME is 1-8 printable characters, no extension")
    return trig, kind, name, parse_script(script)


def rust_bytes(name):
    b = name.encode().ljust(8, b" ")
    return "*b\"" + "".join(chr(c) for c in b) + "\""


def main(argv):
    if len(argv) < 2 or argv[0] in ("-h", "--help"):
        print(__doc__)
        sys.exit(0 if argv and argv[0] in ("-h", "--help") else 2)
    tree = pathlib.Path(argv[0])
    rest = argv[1:]
    tag = "inj"
    if rest[:1] == ["--tag"]:
        if len(rest) < 2 or not re.fullmatch(r"[A-Za-z0-9_-]{1,12}", rest[1]):
            die("--tag needs 1-12 of [A-Za-z0-9_-]")
        tag, rest = rest[1], rest[2:]
    if not rest:
        die("no ARM given")
    arms = [parse_arm(a) for a in rest]

    he = tree / "kernel/src/host_env.rs"
    sl = tree / "kernel/src/shell_loop.rs"
    s = he.read_text()
    if MARK in s:
        die(f"{he} is already instrumented")

    # 1. step() after the trap-side USB poll (pump_devices), as before.
    anchor = "crate::drivers::usb::poll();\n"
    if s.count(anchor) != 2:
        die("host_env.rs: expected 2 usb::poll sites")
    s = s.replace(anchor, anchor + f"        inj_arm::step(); // {MARK}\n", 1)

    # 2. the module.
    consts = []
    for i, (trig, kind, name, steps) in enumerate(arms):
        body = ", ".join(f"({a}, {b:#04x})" for a, b in steps)
        consts.append(f"    const S{i}: &[(u32, u8)] = &[{body}];")
    table = ",\n".join(
        f"        Arm {{ trig: \"{trig}\", kind: {kind}, name: {rust_bytes(name)}, script: S{i} }}"
        for i, (trig, kind, name, _) in enumerate(arms)
    )
    n = len(arms)
    s += f'''
// {MARK} BEGIN -- tools/inject-arm.py: scripted IKBD bytes armed on a program
// identity (T10). Scratch only.
/// Scratch IKBD injector armed on program-identity events (tools/inject-arm.py).
pub mod inj_arm {{
    use core::sync::atomic::{{AtomicBool, AtomicU32, AtomicUsize, Ordering}};

    struct Arm {{
        trig: &'static str,
        /// 0 = desktop up, 1 = launch:NAME, 2 = up:NAME.
        kind: u8,
        name: [u8; 8],
        script: &'static [(u32, u8)],
    }}
{chr(10).join(consts)}
    static ARMS: [Arm; {n}] = [
{table},
    ];
    /// Per arm: 0 = waiting, 1 = event seen (arms at the next tick), 2 = armed.
    static STATE: [AtomicU32; {n}] = [const {{ AtomicU32::new(0) }}; {n}];
    static T0: [AtomicU32; {n}] = [const {{ AtomicU32::new(0) }}; {n}];
    static NEXT: [AtomicUsize; {n}] = [const {{ AtomicUsize::new(0) }}; {n}];

    /// The shell child the injector last saw (`usize::MAX` = none), its name.
    static SEEN_SLOT: AtomicUsize = AtomicUsize::new(usize::MAX);
    static mut SEEN_NAME: [u8; 8] = [b' '; 8];
    /// `up:` -- the child's `appl_init` seen, its next `evnt_multi` is "up".
    static CHILD_INIT: AtomicBool = AtomicBool::new(false);

    /// `p_nameit` of `D.s_cmd`'s base name (aes/gempd.c:77-86).
    fn child_name() -> [u8; 8] {{
        let mut cmd = [0u8; 128];
        let len = tos_api::aes::shell::s_cmd(&mut cmd).min(cmd.len());
        let cmd = &cmd[..len];
        let start = cmd.iter().rposition(|&c| c == b'\\\\' || c == b':').map_or(0, |i| i + 1);
        let mut out = [b' '; 8];
        for (o, &c) in out.iter_mut().zip(cmd[start..].iter().take_while(|&&c| c != 0 && c != b'.')) {{
            *o = c;
        }}
        out
    }}

    /// Track the launcher's published child; a new one fires `launch:` arms.
    fn track_child() -> Option<usize> {{
        let cur = crate::launch::shell_child_slot();
        let seen = SEEN_SLOT.load(Ordering::Relaxed);
        match cur {{
            None => {{
                if seen != usize::MAX {{
                    SEEN_SLOT.store(usize::MAX, Ordering::Relaxed);
                    CHILD_INIT.store(false, Ordering::Relaxed);
                }}
            }}
            Some(slot) if slot != seen => {{
                SEEN_SLOT.store(slot, Ordering::Relaxed);
                CHILD_INIT.store(false, Ordering::Relaxed);
                let name = child_name();
                // SAFETY: single CPU; written only here, read by `event`.
                unsafe {{ SEEN_NAME = name }};
                event(1, &name);
            }}
            Some(_) => {{}}
        }}
        cur
    }}

    /// An identity event of `kind` for the program `name`: every waiting arm
    /// of that kind (and name) is armed at the next tick.
    fn event(kind: u8, name: &[u8; 8]) {{
        for (i, a) in ARMS.iter().enumerate() {{
            if a.kind == kind && (kind == 0 || &a.name == name) {{
                let _ = STATE[i].compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed);
            }}
        }}
    }}

    /// The kernel's "[desk] desktop up" (shell_loop.rs, at the line).
    pub fn desktop_up() {{
        event(0, &[b' '; 8]);
    }}

    /// Every GEM trap (top of `observe_desktop_gem_call`): the shell child's
    /// first `evnt_multi` after its `appl_init` fires the `up:` arms. Reads
    /// `control[0]` the way that function does.
    pub fn gem_call(args: &tos_api::trap::TrapArgs) {{
        use tos_api::bdos::process::{{current_abi, Abi}};
        let Some(child) = track_child() else {{ return }};
        if !args.space.is_user()
            || tos_api::bdos::process::sched::running_slot() != child
            || args.regs[0] != 200
            || args.regs[1] == 0
        {{
            return;
        }}
        let control = match current_abi() {{
            Abi::Lp64 => args.space.get_u64(args.regs[1]),
            Abi::Ilp32 => args.space.get_u32(args.regs[1]).map(u64::from),
        }};
        let Ok(control) = control else {{ return }};
        if control == 0 {{
            return;
        }}
        let Ok(op) = args.space.get_u16(control) else {{ return }};
        match op {{
            10 => CHILD_INIT.store(true, Ordering::Relaxed),
            25 if CHILD_INIT.swap(false, Ordering::Relaxed) => {{
                // SAFETY: single CPU; see `track_child`.
                let name = unsafe {{ SEEN_NAME }};
                event(2, &name);
            }}
            _ => {{}}
        }}
    }}

    /// One injector tick (from `pump_devices`).
    pub fn step() {{
        track_child();
        let now = tos_api::bios::tick::hz_200();
        for (i, a) in ARMS.iter().enumerate() {{
            match STATE[i].load(Ordering::Acquire) {{
                1 => {{
                    T0[i].store(now.max(1), Ordering::Relaxed);
                    STATE[i].store(2, Ordering::Release);
                    let slot = crate::launch::shell_child_slot();
                    // SAFETY: single CPU; see `track_child`.
                    let name = unsafe {{ SEEN_NAME }};
                    crate::kprintln!(
                        "[{tag}] {{}} armed at hz_200 {{}} (slot {{:?}} name {{}})",
                        a.trig,
                        now,
                        slot,
                        core::str::from_utf8(&name).unwrap_or("?").trim_end()
                    );
                }}
                2 => {{
                    let t0 = T0[i].load(Ordering::Relaxed);
                    loop {{
                        let k = NEXT[i].load(Ordering::Relaxed);
                        let Some(&(at, b)) = a.script.get(k) else {{ break }};
                        if now.wrapping_sub(t0) < at {{
                            break;
                        }}
                        crate::kprintln!(
                            "[{tag}] {{}} feed {{:#04x}} at hz_200 {{}} (+{{}})",
                            a.trig,
                            b,
                            now,
                            now.wrapping_sub(t0)
                        );
                        crate::ikbd::feed(b);
                        NEXT[i].store(k + 1, Ordering::Relaxed);
                    }}
                }}
                _ => {{}}
            }}
        }}
    }}
}}
// {MARK} END
'''
    he.write_text(s)

    # 3. the identity events in the shell loop.
    t = sl.read_text()
    if any(k == 0 for _, k, _, _ in arms):
        old = '            kprintln!(\n                "[desk] desktop up ({} drives)",'
        if t.count(old) != 1:
            die("shell_loop.rs: desktop-up anchor not found exactly once")
        t = t.replace(old, f"            crate::host_env::inj_arm::desktop_up(); // {MARK}\n" + old)
    if any(k == 2 for _, k, _, _ in arms):
        old = "pub fn observe_desktop_gem_call(args: &TrapArgs) {\n"
        if t.count(old) != 1:
            die("shell_loop.rs: observe_desktop_gem_call anchor not found exactly once")
        t = t.replace(old, old + f"    crate::host_env::inj_arm::gem_call(args); // {MARK}\n")
    sl.write_text(t)
    print(f"inject-arm.py: {tree}: tag [{tag}], " + ", ".join(
        f"{trig} ({len(steps)} bytes)" for trig, _, _, steps in arms))


if __name__ == "__main__":
    main(sys.argv[1:])
