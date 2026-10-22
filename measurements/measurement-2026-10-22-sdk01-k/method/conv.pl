undef $/;
for my $f (@ARGV) {
  open my $fh, '<', $f or die; my $s = <$fh>; close $fh;
  $s =~ s|^(\. "\$SDK/toolchain\.sh")[^\n]*\n|$1   # LLVM tools: PATH / LLVM packages first, pinned Rust sysroot as fallback (SDK01 I, K)\n|m;
  $s =~ s|^BIN="\$\(newtos_rust_bindir\)"\n||m;
  $s =~ s|\$\{AR:-\$BIN/llvm-ar\}|\${AR:-\$(newtos_tool llvm-ar)}|g;
  $s =~ s|\$\{LLD:-\$BIN/rust-lld\}|\${LLD:-\$(newtos_tool ld.lld)}|g;
  $s =~ s|LLD="\$BIN/rust-lld"|LLD="\$(newtos_tool ld.lld)"|g;
  $s =~ s|AR="\$BIN/llvm-ar"|AR="\$(newtos_tool llvm-ar)"|g;
  $s =~ s|"\$BIN/(llvm-[a-z]+)"|"\$(newtos_tool $1)"|g;
  $s =~ s|"\$LLD" -flavor gnu|newtos_lld|g;
  open $fh, '>', $f or die; print $fh $s; close $fh;
}
