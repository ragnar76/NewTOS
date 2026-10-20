#!/usr/bin/env bash
# ar-members.sh <old.a> <new.a> <ar>
# Explains a differing archive: extracts both with <ar> into scratch dirs
# (TMPDIR) and compares member names, order and member bytes. Prints the
# member header dates (ar tv) of both so a timestamp-only difference shows.
set -u
OLD=$1; NEW=$2; AR=$3
t="$(mktemp -d)"; mkdir "$t/o" "$t/n"
( cd "$t/o" && "$AR" x "$OLD" ) && ( cd "$t/n" && "$AR" x "$NEW" ) || { echo "extract failed"; exit 2; }
lo="$("$AR" t "$OLD" | sed '/^__\.SYMDEF/d')"; ln="$("$AR" t "$NEW" | sed '/^__\.SYMDEF/d')"
[ "$lo" = "$ln" ] && echo "member list+order: identical ($(printf '%s\n' "$lo" | wc -l | tr -d ' ') members)" || echo "member list+order: DIFFERENT"
d=0; for m in $lo; do cmp -s "$t/o/$m" "$t/n/$m" || { echo "member differs: $m"; d=$((d+1)); }; done
echo "members with different bytes: $d"
echo "first member header, old: $("$AR" tv "$OLD" | sed -n '1p;')"
echo "first member header, new: $("$AR" tv "$NEW" | sed -n '1p;')"
rm -rf "$t"
