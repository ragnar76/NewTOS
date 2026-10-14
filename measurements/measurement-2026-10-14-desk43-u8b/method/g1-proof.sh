#!/usr/bin/env bash
# g1-proof.sh <workdir> -- DESK43 U8b: "bypass = compiler error" (guard G1, review §4.2), shown, not asserted.
# On a SCRATCH copy of the shared tree (make-scratch.sh; nothing here touches the shared tree):
#   0  the copy as is: `cargo check -p kernel` (board-rpi4) passes
#   1  the desktop names the AES server directly  (`tos_api::aes::reinit()` in programs/desktop/src/lib.rs)
#   2  the desktop names the kernel                (`kernel::shell_loop::observe_desktop_gem_call`)
#   3  the desktop uses the kernel's print macro    (`crate::kprintln!`, the U8a-removed form)
#      -> each must FAIL to compile (the desktop crate has no such dependency); lib.rs restored after each
#   4  someone adds `tos-api` to programs/desktop/Cargo.toml: the drift probe's group D and the xtask host test
#      `programs_tests::desktop_depends_only_on_newtos_gem_and_tos_types` must both report it
# Output: ../checks/u8b-g1-proof.txt
set -uo pipefail
W="${1:?workdir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="$(cd "$HERE/.." && pwd)"
T="$W/tree-g1"
rm -rf "$T"; bash "$HERE/make-scratch.sh" "$T"
LIB="$T/programs/desktop/src/lib.rs"; cp "$LIB" "$W/lib.rs.orig"
chk() { (cd "$T" && cargo check -p kernel --no-default-features --features board-rpi4 2>&1); }
{
echo "== 0 unmodified copy"
chk | grep -E '^error|Finished' | head -3
n=1
for body in 'tos_api::aes::reinit();' 'kernel::shell_loop::observe_desktop_gem_call(core::ptr::null());' 'crate::kprintln!("x");'; do
  cp "$W/lib.rs.orig" "$LIB"
  printf '\n/// G1 probe (scratch)\npub fn g1_probe() {\n    %s\n}\n' "$body" >> "$LIB"
  echo "== $n desktop gets: $body"
  chk | grep -E '^error|^ *--> programs/desktop|Finished|could not compile' | head -4
  n=$((n+1))
done
cp "$W/lib.rs.orig" "$LIB"
echo "== 4 programs/desktop/Cargo.toml gets: tos-api = { path = \"../../tos-api\" }"
sed -i '' 's#^tos-types = { path = "../../tos-types" }#&\ntos-api = { path = "../../tos-api" }#' "$T/programs/desktop/Cargo.toml"
grep -n '^tos-api' "$T/programs/desktop/Cargo.toml"
(cd "$T" && bash tools/desk-drift-probe.sh -v 2>&1) | sed -n '/^D /,/^unsafe/p' | head -6
(cd "$T" && bash tools/desk-drift-probe.sh -q 2>&1)
(cd "$T" && cargo test -p xtask desktop_depends_only 2>&1) | grep -E 'panicked|dependencies:|test result' | head -4
} > "$M/checks/u8b-g1-proof.txt" 2>&1
cat "$M/checks/u8b-g1-proof.txt"
