#!/usr/bin/env bash
# build-apps.sh <new|old> -- all test apps against the variant's libc.a (sdk clone in scratch)
V=$1; cd "$(dirname "$0")/$V/sdk" || exit 1
for b in apps/build-qed.sh apps/build-teradesk.sh zview/build-zview.sh apps/build-doom.sh apps/build-coremark.sh apps/apitest/build-apitest.sh; do
  echo "=== $b"; bash $b; echo "rc=$? $b"
done
