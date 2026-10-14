#!/usr/bin/env bash
#
# desk-cite-list.sh — list every desktop `fn` without an EmuTOS citation.
#
# Guard G4 of `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §4 (built in
# DESK43 U5a); the rule it serves is plan v2 §5 (c): *every receipt lists each
# touched `fn` without an EmuTOS `file:line`*. The receipt rule itself is in
# `docs/api/desktop/_rebuild-plan.md` §12.
#
#   tools/desk-cite-list.sh                 # per-file table + summary line
#   tools/desk-cite-list.sh -v              # …plus every uncited fn (file:line name)
#   tools/desk-cite-list.sh --since <rev>   # only fns touched since <rev>
#                                           # (git diff -M, renames followed;
#                                           # untracked files count
#                                           # as touched entirely) — the list a
#                                           # unit's receipt carries
#   tools/desk-cite-list.sh -q              # only the summary line
#   DESK_SRC=<dir> tools/desk-cite-list.sh  # another desktop source directory
#
# Last line, always: `DESK-CITE: fns=<n> cited=<c> newtos=<k> uncited=<u>`.
#
# ── What counts ────────────────────────────────────────────────────────────
#
# A `fn` (any visibility, `const`/`unsafe`/`async`/`extern`, free or in an
# `impl`, nested ones included) is **cited** when the comment block directly
# above it — the contiguous run of `///`, `//` and `#[…]` lines, no blank line —
# contains an EmuTOS source position: `name.c:N`, `name.h:N` or `name.S:N`
# (ranges `name.c:N-M` included).
#
# A `fn` that has deliberately no EmuTOS counterpart carries the marker
#
#     // NEWTOS: <one line why — e.g. AArch64 trap binding, Rust plumbing>
#
# in that block and is counted **newtos**, not uncited. The marker is a claim,
# not a proof: a receipt lists the NEWTOS fns it added, and review checks them.
#
# Everything else is **uncited**. That is a candidate list, not a defect list:
# many are helpers (`name_str`, `default`, `fmt`); some cite EmuTOS by name in
# prose or in the body. The number is reported so it can be watched, not
# driven to zero by decoration.
#
# `--since <rev>`: a fn counts as touched when a changed line (per
# `git diff -U0 <rev>`) falls between the first line of its comment block and
# the line before the next fn's block. That span is approximate for the last
# fn of an `impl` (it extends to the next fn) — over-reporting, never under.
#
set -euo pipefail

usage() {
    sed -n '2,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//; $d'
    exit 0
}

VERBOSE=0
QUIET=0
SINCE=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -v|--verbose) VERBOSE=1 ;;
        -q|--quiet)   QUIET=1 ;;
        --since)      shift; SINCE="${1:?--since needs a git revision}" ;;
        -h|--help)    usage ;;
        *) echo "desk-cite-list: unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then :; else
    ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fi
cd "$ROOT"

GREP=/usr/bin/grep
[ -x "$GREP" ] || GREP=grep
die() { echo "desk-cite-list: $*" >&2; exit 1; }

# Same source location rule as tools/desk-drift-probe.sh: DESK_SRC, else the
# kernel module, else the crate named `desktop`; never an empty list.
if [ -n "${DESK_SRC:-}" ]; then
    DESK_DIR="${DESK_SRC%/}"
elif [ -d kernel/src/desk ]; then
    DESK_DIR=kernel/src/desk
else
    DESK_DIR=""
    for t in $(find . \( -name target -o -name references -o -name .git -o -name Applications \
                         -o -name measurements \) -prune -o -name Cargo.toml -print 2>/dev/null | sort); do
        if $GREP -qE '^[[:space:]]*name[[:space:]]*=[[:space:]]*"desktop"' "$t"; then
            DESK_DIR="$(dirname "${t#./}")/src"
        fi
    done
    [ -n "$DESK_DIR" ] || die "no desktop source found (kernel/src/desk/ gone, no crate named \"desktop\"); set DESK_SRC"
fi
[ -d "$DESK_DIR" ] || die "$DESK_DIR is not a directory"
FILES=$(find "$DESK_DIR" -name '*.rs' -type f | sort)
[ -n "$FILES" ] || die "$DESK_DIR holds no .rs file"

TMP=$(mktemp -d "${TMPDIR:-/tmp}/desk-cite.XXXXXX")
trap 'rm -rf "$TMP"' EXIT

# Changed lines since <rev>: "file<TAB>line" records.
: >"$TMP/changed"
if [ -n "$SINCE" ]; then
    git rev-parse --verify -q "$SINCE^{commit}" >/dev/null || die "unknown revision: $SINCE"
    # Rename-aware over the whole tree, then filtered to the desktop source: a
    # file that moved into $DESK_DIR since <rev> (DESK43 U8b: kernel/src/desk/
    # -> programs/desktop/src/) counts only its changed lines, not every line
    # (a pathspec of the new directory alone would hide the old path from
    # rename detection and report each moved file as entirely new).
    git diff -U0 -M "$SINCE" | awk -v dir="$DESK_DIR/" '
        /^\+\+\+ / { f = substr($0, 5); sub(/^b\//, "", f); if (index(f, dir) != 1) f = ""; next }
        /^@@ / && f == "" { next }
        /^@@ / {
            h = $0; sub(/^@@ -[0-9,]+ \+/, "", h); sub(/ .*$/, "", h)
            n = split(h, p, ","); start = p[1] + 0; cnt = (n > 1) ? p[2] + 0 : 1
            for (i = 0; i < cnt; i++) printf "%s\t%d\n", f, start + i
        }' >>"$TMP/changed"
    git ls-files --others --exclude-standard -- "$DESK_DIR" | while read -r f; do
        printf '%s\t*\n' "$f"
    done >>"$TMP/changed"
fi

# One record per fn: file, fn line, block start line, next block start (span
# end + 1), status (cited|newtos|uncited), name.
# shellcheck disable=SC2086
awk '
    function flush_prev(endline) {
        if (pf != "") printf "%s\t%d\t%d\t%d\t%s\t%s\n", pf, pl, ps, endline, pst, pn
        pf = ""
    }
    FNR == 1 { flush_prev(999999999); blk = ""; bstart = 0 }
    /^[ \t]*\/\// || /^[ \t]*#\[/ {
        if (blk == "") bstart = FNR
        blk = blk "\n" $0
        next
    }
    /^[ \t]*(pub(\([^)]*\))?[ \t]+)?((const|async|unsafe|extern([ \t]+"[^"]*")?)[ \t]+)*fn[ \t]+[A-Za-z_][A-Za-z0-9_]*/ {
        name = $0
        sub(/^.*fn[ \t]+/, "", name); sub(/[^A-Za-z0-9_].*$/, "", name)
        start = (blk == "") ? FNR : bstart
        flush_prev(start)
        if (blk ~ /[A-Za-z0-9_]+\.(c|h|S):[0-9]/) st = "cited"
        else if (blk ~ /\n[ \t]*\/\/+!?[ \t]*NEWTOS:/) st = "newtos"
        else st = "uncited"
        pf = FILENAME; pl = FNR; ps = start; pst = st; pn = name
        blk = ""; bstart = 0
        next
    }
    { blk = ""; bstart = 0 }
    END { flush_prev(999999999) }
' $FILES >"$TMP/fns"

if [ -n "$SINCE" ]; then
    awk -F'\t' '
        NR == FNR { if ($2 == "*") whole[$1] = 1; else ch[$1, $2] = 1; next }
        {
            hit = ($1 in whole)
            for (l = $3; !hit && l < $4 && l < $3 + 100000; l++) if ((($1, l) in ch)) hit = 1
            if (hit) print
        }' "$TMP/changed" "$TMP/fns" >"$TMP/sel"
    SCOPE="fns touched since $SINCE"
else
    cp "$TMP/fns" "$TMP/sel"
    SCOPE="all fns"
fi

read -r N C K U < <(awk -F'\t' '{n++; s[$5]++} END {printf "%d %d %d %d\n", n, s["cited"], s["newtos"], s["uncited"]}' "$TMP/sel")

if [ "$QUIET" -eq 0 ]; then
    echo "NewTOS desktop citation list — $DESK_DIR/ ($SCOPE)"
    echo
    printf '%-34s %5s %6s %6s %8s\n' file fns cited newtos uncited
    awk -F'\t' '{n[$1]++; s[$1, $5]++} END {for (f in n) printf "%-34s %5d %6d %6d %8d\n", f, n[f], s[f, "cited"], s[f, "newtos"], s[f, "uncited"]}' "$TMP/sel" | sort
    echo
    if [ "$VERBOSE" -eq 1 ] || [ -n "$SINCE" ]; then
        echo "uncited:"
        awk -F'\t' '$5 == "uncited" {printf "  %s:%d  %s\n", $1, $2, $6}' "$TMP/sel" | sort -t: -k1,1 -k2,2n
        echo "newtos:"
        awk -F'\t' '$5 == "newtos" {printf "  %s:%d  %s\n", $1, $2, $6}' "$TMP/sel" | sort -t: -k1,1 -k2,2n
        echo
    fi
fi
echo "DESK-CITE: fns=$N cited=$C newtos=$K uncited=$U"
