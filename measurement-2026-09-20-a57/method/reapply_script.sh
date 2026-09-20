#!/usr/bin/env bash
# reapply_script.sh <scratch-dir> <script.rs> -- swap the injector SCRIPT, keep the DESK_UP arming
set -euo pipefail
SCR="$1"; SCRIPT="$2"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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
