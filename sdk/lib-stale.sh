# lib-stale.sh -- dependency declaration, staleness test and failure
# bookkeeping shared by the ported libraries (libc = mintlib, gemlib, cflib)
# and by every script that links against them.
#
# WHY THIS FILE EXISTS
#
# The build scripts used to rebuild a library only when its archive was
# ABSENT. A stale archive was reused in silence, so an application could
# be linked against sources it no longer matched. That cost an hour on
# 2026-09-02: the gemlib overlay header was fixed, cflib.a stayed at its
# 19 July build, and QED went on carrying the defect while TeraDesk --
# rebuilt that day -- did not. An application whose behaviour does not
# follow its sources is the worst thing to debug against.
#
# THE 2026-09-02 FIX WAS INCOMPLETE  (TASK/T03, both holes reproduced)
#
# The first version of this file watched "the overlay directory and the build
# script itself", and every caller repeated that list by hand. Both halves of
# that sentence were wrong:
#
#   (a) The list was too short. sdk/libc/newtos-arch/ -- eleven files WE write
#       -- and sdk/include/newtos.h are compiled into libc.a and were watched
#       by nobody: appending a function to newtos-arch/do_stat.c left libc.a
#       byte-identical and build-qed.sh reported nothing out of date.
#   (b) The libraries were never chained to each other. gemlib.a is compiled
#       against mintlib's headers, cflib.a against gemlib's AND mintlib's.
#       Widening OBJECT in sdk/gemlib/overlay/mt_gem.h.in rebuilt gemlib.a and
#       QED's 37 objects on the NEW layout while cflib.a stayed byte-identical
#       on the OLD one -- one program, two struct layouts. That is the
#       2026-09-02 incident again, moved up one level.
#
# Both holes have the same root: the dependency knowledge was duplicated across
# six call sites, so extending it meant editing six scripts and everybody
# forgot one. It now lives here exactly once, in lib_chain()/lib_watch(), and a
# caller says `lib_ensure cflib`.
#
# WHAT IS WATCHED
#
#   libc.a    sdk/libc/overlay/       our edits to the vendored mintlib
#             sdk/libc/newtos-arch/   the AArch64 shims we write ourselves
#             sdk/include/            newtos.h, included by those shims
#             sdk/libc/build-libc.sh
#   gemlib.a  sdk/gemlib/overlay/, sdk/gemlib/build-gemlib.sh   + libc.a
#   cflib.a   sdk/cflib/overlay/,  sdk/cflib/build-cflib.sh     + gemlib.a, libc.a
#
# NOT watched, deliberately:
#   - The extracted vendor trees (sdk/libc/mintlib/, sdk/gemlib/gemlib-0.44.0/,
#     sdk/cflib/cflib-21/). They are re-created from tarball+overlay on every
#     run, so watching them would make every library permanently stale.
#   - The pristine tarballs. Replacing one has NO effect without
#     MINTLIB_REFRESH=1 / GEMLIB_REFRESH=1 / CFLIB_REFRESH=1, because the build
#     scripts only extract when the tree is absent; a rebuild triggered by the
#     tarball's timestamp would therefore be a rebuild that changes nothing.
#
# WHY A TIMESTAMP CHAIN IS ENOUGH, AND DOES NOT LOOP
#
# libc -> gemlib -> cflib is a chain, not a cycle, and lib_ensure walks it in
# that order within one process, so after any rebuild the archives are left in
# ascending mtime order and the next run finds nothing stale. Nothing
# downstream feeds back upstream. The applications never take part in the test
# at all: build-qed.sh / build-teradesk.sh delete their objects and relink from
# scratch on every run, so a .NAP older than a library is not a state this test
# can observe -- and an explicit per-library dependency stamp would buy nothing
# a mtime comparison over this DAG does not already give.

LIB_SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"      # .../sdk

# --- the primitive -----------------------------------------------------------
# lib_stale <archive> <path...>  -- true when the archive is missing, or older
# than anything under the given paths. Sets LIB_STALE_REASON to what made it
# stale ("missing", or the offending path).
lib_stale() {
  local archive=$1; shift
  local p newer
  LIB_STALE_REASON=""
  if [ ! -f "$archive" ]; then LIB_STALE_REASON="missing"; return 0; fi
  for p in "$@"; do
    [ -e "$p" ] || continue
    newer="$(find "$p" -newer "$archive" -print -quit 2>/dev/null)"
    if [ -n "$newer" ]; then
      LIB_STALE_REASON="${newer#$LIB_SDK/}"
      return 0
    fi
  done
  return 1
}

# --- the declaration: one place, no more per-caller copies --------------------
lib_archive() {
  case "$1" in
    libc)   printf '%s\n' "$LIB_SDK/libc/libc.a" ;;
    gemlib) printf '%s\n' "$LIB_SDK/gemlib/gemlib.a" ;;
    cflib)  printf '%s\n' "$LIB_SDK/cflib/cflib.a" ;;
    *) echo "lib-stale.sh: unknown library '$1'" >&2; return 1 ;;
  esac
}

lib_builder() {
  case "$1" in
    libc)   printf '%s\n' "$LIB_SDK/libc/build-libc.sh" ;;
    gemlib) printf '%s\n' "$LIB_SDK/gemlib/build-gemlib.sh" ;;
    cflib)  printf '%s\n' "$LIB_SDK/cflib/build-cflib.sh" ;;
    *) echo "lib-stale.sh: unknown library '$1'" >&2; return 1 ;;
  esac
}

# lib_chain <name> -- every library that must be current for <name> to be
# built correctly, in build order, <name> itself last.
lib_chain() {
  case "$1" in
    libc)   printf 'libc\n' ;;
    gemlib) printf 'libc\ngemlib\n' ;;
    cflib)  printf 'libc\ngemlib\ncflib\n' ;;
    *) echo "lib-stale.sh: unknown library '$1'" >&2; return 1 ;;
  esac
}

# lib_watch <name> -- fill the LIB_WATCH array with everything whose change
# must invalidate <name>'s archive: the archives it is built against, then the
# sources we edit ourselves.
lib_watch() {
  local up
  LIB_WATCH=()
  for up in $(lib_chain "$1"); do
    [ "$up" = "$1" ] && continue
    LIB_WATCH[${#LIB_WATCH[@]}]="$(lib_archive "$up")"
  done
  case "$1" in
    libc)
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/libc/overlay"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/libc/newtos-arch"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/include"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/libc/build-libc.sh"
      ;;
    gemlib)
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/gemlib/overlay"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/gemlib/build-gemlib.sh"
      ;;
    cflib)
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/cflib/overlay"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/cflib/build-cflib.sh"
      ;;
    *) echo "lib-stale.sh: unknown library '$1'" >&2; return 1 ;;
  esac
  return 0
}

# --- the entry point every caller uses ---------------------------------------
# lib_ensure <name> -- bring <name> and everything it is built against up to
# date, in dependency order.
#
# It does NOT swallow the builder's report (TASK/T02): the builder's
# "compiled OK=" block -- count, skipped files, verdict against the recorded
# failure set -- is echoed, and if the builder fails its whole log is printed.
# Before this, build-qed.sh/build-teradesk.sh/build-cflib.sh ran the library
# builds with >/dev/null 2>&1, which is how two mintlib files stayed broken for
# months without a single line of output anywhere.
lib_ensure() {
  local want=$1 lib archive builder log
  for lib in $(lib_chain "$want"); do
    archive="$(lib_archive "$lib")" || return 1
    builder="$(lib_builder "$lib")" || return 1
    lib_watch "$lib" || return 1
    lib_stale "$archive" ${LIB_WATCH[@]+"${LIB_WATCH[@]}"} || continue
    echo "$(basename "$archive") out of date [$LIB_STALE_REASON] -- running $(basename "$builder")…"
    log="$(dirname "$builder")/build/$(basename "$builder" .sh).log"
    mkdir -p "$(dirname "$log")"
    if "$builder" > "$log" 2>&1; then
      sed -n '/^compiled OK=/,/^$/p' "$log" | sed '/^$/d;s/^/  /'
      echo "  (full log: ${log#$LIB_SDK/})"
    else
      echo "error: $(basename "$builder") FAILED -- its full output follows:" >&2
      sed 's/^/  /' "$log" >&2
      return 1
    fi
  done
  return 0
}

# --- failure bookkeeping for the library builds (TASK/T02) -------------------
# lib_check_failures <label> <failed.txt> <expected.txt> [<err-map.txt>]
#
# Print the list of files that did not compile -- which build-cflib.sh and
# build-gemlib-m68k.sh always did and build-libc.sh, the one script that
# actually HAS failures, never did -- and compare the set against the recorded
# one.
#
#   a file that failed and is NOT recorded  -> named with its first compiler
#                                              diagnostic, return 1 (the caller
#                                              stops BEFORE writing the archive,
#                                              so a silently degraded archive is
#                                              never left on disk)
#   a recorded file that now compiles       -> announced, so the record can be
#                                              shortened (that is what nobody
#                                              noticed when e503efd fixed
#                                              setsysvar.c and sysinfo.c)
#
# err-map.txt is optional and has "<source path><TAB><path to its .err file>"
# per line; the build scripts write it in their compile loop.
lib_check_failures() {
  local label=$1 failed=$2 expected=$3 errmap=${4:-}
  local tmpd n_failed n_expected n_new f e rc
  tmpd="$(mktemp -d)" || return 1
  rc=0

  if [ -f "$failed" ]; then sort -u "$failed" > "$tmpd/failed"; else : > "$tmpd/failed"; fi
  if [ -f "$expected" ]; then
    sed -e 's/#.*//' -e 's/[[:space:]].*$//' "$expected" | grep -v '^$' | sort -u > "$tmpd/expected"
  else
    : > "$tmpd/expected"
  fi
  n_failed=$(wc -l < "$tmpd/failed" | tr -d ' ')
  n_expected=$(wc -l < "$tmpd/expected" | tr -d ' ')

  if [ "$n_failed" -gt 0 ]; then
    echo "skipped (did not compile for AArch64/LP64):"
    sed 's/^/    /' "$tmpd/failed"
  fi

  comm -23 "$tmpd/failed" "$tmpd/expected" > "$tmpd/new"
  comm -13 "$tmpd/failed" "$tmpd/expected" > "$tmpd/gone"
  n_new=$(wc -l < "$tmpd/new" | tr -d ' ')

  if [ -s "$tmpd/gone" ]; then
    echo "recorded failures that now COMPILE -- drop them from ${expected#$LIB_SDK/}:"
    sed 's/^/    /' "$tmpd/gone"
  fi

  if [ -s "$tmpd/new" ]; then
    echo "STOP: $label failure(s) NOT recorded in ${expected#$LIB_SDK/}:"
    while IFS= read -r f; do
      echo "  === $f ==="
      e=""
      if [ -n "$errmap" ] && [ -f "$errmap" ]; then
        e=$(awk -F'\t' -v k="$f" '$1==k{print $2; exit}' "$errmap")
      fi
      if [ -n "$e" ] && [ -f "$e" ]; then sed 's/^/      /' "$e" | head -20; fi
    done < "$tmpd/new"
    echo "  Fix the toolchain. Only if the file is genuinely out of reach on"
    echo "  AArch64, record it with its reason in ${expected#$LIB_SDK/}."
    rc=1
  else
    echo "failures match ${expected#$LIB_SDK/}: $n_failed today, $n_expected recorded, $n_new new"
  fi

  rm -rf "$tmpd"
  return $rc
}
