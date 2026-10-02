#!/usr/bin/env bash
#
# build-libc.sh -- build mintlib-0.60.1 into a NewTOS/AArch64 libc.a and link
# the libtest client into a native program (.NCP).
#
# This replicates mintlib's own build rules (see sdk/libc/mintlib/buildrules and
# sdk/libc/mintlib/configvars) but retargeted for NewTOS/AArch64:
#
#   - clang -target aarch64-unknown-none-elf, freestanding, no host libc/CRT.
#   - The mintlib per-directory include search path (buildrules `INCLUDES`):
#       -nostdinc -I<srcdir> -I<top> -I<top>/include -I<top>/mintlib
#       -I<top>/stdlib  + clang's freestanding headers (stddef/stdarg/stdint),
#     which stand in for the gcc "fixed includes" the m68k build pointed
#     `includepath` at.
#   - The mintlib DEFS: -DHAVE_CONFIG_H -D_LIBC -D_REENTRANT (buildrules), plus
#     -D_GNU_SOURCE.
#
# NewTOS-specific deltas from the pristine m68k build:
#   - -D__NO_LONG_DOUBLE_MATH: AArch64 `long double` is 128-bit IEEE quad, which
#     would make the float printf engine (stdio/printf_fp.c) emit soft-float
#     quad libcalls (__lttf2 & friends) we have no compiler-rt for. Forcing
#     long double == double routes float printf through hardware `double`
#     (%f/%e/%g fully correct; %Lf handled as double). No stub.
#   - Arch shims in sdk/libc/newtos-arch/ replace m68k-only assembly:
#       memops.c  -> memcpy/memmove/memset/bcopy/bzero (were bcopy.S/memset.S)
#       proc.S    -> _exit/_Exit/__libc_exit via the Pterm trap (was libc_exit.S)
#       setjmp.S  -> sigsetjmp/setjmp/siglongjmp/longjmp per AAPCS64 (was the
#                    m68k mintlib/setjmp.S); jmp_buf grows to long[22] in the
#                    overlay include/setjmp.h (TASK/T07)
#       heap_init.c -> points mintlib's malloc arena at the free TPA region.
#       cmdargs.c -> argc/argv from the BasePage command tail p_cmdlin (the
#                    argument half of crtinit.c parseargs(), which does not
#                    compile here); defines __libc_argc/__libc_argv.
#       getcookie.c -> Getcookie() via the Ssystem trap (was m68k getcookie.S,
#                      which scans the _p_cookies sysvar / trap #1). Pulled by
#                      cflib's cookie.c feature probes.
#   - Edits to vendored files live in sdk/libc/overlay/ (applied over the
#     freshly-extracted mintlib tree by the preamble below):
#       include/bits/byteswap.h  m68k ror/swap  -> __builtin_bswap32
#       stdio/vfprintf.c         LP64 is_longlong assignment fix (SET_IS_LONGLONG)
#       include/mint/{osbind,mintbind,falcon,metados}.h  trap-macro retarget
#       include/{features,linker}.h  generated config headers
#       include/setjmp.h         jmp_buf = long[22] on AArch64 (setjmp.S layout)
#       stdlib/gmp-mparam.h      GMP limb width from `long' (64 on LP64): the
#                                pristine 32 broke printf("%f") (TASK/PORT10)
#       include/{stdint,inttypes}.h  intptr_t/uintptr_t + limits + PRI/SCN*PTR
#                                pointer-wide on LP64 (TASK/PORT10)
#       mintlib/spawn{ve,vp}.c   spawnl/spawnle/spawnlp via va_arg instead of
#                                m68k va_list arithmetic (TASK/PORT10)
#       posix/system.c           system() without mintlib's POSIX branch
#                                (vfork/execve/waitpid): always the TOS
#                                branch via spawnvp (TASK/PORT10; removed
#                                again by TASK/PROC01)
#   - sdk/libc/crt0.S: libc-aware crt0 (sets SP from p_hitpa, calls
#     __libc_init_heap and __libc_parseargs, then main) -- distinct from
#     sdk/crt0.S used by the libc-less examples.
#
# The linker pulls only the archive members it needs, so libc.a is compiled from
# whole core directories best-effort (files that fail to assemble for AArch64 --
# m68k asm, unrelated helpers -- are skipped and logged; if a *needed* one is
# skipped it surfaces as an undefined symbol at link time).
#
# Usage:  sdk/libc/build-libc.sh [client.c] [output.NCP]
#   client.c   defaults to sdk/libc/tests/libtest.c
#   output.NCP defaults to <builddir>/libtest.NCP
set -uo pipefail

SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../sdk
LIBC="$SDK/libc"
TOP="$LIBC/mintlib"                                          # mintlib top_srcdir
CLIENT="${1:-$LIBC/tests/libtest.c}"
BUILD="$LIBC/build"
OUT="${2:-$BUILD/$(basename "${CLIENT%.c}").NCP}"
OBJ="$BUILD/obj"
mkdir -p "$OBJ"

# Shared failure bookkeeping (lib_check_failures) -- see sdk/lib-stale.sh.
. "$SDK/lib-stale.sh"

# --- ensure the mintlib source tree is present (extract tarball + overlay) ----
# The pristine mintlib-0.60.1 source is vendored as a TARBALL; our NewTOS edits
# live in sdk/libc/overlay/ (mirroring the mintlib layout). The extracted
# sdk/libc/mintlib/ tree is a build artifact (gitignored). Set MINTLIB_REFRESH=1
# to force a clean re-extract from the pristine tarball.
TARBALL="$LIBC/mintlib-0.60.1.tar.xz"
OVERLAY="$LIBC/overlay"
if [ "${MINTLIB_REFRESH:-0}" = "1" ]; then rm -rf "$TOP"; fi
if [ ! -d "$TOP" ]; then
  [ -f "$TARBALL" ] || { echo "error: pristine tarball $TARBALL missing" >&2; exit 1; }
  echo "extracting pristine mintlib from $(basename "$TARBALL")…"
  tmp="$(mktemp -d)"
  tar xf "$TARBALL" -C "$tmp"
  mv "$tmp/mintlib-0.60.1" "$TOP"
  rmdir "$tmp" 2>/dev/null || true
fi
# Always (re-)apply the NewTOS overlay so our edits are present after any extract.
if [ -d "$OVERLAY" ]; then
  ( cd "$OVERLAY" && find . -type f -print0 | while IFS= read -r -d '' f; do
      mkdir -p "$TOP/$(dirname "$f")"
      cp "$f" "$TOP/$f"
    done )
fi

# --- toolchain ---------------------------------------------------------------
CLANG="${CLANG:-clang}"
command -v "$CLANG" >/dev/null 2>&1 || { echo "error: clang not found" >&2; exit 1; }
RES="$($CLANG -print-resource-dir)/include"
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
AR="${AR:-$BIN/llvm-ar}"
LLD="${LLD:-$BIN/rust-lld}"
READOBJ="$BIN/llvm-readobj"
for t in "$AR" "$LLD"; do
    [ -x "$t" ] || { echo "error: missing LLVM tool: $t" >&2; exit 1; }
done

# --- flags (mintlib buildrules/configvars, retargeted) -----------------------
CFLAGS=(-target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2
        -fno-pic -fno-builtin -nostdinc
        -D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -D__NO_LONG_DOUBLE_MATH
        -Wno-macro-redefined)
INCS=(-I"$TOP" -I"$TOP/include" -I"$TOP/mintlib" -I"$TOP/stdlib" -isystem "$RES")

# mintlib core directories the linker draws from (buildrules `srcdirs` subset
# relevant to a stdio/stdlib/malloc client; the archive is lazily resolved).
DIRS="stdio stdlib string mintlib unix syscall time misc multibyte signal termios locale dirent gmp posix tz"

# mintlib .c files replaced by lean NewTOS glue in newtos-arch/ (see below).
# These are dropped from the archive so the linker takes OUR definitions:
#   mintlib/do_fstat.c -> newtos-arch/do_fstat.c  (lean __do_fstat: st_mode +
#     honest st_size via Fseek only; no time/owner/tty resolution tail)
#   mintlib/isatty.c   -> newtos-arch/isatty.c    (fd 0-2 == tty; also carries
#     the __open_stat[] storage the file layer references)
#   mintlib/do_stat.c  -> newtos-arch/do_stat.c   (lean __sys_stat: real Fstat64
#     trap only, no timestamp/owner tail; __quickstat's Fsfirst emulation does
#     the real path stat on -ENOSYS)
#   unix/ioctl.c -> newtos-arch/ioctl.c  (forwards to the GEMDOS Fcntl trap).
#     Until TASK/X01 this file was left in the list because it did not compile
#     anyway: it uses Super()/SuperToUser(), and SuperToUser() still carried
#     m68k inline asm. With both retargeted (sdk/libc/overlay/include/mint/
#     osbind.h) it DOES compile now -- and it must still not be used: its body
#     is the m68k tty path, poking low memory for the line discipline. It is
#     excluded explicitly so the archive carries exactly one `ioctl`, ours.
# unix/access.c -> newtos-arch/access.c  (lean access; drops the Unix passwd/
#   group DB tail getgroups->getpwuid/getgrent/setgrent/endgrent, which has no
#   TOS analog -- pulled by tz/localtime.c's access() call)
# mintlib/thread.c: MiNT Pxxx thread-fork (tfork) — builds a child BASEPAGE and
# treats its fields as native pointers. Incompatible with NewTOS's LP64 256-byte
# basepage (a 4-byte p_env is not a char*) AND with the single-tasking model; it
# was already semantically broken on LP64 (376-byte basepage the kernel can't read)
# and is unreferenced by any built app (QED/TeraDesk link with 0 undef). Excluded.
EXCLUDE="mintlib/do_fstat.c mintlib/isatty.c mintlib/do_stat.c unix/access.c mintlib/thread.c unix/ioctl.c"
is_excluded() { case " $EXCLUDE " in *" $1 "*) return 0;; esac; return 1; }

echo "clang : $($CLANG --version | head -1)"
echo "ar    : $AR"
echo "lld   : $LLD"
echo

# --- compile mintlib core into objects --------------------------------------
rm -f "$OBJ"/*.o "$OBJ"/*.err
: > "$BUILD/compiled.txt"; : > "$BUILD/failed.txt"; : > "$BUILD/excluded.txt"
: > "$BUILD/failed-err.txt"   # "<source><TAB><its .err>" -- lib_check_failures
                              # quotes the diagnostic of an UNRECORDED failure
ok=0; fail=0
for d in $DIRS; do
    for c in "$TOP/$d"/*.c; do
        [ -e "$c" ] || continue
        b=$(basename "${c%.c}")
        case "$b" in test-*|*-skeleton|ftest|xpg_basename|zdump|zic|tzselect) continue;; esac
        if is_excluded "$d/$b.c"; then echo "$d/$b.c" >> "$BUILD/excluded.txt"; continue; fi
        o="$OBJ/${d}__${b}.o"
        # Per-file DEFS, replicating mintlib buildrules. ident.c needs the
        # library VERSION string (DEFS-ident.c = -DVERSION="$(VERSION)"); without
        # it the `char __Ident_gnulib[] = "..." VERSION " $";` concat is a syntax
        # error and the object (which QED's rsc.c references) is missing.
        DEFS_FILE=()
        case "$d/$b" in mintlib/ident) DEFS_FILE=(-DVERSION='"0.60.1"');; esac
        if "$CLANG" "${CFLAGS[@]}" ${DEFS_FILE[@]+"${DEFS_FILE[@]}"} -I"$TOP/$d" "${INCS[@]}" -c "$c" -o "$o" 2>"$OBJ/${d}__${b}.err"; then
            echo "$d/$b.c" >> "$BUILD/compiled.txt"; ok=$((ok+1))
        else
            echo "$d/$b.c" >> "$BUILD/failed.txt"; fail=$((fail+1)); rm -f "$o"
            printf '%s\t%s\n' "$d/$b.c" "$OBJ/${d}__${b}.err" >> "$BUILD/failed-err.txt"
        fi
    done
done

# --- the failure list is REPORTED, and measured against the record -----------
# Until TASK/T02 this script only ever printed "compiled OK=613 skipped=11":
# WHICH files failed went to build/failed.txt and nowhere else, and its three
# callers ran it with >/dev/null 2>&1 on top. A new breakage was invisible all
# the way up to "build-qed EXIT=0". Now the list is printed, an unrecorded
# failure stops the build BEFORE the archive is written (so a silently degraded
# libc.a is never left behind for the next run to reuse), and a recorded file
# that started compiling is announced.
echo "compiled OK=$ok  skipped=$fail  (of $((ok+fail)))"
lib_check_failures "libc" "$BUILD/failed.txt" "$LIBC/expected-failures.txt" \
                   "$BUILD/failed-err.txt" || exit 1

# --- NewTOS arch shims -------------------------------------------------------
# memcpy/memmove/memset/... (were bcopy.S/memset.S), _exit (was libc_exit.S),
# the malloc-arena/heap init, and the lean stat/tty/ioctl glue replacing the
# excluded mintlib files. The stat/tty/ioctl shims need the full mintlib
# include path (struct stat, lib.h), so use $INCS for them; heap_init.c too,
# since it sets `_base` through <mint/basepage.h> (TASK/T05).
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$TOP/include" -isystem "$RES" \
    -c "$LIBC/newtos-arch/memops.c"    -o "$OBJ/arch__memops.o"   || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/heap_init.c" -o "$OBJ/arch__heapinit.o" || exit 1
# argc/argv from the BasePage command tail -- the argument half of mintlib's
# crtinit.c parseargs() (that file does not compile here). Defines
# __libc_argc/__libc_argv, which crtinit.c would otherwise own and which
# posix/wordexp.c references (TASK/PORT01).
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/cmdargs.c"   -o "$OBJ/arch__cmdargs.o"  || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$TOP/include" \
    -c "$LIBC/newtos-arch/proc.S"      -o "$OBJ/arch__proc.o"     || exit 1
# setjmp/longjmp -- the AArch64 retarget of mintlib/setjmp.S (m68k, never
# assembled here). Reads mintlib's own _sigmask (signal/initsig.c) and __mint
# (mintlib/globals.c), so both archive members resolve it; jmp_buf is the
# overlay include/setjmp.h's long[22]. Host-checked by tests/setjmp-host.sh.
"$CLANG" "${CFLAGS[@]}" -I"$TOP/include" \
    -c "$LIBC/newtos-arch/setjmp.S"    -o "$OBJ/arch__setjmp.o"   || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/do_fstat.c"  -o "$OBJ/arch__do_fstat.o" || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/isatty.c"    -o "$OBJ/arch__isatty.o"   || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/ioctl.c"     -o "$OBJ/arch__ioctl.o"    || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/do_stat.c"   -o "$OBJ/arch__do_stat.o"  || exit 1
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/getcookie.c" -o "$OBJ/arch__getcookie.o" || exit 1
# get_sysvar() -- the AArch64 replacement for the m68k mintlib/getsysvar.S
# (never assembled here). With it, mintlib's OWN time/clock.c compiles and
# links unchanged: it reads the 200 Hz tick _hz_200 (0x4ba) out of the
# process's low-memory sysvar area, which the kernel's 200 Hz timer interrupt
# keeps live (tos-api/src/bios/tick.rs). Also: lean access() without the Unix
# passwd/group tail (replaces unix/access.c), and the __extenddftf2 f64->f128
# compiler-rt builtin the AArch64 long-double ABI needs (pulled by strtold via
# sscanf/vfscanf).
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/getsysvar.c" -o "$OBJ/arch__getsysvar.o" || exit 1
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/access.c"    -o "$OBJ/arch__access.o"    || exit 1
"$CLANG" "${CFLAGS[@]}" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/softfloat.c" -o "$OBJ/arch__softfloat.o" || exit 1

# --- archive -----------------------------------------------------------------
rm -f "$LIBC/libc.a"
"$AR" cr "$LIBC/libc.a" "$OBJ"/*.o || exit 1
echo "archived -> $LIBC/libc.a ($(wc -c < "$LIBC/libc.a" | tr -d ' ') bytes)"
echo

# --- CRT + client ------------------------------------------------------------
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" -I"$TOP/include" -I"$TOP/mintlib" \
    -c "$LIBC/crt0.S" -o "$BUILD/crt0.o" || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$TOP" "${INCS[@]}" \
    -c "$CLIENT" -o "$BUILD/client.o" || exit 1

# --- link (crt0 + client + libc.a via the NewTOS TPA linker script) ----------
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -no-pie -z max-page-size=0x1000 \
    --build-id=none --why-extract="$BUILD/why.txt" \
    "$BUILD/crt0.o" "$BUILD/client.o" "$LIBC/libc.a" -o "$OUT" 2>"$BUILD/link.err"
lr=$?

if [ $lr -eq 0 ]; then
    echo "LINK OK -> $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
    "$READOBJ" --file-headers "$OUT" | grep -iE 'Type:|Machine:|Entry:'
else
    echo "LINK FAILED -- residual undefined symbols (blockers):"
    grep -oE "undefined symbol: [^ ]+" "$BUILD/link.err" | awk '{print $3}' | sort -u | sed 's/^/    /'
    echo
    echo "See $BUILD/why.txt for the archive pull chains that reference them."
    exit 1
fi
