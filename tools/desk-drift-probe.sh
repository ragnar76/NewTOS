#!/usr/bin/env bash
#
# desk-drift-probe.sh — count the desktop's remaining bypasses, as one number.
#
# This is the instrument measure 2 of `docs/analysis/desktop-userspace-plan-v2.md`
# §6.4 asks for: the count from §2.2 as a single reproducible command. Every
# DESK43 work unit reports this number, and it **must fall monotonically**. A
# unit that cannot show the number is not accepted. Version 2 (DESK43 U5a,
# guard G5 of `docs/analysis/desk43-u5-u10-review-2026-10-14.md` §4) closes the
# blind spots §2.4/§4.3 of that review found; see "Version 2" below.
#
#   tools/desk-drift-probe.sh            # the table + the total
#   tools/desk-drift-probe.sh -v         # …plus every counted file:line
#   tools/desk-drift-probe.sh -q         # only  DRIFT-PROBE TOTAL: <n>
#   tools/desk-drift-probe.sh -u         # only  DRIFT-PROBE UNSAFE: <n>
#   DESK_SRC=<dir> tools/desk-drift-probe.sh   # scan <dir> as the desktop
#                                        # source (kernel module if it lies
#                                        # under kernel/src/, else a crate)
#
# The last line of the default and -v output is always
# `DRIFT-PROBE TOTAL: <n>` (scripts take it with `tail -1`).
#
# ── What a "bypass" is here ────────────────────────────────────────────────
#
# The end state (review §3.1, unit U8b) is a crate `desktop` whose only
# dependencies are `newtos-gem` and `tos-types`. Anything the desktop names
# that would **not compile** in that crate is a bypass. Concretely:
#
#   A  a `tos_api::` path in the desktop source — the AES/VDI/GEMDOS server
#      reached as a Rust function instead of through a trap
#   B  while the desktop is the kernel module `kernel/src/desk/`: a `crate::`
#      path in it — the kernel reached directly (`crate::desk::…`, the
#      desktop's own module, is not counted). Once the desktop is its own
#      crate, `crate::` is the desktop itself and B counts `kernel::` paths
#      instead.
#   C  the rest of the kernel naming the desktop: the `desk` module
#      (`mod desk;`, `crate::desk…`, `desk::…`) or the `desktop` crate
#      (`desktop::…`, `extern crate desktop`, and the `desktop` dependency line
#      in `kernel/Cargo.toml`) — the dependency in the other direction, which
#      U9/U10 remove
#   D  a dependency in the desktop crate's `Cargo.toml` other than
#      `newtos-gem` and `tos-types` (guard G1: the dependency graph is the
#      compiler-enforced boundary; this group says when it is not). Zero by
#      construction while there is no desktop crate.
#
# `core::`, `alloc::`, `tos_types::` and the desktop-internal `super::`/`self::`
# are **not** bypasses: they survive the move unchanged.
#
# Not part of TOTAL, reported beside it: the number of code lines in the
# desktop source containing `unsafe`. It is a **non-increasing** number: the
# baseline is UNSAFE_BASELINE below; a run that finds more prints a loud
# error and exits 3 (after printing TOTAL). A unit that lowers it lowers the
# baseline in the same change.
#
# ── What counts as a line of code ──────────────────────────────────────────
#
# Comments are removed before matching: everything from `//` to the end of the
# line (so `//!`, `///` and trailing comments cannot inflate the count) and
# `/* … */` block comments, including ones spanning several lines. (§2.2 of the
# plan counted 36 raw `tos_api::` hits in `kernel/src/desk/` of which the
# majority were prose — the raw grep is not the measure.) String and char
# literals are recognised, so `b"c:/*.txt"` stays code; raw strings (`r"…"`)
# are not — the kernel has none (2026-10-14).
#
# A `use` statement spanning several lines is joined first and counted as
# **one** line (its first). Brace lists are expanded per path:
# `use crate::{sync::SpinMutex, kprintln};` is one line in the groups
# `crate::sync` and `crate::kprintln`.
#
# ── Version 2 (DESK43 U5a) ─────────────────────────────────────────────────
#
#   * No empty zero. Version 1 printed `TOTAL: 0` and exited when
#     `kernel/src/desk/` did not exist — the intended U10 end state, but also a
#     silent "green" when the files had only moved. Version 2 looks for the
#     desktop crate (a `Cargo.toml` with `name = "desktop"`) when the module is
#     gone and **fails loudly** (exit 1) when it finds neither.
#   * Subdirectories of the desktop source are scanned (v1: `*.rs` of the top
#     directory only).
#   * Brace `use` lists (`crate::{…}`, `tos_api::{…}`) are counted (v1 matched
#     only `crate::name`).
#   * Group C also matches the `desktop` crate (after a rename to `desktop::`
#     v1's `desk::` pattern would have counted a vacuous 0).
#   * Group D (desktop `Cargo.toml` dependencies) and the `unsafe` count are new.
#   With `kernel/src/desk/` in place and no brace lists in it, v2 counts exactly
#   what v1 counted (31 at U5a, review §1.1).
#
# ── What this probe deliberately does NOT see ──────────────────────────────
#
# Plan §6.1: the 2026-09-14 review found nine bypasses that **no** grep for
# `tos_api::` can see, because they touch no AES symbol at all — hand-chained
# `ob_add` instead of `objc_add`, direct `ob_state` writes instead of
# `menu_ienable`/`menu_icheck`, four private copies of `screen_dims()` instead
# of `wind_get`. A falling number here is necessary, not sufficient; it does
# not replace reading the code against EmuTOS (guards G2/G3/G4 of the review:
# the call-sequence oracle, `cargo xtask emutos-consts`,
# `tools/desk-cite-list.sh`).
#
set -euo pipefail

usage() {
    sed -n '2,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//; $d'
    exit 0
}

# Baseline of the `unsafe` count (kernel/src/desk/). Lower it when a unit
# removes `unsafe` lines; never raise it. U5a: 24. DESK43 U6b: 20 (the four
# trap `asm!` blocks of the AES/GEMDOS/VDI/XBIOS bindings moved to
# programs/newtos-gem).
UNSAFE_BASELINE=20

VERBOSE=0
QUIET=0
UNSAFE_ONLY=0
for a in "$@"; do
    case "$a" in
        -v|--verbose) VERBOSE=1 ;;
        -q|--quiet)   QUIET=1 ;;
        -u|--unsafe)  UNSAFE_ONLY=1 ;;
        -h|--help)    usage ;;
        *) echo "desk-drift-probe: unknown option: $a" >&2; exit 2 ;;
    esac
done

# Repo root: from git when available, else one level up from this script.
if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then :; else
    ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fi
cd "$ROOT"

# `grep` on this host is a ugrep wrapper that has swallowed `--include` globs
# before ([[shell-grep-is-ugrep-wrapper]]), so the probe pins the real one.
GREP=/usr/bin/grep
[ -x "$GREP" ] || GREP=grep

die() { echo "desk-drift-probe: $*" >&2; exit 1; }

# ── Locate the desktop source ───────────────────────────────────────────────
#
# MODE=module: the desktop is `kernel/src/desk/`, part of the kernel crate.
# MODE=crate:  the desktop is a crate named `desktop` (its `src/`).
DESK_DIR=""
DESK_TOML=""
MODE=""
if [ -n "${DESK_SRC:-}" ]; then
    [ -d "$DESK_SRC" ] || die "DESK_SRC=$DESK_SRC is not a directory"
    DESK_DIR="${DESK_SRC%/}"
    case "$DESK_DIR" in
        kernel/src/*) MODE=module ;;
        *) MODE=crate
           [ -f "$(dirname "$DESK_DIR")/Cargo.toml" ] && DESK_TOML="$(dirname "$DESK_DIR")/Cargo.toml" ;;
    esac
elif [ -d kernel/src/desk ]; then
    DESK_DIR=kernel/src/desk
    MODE=module
else
    # The module is gone: the desktop must now be a crate called `desktop`.
    # Every Cargo.toml of the workspace (not target/, references/, sdk/ builds).
    for t in $(find . \( -name target -o -name references -o -name .git -o -name node_modules \
                         -o -name Applications -o -name measurements \) -prune -o \
                    -name Cargo.toml -print 2>/dev/null | sort); do
        if $GREP -qE '^[[:space:]]*name[[:space:]]*=[[:space:]]*"desktop"' "$t"; then
            [ -z "$DESK_TOML" ] || die "two crates named \"desktop\": $DESK_TOML and $t"
            DESK_TOML="${t#./}"
        fi
    done
    [ -n "$DESK_TOML" ] || die "no desktop source found: kernel/src/desk/ does not exist and no Cargo.toml declares name = \"desktop\". Refusing to report an empty 0 — if the desktop has moved, point DESK_SRC at it."
    DESK_DIR="$(dirname "$DESK_TOML")/src"
    [ -d "$DESK_DIR" ] || die "$DESK_TOML has no src/ directory"
    MODE=crate
fi

DESK_FILES=$(find "$DESK_DIR" -name '*.rs' -type f | sort)
[ -n "$DESK_FILES" ] || die "$DESK_DIR holds no .rs file — refusing to report an empty 0"
NFILES=$(echo "$DESK_FILES" | wc -l | tr -d ' ')

TMP=$(mktemp -d "${TMPDIR:-/tmp}/desk-drift.XXXXXX")
trap 'rm -rf "$TMP"' EXIT

# Shared awk function: remove the comments from one physical line, carrying
# the open block-comment / open string state (INBLOCK, INSTR) across lines.
# String and char literals are kept verbatim, so `b"c:/*.txt"` is code, not the
# start of a comment. Raw strings (`r"…"`, `r#"…"#`) are not recognised; the
# kernel has none (2026-10-14).
AWK_STRIP='
function strip(s,    out, k, n, c, c2, m, i, j) {
    if (!INBLOCK && !INSTR && index(s, "\"") == 0 && index(s, "'"'"'") == 0 && index(s, "/*") == 0) {
        i = index(s, "//")
        return (i > 0) ? substr(s, 1, i - 1) : s
    }
    out = ""; n = length(s); k = 1
    while (k <= n) {
        c = substr(s, k, 1); c2 = substr(s, k, 2)
        if (INBLOCK) { if (c2 == "*/") { INBLOCK = 0; k += 2 } else k++; continue }
        if (INSTR) {
            out = out c
            if (c == "\\") { out = out substr(s, k + 1, 1); k += 2; continue }
            if (c == "\"") INSTR = 0
            k++; continue
        }
        if (c2 == "//") return out
        if (c2 == "/*") { INBLOCK = 1; out = out " "; k += 2; continue }
        if (c == "\"") { INSTR = 1; out = out c; k++; continue }
        if (c == "'"'"'") {
            if (substr(s, k + 1, 1) == "\\") {
                m = index(substr(s, k + 2), "'"'"'")
                if (m > 0) { out = out substr(s, k, m + 2); k += m + 2; continue }
            } else if (substr(s, k + 2, 1) == "'"'"'") { out = out substr(s, k, 3); k += 3; continue }
        }
        out = out c; k++
    }
    return out
}
'

# scan <outfile> <roots-regex> <segs> <keep-regex> <drop-regex> <moddecl> <file>...
#   Emits one "group<TAB>file:line<TAB>text" record per (code line, group).
#   <roots-regex>  alternation of path roots, e.g. `tos_api` or `crate|desk`;
#                  a root counts only when followed by `::` and not itself the
#                  tail of a longer path (`foo::desk::` is not `desk::`)
#   <segs>         the group is the first <segs> segments of the path
#   <keep-regex>   a path must match it to count ('' = keep every path)
#   <drop-regex>   a path matching it is not counted ('' = drop none)
#   <moddecl>      1 = also count `mod desk;` and `extern crate desktop`
scan() {
    local out="$1" roots="$2" segs="$3" keep="$4" drop="$5" moddecl="$6"; shift 6
    : >"$out"
    [ "$#" -gt 0 ] || return 0
    awk -v roots="$roots" -v segs="$segs" -v keep="$keep" -v drop="$drop" \
        -v moddecl="$moddecl" -v blockwarn="$TMP/blockwarn" "$AWK_STRIP"'
        function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t]+$/, "", s); return s }
        # Expand a brace list: "a::{b, c::{d, e}}" -> "a::b\na::c::d\na::c::e".
        function expand(s,    i, k, c, depth, head, start, item, out, part) {
            i = index(s, "{")
            if (i == 0) return s
            head = substr(s, 1, i - 1)
            depth = 0; out = ""; start = i + 1
            for (k = i; k <= length(s); k++) {
                c = substr(s, k, 1)
                if (c == "{") {
                    depth++
                    if (depth == 1) start = k + 1
                } else if (c == "}" || (c == "," && depth == 1)) {
                    if (c == "}") depth--
                    if (depth == 0 || c == ",") {
                        item = trim(substr(s, start, k - start))
                        if (item != "") {
                            part = expand(head item)
                            out = (out == "") ? part : out "\n" part
                        }
                        start = k + 1
                        if (depth == 0) break
                    }
                }
            }
            return out
        }
        # The path starting at position p of `code`: identifier characters and
        # `::`, plus one balanced brace list right after a `::`. Sets PLEN to
        # the number of characters consumed.
        function path_at(code, p,    k, c, s, depth) {
            s = ""
            for (k = p; k <= length(code); k++) {
                c = substr(code, k, 1)
                if (c ~ /[A-Za-z0-9_:]/) { s = s c; continue }
                if (c == "{" && substr(s, length(s) - 1) == "::") {
                    depth = 0
                    for (; k <= length(code); k++) {
                        c = substr(code, k, 1); s = s c
                        if (c == "{") depth++
                        else if (c == "}") { depth--; if (depth == 0) break }
                    }
                }
                break
            }
            PLEN = length(s)
            sub(/:+$/, "", s)
            return s
        }
        function group_of(path,    n, p, g, i) {
            n = split(path, p, "::")
            g = p[1]
            for (i = 2; i <= segs && i <= n; i++) {
                if (p[i] == "") break
                g = g "::" p[i]
            }
            return g
        }
        function emit(g, file, line, text,    key) {
            key = file ":" line "\t" g
            if (key in seen) return
            seen[key] = 1
            printf "%s\t%s:%s\t%s\n", g, file, line, text
        }
        function process(code, file, line, text,    rest, off, pos, prev, prev3, ok, path, n, paths, i, pp) {
            if (moddecl == 1) {
                if (code ~ /^[ \t]*(pub(\([^)]*\))?[ \t]+)?mod[ \t]+desk[ \t]*;/) emit("mod desk", file, line, text)
                if (code ~ /(^|[^A-Za-z0-9_])extern[ \t]+crate[ \t]+desktop([^A-Za-z0-9_]|$)/) emit("extern crate desktop", file, line, text)
            }
            rest = code; off = 0
            while (match(rest, "(" roots ")::")) {
                pos = off + RSTART
                prev = (pos > 1) ? substr(code, pos - 1, 1) : ""
                ok = (prev !~ /[A-Za-z0-9_]/)
                if (ok && prev == ":") {
                    prev3 = (pos > 3) ? substr(code, pos - 3, 1) : ""
                    if (prev3 ~ /[A-Za-z0-9_>]/) ok = 0
                }
                PLEN = RLENGTH
                if (ok) {
                    path = path_at(code, pos)
                    n = split(expand(path), paths, "\n")
                    for (i = 1; i <= n; i++) {
                        pp = trim(paths[i])
                        gsub(/[ \t]/, "", pp)
                        sub(/::self$/, "", pp)
                        if (keep != "" && pp !~ keep) continue
                        if (drop != "" && pp ~ drop) continue
                        emit(group_of(pp), file, line, text)
                    }
                }
                if (PLEN < 1) PLEN = 1
                off = pos + PLEN - 1
                rest = substr(code, off + 1)
            }
        }
        FNR == 1 {
            if (INBLOCK && prevfile != "") print prevfile >> blockwarn
            INBLOCK = 0; INSTR = 0; inuse = 0
        }
        { prevfile = FILENAME }
        {
            text = $0
            code = strip($0)
            if (inuse) {
                ucode = ucode " " code
                if (index(code, ";") > 0) { process(ucode, ufile, uline, utext); inuse = 0 }
                next
            }
            if (code ~ /^[ \t]*(pub(\([^)]*\))?[ \t]+)?use[ \t]/ && index(code, ";") == 0) {
                inuse = 1; ucode = code; ufile = FILENAME; uline = FNR; utext = text
                next
            }
            process(code, FILENAME, FNR, text)
        }
        END {
            if (INBLOCK) print prevfile >> blockwarn
        }
    ' "$@" >"$out"
}

# count_unsafe <file>... — code lines naming `unsafe` (comments removed as above).
count_unsafe() {
    awk -v verbose="$VERBOSE" "$AWK_STRIP"'
        FNR == 1 { INBLOCK = 0; INSTR = 0 }
        { c = strip($0); if (c ~ /(^|[^A-Za-z0-9_])unsafe([^A-Za-z0-9_]|$)/) { n++; if (verbose == 1) { t = $0; gsub(/^[ \t]+/, "", t); printf "      %-34s %s\n", FILENAME ":" FNR, t > "/dev/stderr" } } }
        END { print n + 0 }
    ' "$@"
}

# cargo_deps <Cargo.toml> — "name<TAB>line" for every dependency of every
# [*dependencies] table (inline `name = …`, dotted `name.workspace = true`, and
# `[dependencies.name]` table headers).
cargo_deps() {
    awk '
        /^[ \t]*\[/ {
            h = $0; gsub(/[ \t\[\]]/, "", h)
            indeps = (h ~ /(^|\.)(dev-|build-)?dependencies$/)
            if (h ~ /(^|\.)(dev-|build-)?dependencies\.[A-Za-z0-9_-]+$/) {
                n = h; sub(/^.*dependencies\./, "", n); printf "%s\t%d\n", n, FNR
                indeps = 0
            }
            next
        }
        indeps && /^[ \t]*[A-Za-z0-9_-]+[ \t]*(\.|=)/ {
            n = $0; sub(/^[ \t]*/, "", n); sub(/[ \t]*(\.|=).*$/, "", n)
            printf "%s\t%d\n", n, FNR
        }
    ' "$1"
}

: >"$TMP/blockwarn"

# shellcheck disable=SC2086
scan "$TMP/a" 'tos_api' 3 '' '' 0 $DESK_FILES
if [ "$MODE" = module ]; then
    # shellcheck disable=SC2086
    scan "$TMP/b" 'crate' 2 '' '^crate::desk(::|$)' 0 $DESK_FILES
    B_LABEL="B  $DESK_DIR/ -> crate:: (kernel internals)"
else
    # shellcheck disable=SC2086
    scan "$TMP/b" 'kernel' 2 '' '' 0 $DESK_FILES
    B_LABEL="B  $DESK_DIR/ -> kernel:: (kernel internals)"
fi
A_LABEL="A  $DESK_DIR/ -> tos_api::"

# Group C: the rest of the kernel naming the desktop. `pub mod desk;` is part
# of it — it is the declaration U8b/U10 remove.
KERNEL_FILES=$(find kernel/src -name '*.rs' -type f | $GREP -v "^$DESK_DIR/" | sort)
# shellcheck disable=SC2086
scan "$TMP/c" 'crate|desk|desktop' 2 '^(crate::desk(::|$)|desk::|desktop::)' '' 1 $KERNEL_FILES
# …and the kernel crate's dependency on the desktop crate.
if [ -f kernel/Cargo.toml ]; then
    cargo_deps kernel/Cargo.toml | awk -F'\t' -v f=kernel/Cargo.toml '$1 == "desktop" { printf "Cargo dependency desktop\t%s:%s\t%s = …\n", f, $2, $1 }' >>"$TMP/c"
fi

# Group D: the desktop crate's dependencies beyond the allowed two.
: >"$TMP/d"
if [ -n "$DESK_TOML" ]; then
    cargo_deps "$DESK_TOML" | awk -F'\t' -v f="$DESK_TOML" '$1 != "newtos-gem" && $1 != "tos-types" { printf "%s\t%s:%s\tdependency %s\n", $1, f, $2, $1 }' >"$TMP/d"
fi

if [ -s "$TMP/blockwarn" ]; then
    echo "desk-drift-probe: WARNING: unterminated /* block comment in: $(sort -u "$TMP/blockwarn" | tr '\n' ' ')" >&2
fi

count_lines() { cut -f2 "$1" | sort -u | $GREP -c . || true; }

A=$(count_lines "$TMP/a")
B=$(count_lines "$TMP/b")
C=$(count_lines "$TMP/c")
D=$(count_lines "$TMP/d")
TOTAL=$((A + B + C + D))
# shellcheck disable=SC2086
UNSAFE=$(VERBOSE=0 count_unsafe $DESK_FILES)

if [ "$UNSAFE_ONLY" -eq 1 ]; then
    echo "DRIFT-PROBE UNSAFE: $UNSAFE"
    [ "$UNSAFE" -le "$UNSAFE_BASELINE" ] || exit 3
    exit 0
fi

if [ "$QUIET" -eq 0 ]; then
    HEAD=$(git rev-parse --short HEAD 2>/dev/null || echo '?')
    DIRTY=$( (git status --porcelain --untracked-files=no 2>/dev/null || true) | wc -l | tr -d ' ')
    echo "NewTOS desktop drift probe — $ROOT @ $HEAD (${DIRTY} modified file(s))"
    if [ "$MODE" = module ]; then
        echo "desktop source: $DESK_DIR/ (kernel module, $NFILES .rs file(s), subdirectories included)"
    else
        echo "desktop source: $DESK_DIR/ (crate ${DESK_TOML}, $NFILES .rs file(s), subdirectories included)"
    fi
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

    section "$A_LABEL"                                          "$TMP/a" "$A"
    section "$B_LABEL"                                          "$TMP/b" "$B"
    if [ "$MODE" = module ]; then
        section "C  kernel/ -> desk module (outside kernel/src/desk/)" "$TMP/c" "$C"
    else
        section "C  kernel/ -> desktop crate (code + kernel/Cargo.toml)" "$TMP/c" "$C"
    fi
    if [ -n "$DESK_TOML" ]; then
        section "D  $DESK_TOML -> deps other than newtos-gem/tos-types" "$TMP/d" "$D"
    else
        printf '%-58s %3s line(s)   (no desktop crate yet)\n\n' "D  desktop Cargo.toml -> deps other than newtos-gem/tos-types" "$D"
    fi
    printf '%-58s %3s code line(s)   [not in TOTAL; baseline %s, must not rise]\n' \
        "unsafe  in $DESK_DIR/" "$UNSAFE" "$UNSAFE_BASELINE"
    if [ "$VERBOSE" -eq 1 ]; then
        # shellcheck disable=SC2086
        VERBOSE=1 count_unsafe $DESK_FILES 2>&1 >/dev/null | sort -t: -k1,1 -k2,2n
    fi
    echo
fi

echo "DRIFT-PROBE TOTAL: $TOTAL"
if [ "$UNSAFE" -gt "$UNSAFE_BASELINE" ]; then
    echo "desk-drift-probe: ERROR: unsafe lines rose from $UNSAFE_BASELINE to $UNSAFE (must not rise; see -v)" >&2
    exit 3
fi
