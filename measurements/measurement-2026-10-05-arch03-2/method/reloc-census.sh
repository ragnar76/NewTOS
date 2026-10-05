#!/usr/bin/env bash
# reloc-census.sh <elf>... -- per program: e_type, e_flags, program headers,
# dynamic tags of interest and a histogram of the dynamic relocation TYPES the
# linker emitted (ARCH03 0d: only R_AARCH64_RELATIVE may appear). Read-only.
set -uo pipefail
RO="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/llvm-readobj"
for f in "$@"; do
  name=$(basename "$f")
  typ=$("$RO" --elf-output-style=GNU -h "$f" | awk -F: '/^  Type:/{gsub(/^ +/,"",$2);print $2}')
  flg=$("$RO" --elf-output-style=GNU -h "$f" | awk -F: '/^  Flags:/{gsub(/^ +/,"",$2);print $2}')
  phd=$("$RO" --elf-output-style=GNU -l "$f" | awk '/^  [A-Z_]+ +0x/{print $1}' | sort | uniq -c | awk '{printf "%s×%s ", $1, $2}')
  tags=$("$RO" --elf-output-style=GNU -d "$f" | awk '/\(/{gsub(/[()]/,"",$2); print $2}' | grep -E '^(RELA|RELASZ|RELAENT|RELACOUNT|REL|RELSZ|JMPREL|PLTRELSZ|TEXTREL|NEEDED|FLAGS|RELR)$' | sort -u | tr '\n' ' ')
  rel=$("$RO" --elf-output-style=GNU -r "$f" | awk '/R_AARCH64_/{print $3}' | sort | uniq -c | awk '{printf "%s=%s ", $2, $1}')
  [ -z "$rel" ] && rel="(none)"
  printf '%-16s type=%-28s e_flags=%s phdrs=[%s] dyn=[%s] relocs: %s\n' "$name" "$typ" "$flg" "${phd% }" "${tags% }" "$rel"
done
