#!/usr/bin/env python3
"""analyze.py <raw-dir> <obj-dir> <gemlib.a> -- per-function table from a Hatari
CPU profile log (hatari.log of run-profile.sh).

Input: 'profile addresses' dumps (per-instruction: instructions, cycles,
i-cache misses, d-cache hits), 'profile stats' totals, 'profile callers'.
Function = nearest preceding symbol of the dump; labels without a leading
underscore (libc asm sub-labels) belong to the preceding underscore symbol.
Category = source object of the symbol (nm of the build's .o files).
Output: <raw-dir>/functions.csv, categories.csv, summary.json (printed too).
"""
import re, sys, json, subprocess, os, collections, csv

raw, objdir, gemlib = sys.argv[1], sys.argv[2], sys.argv[3]
NM = "/opt/cross-mint/bin/m68k-atari-mint-nm"
import gzip
_p = os.path.join(raw, "hatari.log")
log = (gzip.open(_p + ".gz", "rt", errors="replace") if os.path.exists(_p + ".gz") else open(_p, errors="replace")).read().splitlines()

# ---- symbol -> object -------------------------------------------------------
sym2obj = {}
for f in sorted(os.listdir(objdir)):
    if not f.endswith(".o"):
        continue
    out = subprocess.run([NM, os.path.join(objdir, f)], capture_output=True, text=True).stdout
    for l in out.splitlines():
        p = l.split()
        if len(p) == 3 and p[1] in "Tt" and not p[2].startswith(".L"):
            sym2obj.setdefault(p[2], f[:-2])
gem = set()
out = subprocess.run([NM, gemlib], capture_output=True, text=True).stdout
for l in out.splitlines():
    p = l.split()
    if len(p) == 3 and p[1] in "Tt":
        gem.add(p[2])

RENDER = {"r_main", "r_bsp", "r_segs", "r_plane", "r_draw", "r_things", "r_data", "r_sky", "tables"}
FIXED = {"m_fixed"}
LOGIC = {"p_ceilng", "p_doors", "p_enemy", "p_floor", "p_inter", "p_lights", "p_map", "p_maputl",
         "p_mobj", "p_plats", "p_pspr", "p_saveg", "p_setup", "p_sight", "p_spec", "p_switch",
         "p_telept", "p_tick", "p_user", "g_game", "d_main", "d_net", "d_loop", "d_event",
         "d_items", "d_iwad", "d_mode", "info", "m_random", "m_cheat", "doomstat", "dstrings"}
UI = {"v_video", "st_stuff", "st_lib", "hu_stuff", "hu_lib", "wi_stuff", "am_map", "f_finale",
      "f_wipe", "m_menu", "m_bbox"}
PLAT = {"doomgeneric_gem", "doomgeneric"}

def category(sym):
    base = sym.split(".")[0] if sym.startswith("_") else sym
    o = sym2obj.get(sym) or sym2obj.get(base)
    if o is None:
        if sym in gem or base in gem:
            return "platform: gemlib (AES/VDI binding)"
        if re.match(r"^___?(u?div|u?mod|mul|ashl|ashr|lshr)[a-z]*[di]i3$", sym) or sym.startswith("___"):
            return "fixed-point/libgcc helper"
        return "libc"
    if o in RENDER: return "Doom renderer"
    if o in FIXED: return "fixed-point (m_fixed)"
    if o in LOGIC: return "game logic"
    if o in UI: return "Doom UI/video (v_video, status bar, HUD, wipe, menu)"
    if o in PLAT: return "platform layer (doomgeneric_gem.c)"
    return "Doom support (zone, WAD, misc, i_*, m_*)"

# ---- sections ---------------------------------------------------------------
def sect(start_pat):
    for i, l in enumerate(log):
        if l.startswith(start_pat):
            return i
    return None
i_stats = sect("> profile stats")
i_cyc = sect("> profile cycles")
i_call = sect("> profile callers")
i_a1 = sect("> profile addresses 0x101a000")
i_a2 = sect("> profile addresses 0xe00000")
assert None not in (i_stats, i_call, i_a1, i_a2), "profile output missing"

# ---- totals (profile stats) -------------------------------------------------
tot = {"instr": 0, "cycles": 0, "imiss": 0, "dhit": 0}
areas = {}
cur = None
seg = log[i_stats:i_cyc]
for k, l in enumerate(seg):
    m = re.match(r"^(Normal RAM|ROM TOS|Cartridge ROM|TT-RAM)", l)
    if m: cur = m.group(1)
    m = re.match(r"^- (executed instructions|used cycles|instruction cache misses|data cache hits):", l)
    if m:
        v = int(re.match(r"\s*(\d+)", seg[k + 1]).group(1))
        areas.setdefault(cur, {})[m.group(1)] = v
for a in areas.values():
    tot["instr"] += a.get("executed instructions", 0)
    tot["cycles"] += a.get("used cycles", 0)
    tot["imiss"] += a.get("instruction cache misses", 0)
    tot["dhit"] += a.get("data cache hits", 0)

# ---- per-function from address dumps ----------------------------------------
instr_re = re.compile(r"^([0-9a-f]{8}) .*% \((\d+), (\d+), (\d+), (\d+)\)\s*$")
fn = collections.OrderedDict()
def feed(lines, rom):
    cur = None; parent = None
    for l in lines:
        m = instr_re.match(l)
        if m:
            name = "TOS ROM" if rom else (parent or "?")
            d = fn.setdefault(name, [0, 0, 0, 0])
            for k in range(4): d[k] += int(m.group(2 + k))
            continue
        m = re.match(r"^([^\s\[#>].*):$", l)
        if m and not rom:
            s = m.group(1)
            if s.startswith("_") or parent is None:
                parent = s
            # labels w/o underscore stay inside the parent
feed(log[i_a1 + 1:i_a2], False)
feed(log[i_a2 + 1:], True)

rows = []
for name, d in fn.items():
    cat = "TOS ROM (BIOS/VDI/AES code run by traps)" if name == "TOS ROM" else category(name)
    rows.append((name, cat, d[0], d[1], d[2], d[3]))
sumc = sum(r[3] for r in rows); sumi = sum(r[2] for r in rows)
rows.sort(key=lambda r: -r[3])
with open(os.path.join(raw, "functions.csv"), "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["function", "category", "instructions", "cycles", "share_cycles_pct", "icache_misses", "dcache_hits"])
    for r in rows:
        w.writerow([r[0], r[1], r[2], r[3], f"{100*r[3]/sumc:.3f}", r[4], r[5]])
cats = collections.defaultdict(lambda: [0, 0])
for r in rows:
    cats[r[1]][0] += r[2]; cats[r[1]][1] += r[3]
with open(os.path.join(raw, "categories.csv"), "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["category", "instructions", "cycles", "share_cycles_pct"])
    for c, (i, cy) in sorted(cats.items(), key=lambda x: -x[1][1]):
        w.writerow([c, i, cy, f"{100*cy/sumc:.3f}"])

# ---- callers: frames + inclusive cycles of selected functions ---------------
calls = {}; incl = {}
for l in log[i_call:i_a1]:
    m = re.match(r"^0x[0-9a-f]+: (.*), (\S+)$", l)
    if not m: continue
    name = m.group(2)
    for ent in m.group(1).split(", "):
        e = re.match(r"^0x[0-9a-f]+ = (\d+) (\w)(?: (\d+)/(\d+)/(\d+)/(\d+)/(\d+))?", ent)
        if e and e.group(2) == "s":
            calls[name] = calls.get(name, 0) + int(e.group(1))
            if e.group(5):
                incl[name] = incl.get(name, 0) + int(e.group(5))
frames = calls.get("_R_RenderPlayerView", 0)  # completed calls; +1 call started but not returned at the stop breakpoint
res = {"raw": raw, "stats_total": tot, "dump_sum": {"instr": sumi, "cycles": sumc},
       "frames_R_RenderPlayerView_calls": frames,
       "cycles_per_frame": sumc / frames if frames else None,
       "instr_per_frame": sumi / frames if frames else None,
       "categories": {c: {"cycles": v[1], "share_pct": round(100*v[1]/sumc, 2), "instr": v[0]} for c, v in cats.items()},
       "calls": {k: calls.get(k) for k in ("_FixedDiv", "_FixedMul", "___divdi3", "___muldi3", "___udivdi3", "_R_DrawColumn", "_R_DrawColumnLow", "_R_DrawSpan", "_R_DrawSpanLow", "_vr_trnfm", "_vro_cpyfm", "_R_RenderPlayerView", "_DG_DrawFrame")},
       "inclusive_cycles": {k: incl.get(k) for k in ("_R_RenderPlayerView", "_vr_trnfm", "_vro_cpyfm", "_DG_DrawFrame", "_FixedDiv", "___divdi3", "_P_Ticker", "_G_Ticker", "_R_DrawPlanes", "_R_RenderSegLoop", "_R_DrawMasked")},
       "top": [(r[0], r[1], r[2], r[3], round(100*r[3]/sumc, 2)) for r in rows[:30]]}
json.dump(res, open(os.path.join(raw, "summary.json"), "w"), indent=1)
print(json.dumps(res, indent=1))
