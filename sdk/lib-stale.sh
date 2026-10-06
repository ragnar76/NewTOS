# lib-stale.sh -- dependency declaration, staleness test and failure
# bookkeeping shared by the ported libraries (libc = mintlib, gemlib, cflib,
# libm = OpenLibm) and by every script that links against them.
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
#   (a) The list was too short. sdk/libc/newtos-arch/ -- twelve files WE write
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
# forgot one. It now lives here exactly once, in lib_chain()/lib_inputs(), and
# a caller says `lib_ensure cflib`.
#
# WHAT IS WATCHED
#
# Per library, its OWN sources -- and, because each is compiled against the one
# below it, everything the ones below it watch as well:
#
#   libc.a    sdk/libc/overlay/       our edits to the vendored mintlib
#             sdk/libc/newtos-arch/   the AArch64 shims we write ourselves
#             sdk/include/            newtos.h, included by those shims
#             sdk/libc/build-libc.sh
#   gemlib.a  sdk/gemlib/overlay/, sdk/gemlib/build-gemlib.sh   + all of libc's
#   cflib.a   sdk/cflib/overlay/,  sdk/cflib/build-cflib.sh     + all of gemlib's
#   libm.a    sdk/libm/overlay/,   sdk/libm/build-libm.sh       + all of libc's
#             (OpenLibm, TASK/T07: compiled against mintlib's headers, linked
#             with libc.a; gemlib and cflib do not depend on it, so it is a
#             second branch off libc, not a link in the gemlib/cflib chain)
#   libldg.a  sdk/ldg/overlay/, sdk/ldg/build-ldg.sh,           + all of libc's
#             sdk/ldg/ldg-svn-r133.tar.xz + ldg-sources.sha256
#             (LDG's devel library, TASK/LDG-M1: compiled against mintlib's
#             headers only, a third branch off libc like libm; the tarball is
#             watched because build-ldg.sh re-extracts when it is newer than
#             the extracted tree -- the PORT03 rule below)
#   zlib, libpng, libjpeg, giflib, libexif   (TASK/LDG-M2: the third-party
#             libraries of zView's codecs) -- each: its overlay/, its
#             build-<lib>.sh, its pinned tarball + <lib>-sources.sha256, and
#             the shared sdk/vendor-lib.sh; + all of libc's. libpng is
#             compiled against zlib.h and OpenLibm's math.h (pngpriv.h), so
#             its chain is libc -> zlib -> libm -> libpng; libexif needs
#             math.h too (libc -> libm -> libexif); libjpeg and giflib are
#             plain branches off libc like ldg. Same tarball rule as ldg:
#             vendor-lib.sh re-extracts when the tarball is newer than the
#             extracted tree.
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
# A TIMESTAMP CHAIN IS ENOUGH -- OVER THE SOURCES, NOT OVER THE ARCHIVES
#
# What each library is tested against is the transitive union of the SOURCE
# inputs: cflib is tested against cflib's overlay AND gemlib's overlay AND
# mintlib's overlay/newtos-arch/include, all of them against cflib.a's own
# mtime. The tempting shorter form -- test cflib.a against gemlib.a and libc.a
# -- was built first and then measured, and it is WRONG: widening OBJECT in
# sdk/gemlib/overlay/mt_gem.h.in rebuilds gemlib.a to the byte-identical
# 745634-byte archive (gemlib's bindings marshal parameter blocks; they do not
# index OBJECT arrays), so an archive-to-archive chain never fires and cflib
# stays on the old layout -- the very defect it was supposed to close. An
# upstream archive is not a proxy for "an upstream header changed".
#
# Watching the sources also removes the only real cost of a timestamp chain.
# build-libc.sh (and build-apitest.sh, which runs it) rewrites libc.a
# unconditionally; if libc.a were the trigger, every such run would drag gemlib
# and cflib through a rebuild that produces the same bytes. Since the trigger is
# the sources and they did not move, nothing follows.
#
# It converges, and cannot loop: libc -> gemlib -> cflib is a chain, lib_ensure
# walks it in that order within one process, and after the pass each archive is
# newer than every source it watches. Measured: after the cascade fires, a
# second and a third run build nothing. The applications never take part at all
# -- build-qed.sh / build-teradesk.sh delete their objects and relink from
# scratch on every run, so a .NAP older than a library is not a state this test
# can observe.

LIB_SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"      # .../sdk

# --- the primitive -----------------------------------------------------------
# lib_stale <archive> <path...>  -- true when the archive is missing, or older
# than anything under the given paths. Sets LIB_STALE_REASON to what made it
# stale (the missing archive, or the offending path).
lib_stale() {
  local archive=$1; shift
  local p newer
  LIB_STALE_REASON=""
  if [ ! -f "$archive" ]; then LIB_STALE_REASON="$(basename "$archive") missing"; return 0; fi
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
    libm)   printf '%s\n' "$LIB_SDK/libm/libm.a" ;;
    ldg)    printf '%s\n' "$LIB_SDK/ldg/libldg.a" ;;
    zlib)    printf '%s\n' "$LIB_SDK/zlib/libz.a" ;;
    libpng)  printf '%s\n' "$LIB_SDK/libpng/libpng.a" ;;
    libjpeg) printf '%s\n' "$LIB_SDK/libjpeg/libjpeg.a" ;;
    giflib)  printf '%s\n' "$LIB_SDK/giflib/libgif.a" ;;
    libexif) printf '%s\n' "$LIB_SDK/libexif/libexif.a" ;;
    *) echo "lib-stale.sh: unknown library '$1'" >&2; return 1 ;;
  esac
}

lib_builder() {
  case "$1" in
    libc)   printf '%s\n' "$LIB_SDK/libc/build-libc.sh" ;;
    gemlib) printf '%s\n' "$LIB_SDK/gemlib/build-gemlib.sh" ;;
    cflib)  printf '%s\n' "$LIB_SDK/cflib/build-cflib.sh" ;;
    libm)   printf '%s\n' "$LIB_SDK/libm/build-libm.sh" ;;
    ldg)    printf '%s\n' "$LIB_SDK/ldg/build-ldg.sh" ;;
    zlib|libpng|libjpeg|giflib|libexif)
            printf '%s\n' "$LIB_SDK/$1/build-$1.sh" ;;
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
    libm)   printf 'libc\nlibm\n' ;;
    ldg)    printf 'libc\nldg\n' ;;
    zlib)    printf 'libc\nzlib\n' ;;
    libpng)  printf 'libc\nzlib\nlibm\nlibpng\n' ;;
    libjpeg) printf 'libc\nlibjpeg\n' ;;
    giflib)  printf 'libc\ngiflib\n' ;;
    libexif) printf 'libc\nlibm\nlibexif\n' ;;
    *) echo "lib-stale.sh: unknown library '$1'" >&2; return 1 ;;
  esac
}

# lib_inputs <name> -- append to LIB_WATCH everything WE edit that ends up in
# <name>'s archive. Not appended, deliberately: the extracted vendor trees (they
# are re-created from tarball+overlay on every run, so watching them would make
# every library permanently stale).
#
# The PRISTINE TARBALLS used to be excluded here with the same reasoning --
# "replacing one has no effect without MINTLIB_REFRESH=1 / GEMLIB_REFRESH=1 /
# CFLIB_REFRESH=1, so a rebuild triggered by one would be a rebuild that
# changes nothing". PORT03 showed that this makes a VERSION SWAP invisible: the
# extracted tree is not tracked, so on every existing working copy the new
# tarball sat there while the old sources kept being compiled, with no output
# anywhere saying so -- the 2026-09-02 stale-archive incident in a new place.
# cflib's tarball is therefore watched, and build-cflib.sh re-extracts when the
# tarball is newer than the extracted tree, which is what turns that rebuild
# into one that does change something. mintlib's and gemlib's build scripts
# still only extract when the tree is ABSENT, so watching their tarballs would
# indeed rebuild nothing; they get the same pair of changes when their pin
# moves (TASK/PORT03, "Abschluss").
lib_inputs() {
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
      # Globbed, not spelled out: the vendored tarball carries its commit pin in
      # its name, and the pin must be changed in ONE place (build-cflib.sh).
      # An `if` and not `[ -e ] && …`: with no match the glob stays literal and
      # the test fails, which under a caller's `set -e` would end the build
      # here instead of in build-cflib.sh, where the message is.
      for _t in "$LIB_SDK"/cflib/cflib-21-*.tar.xz; do
        if [ -e "$_t" ]; then LIB_WATCH[${#LIB_WATCH[@]}]="$_t"; fi
      done
      ;;
    libm)
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/libm/overlay"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/libm/build-libm.sh"
      ;;
    ldg)
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/ldg/overlay"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/ldg/build-ldg.sh"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/ldg/ldg-svn-r133.tar.xz"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/ldg/ldg-sources.sha256"
      ;;
    zlib|libpng|libjpeg|giflib|libexif)
      # The pinned tarball is globbed in the library's directory (its
      # detached .asc signature skipped): the version lives in ONE place,
      # build-<lib>.sh (cflib's reasoning).
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/$1/overlay"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/$1/build-$1.sh"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/$1/$1-sources.sha256"
      LIB_WATCH[${#LIB_WATCH[@]}]="$LIB_SDK/vendor-lib.sh"
      for _t in "$LIB_SDK/$1"/*.tar.*; do
        case "$_t" in *.asc) continue ;; esac
        if [ -e "$_t" ]; then LIB_WATCH[${#LIB_WATCH[@]}]="$_t"; fi
      done
      ;;
    *) echo "lib-stale.sh: unknown library '$1'" >&2; return 1 ;;
  esac
  return 0
}

# lib_watch <name> -- the whole set <name>'s archive must be newer than: its own
# sources AND the sources of every library it is built against. cflib.a is
# compiled against gemlib's and mintlib's HEADERS, so gemlib's and mintlib's
# overlays belong here -- their archives do not (see the header).
lib_watch() {
  local up
  LIB_WATCH=()
  for up in $(lib_chain "$1"); do
    lib_inputs "$up" || return 1
  done
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
