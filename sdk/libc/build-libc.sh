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
#       heap_init.c -> __libc_crtinit: mintlib crtinit.c _crtinit/_acc_main
#                    for TOS (_stksize policy, Mshrink of the TPA, malloc via
#                    GEMDOS Malloc); called by crt0.S (ARCH03 E9). First
#                    sets malloc's MAXHUNK to 16 MiB (MEM05 part B).
#       cmdargs.c -> crtinit.c parseargs(), which does not compile here:
#                    environ from p_env, argc/argv from its ARGV= part or the
#                    BasePage command tail p_cmdlin, program_invocation_name;
#                    defines __libc_argc/__libc_argv (PORT01, PORT09).
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
#       mintlib/spawn{vp,}.c, posix/execvp.c  program-name search list =
#                                NewTOS's native { ntp, nap, ncp } instead of
#                                { ttp, prg, tos } (user decision 2026-10-02)
#       mintlib/quickstat.c      exec bits (0111) for the native .ntp/.nap/
#                                .ncp, case-insensitive, instead of mintlib's
#                                lower-case .app/.gtp/.ttp/.prg/.tos (user
#                                decision 2026-10-02)
#       mintlib/thread.c         LP64 casts for the 32-bit basepage fields
#                                (tfork, __waitval, __waittime; TASK/PROC01)
#       mintlib/malloc.c         + __newtos_malloc_maxhunk(siz), which sets
#                                only MAXHUNK (the ceiling of the hunk
#                                doubling); heap_init.c calls it with 16 MiB
#                                (TASK/MEM05 part B, design E6)
#       include/sys/wait.h       struct __wait bit-fields in little-endian
#                                order (same int layout: exit code bits
#                                8..15, termsig 0..6), so WEXITSTATUS & co.
#                                read wait4's status word (TASK/PORT-WAITH)
#   - TASK/PROC01: mintlib's vfork (unix/vfork.S) and _setstack
#     (mintlib/setstack.S) are m68k; newtos-arch/vfork.S and setstack.S are
#     their AArch64 retargets. With them and thread.c, posix/system.c is
#     mintlib's own again (the PORT10 overlay without the POSIX branch is gone).
#   - sdk/libc/crt0.S: libc-aware crt0 (calls __libc_crtinit, then mintlib's
#     own _main = __libc_main, which ends in exit(main(...)), as crtinit.c's
#     _crtinit / _acc_main do) -- distinct from sdk/crt0.S used by the
#     libc-less examples.
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
        -fPIE -fno-builtin -nostdinc
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
# mintlib/thread.c (tfork, __waitval, __waittime) was excluded until TASK/PROC01:
# it treated the basepage fields as native pointers, and NewTOS had no
# Pexec 5/6/7. It builds now from the LP64 overlay (overlay/mintlib/thread.c)
# against the kernel's Pexec 5 + 4 (ARCH03 stage 2).
EXCLUDE="mintlib/do_fstat.c mintlib/do_stat.c unix/access.c unix/ioctl.c"
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
        # syscall/main.c is the `main` of mintlib's HOST generator (syscall/
        # Makefile gen_objs: check generate main pars scan traps -> the tool
        # that writes the trap bindings), never a library member: mintlib's
        # library takes only the generated $(TRAPS) from that directory
        # (syscall/SRCFILES). In libc.a it was a trap -- it defines `main`
        # and a second `_stksize` (64 KiB, main.c:41), so a program without
        # its own main, or one that looks up `_stksize`, could pull it in.
        case "$d/$b" in syscall/main) echo "$d/$b.c" >> "$BUILD/excluded.txt"; continue;; esac
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
# vfork (TOS branch: tfork = Pexec 5 + Pexec 4 on the caller's code) and
# _setstack -- AArch64 retargets of mintlib's m68k unix/vfork.S and
# mintlib/setstack.S (TASK/PROC01). vfork reads __mint and writes errno.
"$CLANG" "${CFLAGS[@]}" -I"$TOP/include" \
    -c "$LIBC/newtos-arch/vfork.S"     -o "$OBJ/arch__vfork.o"    || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$TOP/include" \
    -c "$LIBC/newtos-arch/setstack.S"  -o "$OBJ/arch__setstack.o" || exit 1
"$CLANG" "${CFLAGS[@]}" -I"$SDK/include" "${INCS[@]}" \
    -c "$LIBC/newtos-arch/do_fstat.c"  -o "$OBJ/arch__do_fstat.o" || exit 1
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
"$LLD" -flavor gnu -T "$SDK/newtos.ld" -pie --no-dynamic-linker -z text -z max-page-size=0x1000 \
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
