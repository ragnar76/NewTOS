#!/usr/bin/env bash
#
# desk-drift-probe.sh — count the desktop's remaining bypasses, as one number.
#
# This is the instrument measure 2 of `docs/analysis/desktop-userspace-plan-v2.md`
# §6.4 asks for: the count from §2.2 as a single reproducible command. Every
# work unit U1..U10 of that plan reports this number, and it **must fall
# monotonically**. A unit that cannot show the number is not accepted.
#
#   tools/desk-drift-probe.sh            # the table + the total
#   tools/desk-drift-probe.sh -v         # …plus every counted file:line
#   tools/desk-drift-probe.sh -q         # only  DRIFT-PROBE TOTAL: <n>
#
# ── What a "bypass" is here ────────────────────────────────────────────────
#
# The end state (plan §5, U8) is `programs/desktop`, a crate whose only
# dependencies are `newtos-gem` and `tos-types`. Anything the desktop names
# today that would **not compile** in that crate is a bypass. Concretely:
#
#   A  a `tos_api::` path inside `kernel/src/desk/`  — the AES/VDI/GEMDOS
#      server reached as a Rust function instead of through a trap
#   B  a `crate::` path inside `kernel/src/desk/`    — the kernel reached
#      directly (`crate::desk::…`, the desktop's own module, is not counted)
#   C  a reference to the `desk` module from the rest of `kernel/` — the
#      dependency in the other direction, which U4/U9/U10 remove
#
# `core::`, `alloc::`, `tos_types::` and the desktop-internal `super::`/`self::`
# are **not** bypasses: they survive the move unchanged.
#
# ── What counts as a line of code ──────────────────────────────────────────
#
# Everything from the first `//` on a line to the end of that line is removed
# before matching, so `//!`, `///` and trailing `//` comments cannot inflate
# the count. (§2.2 of the plan counted 36 raw `tos_api::` hits in
# `kernel/src/desk/` of which the majority are prose — the raw grep is not the
# measure.) There are no `/* … */` block comments in `kernel/src/desk/`; every
# `/*` in it sits inside a `///` line quoting EmuTOS C source, so the
# strip-to-end-of-line rule is exact for this tree. If a real block comment
# ever appears there, this heuristic has to be revisited.
#
# ── What this probe deliberately does NOT see ──────────────────────────────
#
# Plan §6.1: the 2026-09-14 review found nine bypasses that **no** grep for
# `tos_api::` can see, because they touch no AES symbol at all — hand-chained
# `ob_add` instead of `objc_add`, direct `ob_state` writes instead of
# `menu_ienable`/`menu_icheck`, four private copies of `screen_dims()` instead
# of `wind_get`. A falling number here is necessary, not sufficient; it does
# not replace reading the code against EmuTOS.
#
set -euo pipefail

usage() {
    sed -n '2,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//; $d'
    exit 0
}

VERBOSE=0
QUIET=0
for a in "$@"; do
    case "$a" in
        -v|--verbose) VERBOSE=1 ;;
        -q|--quiet)   QUIET=1 ;;
        -h|--help)    usage ;;
        *) echo "desk-drift-probe: unknown option: $a" >&2; exit 2 ;;
    esac
done

# Repo root: from git when available, else two levels up from this script.
if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then :; else
    ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fi
cd "$ROOT"

DESK_DIR="kernel/src/desk"
if [ ! -d "$DESK_DIR" ]; then
    # U10's end state: the module is gone, so every group is empty by
    # construction. Say so rather than failing.
    [ "$QUIET" -eq 1 ] || echo "kernel/src/desk/ does not exist — the desktop has left the kernel."
    echo "DRIFT-PROBE TOTAL: 0"
    exit 0
fi

# `grep` on this host is a ugrep wrapper that has swallowed `--include` globs
# before ([[shell-grep-is-ugrep-wrapper]]), so the probe pins the real one.
GREP=/usr/bin/grep
[ -x "$GREP" ] || GREP=grep

TMP=$(mktemp -d "${TMPDIR:-/tmp}/desk-drift.XXXXXX")
trap 'rm -rf "$TMP"' EXIT

# scan <outfile> <match-regex> <symbol-regex> <group-segments> <file>...
#   Emits one "group<TAB>file:line<TAB>text" record per matching code line.
scan() {
    local out="$1" match="$2" symre="$3" segs="$4"; shift 4
    : >"$out"
    [ "$#" -gt 0 ] || return 0
    $GREP -nE "$match" "$@" /dev/null 2>/dev/null \
    | awk -v symre="$symre" -v segs="$segs" -F: '
        {
            file = $1; line = $2;
            text = $0;
            sub(/^[^:]*:[0-9]+:/, "", text);
            code = text;
            sub(/\/\/.*$/, "", code);          # strip line comments
            if (code !~ symre) next;           # comment-only mention
            rest = code;
            printed = 0;
            while (match(rest, symre)) {
                sym = substr(rest, RSTART, RLENGTH);
                rest = substr(rest, RSTART + RLENGTH);
                sub(/^[^A-Za-z0-9_]+/, "", sym);   # the regex may lead with a delimiter
                # Reduce the symbol to its first <segs> path segments.
                n = split(sym, p, "::");
                g = p[1];
                for (i = 2; i <= segs && i <= n; i++) {
                    if (p[i] == "") break;
                    g = g "::" p[i];
                }
                key = file ":" line "\t" g;
                if (!(key in seen)) {
                    seen[key] = 1;
                    printf "%s\t%s:%s\t%s\n", g, file, line, text;
                    printed = 1;
                }
            }
            if (printed) lines[file ":" line] = 1;
        }
    ' >"$out"
}

DESK_FILES=$(ls "$DESK_DIR"/*.rs 2>/dev/null || true)
# shellcheck disable=SC2086
scan "$TMP/a" 'tos_api::' 'tos_api(::[A-Za-z0-9_]+)+' 3 $DESK_FILES
# shellcheck disable=SC2086
scan "$TMP/b" 'crate::'   'crate(::[A-Za-z0-9_]+)+'   2 $DESK_FILES

# Group C: the rest of the kernel naming the desktop module. `pub mod desk;`
# is part of it — it is the declaration U10 removes.
KERNEL_FILES=$(find kernel/src -name '*.rs' -not -path "$DESK_DIR/*" | sort)
# shellcheck disable=SC2086
scan "$TMP/c" '(crate::desk|[^A-Za-z0-9_:]desk::|^[[:space:]]*(pub )?mod desk;)' \
              '(crate::desk|(^|[^A-Za-z0-9_:])desk::[A-Za-z0-9_]+|mod desk)' 2 $KERNEL_FILES

# Drop `crate::desk` self-references inside group B (they are internal).
$GREP -v -E '^crate::desk\b' "$TMP/b" >"$TMP/b2" || true
mv "$TMP/b2" "$TMP/b"

count_lines() { cut -f2 "$1" | sort -u | wc -l | tr -d ' '; }

A=$(count_lines "$TMP/a")
B=$(count_lines "$TMP/b")
C=$(count_lines "$TMP/c")
TOTAL=$((A + B + C))

if [ "$QUIET" -eq 0 ]; then
    HEAD=$(git rev-parse --short HEAD 2>/dev/null || echo '?')
    DIRTY=$(git status --porcelain --untracked-files=no 2>/dev/null | wc -l | tr -d ' ')
    echo "NewTOS desktop drift probe — $ROOT @ $HEAD (${DIRTY} modified file(s))"
    echo

    section() { # <label> <file> <count>
        printf '%-58s %3s code line(s)\n' "$1" "$3"
        if [ "$3" != "0" ]; then
            awk -F'\t' '{c[$1]++} END {for (g in c) printf "    %-54s %3d\n", g, c[g]}' "$2" | sort
        fi
        if [ "$VERBOSE" -eq 1 ] && [ "$3" != "0" ]; then
            awk -F'\t' '{gsub(/^[[:space:]]+/, "", $3); printf "      %-34s %s\n", $2, $3}' "$2" \
                | sort -u
        fi
        echo
    }

    section "A  kernel/src/desk/ -> tos_api::"                  "$TMP/a" "$A"
    section "B  kernel/src/desk/ -> crate:: (kernel internals)" "$TMP/b" "$B"
    section "C  kernel/ -> desk module (outside kernel/src/desk/)" "$TMP/c" "$C"
fi

echo "DRIFT-PROBE TOTAL: $TOTAL"
