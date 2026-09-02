# lib-stale.sh -- shared staleness test for the ported libraries.
#
# The build scripts used to rebuild a library only when its archive was
# ABSENT. A stale archive was reused in silence, so an application could
# be linked against sources it no longer matched. That cost an hour on
# 2026-09-02: the gemlib overlay header was fixed, cflib.a stayed at its
# 19 July build, and QED went on carrying the defect while TeraDesk --
# rebuilt that day -- did not. An application whose behaviour does not
# follow its sources is the worst thing to debug against.
#
# `lib_stale <archive> <path...>` is true when the archive is missing, or
# older than anything under the given paths.
#
# The vendored trees are extracted from pinned tarballs by the build
# scripts and do not change, so what is watched is what we actually edit:
# the overlay directory and the build script itself.
lib_stale() {
  archive=$1; shift
  [ -f "$archive" ] || return 0
  for p in "$@"; do
    [ -e "$p" ] || continue
    if [ -n "$(find "$p" -newer "$archive" -print -quit 2>/dev/null)" ]; then
      return 0
    fi
  done
  return 1
}
