#!/usr/bin/env bash
# probe-fixture.sh <workdir> — DESK43 U5a: drift probe v2 against two scratch
# trees built from a copy of kernel/ (never the shared tree):
#   case 1: kernel/src/desk/ removed, no desktop crate  -> must fail (exit 1), not "0"
#   case 2: desk/ moved to desktop/src/ as a crate with a forbidden dependency
#           (tos-api, spin), a subdirectory file with brace `use` lists, a
#           multi-line `use tos_api::{…}`, a block comment and a `"c:/*.txt"`
#           string, one extra `unsafe`; kernel names `desktop::` and depends on
#           it in kernel/Cargo.toml -> crate mode, groups A/B/C/D, exit 3 (unsafe rose)
set -uo pipefail
W="${1:?workdir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
F="$W/probe-fixture"; rm -rf "$F"; mkdir -p "$F/tools" "$F/kernel"
cp "$WS/tools/desk-drift-probe.sh" "$F/tools/"; cp -R "$WS/kernel/src" "$F/kernel/src"; cp "$WS/kernel/Cargo.toml" "$F/kernel/"
echo "=== case 1: no desktop source"
mv "$F/kernel/src/desk" "$F/desk.saved"
bash "$F/tools/desk-drift-probe.sh"; echo "case 1 exit=$?"
echo "=== case 2: desktop crate"
mkdir -p "$F/desktop/src/sub"; mv "$F/desk.saved/"* "$F/desktop/src/"
cat > "$F/desktop/Cargo.toml" <<'T'
[package]
name = "desktop"
version = "0.1.0"

[dependencies]
tos-types = { path = "../tos-types" }
newtos-gem.workspace = true
tos-api = { path = "../tos-api" }

[dependencies.spin]
version = "0.9"
T
cat > "$F/desktop/src/sub/x.rs" <<'T'
use crate::{sync::SpinMutex, kprintln};
use kernel::{a::b, c};
use tos_api::{
    aes::object::{Object, TedInfo}, // a comment with tos_api::fake
    vdi::font::CELL_W,
};
/* block
   tos_api::hidden::in_comment
*/
fn f() { let s = b"c:/*.txt"; let t = tos_api::x::y::z(); unsafe { } }
T
sed -i '' 's/^pub mod desk;/\/\/ moved/' "$F/kernel/src/lib.rs"
sed -i '' 's/        desk::reset();/        desktop::reset();/' "$F/kernel/src/lib.rs"
sed -i '' 's/crate::desk::/desktop::/' "$F/kernel/src/shell_loop.rs"
printf 'desktop = { path = "../desktop" }\n' >> "$F/kernel/Cargo.toml"
bash "$F/tools/desk-drift-probe.sh" -v 2>&1 | grep -v '^      desktop/src/[a-z]*\.rs:[0-9]* *\(use tos_api\|let o = unsafe\|.*unsafe\)'
echo "case 2 exit=${PIPESTATUS[0]}"
rm -rf "$F"
