#!/usr/bin/env python3
"""Deterministic graphify build for NewTOS — no LLM, no subagents.

graphify's default doc extraction dispatches LLM subagents, which are slow,
non-reproducible, and (here) left the spec docs as a disconnected island. This
script builds the whole knowledge graph deterministically instead:

  * code  — graphify's AST extractor (already deterministic).
  * docs  — one node per Markdown file, labelled by the API symbol its
            filename/H1 encodes (``3d-fopen.md`` -> ``Fopen``), grouped under a
            subsystem node (GEMDOS/BIOS/XBIOS/VDI/AES/USB/…), and cross-linked
            by the ``[text](other.md)`` references the docs already contain.

It writes the raw extraction, then hands off to ``graphify-postprocess.py``
(label-match doc<->code, module-tree, community labelling, HTML). Re-running it
produces an identical graph every time (modulo Leiden's clustering randomness).

    <graphify-python> tools/graphify-build.py     # or: python3 tools/graphify-build.py
"""
from __future__ import annotations

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
    _rec = Path("graphify-out/.graphify_python")
    _py = _rec.read_text().strip() if _rec.exists() else ""
    if not _py:
        _bin = subprocess.run(["bash", "-lc", "command -v graphify"],
                              capture_output=True, text=True).stdout.strip()
        if _bin:
            _py = Path(_bin).read_text().splitlines()[0].lstrip("#!").strip()
    if _py and Path(_py).exists() and os.path.realpath(_py) != os.path.realpath(sys.executable):
        os.execv(_py, [_py, *sys.argv])
    sys.exit("graphify is not importable; run me with graphify's Python.")

from graphify.detect import detect
from graphify.extract import collect_files, extract

ROOT = Path(".").resolve()
OUT = Path("graphify-out")

# API subsystems that get their own grouping node.
SUBSYS_ORDER = ["gemdos", "bios", "xbios", "vdi", "aes", "usb", "structures"]


def rel(p: str) -> str:
    return os.path.relpath(p, ROOT)


def nice_symbol(sym: str) -> str:
    # cosmetic only (matching is case-insensitive downstream)
    if "_" in sym or not sym.isalpha():
        return sym
    return sym[:1].upper() + sym[1:]


def clean_h1(text: str) -> str | None:
    for line in text.splitlines():
        m = re.match(r"^#\s+(.+?)\s*$", line)
        if m:
            title = m.group(1)
            for sep in (" — ", " – ", " - ", ": ", " ("):
                if sep in title:
                    title = title.split(sep)[0]
            title = re.sub(r"[`*_]", "", title).strip()[:60]
            if re.sub(r"[^a-z0-9]", "", title.lower()):  # reject punctuation-only (=== rules)
                return title
    return None


def node_id(path_noext: str, sym: str) -> str:
    stem = re.sub(r"[^a-z0-9]", "_", path_noext.lower())
    return f"{stem}_{re.sub(r'[^a-z0-9]', '_', sym.lower())}"


def extract_docs(doc_paths: list[str]):
    """Deterministic doc graph: file nodes + subsystem grouping + xrefs."""
    nodes, edges = [], []
    subsys_ids: dict[str, str] = {}
    docnode: dict[str, str] = {}

    def subsystem(key: str, label: str) -> str:
        if key not in subsys_ids:
            nid = f"subsystem_{key}"
            subsys_ids[key] = nid
            nodes.append({"id": nid, "label": label, "file_type": "concept",
                          "source_file": "", "source_location": None})
        return subsys_ids[key]

    for P in doc_paths:
        try:
            content = (ROOT / P).read_text(encoding="utf-8", errors="replace")
        except Exception:
            content = ""
        parts = P.split("/")
        base = parts[-1].rsplit(".", 1)[0]
        m = re.match(r"^[0-9a-fA-F]{1,3}-(.+)$", base)         # strip 3d- / 00- / 154-
        sym = m.group(1) if m else base
        is_api_fn = P.startswith("docs/api/") and not base.startswith("_")
        if is_api_fn:
            label = nice_symbol(sym)
        else:
            label = clean_h1(content) or nice_symbol(sym.lstrip("_"))
        nid = node_id(P.rsplit(".", 1)[0], sym)
        nodes.append({"id": nid, "label": label, "file_type": "document",
                      "source_file": P, "source_location": None})
        docnode[P] = nid

        # subsystem grouping
        key = None
        if P.startswith("docs/api/") and len(parts) >= 3:
            key = parts[2]
        elif P.startswith("phases/"):
            key = "phases"
        elif P.startswith("docs/hardware/"):
            key = "hardware"
        elif P.startswith("docs/"):
            key = "docs"
        if key:
            label_map = {"gemdos": "GEMDOS", "bios": "BIOS", "xbios": "XBIOS",
                         "vdi": "VDI", "aes": "AES", "usb": "USB",
                         "structures": "TOS Structures", "phases": "Phase Roadmap",
                         "hardware": "Hardware Docs", "docs": "Project Docs"}
            snid = subsystem(key, label_map.get(key, key.upper()))
            edges.append({"source": snid, "target": nid, "relation": "contains",
                          "confidence": "EXTRACTED", "confidence_score": 1.0,
                          "source_file": P, "weight": 1.0, "_origin": "doc-subsystem"})

    # explicit cross-references: [text](other.md)
    seen = set()
    for P in doc_paths:
        try:
            content = (ROOT / P).read_text(encoding="utf-8", errors="replace")
        except Exception:
            continue
        for mm in re.finditer(r"\]\(([^)\s]+\.md)(?:#[^)]*)?\)", content):
            tgt = os.path.normpath(os.path.join(os.path.dirname(P), mm.group(1)))
            if tgt in docnode and tgt != P:
                key = (docnode[P], docnode[tgt])
                if key in seen:
                    continue
                seen.add(key)
                edges.append({"source": docnode[P], "target": docnode[tgt],
                              "relation": "references", "confidence": "EXTRACTED",
                              "confidence_score": 1.0, "source_file": P,
                              "weight": 1.0, "_origin": "doc-xref"})
    return nodes, edges


def main():
    OUT.mkdir(exist_ok=True)
    (OUT / ".graphify_python").write_text(sys.executable, encoding="utf-8")

    det = detect(ROOT)
    (OUT / ".graphify_detect.json").write_text(json.dumps(det, ensure_ascii=False), encoding="utf-8")
    code = det["files"].get("code", [])
    docs = sorted(rel(p) for p in det["files"].get("document", []))
    print(f"detect: {len(code)} code files, {len(docs)} doc files")

    # code AST
    cfiles = []
    for f in code:
        cfiles.extend(collect_files(Path(f)) if Path(f).is_dir() else [Path(f)])
    ast = extract(cfiles, cache_root=ROOT) if cfiles else {"nodes": [], "edges": []}
    print(f"AST: {len(ast['nodes'])} nodes, {len(ast['edges'])} edges")

    # deterministic docs
    dnodes, dedges = extract_docs(docs)
    print(f"docs: {len(dnodes)} nodes, {len(dedges)} edges (deterministic)")

    # merge -> raw extraction
    seen = {n["id"] for n in ast["nodes"]}
    nodes = list(ast["nodes"])
    for n in dnodes:
        if n["id"] not in seen:
            nodes.append(n)
            seen.add(n["id"])
    merged = {"nodes": nodes, "edges": ast["edges"] + dedges,
              "hyperedges": [], "input_tokens": 0, "output_tokens": 0}
    (OUT / ".graphify_extract.json").write_text(json.dumps(merged, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"extract: {len(nodes)} nodes, {len(merged['edges'])} edges")

    # hand off to the post-processor (label-match, module-tree, cluster, label, html)
    pp = Path(__file__).with_name("graphify-postprocess.py")
    print("--- post-processing ---")
    subprocess.run([sys.executable, str(pp)], check=True)


if __name__ == "__main__":
    main()
