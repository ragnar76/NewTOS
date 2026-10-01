#!/usr/bin/env bash
# mkscripts.sh TERA.actions|- DESK.actions|- > scripts.rs
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
gen() { # name actions
  if [ "$2" = "-" ]; then echo "    const $1: &[(u32, Step)] = &[];"; else python3 "$HERE/gen_script.py" "$2" | sed "s/const SCRIPT:/const $1:/"; fi
}
gen SCRIPT_TERA "$1"; gen SCRIPT_DESK "$2"
