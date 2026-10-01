#!/usr/bin/env bash
# make_scratch.sh <ws> <scratch-dir> <object.rs-source|HEAD>
# Copies the crates into a scratch dir, applies the A46 injector scaffolding
# (scratch copy only), and optionally swaps in HEAD's object.rs ("before").
set -euo pipefail
WS="$1"; SCR="$2"; OBJ="$3"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -rf "$SCR"; mkdir -p "$SCR"
for d in Cargo.toml Cargo.lock rust-toolchain.toml .cargo kernel tos-api tos-types tools; do
  rsync -a --exclude target --exclude __pycache__ "$WS/$d" "$SCR/"
done
if [ "$OBJ" = "HEAD" ]; then
  git -C "$WS" show HEAD:tos-api/src/aes/object.rs > "$SCR/tos-api/src/aes/object.rs"
  echo "object.rs = HEAD ($(git -C "$WS" rev-parse --short HEAD))"
else
  echo "object.rs = working tree"
fi
python3 - "$SCR" <<'PY'
import sys
scr = sys.argv[1]
# 1) Host::debug_write default (serial-only sink)
p = scr + '/tos-api/src/host.rs'; s = open(p).read()
if 'fn debug_write' not in s:
    a = "    /// Non-blocking read from the console input device."
    assert s.count(a) == 1
    s = s.replace(a, "    /// A46-TEMP (scratch copy only): serial-only debug write.\n    fn debug_write(&self, s: &[u8]) {\n        self.console_write_bytes(s)\n    }\n\n" + a, 1)
    open(p, 'w').write(s)
# 2) H02_TRACE static named by Step::Arm (unused)
p = scr + '/tos-api/src/aes/mod.rs'; s = open(p).read()
if 'H02_TRACE' not in s:
    s += "\n// A46-TEMP (scratch copy only): named by the injector's Step::Arm, unused.\npub static H02_TRACE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);\n"
    open(p, 'w').write(s)
# 3) desktop-up flag for the dual arming
p = scr + '/kernel/src/desk/mod.rs'; s = open(p).read()
a = '    kprintln!(\n        "[desk] desktop up ({} drives)",'
assert s.count(a) == 1
s = s.replace(a, '    crate::host_env::h02_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // A46-TEMP\n' + a, 1)
open(p, 'w').write(s)
PY
python3 "$HERE/apply_injector.py" "$SCR" "$HERE/menu1_script.rs"
# 4) dual arming: EL0 program running OR desktop up
python3 - "$SCR" <<'PY'
import sys
p = sys.argv[1] + '/kernel/src/host_env.rs'; s = open(p).read()
a = "        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() {\n            return; // only while an EL0 program is the running process\n        }\n"
assert s.count(a) == 1
s = s.replace(a, "        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() && !DESK_UP.load(Ordering::Acquire) {\n            return; // A46: while an EL0 program runs, or once our desktop is up\n        }\n", 1)
b = "    static T0: AtomicU32 = AtomicU32::new(0);\n"
assert s.count(b) == 1
s = s.replace(b, b + "    pub static DESK_UP: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false); // A46-TEMP\n", 1)
open(p, 'w').write(s)
PY
echo "scratch ready: $SCR"
