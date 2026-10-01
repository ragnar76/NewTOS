#!/usr/bin/env bash
# mkscript.sh <actions> <out.rs>  -- SCRIPT_DESK from the action list plus the
# empty SCRIPT_TERA the a78 script selector needs.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
python3 "$HERE/gen_script.py" "$1" SCRIPT_DESK > "$2"
printf '    const SCRIPT_TERA: &[(u32, Step)] = &[];\n' >> "$2"
