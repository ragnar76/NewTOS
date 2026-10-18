# mklong.py <out> -- LONG.TXT for the Ctrl-S/Ctrl-Q run: 6000 numbered lines, LF line ends (EmuCON's `cat` adds the CR).
import sys
with open(sys.argv[1], "w", newline="\n") as f:
    for i in range(1, 6001):
        f.write(f"line {i:05d}: the quick brown fox jumps over the lazy dog 0123456789\n")
    f.write("END OF LONG.TXT\n")
