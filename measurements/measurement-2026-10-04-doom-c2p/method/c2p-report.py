#!/usr/bin/env python3
"""c2p-report.py <measurement-dir> -- before/after and candidate table from
raw/*/functions.csv + summary.json (run analyze.py on every raw dir first).

c2p cost = the cycles of the c2p function itself (after/candidates: the
separate symbol *_row*; before: _DG_DrawFrame, where the old c2p was inlined
together with the per-frame glue -- the profile run's 155 cyc/px figure is
the same quantity). Per pixel: / (299 frames * 64000 pixels); the window
holds 299 completed frames (RUNINFO). Hatari 030 model at 32.085 MHz."""
import csv, json, os, sys
M = sys.argv[1]; HZ = 32084988.0; PX = 320 * 200
ORDER = ["before-plain", "before-enh", "after-plain", "after-enh",
         "cand-4tab-plain", "cand-4tab-enh", "cand-16x4tab-plain", "cand-merge5-plain",
         "cand-1tab-plain", "cand-4tab2ph-plain", "cand-1tab2ph-plain"]
out = []
P = out.append
P(f"{'run':22s}{'c2p symbol':26s}{'c2p cyc/px':>11s}{'instr/px':>9s}{'cyc/instr':>10s}"
  f"{'DrawFrame+c2p cyc/px':>21s}{'Mcyc/frame':>11s}{'fps':>7s}{'vs before':>10s}")
base = {}
for r in ORDER:
    d = f"{M}/raw/{r}"
    if not os.path.exists(f"{d}/summary.json"):
        continue
    s = json.load(open(f"{d}/summary.json")); fr = s["frames_R_RenderPlayerView_calls"]
    rows = list(csv.DictReader(open(f"{d}/functions.csv")))
    fn = {x["function"]: x for x in rows}
    c2p = [x for x in rows if "_row" in x["function"] and "read_row" not in x["function"]]
    sym = c2p[0]["function"] if c2p else "_DG_DrawFrame (inlined)"
    src = c2p[0] if c2p else fn["_DG_DrawFrame"]
    cyc = int(src["cycles"]); ins = int(src["instructions"])
    df = int(fn["_DG_DrawFrame"]["cycles"]) + (cyc if c2p else 0)
    cpf = s["cycles_per_frame"]
    kind = r.split("-")[-1]
    if r.startswith("before"):
        base[kind] = cpf
    rel = f"{100 * (cpf / base[kind] - 1):9.1f}%" if kind in base else ""
    P(f"{r:22s}{sym:26s}{cyc / fr / PX:11.1f}{ins / fr / PX:9.2f}{cyc / ins:10.2f}"
      f"{df / fr / PX:21.1f}{cpf / 1e6:11.2f}{HZ / cpf:7.2f}{rel}")
for r in ("after-plain", "after-enh"):
    d = f"{M}/raw/{r}"
    if not os.path.exists(f"{d}/summary.json"):
        continue
    s = json.load(open(f"{d}/summary.json")); fr = s["frames_R_RenderPlayerView_calls"]
    tot = s["dump_sum"]["cycles"]; ti = s["dump_sum"]["instr"]
    rows = list(csv.DictReader(open(f"{d}/functions.csv")))
    P(""); P(f"== {r}: categories")
    for c, v in sorted(s["categories"].items(), key=lambda x: -x[1]["cycles"]):
        P(f"  {v['share_pct']:6.2f}%  {v['cycles'] / fr / 1e3:9.0f} kcyc/frame  {100 * v['instr'] / ti:6.2f}% instr   {c}")
    P(f"== {r}: top 20 functions (cycles share; instr share; kcyc/frame)")
    for i, x in enumerate(rows[:20], 1):
        c = int(x["cycles"]); n = int(x["instructions"])
        P(f"  {i:2d} {100 * c / tot:6.2f}% {100 * n / ti:6.2f}%i {c / fr / 1e3:8.0f}k  {x['function']:28s} {x['category']}")
open(f"{M}/raw/C2P-REPORT.txt", "w").write("\n".join(out) + "\n")
print("\n".join(out))
