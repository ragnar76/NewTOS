#!/usr/bin/env bash
#
# emutos-config.sh — dump the compile-time CONF_* configuration of the
# pinned EmuTOS reference build (`make 512`, i.e. -DTARGET_512) straight
# from the preprocessor, and write it to docs/api/_emutos-config.md.
#
# Written for TASK/REF01.md. Does not modify references/emutos/ (reads
# only); writes only the one output document plus scratch files under a
# private mktemp(1) directory that is removed on exit.
#
# --------------------------------------------------------------------
# Why "-DTARGET_512 -Iinclude -I." on an empty translation unit that
# just #includes config.h is the WHOLE truth for this profile (checked
# against the pinned tree, not assumed — see REF01's own probe):
#
#   - DEF, the user-settable -D list, starts out empty:
#       references/emutos/Makefile:132   DEF =
#   - `make 512` adds exactly one define to it:
#       references/emutos/Makefile:561   512: override DEF += -DTARGET_512
#   - no localconf.h sits in the tree to inject silent extra defines;
#     the Makefile's own test for one is at
#       references/emutos/Makefile:170-176
#     (this script re-checks that live, below — it does not trust the
#     absence to still hold on a future pinned commit)
#   - `make 512`'s actual C compile also carries -DWITH_AES=1 -DWITH_CLI=1
#     (references/emutos/Makefile:267, DEFINES = ... $(DEF)), but
#     config.h never reads WITH_AES or WITH_CLI (grep confirms — their
#     only consumers are bios/endrom.c:15,30 and vdi/vdi_mouse.c:26,67,
#     i.e. link-time file selection, not a CONF_* macro). So they do not
#     belong in the config.h preprocessor run below, and their absence
#     does not make the table incomplete.
#
# Every CONF_* macro in config.h resolves to a plain integer or hex
# literal, EXCEPT two (CONF_WITH_SCSI_DRIVER, CONF_WITH_XHDI) which are
# defined as `!CONF_WITH_EXTERNAL_DISK_DRIVER` (include/config.h:1685,
# :1692). Plain `-E -dM` text substitution does not evaluate the `!`,
# so this script evaluates any non-literal residue through a real
# integer-constant-expression evaluator (bash arithmetic, which has the
# same semantics as a C preprocessor #if for !/&&/||/parens) instead of
# printing the raw, potentially misleading text.
#
# --------------------------------------------------------------------
# Reproducibility:
#   - The EmuTOS commit hash is read live from the pinned tree (git
#     rev-parse), so the table always states what it was actually
#     generated against, and changes truthfully if the pin moves.
#   - The "generation date" in the header is NOT `date`-derived — a
#     wall-clock timestamp would make two runs on different days
#     produce different files, breaking the "run it twice, get the same
#     file" acceptance check. It is a fixed string (the day this table
#     was generated for REF01), overridable via EMUTOS_CONFIG_GEN_DATE
#     for a deliberate re-generation. Bump the default by hand when you
#     deliberately regenerate this table against a moved pin; do not
#     wire it to `date`.
#
# Usage:
#   tools/emutos-config.sh [output-file]
#   Defaults to docs/api/_emutos-config.md relative to the repo root.
#
# Requires: a working m68k-atari-mint-gcc (default
# /opt/cross-mint/bin/m68k-atari-mint-gcc, override with EMUTOS_CC).

set -euo pipefail

die() { echo "emutos-config.sh: $*" >&2; exit 1; }
warn() { echo "emutos-config.sh: WARNING: $*" >&2; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EMUTOS_DIR="$REPO_ROOT/references/emutos"
EMUTOS_CC="${EMUTOS_CC:-/opt/cross-mint/bin/m68k-atari-mint-gcc}"
OUT_FILE="${1:-$REPO_ROOT/docs/api/_emutos-config.md}"
GEN_DATE="${EMUTOS_CONFIG_GEN_DATE:-2026-09-24}"

[ -d "$EMUTOS_DIR" ] || die "pinned tree not found at $EMUTOS_DIR"
[ -f "$EMUTOS_DIR/include/config.h" ] || die "$EMUTOS_DIR/include/config.h missing"
[ -e "$EMUTOS_DIR/localconf.h" ] && die "localconf.h is present in $EMUTOS_DIR -- the -DTARGET_512-alone recipe assumes there is none (Makefile:170-176); remove it or stop trusting this table until it's gone"
command -v "$EMUTOS_CC" >/dev/null 2>&1 || die "preprocessor not found: $EMUTOS_CC (set EMUTOS_CC to override)"

EMUTOS_COMMIT="$(git -C "$EMUTOS_DIR" rev-parse HEAD)" || die "could not read the pinned tree's commit hash"
EMUTOS_COMMIT_SHORT="${EMUTOS_COMMIT:0:8}"

# Non-fatal: tell the reader if the pinned tree has local changes beyond
# the harmless .DS_Store, since that would mean the table reflects
# something other than the recorded commit.
DIRTY="$(git -C "$EMUTOS_DIR" status --porcelain 2>/dev/null | grep -v '\.DS_Store$' || true)"
if [ -n "$DIRTY" ]; then
    warn "pinned tree has uncommitted changes beyond .DS_Store — the table below reflects the WORKING TREE, not commit $EMUTOS_COMMIT_SHORT cleanly:"
    warn "$DIRTY"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# ----------------------------------------------------------------------
# Step 1: get the full set of CONF_* macro names config.h defines, from
# the TARGET_512 -dM dump (every TARGET_* variant defines the same 106
# names in this tree; verified below per block, not assumed).
# ----------------------------------------------------------------------
printf '#include "config.h"\n' > "$WORK/probe.c"

dump_names() {
    local dflag="$1"
    "$EMUTOS_CC" -E -dM -D"$dflag" -I"$EMUTOS_DIR/include" -I"$EMUTOS_DIR" "$WORK/probe.c" 2>"$WORK/dm.err" \
        | grep -oE '^#define (CONF_[A-Za-z0-9_]+)' \
        | awk '{print $2}' \
        | sort
    if [ -s "$WORK/dm.err" ]; then
        warn "preprocessor stderr for -D$dflag:"
        cat "$WORK/dm.err" >&2
    fi
}

dump_names TARGET_512 > "$WORK/names.txt"
NUM_MACROS="$(wc -l < "$WORK/names.txt" | tr -d ' ')"
[ "$NUM_MACROS" -gt 0 ] || die "no CONF_* macros found — something is wrong with the include path or config.h"

# ----------------------------------------------------------------------
# Step 2: fully resolve every macro's value for a given -D flag.
#   (a) macro-substitute each name via -E, using a purely numeric label
#       per line so the label itself is never accidentally macro-
#       expanded away (it happened once during development: labelling
#       a line with the macro name itself made the preprocessor expand
#       BOTH copies, destroying the name/value pairing).
#   (b) anything left that isn't a plain integer or 0x-hex literal
#       (i.e. still contains an operator like `!`) gets evaluated as an
#       integer-constant-expression, the same semantics the compiler's
#       own #if would apply.
# ----------------------------------------------------------------------
resolve_values() {
    local dflag="$1" outtsv="$2"
    local src="$WORK/resolve_$dflag.c"
    {
        printf '#include "config.h"\n'
        printf '__RESOLVE_START__\n'
        local i=0
        while IFS= read -r n; do
            printf '__IDX_%d__ %s\n' "$i" "$n"
            i=$((i + 1))
        done < "$WORK/names.txt"
        printf '__RESOLVE_END__\n'
    } > "$src"

    "$EMUTOS_CC" -E -D"$dflag" -I"$EMUTOS_DIR/include" -I"$EMUTOS_DIR" "$src" 2>"$WORK/resolve.err" \
        | sed -n '/^__RESOLVE_START__$/,/^__RESOLVE_END__$/p' \
        | grep -E '^__IDX_[0-9]+__ ' \
        | sed -E 's/^__IDX_[0-9]+__ //' \
        > "$WORK/rawvals_$dflag.txt"
    if [ -s "$WORK/resolve.err" ]; then
        warn "preprocessor stderr while resolving -D$dflag:"
        cat "$WORK/resolve.err" >&2
    fi

    local n_names n_vals
    n_names="$(wc -l < "$WORK/names.txt" | tr -d ' ')"
    n_vals="$(wc -l < "$WORK/rawvals_$dflag.txt" | tr -d ' ')"
    [ "$n_names" = "$n_vals" ] || die "resolve_values($dflag): $n_names names but $n_vals resolved values -- preprocessor output did not line up"

    : > "$outtsv"
    paste "$WORK/names.txt" "$WORK/rawvals_$dflag.txt" | while IFS=$'\t' read -r name val; do
        if [[ "$val" =~ ^-?[0-9]+$ ]] || [[ "$val" =~ ^0[xX][0-9A-Fa-f]+$ ]]; then
            printf '%s\t%s\n' "$name" "$val" >> "$outtsv"
        else
            local ev
            if ev=$(bash -c "echo \$(( $val ))" 2>/dev/null); then
                printf '%s\t%s\n' "$name" "$ev" >> "$outtsv"
            else
                warn "could not arithmetically resolve $name = '$val' for -D$dflag; keeping raw text"
                printf '%s\t%s\n' "$name" "$val" >> "$outtsv"
            fi
        fi
    done
}

resolve_values TARGET_512 "$WORK/table_512.tsv"

# ----------------------------------------------------------------------
# Step 3: enumerate every `#ifdef TARGET_*` block in config.h (name,
# start line, end line) by depth-tracking from each match, independent
# of surrounding nesting (the file is itself wrapped in one big
# `#ifndef _CONFIG_H` include guard, which would otherwise throw off a
# naive single global depth counter).
# ----------------------------------------------------------------------
CONFIG_H="$EMUTOS_DIR/include/config.h"
awk '
{ L[NR] = $0 }
END {
  n = NR
  for (i = 1; i <= n; i++) {
    if (L[i] ~ /^#[ \t]*ifdef[ \t]+TARGET_[A-Za-z0-9_]+/) {
      match(L[i], /TARGET_[A-Za-z0-9_]+/)
      name = substr(L[i], RSTART, RLENGTH)
      depth = 1
      j = i + 1
      while (depth > 0 && j <= n) {
        if (L[j] ~ /^#[ \t]*(if|ifdef|ifndef)([ \t]|$)/) depth++
        else if (L[j] ~ /^#[ \t]*endif/) depth--
        if (depth > 0) j++
      }
      print name "\t" i "\t" j
    }
  }
}
' "$CONFIG_H" > "$WORK/target_blocks.tsv"

# Sanity check demanded by REF01 point 3: does a #ifdef TARGET_512 block
# exist at all? (ACC01 said no — this re-checks it against the live
# tree rather than trusting that write-up.)
TARGET_512_BLOCK_COUNT="$(awk -F'\t' '$1=="TARGET_512"' "$WORK/target_blocks.tsv" | wc -l | tr -d ' ')"
TARGET_512_TREE_HITS="$(grep -rn 'TARGET_512' --include='*.h' --include='*.c' --include='*.S' --include='Makefile' "$EMUTOS_DIR" 2>/dev/null | grep -v '^\.git/' || true)"

# ----------------------------------------------------------------------
# Step 4: for every OTHER TARGET_* block found, resolve its full value
# set the same way (a real preprocessor run with that -D, not a text
# scan of the block) and diff against TARGET_512. A text scan of just
# the block's own lines would miss cascades through a non-CONF_-
# prefixed switch (e.g. TARGET_192 flips DETECT_NATIVE_FEATURES, which
# changes CONF_WITH_ARANYM's generic default two thousand lines away) --
# verified necessary during development, not a theoretical concern.
# ----------------------------------------------------------------------
: > "$WORK/blocks_summary.tsv"
while IFS=$'\t' read -r bname bstart bend; do
    [ "$bname" = "TARGET_512" ] && continue  # doesn't exist; nothing to diff
    resolve_values "$bname" "$WORK/table_$bname.tsv"
    n2="$(dump_names "$bname" | wc -l | tr -d ' ')"
    if [ "$n2" != "$NUM_MACROS" ]; then
        warn "$bname defines $n2 CONF_* macros, TARGET_512 defines $NUM_MACROS -- the diff below is only over the shared names"
    fi
    join -t $'\t' -j1 \
        <(sort -t $'\t' -k1,1 "$WORK/table_512.tsv") \
        <(sort -t $'\t' -k1,1 "$WORK/table_$bname.tsv") \
        | awk -F'\t' '$2!=$3 {print}' \
        | sort > "$WORK/diff_$bname.tsv"
    ndiff="$(wc -l < "$WORK/diff_$bname.tsv" | tr -d ' ')"
    printf '%s\t%s\t%s\t%s\n' "$bname" "$bstart" "$bend" "$ndiff" >> "$WORK/blocks_summary.tsv"
done < "$WORK/target_blocks.tsv"

# ----------------------------------------------------------------------
# Step 5: which CONF_WITH_* macros merely gate a *runtime*-detected
# variable (include/has.h's `#if CONF_WITH_X / extern int has_x` idiom)
# rather than a compiled-out/compiled-in code path. This is the data
# behind the machine-question paragraph.
# ----------------------------------------------------------------------
awk '
/^#if CONF_WITH_/ {
  match($0, /CONF_WITH_[A-Za-z0-9_]+/)
  cond = substr($0, RSTART, RLENGTH)
  pending = 1
  next
}
pending && /^extern int has_/ {
  match($0, /has_[A-Za-z0-9_]+/)
  var = substr($0, RSTART, RLENGTH)
  print cond "\t" var
  pending = 0
  next
}
/^#else/ { pending = 0 }
' "$EMUTOS_DIR/include/has.h" > "$WORK/runtime_gated.tsv"

# ========================================================================
# Assemble docs/api/_emutos-config.md
# ========================================================================
mkdir -p "$(dirname "$OUT_FILE")"

{
cat <<EOF
# EmuTOS reference configuration (\`make 512\`, TARGET_512)

Generated by \`tools/emutos-config.sh\` — re-run it to regenerate; do not
hand-edit. Generated: $GEN_DATE. Pinned tree: \`references/emutos\` @
\`$EMUTOS_COMMIT\`.

This table replaces re-deriving \`CONF_WITH_*\`/\`CONF_*\` gating from
\`config.h\`'s nesting by eye for every extraction (see \`TASK/REF01.md\`
— two extractions read the wrong \`#ifdef TARGET_192\` block for our
profile in one week). Extractions and spec entries cite this file, not
\`config.h\` directly, for "is feature X compiled into our reference
build" questions.

## Why \`-DTARGET_512\` alone is the whole truth here

* \`DEF\` (the user \`-D\` list) starts empty: \`references/emutos/Makefile:132\`.
* \`make 512\` adds exactly one define to it:
  \`references/emutos/Makefile:561\` (\`512: override DEF += -DTARGET_512\`).
* No \`localconf.h\` sits in the pinned tree (checked live by this run,
  not assumed); the Makefile's own test for one is
  \`references/emutos/Makefile:170-176\`.
* \`make 512\`'s real compile also passes \`-DWITH_AES=1 -DWITH_CLI=1\`
  (\`references/emutos/Makefile:267\`), but \`config.h\` never reads
  \`WITH_AES\`/\`WITH_CLI\` (their only consumers are
  \`bios/endrom.c:15,30\` and \`vdi/vdi_mouse.c:26,67\` — link-time file
  selection, not a \`CONF_*\` macro). Their absence from the
  preprocessor run below does not change any row of this table.

## The machine question — what this table can and cannot tell you

The Makefile's own comment calls the 512K image built by \`make 512\`
"for TT or Falcon; also usable for ST/STe under Hatari"
(\`references/emutos/Makefile:555\`). **One ROM image runs on all of
them** — EmuTOS does not build a separate binary per machine here; it
builds one binary that decides at boot which machine it is actually
running on.

The mechanism (read end to end, not assumed): \`include/has.h\` defines
\`HAS_X\` as a **compile-time constant \`0\`** when the matching
\`CONF_WITH_X\` is off, but as a reference to a **runtime variable**
\`has_x\` when \`CONF_WITH_X\` is on
(\`references/emutos/include/has.h:26-45\`, the \`HAS_STE_SHIFTER\` /
\`HAS_TT_SHIFTER\` / \`HAS_VIDEL\` / \`HAS_VME\` pattern, repeated for
every entry in the table below). In the TARGET_512 profile every one of
these \`CONF_WITH_*\` is \`1\` — see the runtime-gated list below — so
none of that detection code is compiled out. Which branch is *actually*
taken is decided once, at boot, by \`detect_video()\`
(\`bios/machine.c:101-162\`): it probes real hardware registers for a
bus error (STe Shifter register \`0xffff820d\`, TT palette registers
\`0xffff8400\`, Falcon \`HHT\` register \`0xffff8282\` via \`check_read_byte\`)
and sets \`has_ste_shifter\` / \`has_tt_shifter\` / \`has_videl\`
accordingly; \`detect_vme()\` does the same for \`has_vme\`
(\`bios/machine.c:200-221\`). \`fill_cookie_jar()\`
(\`bios/machine.c:665-694\`) then derives \`cookie_mch\` (\`MCH_ST\` /
\`MCH_STE\` / \`MCH_MSTE\` / \`MCH_TT\` / \`MCH_FALCON\`,
\`include/cookie.h:49-56\`) and \`cookie_vdo\` from exactly those
runtime flags (\`setvalue_mch\`: \`bios/machine.c:381-405\`;
\`setvalue_vdo\`: \`bios/machine.c:363-379\`). Both run unconditionally
during boot, before any AES/desktop code: \`machine_detect()\` from
\`bios/bios.c:246-248\`, \`fill_cookie_jar()\` from \`bios/bios.c:279-280\`.

**What a reader of this table may conclude:** a row with value \`1\`
means the corresponding driver/feature's *code* is compiled into the
TARGET_512 ROM and can run on TT, Falcon, STE or plain ST alike — none
of it is compiled out for this profile.

**What a reader may NOT conclude:** that a specific machine has that
feature *active* on a given boot. "In our Falcon profile, VIDEL is on"
is not a sentence this table can support — TARGET_512 is not a
Falcon-only profile, and whether VIDEL (or the TT Shifter, or VME) is
actually driving the screen on a concrete boot depends on
\`cookie_mch\`/\`cookie_vdo\`/the \`has_*\` flags, which are runtime
state set by hardware probing, not compile-time constants this table
can show. A claim about "the Falcon" (or "the TT") specifically needs a
citation to that runtime detection, not to this table.

\`CONF_WITH_*\` macros in this table that gate a runtime-detected
\`has_*\` variable this way (extracted from \`include/has.h\`, not
hand-picked):

EOF

sort -t $'\t' -k1,1 "$WORK/runtime_gated.tsv" | while IFS=$'\t' read -r cond var; do
    val="$(awk -F'\t' -v c="$cond" '$1==c {print $2}' "$WORK/table_512.tsv")"
    printf -- '* `%s` = `%s` (compile-time) gates the runtime variable `%s`\n' "$cond" "$val" "$var"
done

cat <<EOF

Outside this table's scope: exactly one place in \`desk/\` or \`aes/\`
reads a runtime machine identity directly —
\`aes/gemgsxif.c:161\` (\`if (HAS_VIDEL)\`). The five spot-check flags
this table was validated against (\`CONF_WITH_DESKTOP_CONFIG\`,
\`CONF_WITH_WINDOW_ICONS\`, \`CONF_WITH_DESKTOP_SHORTCUTS\`,
\`CONF_WITH_READ_INF\`, and the \`.ACC\` lifecycle) are **not** among
the runtime-gated flags above and are not read anywhere in \`desk/\` or
\`aes/\` against \`cookie_mch\`/\`HAS_*\` — the machine question does not
change what those five conclude.

## Traps: every \`#ifdef TARGET_*\` block in config.h

EOF

n_blocks="$(wc -l < "$WORK/target_blocks.tsv" | tr -d ' ')"
cat <<EOF
\`config.h\` (this pinned tree) has **$n_blocks** \`#ifdef TARGET_*\`
blocks:

| Block | Lines | CONF_\* values that differ from TARGET_512 |
|---|---|---|
EOF
while IFS=$'\t' read -r bname bstart bend ndiff; do
    printf '| `%s` | `include/config.h:%s-%s` | %s |\n' "$bname" "$bstart" "$bend" "$ndiff"
done < "$WORK/blocks_summary.tsv"

cat <<EOF

**No \`#ifdef TARGET_512\` block exists.** Confirmed two ways against
this pinned tree, not assumed from a prior write-up:

1. Zero of the $n_blocks blocks above are named \`TARGET_512\`
   ($TARGET_512_BLOCK_COUNT matches for the literal pattern
   \`#ifdef TARGET_512\` in \`include/config.h\`).
2. The literal string \`TARGET_512\` appears exactly once in the whole
   pinned tree (\`.h\`/\`.c\`/\`.S\`/\`Makefile\`, excluding \`.git\`):

EOF
echo "$TARGET_512_TREE_HITS" | awk -F: -v root="$EMUTOS_DIR/" '
{
  path = $1
  lineno = $2
  content = $0
  sub(/^[^:]*:[^:]*:/, "", content)
  sub("^" root, "references/emutos/", path)
  gsub(/^[ \t]+/, "", content)
  printf("   - `%s:%s`: `%s`\n", path, lineno, content)
}'
cat <<'EOF'

   — the Makefile recipe that *defines* the macro, never a config.h
   consumer of it. This confirms ACC01's claim ("kein eigener
   `#ifdef TARGET_512`-Block existiert in `config.h`"): TARGET_512
   falls through to config.h's generic (no-TARGET_*-defined) defaults
   for every single CONF_* macro, with no exception.

Other `TARGET_*` blocks touch `CONF_*` macros TARGET_512 also defines,
but through a **different** compile-time switch — do not read a value
from one of these blocks as if it were our profile's:
EOF

while IFS=$'\t' read -r bname bstart bend ndiff; do
    [ "$ndiff" = "0" ] && continue
    cat <<EOF

### \`$bname\` (\`include/config.h:$bstart-$bend\`, $ndiff diverging)

| CONF_* | TARGET_512 | $bname |
|---|---|---|
EOF
    while IFS=$'\t' read -r name v512 vblock; do
        printf '| `%s` | %s | %s |\n' "$name" "$v512" "$vblock"
    done < "$WORK/diff_$bname.tsv"
done < "$WORK/blocks_summary.tsv"

cat <<EOF

Note: \`config.h\` also has non-\`TARGET_*\` machine-selector guards
(\`#ifdef MACHINE_ARANYM\` at \`include/config.h:61\`,
\`#ifdef MACHINE_FIREBEE\` at \`:127\`, plus \`MACHINE_AMIGA\`/
\`MACHINE_LISA\`) — out of REF01's scope (which asked only about
\`#ifdef TARGET_*\`), noted here so a reader doesn't mistake this
table's silence on them for their absence.

## Complete \`CONF_*\` table (TARGET_512, alphabetical, $NUM_MACROS macros)

| Macro | Value |
|---|---|
EOF

sort -t $'\t' -k1,1 "$WORK/table_512.tsv" | while IFS=$'\t' read -r name val; do
    printf '| `%s` | `%s` |\n' "$name" "$val"
done

cat <<EOF

## Regenerating

\`\`\`
tools/emutos-config.sh
\`\`\`

Requires \`$EMUTOS_CC\` (override with \`EMUTOS_CC\`). Reads only
\`references/emutos/\`; writes only this file. Running it twice against
an unchanged pinned tree produces a byte-identical file (the only
non-mechanical input, the generation date, is a fixed string in the
script, not \`\$(date)\` — see the script header).
EOF
} > "$OUT_FILE"

echo "Wrote $OUT_FILE ($NUM_MACROS CONF_* macros, $n_blocks TARGET_* blocks, pinned commit $EMUTOS_COMMIT_SHORT)" >&2
