#!/usr/bin/env python3
"""gen-pattern.py <seed-hex> <length> <out> -- a test file with the FSWRITE
content model: byte i = ((i*7 + i//251) & 0xFF) ^ seed."""
import sys
seed, n = int(sys.argv[1], 16), int(sys.argv[2])
open(sys.argv[3], 'wb').write(bytes((((i * 7) + (i // 251)) & 0xFF) ^ seed for i in range(n)))
