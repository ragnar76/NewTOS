#!/usr/bin/env python3
"""pi-model.py -- the real-Pi cost model of TASK/FS08 part 4 (stdiobuf).

Constants from the REAL Pi 4 run measurements/measurement-2026-10-07-fs08-pi-after/
FSBENCH.LOG (kernel ac17deae, cold, M010_1.DAT = 10 MiB):
  L2 Fread  4 KiB: 2235 ms for 2560 calls     L2 Fread 64 KiB: 955.3 ms for 160 calls
  L2 whole file:    875.1 ms                   L3 stdio 1 KiB:  7569 ms for 10241 freads
Linear fit through the two L2 points: per Fread call C = 0.533 ms, streaming
T0 = 870 ms per 10 MiB (agrees with the whole-file 875 ms). The 1-KiB stdio run
costs 0.654 ms per buffer fill, so a fill is modelled with C_LO = 0.533 and
C_HI = 0.654 ms. In QEMU the stdio per-fread CPU cost is not visible (L3 = L2
whole there, ~2.2 s), so it is taken as negligible here.

  sequential (FSBENCH L3, 10 MiB):  T(B) = T0 + (10 MiB / B) * C
  traced runs (QEMU SD trace of the same workload): T_io = commands * C_LO
                                    + blocks * 512 B / (10 MiB / T0)
"""
C_LO, C_HI, T0 = 0.533, 0.654, 870.0
MIB10 = 10 * 1024 * 1024
print("sequential 10 MiB through stdio, fread(1 KiB) loop (Pi model)")
print(f"{'buffer':>8} {'fills':>6} {'ms (C_lo..C_hi)':>18} {'KiB/s':>14} {'of whole-file 875 ms':>22}")
for kib in (1, 16, 32, 64):
    n = MIB10 // (kib * 1024)
    lo, hi = T0 + n * C_LO, T0 + n * C_HI
    print(f"{kib:>6} K {n:>6} {lo:>8.0f}..{hi:<8.0f} {10240/hi*1000:>6.0f}..{10240/lo*1000:<6.0f} {875.1/hi*100:>9.0f}..{875.1/lo*100:<3.0f} %")
print("measured on the Pi: 1 KiB 7569 ms (1352 KiB/s), one Fread 875.1 ms (11701 KiB/s)")
print()
rate = MIB10 / T0          # bytes per ms
runs = {   # name: (commands, blocks) from the QEMU traces of this run (checks/)
    "doom 90 s, 1 KiB (HEAD libc)": (2603, 5919),
    "doom 90 s, 16 KiB": (4229, 29224),
    "doom 90 s, 32 KiB (this change)": (4269, 55452),
    "doom 90 s, 64 KiB": (4295, 107664),
    "zvw BIG.JPG run, before": (2644, 8317),
    "zvw BIG.JPG run, after": (566, 8319),
}
print("traced runs: SD I/O time on the Pi model (whole run incl. boot)")
for k, (c, b) in runs.items():
    t = c * C_LO + b * 512 / rate
    print(f"  {k:34} {c:>6} cmds {b:>7} blocks ({b*512/1e6:6.1f} MB)  ~{t/1000:5.2f} s")
