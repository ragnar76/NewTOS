#!/bin/sh
#
# newtos-cc-host.sh -- host test of the compiler wrapper sdk/bin/newtos-cc,
# the Makefile include sdk/mk/newtos.mk and the two templates (TASK/SDK01 D).
# Run by sdk/run-host-tests.sh; exit 0 = pass, 77 = skip, else fail.
#
#   1. both templates build with `make` (copies in a temporary directory, so
#      the source tree stays untouched): HELLO.NAP and HELLO.NCP are ELF64
#      AArch64 ET_DYN files carrying the PRGFLAGS note with PF_STANDARD;
#   2. PROT=global in the Makefile puts PRGFLAGS 0x17 into the note
#      (FreeMiNT F_PROT_G in bits 4-7), and the object is rebuilt because the
#      Makefile changed;
#   3. `make card DEST=...` copies the program;
#   4. `newtos-cc -bare` builds sdk/examples/hello.c byte-identical to
#      sdk/build.sh;
#   5. an unknown library is an error, not a silent link.
#
# Skips when libc.a / gemlib.a are not built or no clang / make is found.

SDK=$(cd "$(dirname "$0")/../.." && pwd)
CC_W="$SDK/bin/newtos-cc"
MAKE=${MAKE:-make}
fail=0
ok()  { echo "  ok    $*"; }
bad() { echo "  FAIL  $*"; fail=$((fail + 1)); }

command -v clang >/dev/null 2>&1 || { echo "skip: clang not found"; exit 77; }
command -v "$MAKE" >/dev/null 2>&1 || { echo "skip: make not found"; exit 77; }
[ -f "$SDK/libc/libc.a" ]     || { echo "skip: sdk/libc/libc.a not built"; exit 77; }
[ -f "$SDK/gemlib/gemlib.a" ] || { echo "skip: sdk/gemlib/gemlib.a not built"; exit 77; }

WORK=$(mktemp -d "${TMPDIR:-/tmp}/newtos-cc-host.XXXXXX") || exit 1
trap 'rm -rf "$WORK"' EXIT

# hex dump of a file as one line of "xx " pairs
hexline() { od -An -tx1 -v "$1" | tr -s ' \n' '  '; }
# check_elf <file> <expected PRGFLAGS note value as 4 hex bytes, LE>
check_elf() {
  h=$(hexline "$1")
  case $h in
    " 7f 45 4c 46 02 01 01"*) ;;
    *) bad "$1: not an ELF64 little-endian file"; return ;;
  esac
  # e_type (offset 16) = 3 ET_DYN, e_machine (18) = 0xb7 EM_AARCH64
  et=$(od -An -tx1 -j16 -N4 "$1" | tr -d ' \n')
  [ "$et" = 0300b700 ] || { bad "$1: e_type/e_machine $et, expected 0300b700"; return; }
  # note: namesz 7, descsz 4, type 1, "NewTOS\0" + pad, value
  case $h in
    *" 07 00 00 00 04 00 00 00 01 00 00 00 4e 65 77 54 4f 53 00 00 $2 "*)
      ok "$(basename "$1"): ELF64 AArch64 ET_DYN, PRGFLAGS note $2" ;;
    *) bad "$1: PRGFLAGS note with value $2 not found" ;;
  esac
}

# --- 1. templates --------------------------------------------------------------
for t in gem-hello console-hello; do
  cp -R "$SDK/templates/$t" "$WORK/$t"
  rm -rf "$WORK/$t/build" "$WORK/$t"/HELLO.*
  if ( cd "$WORK/$t" && "$MAKE" NEWTOS_SDK="$SDK" ) > "$WORK/$t.log" 2>&1; then
    ok "$t: make"
  else
    bad "$t: make failed"; sed 's/^/        /' "$WORK/$t.log"
  fi
done
[ -f "$WORK/gem-hello/HELLO.NAP" ]     && check_elf "$WORK/gem-hello/HELLO.NAP" "07 00 00 00"
[ -f "$WORK/console-hello/HELLO.NCP" ] && check_elf "$WORK/console-hello/HELLO.NCP" "07 00 00 00"

# --- 2. PROT in the Makefile ---------------------------------------------------
c="$WORK/console-hello"
if [ -f "$c/HELLO.NCP" ]; then
  awk '/^include /{print "PROT = global"} {print}' "$c/Makefile" > "$c/Makefile.new" &&
    mv "$c/Makefile.new" "$c/Makefile"
  touch -t 203001010000 "$c/Makefile"      # newer than the outputs, whatever the clock grain
  if ( cd "$c" && "$MAKE" NEWTOS_SDK="$SDK" ) > "$WORK/prot.log" 2>&1; then
    grep -q -- "-c hello.c" "$WORK/prot.log" && ok "Makefile change recompiles" \
      || bad "Makefile change did not recompile"
    check_elf "$c/HELLO.NCP" "17 00 00 00"
  else
    bad "PROT=global build failed"; sed 's/^/        /' "$WORK/prot.log"
  fi
fi

# --- 3. make card ----------------------------------------------------------------
if [ -f "$WORK/gem-hello/HELLO.NAP" ]; then
  ( cd "$WORK/gem-hello" && "$MAKE" NEWTOS_SDK="$SDK" card DEST="$WORK/card dir" ) > "$WORK/card.log" 2>&1
  if cmp -s "$WORK/gem-hello/HELLO.NAP" "$WORK/card dir/HELLO.NAP"; then
    ok "make card DEST=<dir with a blank>"
  else
    bad "make card did not copy HELLO.NAP"; sed 's/^/        /' "$WORK/card.log"
  fi
fi

# --- 4. -bare = sdk/build.sh -------------------------------------------------------
if "$SDK/build.sh" "$SDK/examples/hello.c" "$WORK/H-script.NCP" > "$WORK/build-sh.log" 2>&1 &&
   "$CC_W" -bare "$SDK/examples/hello.c" -o "$WORK/H-cc.NCP" > "$WORK/bare.log" 2>&1; then
  cmp -s "$WORK/H-script.NCP" "$WORK/H-cc.NCP" && ok "-bare hello.c byte-identical to sdk/build.sh" \
    || bad "-bare hello.c differs from sdk/build.sh"
else
  bad "sdk/build.sh or newtos-cc -bare failed"; sed 's/^/        /' "$WORK/build-sh.log" "$WORK/bare.log"
fi

# --- 5. unknown library ------------------------------------------------------------
if "$CC_W" -lnosuchlib "$SDK/examples/hello.c" -o "$WORK/X.NCP" > "$WORK/unk.log" 2>&1; then
  bad "-lnosuchlib was accepted"
else
  grep -q "unknown library" "$WORK/unk.log" && ok "-lnosuchlib rejected" \
    || { bad "-lnosuchlib failed without the expected message"; sed 's/^/        /' "$WORK/unk.log"; }
fi

[ $fail -eq 0 ] && echo "newtos-cc-host: all checks passed" || echo "newtos-cc-host: $fail check(s) failed"
exit $fail
