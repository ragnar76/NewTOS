# libshare-link.sh -- the link-time check for LDG modules (.NLD) linked with
# libshare (TASK/LDG-M3-B2). Sourced, not executed.
#
# A module must run on ONE allocator, libshare's (build-libshare.sh says why).
# The archive order makes that so -- libshare.a first in the group, so its
# malloc/free/realloc/calloc/sbrk/chdir/memalign members win the lazy
# resolution over libc.a's -- but nothing in the linker would complain if a
# reference to a name only mintlib's allocator defines (__malloc, __free,
# _mchunk_free_list, __mallocChunkSize, ...) dragged mintlib's malloc.o in
# next to libshare's: mintlib's malloc/free are weak, libshare's strong, so the
# link succeeds and two free lists run side by side. This check reads the
# linker map and fails the build in that case.
#
#   ls_check_module_map <map file> <module name>
#     fails (non-zero) when the map lists any of libc.a's allocator members --
#     mintlib/malloc.c, calloc.c, realloc.c, posix_memalign.c, unix/sbrk.c
#     (mintlib__malloc.o, ...) -- or when libshare.a's malloc.o is absent;
#     prints the libshare members the module did pull.

LS_MINTLIB_ALLOC_MEMBERS="mintlib__malloc.o mintlib__calloc.o mintlib__realloc.o mintlib__posix_memalign.o unix__sbrk.o"

ls_check_module_map() {
  local map=$1 name=$2 m bad=0 pulled
  [ -f "$map" ] || { echo "error: link map $map missing" >&2; return 1; }
  for m in $LS_MINTLIB_ALLOC_MEMBERS; do
    if grep -q "libc\.a($m)" "$map"; then
      echo "error: $name pulls mintlib's allocator member libc.a($m) next to libshare (see sdk/libshare/libshare-link.sh)" >&2
      bad=1
    fi
  done
  if ! grep -q "libshare\.a(malloc\.o)" "$map"; then
    echo "error: $name does not contain libshare.a(malloc.o) -- is libshare.a first in the archive group?" >&2
    bad=1
  fi
  [ $bad -eq 0 ] || return 1
  pulled="$(grep -o 'libshare\.a([^)]*)' "$map" | sed 's/.*(\(.*\))/\1/' | sort -u | tr '\n' ' ')"
  echo "  allocator: libshare only ($pulled); none of libc.a's $LS_MINTLIB_ALLOC_MEMBERS"
  return 0
}

# ls_undefined_count <ELF> -- the undefined symbols of a linked program or
# module (goal 0), not counting ONE expected weak reference: the C start-up
# (sdk/libc/newtos-arch/heap_init.c __libc_crtinit, TASK/MEM05 part B) refers
# weakly to __newtos_malloc_maxhunk, which only mintlib's malloc.o defines
# (overlay mintlib/malloc.c). A module runs on libshare's allocator and must
# NOT pull malloc.o (ls_check_module_map), so in a module that reference stays
# undefined by design: the static link resolves it to 0 without a dynamic
# relocation and __libc_crtinit skips the call. Any other undefined symbol --
# and a strong reference to this one -- still counts. Needs $NM (llvm-nm).
ls_undefined_count() {
  "$NM" -u "$1" 2>/dev/null | grep -v -E '^[[:space:]]*w __newtos_malloc_maxhunk$' | grep -c . || true
}
