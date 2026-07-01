#!/usr/bin/env python3
"""graphify post-processing for NewTOS.

graphify's raw build leaves the doc/spec graph almost disconnected from the
code graph (the semantic subagents never see code node IDs) and labels
communities after their most-connected node — often a bare ``mod.rs`` or a
``selftest_*`` file. This script repairs all of that deterministically (no
LLM, no tokens), so the graph stays a usable architecture map across rebuilds:

  1. label-match  — link a doc concept to the code symbol of the same name
                    (``Fopen`` spec  <->  ``fopen`` code), bridging the two graphs.
  2. module-tree  — add ``contains`` edges up the Rust module tree so empty
                    ``mod.rs`` / data files stop floating as isolated nodes.
  3. relabel      — name each community after a *meaningful* representative
                    (skipping mod.rs / main.rs / selftest_*), mapped through a
                    plain-language table keyed by that representative — NOT by
                    community id, so the names survive re-clustering.

Run it after any full ``/graphify`` build or ``graphify update``:

    <graphify-python> tools/graphify-postprocess.py [graphify-out-dir]

The interpreter must be the one graphify is installed under; the default
``graphify-out/.graphify_python`` records it, so the wrapper below re-execs
into it automatically.
"""
from __future__ import annotations

import collections
import json
import os
import re
import subprocess
import sys
from pathlib import Path

# ── re-exec under graphify's own interpreter if graphify isn't importable ────
try:
    import graphify  # noqa: F401
except ModuleNotFoundError:
    _out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("graphify-out")
    _rec = _out / ".graphify_python"
    _py = _rec.read_text().strip() if _rec.exists() else ""
    if _py and Path(_py).exists() and os.path.realpath(_py) != os.path.realpath(sys.executable):
        os.execv(_py, [_py, *sys.argv])
    sys.exit("graphify is not importable; run me with graphify's Python "
             "(see graphify-out/.graphify_python).")

from graphify.build import build_from_json
from graphify.cluster import cluster, score_all
from graphify.analyze import god_nodes, surprising_connections, suggest_questions
from graphify.report import generate
from graphify.export import to_json

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("graphify-out")

# File-node labels that make useless community names, plus test files: their
# subsystem-under-test is a better name than the test itself.
SKIP_LABELS = {"mod.rs", "main.rs", "lib.rs", "build.rs",
               "desktop_icons.rs", "font_data.rs"}

# Plain-language community names keyed by their representative node's label.
# Ambiguous representatives (mmu.rs, drive.rs, dispatch() — each heads two
# different communities) are deliberately omitted; those fall back to the
# representative label, which is still readable.
REP_TO_NAME = {
    "desktop.rs": "Desktop Shell & Window Mgr", "Volume": "FAT Volume & Directory",
    "Object": "AES Forms & File Selector", "time.rs": "GEMDOS Time & Basepage (+spec)",
    "Xhci": "xHCI USB Driver", "traps.rs": "GEMDOS Process/File/Mem API (+spec)",
    "window.rs": "AES Window Manager", "MD": "BDOS Memory Descriptor Pool",
    "Vdipb": "VDI Attributes & Workstation", "mailbox.rs": "BCM2711 Mailbox & Framebuffer",
    "exceptions.rs": "AArch64 Exceptions & Scheduler", "pcie_rpi4.rs": "BCM2711 PCIe Root Complex",
    "VdiScreen": "VDI Screen Backend", "shell.rs": "Debug Shell + GEMDOS Drive",
    "aes()": "AES Dispatcher & Locks", "RasterBackend": "VDI Mouse Cursor",
    "subdir_test()": "Filesystem Self-Tests", "Vdi": "VDI Init & Info",
    "Aespb": "AES Shell & Scrap", "TrapArgs": "Trap Dispatch & Console",
    "check()": "Self-Tests: BDOS Memory", "Emmc": "eMMC/SD Block Driver",
    "backend.rs": "VDI GDP Primitives", "dtb.rs": "Device-Tree (DTB) Parser",
    "Result": "xtask Build Tool", "frames.rs": "Physical Frame Allocator",
    "menu.rs": "AES Menu Bar", "process.rs": "Process Management",
    "heap.rs": "Kernel Heap Allocator", "resource.rs": "AES Resource (.RSC) Loader",
    "file.rs": "GEMDOS File Handles (OFD)", "workstation.rs": "VDI Workstation Open",
    "Host": "AES Event Manager", "graf.rs": "AES Graphics / Rubberbox",
    "Cconout": "Console Output API (spec)", "chardev.rs": "BIOS Char Device",
    "dirops.rs": "GEMDOS Directory Ops", "appl.rs": "AES Application",
    "kernel_main_rust()": "Kernel Main / Boot", "console.rs": "Console RX Ring",
    "layout.rs": "Kernel Image Layout", "ssystem.rs": "Ssystem & Cookie Jar",
    "KernelHost": "KernelHost Interface", "ParentFrame": "EL0 User Entry",
    "load_to_slot()": "ELF Loader", "input.rs": "VDI Logical Input",
    "raster.rs": "VDI Raster / Copy", "Cnecin": "Console Input API (spec)",
    "pd.rs": "AES Process Descriptor", "cache.rs": "Block Cache",
    "screen.rs": "XBIOS Screen", "cookie.rs": "Cookie Jar",
    "pexec.rs": "Pexec Loader", "error.rs": "GEMDOS Version/Error",
    "Process": "Process State", "elf.rs": "ELF Parser", "trap.rs": "Trap-Class Dispatch",
    "uart.rs": "UART Panic Writer", "fbcon.rs": "Framebuffer Console",
    "system.rs": "XBIOS System", "vwk.rs": "VDI Workstation Handles",
    "text.rs": "VDI Text Output", "registry.rs": "Registry",
    "i18n.rs": "i18n Strings", "terminate_inner()": "Pterm Terminate",
    "mkrsc.py": "RSC Builder Tool", "ikbd.rs": "IKBD Decoder",
    "BlockDev": "Block Device HAL", "arena.rs": "User Arena (EL0)",
    "cookie.rs ": "Cookie Jar", "bench.rs": "Benchmarks", "hid.rs": "HID Translate",
    "PoolStats": "Pool Stats", "pool_stats()": "BDOS Pool Stats",
    "v_escape()": "VDI Escape / Text Cells",
    "STATUS": "Status & Rules (status.md)", "active_fb()": "VDI Screen Backend",
    "VcFb": "BCM2711 Mailbox & Framebuffer", "loader.rs": "ELF Loader",
    "dispatch()": "Trap Dispatch", "run()": "XBIOS Misc (vsync/random)",
}


def classify(file_type: str) -> str:
    if file_type == "code":
        return "code"
    if file_type in ("document", "concept", "rationale"):
        return "doc"
    return file_type


def norm(s: str) -> str:
    return re.sub(r"[^a-z0-9]", "", s.lower())


def load_extraction():
    """Prefer the raw extraction (pre-cleanup) so we keep every node/edge; fall
    back to the persisted graph.json (node-link) after a build's cleanup."""
    ex_path = OUT / ".graphify_extract.json"
    if ex_path.exists():
        ex = json.loads(ex_path.read_text(encoding="utf-8"))
        return ex, "extract"
    gj = json.loads((OUT / "graph.json").read_text(encoding="utf-8"))
    nodes = [{k: v for k, v in n.items()} for n in gj["nodes"]]
    edges = [{"source": e["source"], "target": e["target"],
              "relation": e.get("relation", "references"),
              "confidence": e.get("confidence", "EXTRACTED"),
              "confidence_score": e.get("confidence_score", 1.0),
              "source_file": e.get("source_file", ""), "weight": e.get("weight", 1.0)}
             for e in gj["links"]]
    return {"nodes": nodes, "edges": edges,
            "hyperedges": gj.get("hyperedges", []),
            "input_tokens": 0, "output_tokens": 0}, "graph"


def edge_set(edges):
    s = set()
    for e in edges:
        s.add((e["source"], e["target"]))
        s.add((e["target"], e["source"]))
    return s


def pass_label_match(ex) -> int:
    """Doc concept  <->  code symbol of the same name (implements edges)."""
    lab = {n["id"]: n.get("label", "") for n in ex["nodes"]}
    srcfile = {n["id"]: n.get("source_file", "") for n in ex["nodes"]}
    code_by_norm = collections.defaultdict(list)
    for n in ex["nodes"]:
        if n.get("file_type") == "code":
            lb = n.get("label", "")
            if lb.endswith(".rs"):
                continue
            nm = norm(lb.split("(")[0])
            if len(nm) >= 4:
                code_by_norm[nm].append(n["id"])
    existing = edge_set(ex["edges"])
    added = 0
    for n in ex["nodes"]:
        if classify(n.get("file_type", "")) != "doc":
            continue
        lb = n.get("label", "")
        hit = set()
        for c in {norm(lb), norm(lb.split("(")[0])}:
            if len(c) >= 4 and c in code_by_norm and len(code_by_norm[c]) <= 3:
                hit.update(code_by_norm[c])
        for cid in hit:
            if (cid, n["id"]) in existing:
                continue
            ex["edges"].append({"source": cid, "target": n["id"], "relation": "implements",
                                "confidence": "INFERRED", "confidence_score": 0.9,
                                "source_file": srcfile.get(cid, ""), "weight": 1.0,
                                "_origin": "label-match"})
            existing.add((cid, n["id"]))
            existing.add((n["id"], cid))
            added += 1
    return added


def pass_module_tree(ex) -> int:
    """contains edges up the Rust module tree (file -> its mod.rs / lib.rs)."""
    lab = {n["id"]: n.get("label", "") for n in ex["nodes"]}
    sf = {n["id"]: n.get("source_file", "") for n in ex["nodes"]}
    filenodes = {}
    for n in ex["nodes"]:
        if n.get("file_type") == "code" and lab[n["id"]].endswith(".rs"):
            filenodes[sf[n["id"]]] = n["id"]
    existing = edge_set(ex["edges"])
    added = 0
    for path, fid in filenodes.items():
        segs = path.split("/")
        base = segs[-1]
        d = "/".join(segs[:-1])
        pd = "/".join(segs[:-2])
        if base == "main.rs":
            cands = [d + "/lib.rs"]
        elif base in ("mod.rs", "lib.rs"):
            cands = [pd + "/mod.rs", pd + "/lib.rs"]
        else:
            cands = [d + "/mod.rs", d + "/lib.rs", d + "/main.rs"]
        for c in cands:
            if c in filenodes and filenodes[c] != fid:
                parent = filenodes[c]
                if (parent, fid) not in existing:
                    ex["edges"].append({"source": parent, "target": fid, "relation": "contains",
                                        "confidence": "EXTRACTED", "confidence_score": 1.0,
                                        "source_file": c, "weight": 1.0, "_origin": "module-tree"})
                    existing.add((parent, fid))
                    existing.add((fid, parent))
                    added += 1
                break
    return added


def label_communities(ex, communities):
    lab = {n["id"]: n.get("label", n["id"]) for n in ex["nodes"]}
    deg = collections.Counter()
    for e in ex["edges"]:
        deg[e["source"]] += 1
        deg[e["target"]] += 1

    def bad(i):
        l = lab.get(i, "")
        return l in SKIP_LABELS or l.startswith("selftest")

    labels, plain = {}, 0
    for cid, ids in communities.items():
        ordered = sorted(ids, key=lambda i: deg.get(i, 0), reverse=True)
        good = [i for i in ordered if not bad(i)]
        rep = lab.get(good[0] if good else ordered[0], str(cid))
        if rep in REP_TO_NAME:
            labels[cid] = REP_TO_NAME[rep]
            plain += 1
        else:
            labels[cid] = rep
    return labels, plain


def reconstruct_detection(ex):
    p = OUT / ".graphify_detect.json"
    if p.exists():
        return json.loads(p.read_text(encoding="utf-8"))
    files = collections.defaultdict(set)
    for n in ex["nodes"]:
        sfp = n.get("source_file")
        if not sfp:
            continue
        cat = "code" if n.get("file_type") == "code" else "document"
        files[cat].add(sfp)
    total = sum(len(v) for v in files.values())
    return {"total_files": total, "total_words": 0,
            "files": {k: sorted(v) for k, v in files.items()}}


def main():
    ex, source = load_extraction()
    print(f"loaded {len(ex['nodes'])} nodes, {len(ex['edges'])} edges (from {source})")
    added_lm = pass_label_match(ex)
    added_mt = pass_module_tree(ex)
    print(f"pass 1 label-match: +{added_lm} edges   pass 2 module-tree: +{added_mt} edges")

    detection = reconstruct_detection(ex)
    G = build_from_json(ex, root=".", directed=False)
    communities = cluster(G)
    cohesion = score_all(G, communities)
    gods = god_nodes(G)
    surprises = surprising_connections(G, communities)
    labels, plain = label_communities(ex, communities)
    questions = suggest_questions(G, communities, labels)

    gpath = OUT / "graph.json"
    if gpath.exists():
        gpath.unlink()
    to_json(G, communities, str(gpath))
    (OUT / "GRAPH_REPORT.md").write_text(
        generate(G, communities, cohesion, labels, gods, surprises,
                 detection, {"input": 0, "output": 0}, ".",
                 suggested_questions=questions), encoding="utf-8")
    (OUT / ".graphify_labels.json").write_text(
        json.dumps({str(k): v for k, v in labels.items()}, ensure_ascii=False), encoding="utf-8")
    (OUT / ".graphify_analysis.json").write_text(json.dumps({
        "communities": {str(k): v for k, v in communities.items()},
        "cohesion": {str(k): v for k, v in cohesion.items()},
        "gods": gods, "surprises": surprises, "questions": questions,
    }, indent=2, ensure_ascii=False), encoding="utf-8")

    iso = sum(1 for n in G.nodes() if G.degree(n) == 0)
    print(f"rebuilt: {G.number_of_nodes()} nodes, {G.number_of_edges()} edges, "
          f"{len(communities)} communities ({plain} plain-language labels), {iso} isolated")

    try:
        subprocess.run(["graphify", "export", "html"], check=True,
                       capture_output=True, text=True)
        print("graph.html regenerated")
    except Exception:
        print("note: run `graphify export html` to refresh graph.html")


if __name__ == "__main__":
    main()
