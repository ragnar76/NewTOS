#!/usr/bin/env python3
"""Decode the [a60] AESGLOBAL hexdumps three ways: TOS layout (application.md),
TeraDesk's LP64 AES_GLOBAL struct (library.h:65-77, offsets from offs.c), and
the 8-byte stores/loads rsc_load.c does at &aes_global[5]/[7]."""
import sys, struct
def parse(line):
    hexs = line.split('bytes=')[1].split()
    return bytes.fromhex(''.join(hexs))
def u16(b,o): return struct.unpack_from('<H',b,o)[0]
def u32(b,o): return struct.unpack_from('<I',b,o)[0]
def u64(b,o): return struct.unpack_from('<Q',b,o)[0]
def show(tag, b):
    print(f"== {tag}")
    print("   bytes:", ' '.join(f"{i:02d}:{x:02x}" for i,x in enumerate(b)))
    print(f"   TOS layout   : ap_version={u16(b,0):#06x} ap_count={u16(b,2)} ap_id={u16(b,4)} ap_private={u32(b,6):#x}"
          f" ap_ptree@10={u32(b,10):#x} ap_rscmem@14={u32(b,14):#x} ap_rsclen@18={u16(b,18)} ap_planes@20={u16(b,20)}"
          f" ap_3resv@22={u32(b,22):#x} ap_4resv@26={u32(b,26):#x}")
    print(f"   TeraDesk LP64 struct (library.h): ap_private@8={u64(b,8):#x} ap_ptree@16={u64(b,16):#x} ap_rscmem@24={u64(b,24):#x}"
          f" (ap_rsclen@32/ap_planes@34/ap_3resv@40 lie beyond the 32-byte array)")
    print(f"   rsc_load.c 8-byte view: *(OBJECT***)&aes_global[5] (@10)={u64(b,10):#x}  *(void**)&aes_global[7] (@14)={u64(b,14):#x}")
lines = [l for l in open(sys.argv[1]) if '[a60]' in l and 'bytes=' in l]
for l in lines:
    show(l.strip().split('bytes=')[0].replace('[a60] ',''), parse(l))
