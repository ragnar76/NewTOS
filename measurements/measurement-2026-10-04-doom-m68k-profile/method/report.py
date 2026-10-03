#!/usr/bin/env python3
"""report.py <measurement-dir> -- ranked tables from raw/*/functions.csv + summary.json (run analyze.py first)."""
import json, csv, os, re, sys, collections
M = sys.argv[1]; HZ = 32084988.0
runs = sorted(d for d in os.listdir(M + "/raw") if os.path.exists(f"{M}/raw/{d}/summary.json"))
out = []
def P(s=""): out.append(s)
def load(r):
    return json.load(open(f"{M}/raw/{r}/summary.json")), list(csv.DictReader(open(f"{M}/raw/{r}/functions.csv")))
P("MATRIX (300-frame window = 299 completed R_RenderPlayerView calls; Hatari 030 cycle model, 32.085 MHz)")
P(f"{'run':24s}{'Mcyc/frame':>11s}{'ms/frame':>10s}{'fps':>7s}{'Minstr/fr':>10s}{'cyc/instr':>10s}{'vs a-plain':>11s}")
base = None
for r in runs:
    d, _ = load(r)
    cpf = d["cycles_per_frame"]; ipf = d["instr_per_frame"]
    if r == "a-plain": base = cpf
    P(f"{r:24s}{cpf/1e6:11.2f}{cpf/HZ*1e3:10.1f}{HZ/cpf:7.2f}{ipf/1e6:10.2f}{cpf/ipf:10.2f}" + (f"{100*(cpf/base-1):10.1f}%" if base else ""))
P(); P("PER-FRAME COST BY GROUP (kcyc/frame) and column/span call counts per run")
P(f"{'run':24s}{'platform':>9s}{'renderer':>9s}{'TOS ROM':>9s}{'libc':>7s}{'logic':>7s}{'other':>7s}  DrawColumn[Low]/frame  DrawSpan[Low]/frame")
for r in runs:
    d, _ = load(r); fr = d["frames_R_RenderPlayerView_calls"]; c = d["categories"]
    g = lambda k: sum(v["cycles"] for n, v in c.items() if n.startswith(k)) / fr / 1e3
    plat = g("platform"); ren = g("Doom renderer"); tos = g("TOS ROM"); lc = g("libc"); lg = g("game logic")
    allc = d["dump_sum"]["cycles"] / fr / 1e3; oth = allc - plat - ren - tos - lc - lg
    k = d["calls"]
    P(f"{r:24s}{plat:9.0f}{ren:9.0f}{tos:9.0f}{lc:7.0f}{lg:7.0f}{oth:7.0f}  {(k['_R_DrawColumn'] or 0)/fr:8.0f}/{(k['_R_DrawColumnLow'] or 0)/fr:<8.0f}  {(k['_R_DrawSpan'] or 0)/fr:8.0f}/{(k['_R_DrawSpanLow'] or 0)/fr:<8.0f}")
for r in ("a-plain", "b-enh", "k-emutos-plain", "l-emutos-enh"):
    d, rows = load(r)
    tot = d["dump_sum"]["cycles"]; ti = d["dump_sum"]["instr"]; fr = d["frames_R_RenderPlayerView_calls"]
    P(); P(f"== {r}: categories")
    for c, v in sorted(d["categories"].items(), key=lambda x: -x[1]["cycles"]):
        P(f"  {v['share_pct']:6.2f}%  {v['cycles']/fr/1e3:9.0f} kcyc/frame  {100*v['instr']/ti:6.2f}% instr   {c}")
    P(f"== {r}: top 25 functions (cycles share; instr share; kcyc/frame)")
    for i, x in enumerate(rows[:25], 1):
        c = int(x["cycles"]); n = int(x["instructions"])
        P(f"  {i:2d} {100*c/tot:6.2f}% {100*n/ti:6.2f}%i {c/fr/1e3:8.0f}k  {x['function']:28s} {x['category']}")
open(M + "/raw/REPORT-TABLES.txt", "w").write("\n".join(out) + "\n")
print("\n".join(out))
