#!/usr/bin/env bash
# make_scratch.sh <src-tree> <scratch-dir> <script.rs>
# Copies the crates of <src-tree> into <scratch-dir>, applies the A46 injector
# scaffolding (scratch copy only) plus an A57 trace of wind_get(WF_SCREEN).
set -euo pipefail
WS="$1"; SCR="$2"; SCRIPT="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
python3 - "$SCR" <<'PY'
import sys
scr = sys.argv[1]
p = scr + '/tos-api/src/host.rs'; s = open(p).read()
if 'fn debug_write' not in s:
    a = "    /// Non-blocking read from the console input device."
    assert s.count(a) == 1
    s = s.replace(a, "    /// A57-TEMP (scratch copy only): serial-only debug write.\n    fn debug_write(&self, s: &[u8]) {\n        self.console_write_bytes(s)\n    }\n\n" + a, 1)
    open(p, 'w').write(s)
p = scr + '/tos-api/src/aes/mod.rs'; s = open(p).read()
if 'H02_TRACE' not in s:
    s += "\n// A57-TEMP (scratch copy only): named by the injector's Step::Arm, unused.\npub static H02_TRACE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);\n"
# A57 trace: what an app receives from wind_get(WF_SCREEN)
a = "    let sw = crysbind(&pb);\n"
assert s.count(a) == 1
s = s.replace(a, a + '''    // A57-TEMP (scratch copy only): trace wind_get(WF_SCREEN) answers to the serial line.
    if pb.control(C_OPCODE) == 104 && pb.intin(1) == 17 {
        let o = unsafe { core::slice::from_raw_parts(pb.intout, O_SIZE) };
        struct W;
        impl core::fmt::Write for W {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                if let Some(h) = crate::host::host() { h.debug_write(s.as_bytes()); }
                Ok(())
            }
        }
        use core::fmt::Write as _;
        let _ = write!(W, "[a57] wind_get(WF_SCREEN) handle={} -> int_out=({},{},{},{},{}) addr=0x{:08x} len={}\\r\\n",
            pb.intin(0), o[0], o[1], o[2], o[3], o[4],
            (o[1] as u16 as u32) | ((o[2] as u16 as u32) << 16),
            (o[3] as u16 as u32) | ((o[4] as u16 as u32) << 16));
    }
''', 1)
open(p, 'w').write(s)
p = scr + '/kernel/src/desk/mod.rs'; s = open(p).read()
a = '    kprintln!(\n        "[desk] desktop up ({} drives)",'
assert s.count(a) == 1
s = s.replace(a, '    crate::host_env::h02_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // A57-TEMP\n' + a, 1)
open(p, 'w').write(s)
PY
python3 "$HERE/apply_injector.py" "$SCR" "$SCRIPT"
python3 - "$SCR" <<'PY'
import sys
p = sys.argv[1] + '/kernel/src/host_env.rs'; s = open(p).read()
a = "        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() {\n            return; // only while an EL0 program is the running process\n        }\n"
assert s.count(a) == 1
s = s.replace(a, "        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() && !DESK_UP.load(Ordering::Acquire) {\n            return; // while an EL0 program runs, or once our desktop is up\n        }\n", 1)
b = "    static T0: AtomicU32 = AtomicU32::new(0);\n"
assert s.count(b) == 1
s = s.replace(b, b + "    pub static DESK_UP: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false); // A57-TEMP\n", 1)
open(p, 'w').write(s)
PY
echo "scratch ready: $SCR"
