# toolchain.sh -- find the LLVM linker and binutils (ld.lld, llvm-ar, llvm-nm,
# llvm-readobj, llvm-objdump, llvm-size, ...) for the SDK build scripts and
# sdk/bin/newtos-cc, independent of the directory they are started from.
#
# WHY THIS FILE EXISTS
#
# TASK/SDK01 package I: every build script used to ask `rustc --print sysroot`
# wherever the caller happened to stand; rustup picks the toolchain by the
# current directory, so from outside the checkout the build stopped with
# "missing LLVM tool: /lib/rustlib//bin/llvm-ar". The scripts then asked rustc
# from inside sdk/, through this one file instead of ~20 copies.
#
# TASK/SDK01 package K: building C programs must not need Rust. The tools are
# the same LLVM tools whoever ships them -- a distribution's `lld`/`llvm`
# packages, Homebrew's `llvm`/`lld`, or the pinned Rust nightly's
# llvm-tools-preview component. The pinned Rust sysroot is now only the
# fallback; kernel developers have it anyway (the kernel build itself,
# cargo xtask, is not touched by this file).
#
# RESOLUTION ORDER, per tool (newtos_tool <name>):
#
#   1. NEWTOS_<TOOL>        an explicit file: NEWTOS_LLD (ld.lld), NEWTOS_AR,
#                           NEWTOS_NM, NEWTOS_READOBJ, NEWTOS_OBJDUMP,
#                           NEWTOS_SIZE, NEWTOS_OBJCOPY, ... (llvm-<x> ->
#                           NEWTOS_<X>). Used as given, even if NEWTOS_TOOLCHAIN
#                           says otherwise.
#   2. NEWTOS_LLVM_BIN      a directory searched before everything below
#   3. PATH                 ld.lld / llvm-ar / ... by their plain names
#   4. clang's directory    the bin/ that $CLANG (default clang) really lives
#                           in, symlinks followed: Debian's clang ->
#                           /usr/lib/llvm-14/bin/clang finds that release's
#                           ld.lld even when the `lld` package's /usr/bin link
#                           is absent; a keg-only Homebrew clang given as
#                           CLANG=/opt/homebrew/opt/llvm/bin/clang likewise
#   5. Homebrew prefixes    <prefix>/opt/llvm/bin and <prefix>/opt/lld/bin for
#                           $HOMEBREW_PREFIX, /opt/homebrew, /usr/local,
#                           /home/linuxbrew/.linuxbrew (Homebrew's llvm is
#                           keg-only: not on PATH after `brew install llvm`)
#   6. /usr/lib/llvm-*/bin  Debian/Ubuntu and apt.llvm.org layout, highest
#                           version first
#   7. the pinned Rust nightly's sysroot (rustc asked from inside sdk/, so
#                           rust-toolchain.toml decides): rust-lld for ld.lld,
#                           llvm-* from llvm-tools-preview
#
# NEWTOS_TOOLCHAIN selects the sources:
#   auto (default)  steps 1-7
#   llvm            steps 1-6: never the Rust sysroot
#   rust            steps 1 and 7: the pinned Rust nightly only (the SDK's
#                   behaviour before package K)
#
# The linker is called through newtos_lld, which adds `-flavor gnu` when the
# binary is lld's generic driver (rust-lld, lld) and nothing for ld.lld,
# which is the GNU flavour by its name and REJECTS -flavor after argv[0].
#
# Sourced by bash scripts (`. "$SDK/toolchain.sh"`) and by the POSIX sh
# newtos-cc (which sets NEWTOS_TC_SDK first, sh has no BASH_SOURCE). Defines
# NEWTOS_TC_SDK (the sdk/ directory) and these functions; nothing else:
#
#   newtos_tool <name>       print the path of LLVM tool <name> (ld.lld,
#                            llvm-ar, llvm-nm, ...); on failure print nothing,
#                            explain on stderr which packages to install, and
#                            return 1
#   newtos_lld [args...]     run "$LLD" args..., with -flavor gnu if needed
#   newtos_lld_flavor <path> print "-flavor gnu" or nothing for that linker
#   newtos_rust_sysroot      print the pinned Rust toolchain's sysroot
#   newtos_rust_bindir       print <sysroot>/lib/rustlib/<host>/bin
#
# Reference: docs/sdk/02-prerequisites.md ("Linker and binutils").

if [ -n "${BASH_VERSION:-}" ]; then
  eval 'NEWTOS_TC_SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"'     # .../sdk
elif [ -z "${NEWTOS_TC_SDK:-}" ]; then
  echo "toolchain.sh: set NEWTOS_TC_SDK before sourcing from a non-bash shell" >&2
  return 1 2>/dev/null || exit 1
fi

newtos_rust_sysroot() {
  command -v rustc >/dev/null 2>&1 || return 1
  (cd "$NEWTOS_TC_SDK" && rustc --print sysroot 2>/dev/null)
}

newtos_rust_bindir() {
  _ntc_rc=0
  _ntc_sr="$(newtos_rust_sysroot)" || _ntc_rc=$?
  _ntc_host="$(cd "$NEWTOS_TC_SDK" && rustc -vV 2>/dev/null | sed -n 's/host: //p')" || _ntc_rc=$?
  printf '%s\n' "$_ntc_sr/lib/rustlib/$_ntc_host/bin"
  return $_ntc_rc
}

# _newtos_realdir <command> -- the directory the command really lives in,
# symlinks followed (no readlink -f: not in older macOS)
_newtos_realdir() {
  _ntc_f=$(command -v -- "$1" 2>/dev/null) || return 1
  case $_ntc_f in /*) ;; *) return 1 ;; esac
  while [ -h "$_ntc_f" ]; do
    _ntc_l=$(ls -ld -- "$_ntc_f" | sed 's/.* -> //')
    case $_ntc_l in /*) _ntc_f=$_ntc_l ;; *) _ntc_f=$(dirname -- "$_ntc_f")/$_ntc_l ;; esac
  done
  (cd "$(dirname -- "$_ntc_f")" && pwd)
}

# _newtos_llvm_dirs -- steps 4-6, one directory per line
_newtos_llvm_dirs() {
  _newtos_realdir "${CLANG:-clang}" || true
  for _ntc_p in ${HOMEBREW_PREFIX:-} /opt/homebrew /usr/local /home/linuxbrew/.linuxbrew; do
    printf '%s\n' "$_ntc_p/opt/llvm/bin" "$_ntc_p/opt/lld/bin"
  done
  ls -d /usr/lib/llvm-*/bin 2>/dev/null | sed 's|^/usr/lib/llvm-\([0-9]*\)/bin$|\1 &|' \
    | sort -rn | sed 's/^[0-9]* //'
}

# _newtos_envname <tool> -- ld.lld -> NEWTOS_LLD, llvm-ar -> NEWTOS_AR
_newtos_envname() {
  case $1 in
    ld.lld) printf 'NEWTOS_LLD' ;;
    llvm-*) printf 'NEWTOS_%s' "$(printf '%s' "${1#llvm-}" | tr 'a-z-' 'A-Z_')" ;;
    *)      printf 'NEWTOS_%s' "$(printf '%s' "$1" | tr 'a-z.-' 'A-Z__')" ;;
  esac
}

newtos_tool() {
  _ntc_name=$1
  _ntc_mode=${NEWTOS_TOOLCHAIN:-auto}
  case $_ntc_mode in auto|llvm|rust) ;; *)
    echo "error: NEWTOS_TOOLCHAIN=$_ntc_mode (expected auto, llvm or rust; sdk/toolchain.sh)" >&2
    return 1 ;;
  esac
  # 1. explicit file
  eval "_ntc_x=\${$(_newtos_envname "$_ntc_name"):-}"
  if [ -n "$_ntc_x" ]; then
    if [ -x "$_ntc_x" ] || command -v -- "$_ntc_x" >/dev/null 2>&1; then
      printf '%s\n' "$_ntc_x"; return 0
    fi
    echo "error: $(_newtos_envname "$_ntc_name")=$_ntc_x is not an executable (sdk/toolchain.sh)" >&2
    return 1
  fi
  if [ "$_ntc_mode" != rust ]; then
    # 2. NEWTOS_LLVM_BIN
    if [ -n "${NEWTOS_LLVM_BIN:-}" ] && [ -x "$NEWTOS_LLVM_BIN/$_ntc_name" ]; then
      printf '%s\n' "$NEWTOS_LLVM_BIN/$_ntc_name"; return 0
    fi
    # 3. PATH
    if _ntc_x=$(command -v -- "$_ntc_name" 2>/dev/null) && [ -n "$_ntc_x" ]; then
      printf '%s\n' "$_ntc_x"; return 0
    fi
    # 4.-6. clang's directory, Homebrew, /usr/lib/llvm-*
    _ntc_dirs=$(_newtos_llvm_dirs)
    for _ntc_d in $_ntc_dirs; do
      if [ -x "$_ntc_d/$_ntc_name" ]; then printf '%s\n' "$_ntc_d/$_ntc_name"; return 0; fi
    done
  fi
  if [ "$_ntc_mode" != llvm ]; then
    # 7. the pinned Rust nightly (llvm-tools-preview)
    if _ntc_d=$(newtos_rust_bindir 2>/dev/null); then
      _ntc_x=$_ntc_name
      [ "$_ntc_name" = ld.lld ] && _ntc_x=rust-lld
      if [ -x "$_ntc_d/$_ntc_x" ]; then printf '%s\n' "$_ntc_d/$_ntc_x"; return 0; fi
    fi
  fi
  {
    echo "error: LLVM tool '$_ntc_name' not found (NEWTOS_TOOLCHAIN=$_ntc_mode)."
    echo "  Install LLVM's linker and tools, then run the command again:"
    echo "    Debian/Ubuntu: sudo apt install clang lld llvm"
    echo "    Fedora:        sudo dnf install clang lld llvm"
    echo "    macOS:         brew install llvm lld"
    [ "$_ntc_mode" = llvm ] || \
    echo "  or the pinned Rust nightly with llvm-tools-preview (rustup; kernel developers have it)."
    echo "  Elsewhere: NEWTOS_LLVM_BIN=<directory>, or $(_newtos_envname "$_ntc_name")=<file>."
    echo "  (sdk/toolchain.sh, docs/sdk/02-prerequisites.md)"
  } >&2
  return 1
}

newtos_lld_flavor() {
  case ${1##*/} in
    rust-lld|rust-lld.exe|lld|lld.exe) printf '%s\n' "-flavor gnu" ;;
  esac
}

newtos_lld() {
  case ${LLD##*/} in
    rust-lld|rust-lld.exe|lld|lld.exe) "$LLD" -flavor gnu "$@" ;;
    *) "$LLD" "$@" ;;
  esac
}
