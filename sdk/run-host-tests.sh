#!/usr/bin/env bash
#
# run-host-tests.sh -- build and run every SDK host test in one pass.
#
# These are HOST programs (macOS/arm64 or any LP64 little-endian host): they
# compile a gemlib binding or an arch shim with the host `cc`, call it, and
# check the words/bytes/registers it produced -- no kernel, no QEMU. Each test
# refuses to certify from a host whose word width or byte order differs from
# the AArch64 target. Until TASK/T06 they hung in no script; each file's
# header carries its own build line, and this runner is those lines, verbatim
# in substance, in one place.
#
# Prerequisites: sdk/gemlib/build-gemlib.sh has extracted gemlib-0.44.0 and
# applied the overlay (the tests compile the overlaid bindings); for
# aesglobal_fields the vendored TeraDesk source (sdk/apps/teradesk) must be
# present. Missing prerequisites SKIP that test with a line saying why; a
# failing test FAILS the run.
#
# Usage:  sdk/run-host-tests.sh
# Exit status: the number of failed tests (0 = all passed). Skips do not fail.
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"          # .../sdk
ROOT="$(cd "$SDK/.." && pwd)"                                  # repository root
CC="${CC:-cc}"
GT="$SDK/gemlib/gemlib-0.44.0"                                 # overlaid gemlib tree
MINT="$SDK/libc/mintlib/include"
TESTS="$SDK/gemlib/tests"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

pass=0; fail=0; skip=0
report() {                       # report <name> <rc> [<skip reason>]
  case "$2" in
    0)  echo "PASS  $1"; pass=$((pass+1)) ;;
    77) echo "SKIP  $1 -- $3"; skip=$((skip+1)) ;;
    *)  echo "FAIL  $1 (rc=$2)"; fail=$((fail+1)) ;;
  esac
}

# run_c <name> <cc flags...> -- build $TESTS/<name>.c (+ the listed gemlib
# sources) with the given flags, run it, keep its output in $WORK/<name>.log
# and show it only on failure.
run_c() {
  local name=$1; shift
  local exe="$WORK/$name" log="$WORK/$name.log"
  if ! "$CC" "$@" -o "$exe" > "$log" 2>&1; then
    echo "FAIL  $name (did not build)"; sed 's/^/      /' "$log"; fail=$((fail+1)); return
  fi
  "$exe" >> "$log" 2>&1; local rc=$?
  [ $rc -eq 0 ] || sed 's/^/      /' "$log"
  report "$name" $rc
}

command -v "$CC" >/dev/null 2>&1 || { echo "error: $CC not found" >&2; exit 1; }
if [ ! -f "$GT/mt_gem.h" ]; then
  echo "error: overlaid gemlib tree missing ($GT/mt_gem.h); run sdk/gemlib/build-gemlib.sh" >&2
  exit 1
fi
cd "$ROOT" || exit 1
echo "host: $(uname -m) $(uname -s); cc: $($CC --version | head -1)"
echo

# --- gemlib host tests (sdk/gemlib/tests/<name>.c, build line in each header) --
# obspec_bits: the BFOBSPEC bit-field overlay (mt_gem.h) matches the GEM
# ob_spec layout. Header-only; needs mintlib's include dir for <compiler.h>.
run_c obspec_bits -I "$GT" -I "$MINT" "$TESTS/obspec_bits.c"

# evnt_timer_words (H03): mt_evnt_timer hands int_in[0]=low, int_in[1]=high.
run_c evnt_timer_words -D__NEWTOS__ -I "$GT" -I "$MINT" \
  "$TESTS/evnt_timer_words.c" "$GT/a_evnt_timer.c"

# wind_set_addrin (A81): wind_set_str/_ptr/_ptr_int pass the pointer in
# addr_in[0] with AIN_LEN 1.
run_c wind_set_addrin -D__NEWTOS__ -I "$GT" -I "$MINT" \
  "$TESTS/wind_set_addrin.c" \
  "$GT/a_wind_set_str.c" "$GT/a_wind_set_ptr.c" "$GT/a_wind_set_ptr_int.c"

# intin_long_words (T04): *_intin_long/*_intout_long are 32-bit [lo, hi] in
# place; wind_get_ptr(WF_SCREEN) reads the LONG. Under AddressSanitizer so a
# binding writing 8 bytes into its WORD array is caught, not just miscounted.
run_c intin_long_words -D__NEWTOS__ -fsanitize=address -O1 -I "$GT" -I "$MINT" \
  "$TESTS/intin_long_words.c" \
  "$GT/n_vst_arbpt32.c" "$GT/n_v_create_ctab.c" "$GT/n_vst_track_offset.c" \
  "$GT/n_v_value2color.c" "$GT/a_wind_get_ptr.c" "$GT/vdi_dummy.c"

# ptr_word_count (T04): the six pointer-in-intin bindings declare N_PTRINTS
# WORDs for a pointer.
run_c ptr_word_count -D__NEWTOS__ -I "$GT" -I "$MINT" \
  "$TESTS/ptr_word_count.c" \
  "$GT/n_v_delete_ctab.c" "$GT/n_v_delete_itab.c" "$GT/n_vqt_fontheader.c" \
  "$GT/u_vqt_fontheader.c" "$GT/a_wind_get.c" "$GT/a_wind_sget.c" \
  "$GT/vdi_array2str.c" "$GT/vdi_userdef.c" "$GT/vdi_dummy.c"

# aesglobal_fields (A60): TeraDesk's library.h + the overlay stack address
# ap_ptree/ap_rscmem as 4-byte fields at byte 10/14. Needs the vendored
# TeraDesk source.
if [ -f "$SDK/apps/teradesk/library/xdialog/library.h" ]; then
  run_c aesglobal_fields -D__NEWTOS__ -D_TOS_ONLY -funsigned-char -Wno-macro-redefined \
    -I "$SDK/overlays/teradesk" -I "$SDK/overlays/teradesk/library/xdialog" \
    -I "$SDK/apps/teradesk" -I "$SDK/apps/teradesk/library/xdialog" \
    -I "$SDK/apps/teradesk/library/utility" \
    -I "$GT" -I "$MINT" "$TESTS/aesglobal_fields.c"
else
  report aesglobal_fields 77 "vendored TeraDesk source absent (sdk/apps/teradesk)"
fi

# --- libc host tests -----------------------------------------------------------
# setjmp-host (T07): newtos-arch/setjmp.S round-trips on an AArch64 host; the
# script itself skips (77) on a non-AArch64 host.
"$SDK/libc/tests/setjmp-host.sh" > "$WORK/setjmp-host.log" 2>&1; rc=$?
[ $rc -eq 0 ] || [ $rc -eq 77 ] || sed 's/^/      /' "$WORK/setjmp-host.log"
report setjmp-host $rc "host is not AArch64"

# cmdargs-host (PORT01): newtos-arch/cmdargs.c turns the BasePage command tail
# into the argc/argv mintlib's crtinit.c parseargs() would -- the two DESK19
# tails, the separator rules, MagX quoting, and the densest 128-byte tail;
# since PORT10 also the published empty `environ` and _spawnve's 0x7f tail
# with a p_env that must not be read.
"$SDK/libc/tests/cmdargs-host.sh" > "$WORK/cmdargs-host.log" 2>&1; rc=$?
[ $rc -eq 0 ] || [ $rc -eq 77 ] || sed 's/^/      /' "$WORK/cmdargs-host.log"
report cmdargs-host $rc "mintlib headers absent or host not LP64"

# gmp-host (PORT10): mintlib's GMP limb width matches mp_limb_t on LP64 --
# the 10^(2^i) table __printf_fp reads, __mpn_extract_double, mpn_mul_1
# (the overlaid stdlib/gmp-mparam.h; with the pristine one 17 of 21 fail).
"$SDK/libc/tests/gmp-host.sh" > "$WORK/gmp-host.log" 2>&1; rc=$?
[ $rc -eq 0 ] || [ $rc -eq 77 ] || sed 's/^/      /' "$WORK/gmp-host.log"
report gmp-host $rc "mintlib tree absent or host not LP64"

# spawn-host (PORT10): the LP64 spawn* overlays hand _spawnve the argv/envp
# the m68k va_list cast did (spawnl/spawnle/spawnlp/spawnv/spawnvp).
"$SDK/libc/tests/spawn-host.sh" > "$WORK/spawn-host.log" 2>&1; rc=$?
[ $rc -eq 0 ] || [ $rc -eq 77 ] || sed 's/^/      /' "$WORK/spawn-host.log"
report spawn-host $rc "mintlib tree absent or host not LP64"

# system-host (PORT10): the posix/system.c overlay is mintlib's TOS branch
# only -- word splitting, < > >> via Fopen/Fcreate + Fdup/Fforce around
# spawnvp(P_WAIT), DOOM's zenity probe -> -1 -- and its object references no
# vfork/execve/waitpid/sigaction/access/getenv any more.
"$SDK/libc/tests/system-host.sh" > "$WORK/system-host.log" 2>&1; rc=$?
[ $rc -eq 0 ] || [ $rc -eq 77 ] || sed 's/^/      /' "$WORK/system-host.log"
report system-host $rc "mintlib tree absent or host not LP64"

# quickstat-host (PORT10, user decision 2026-10-02): the overlay
# mintlib/quickstat.c sets the exec bits for .NTP/.NAP/.NCP in any case and no
# longer for mintlib's m68k .app/.gtp/.ttp/.prg/.tos (nor .NAC); access(X_OK)
# through newtos-arch/access.c agrees. With the pristine file 15 of 27 fail.
"$SDK/libc/tests/quickstat-host.sh" > "$WORK/quickstat-host.log" 2>&1; rc=$?
[ $rc -eq 0 ] || [ $rc -eq 77 ] || sed 's/^/      /' "$WORK/quickstat-host.log"
report quickstat-host $rc "mintlib tree absent or host not LP64"

# --- app host tests --------------------------------------------------------------
# c2p-host (PORT06, user decision 2026-10-04): DOOM's table chunky-to-planar
# (sdk/apps/doom/doomgeneric_gem.c, compiled without its GEM half) equals the
# bit-by-bit routine it replaced, kept in the test as the oracle -- 1..32
# planes, standard and interleaved form, odd widths, under ASan/UBSan.
# Byte-order-free by construction (16-bit stores), so no host check needed.
DOOMT="$SDK/apps/doom/tests"
if "$CC" -O1 -g -fsanitize=address,undefined -Wall -Wextra -Werror \
     -o "$WORK/c2p-host" "$DOOMT/c2p-host.c" > "$WORK/c2p-host.log" 2>&1; then
  "$WORK/c2p-host" >> "$WORK/c2p-host.log" 2>&1; rc=$?
  [ $rc -eq 0 ] || sed 's/^/      /' "$WORK/c2p-host.log"
  report c2p-host $rc
else
  echo "FAIL  c2p-host (did not build)"; sed 's/^/      /' "$WORK/c2p-host.log"; fail=$((fail+1))
fi

echo
echo "host tests: $pass passed, $fail failed, $skip skipped"
exit $fail
