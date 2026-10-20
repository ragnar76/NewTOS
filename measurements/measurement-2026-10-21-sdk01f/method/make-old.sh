#!/usr/bin/env bash
# make-old.sh -- the OLD variant (libc before SDK01 F) as it was built for this run: an APFS clone of the working-tree sdk/
# (vendored app sources and the extracted mintlib are gitignored, so git archive is not enough), tools/ cloned, the pinned
# rust-toolchain.toml copied (sdk/toolchain.sh asks rustc from inside the tree), Applications symlinked; the two SDK01 F files
# put back to HEAD; then libc, QED and TeraDesk built in the clone. Everything else is identical to the NEW variant.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
mkdir -p "$S/old" "$S/logs"; cd "$WS"
cp -c -R sdk "$S/old/sdk"; cp -c -R tools "$S/old/tools"; cp rust-toolchain.toml "$S/old/"; ln -sf "$WS/Applications" "$S/old/Applications"
git show "${HEADREV}:sdk/libc/newtos-arch/do_stat.c"  > "$S/old/sdk/libc/newtos-arch/do_stat.c"
git show "${HEADREV}:sdk/libc/newtos-arch/do_fstat.c" > "$S/old/sdk/libc/newtos-arch/do_fstat.c"
(cd "$S/old/sdk/libc" && bash build-libc.sh) > "$S/logs/old-build-libc.log" 2>&1
cd "$S/old/sdk"
for b in apps/build-qed.sh apps/build-teradesk.sh; do bash $b > "$S/logs/old-$(basename $b .sh).log" 2>&1; done
