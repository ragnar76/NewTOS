#!/usr/bin/env python3
"""TOS-API spec-vs-implementation coverage check for NewTOS.

Soll (the extracted spec, ``docs/api/<sub>/<NN>-<name>.md``) vs. Ist (the code).
For every per-function spec file whose name encodes a trap number
(``3d-fopen.md`` -> GEMDOS 0x3d) it decides:

  IMPLEMENTED  — a handler function exists (``fn <name>`` anywhere in tos-api or
                 the kernel, incl. renamed process handlers via ALIAS, or the
                 GEMDOS number is wired in ``TABLE``). Native reimplementations
                 live in different files/names, so this searches the whole tree,
                 not just the subsystem dispatcher.
  OPEN (doc'd) — no handler, but the spec marks it out-of-scope: an XBIOS
                 ``_triage`` row flagged stub/🟡 (no floppy/MIDI/YM2149 HW), or
                 the function's own doc says out-of-scope / unimplemented.
  DISCREPANCY  — no handler and not marked out-of-scope, yet its phase is done
                 per STATUS.md → decide: implement it, or document why not.

Grep/AST-name based → a targeting aid, not a verifier. Verify each DISCREPANCY
against the dispatcher before acting.  Run:  python3 tools/validate-coverage.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(".").resolve()
API = ROOT / "docs" / "api"
IMPL_DIRS = [ROOT / "tos-api" / "src", ROOT / "kernel" / "src"]

SUBSYS = {
    "gemdos": "Ph02/04/06/07/09",
    "bios": "Ph03/08",
    "xbios": "Ph11",
    "vdi": "Ph12",
}

# spec name -> actual handler fn name where the native reimplementation renamed
# it (process family is intercepted in the kernel, not the GEMDOS table).
ALIAS = {
    "pterm": "terminate", "pterm0": "terminate_zero", "ptermres": "terminate_resident",
    "pexec": "handle_pexec",
}

OOS_DOC = re.compile(
    r"out[- ]of[- ]scope|unimplemented|not implemented|nicht implementiert|"
    r"no .{0,20}hardware|honest.{0,20}unsupported|\bstub\b|"
    r"decision deferred|\bdeferred\b|to be decided|decide when", re.IGNORECASE)


def read(p: Path) -> str:
    try:
        return p.read_text(encoding="utf-8", errors="replace")
    except Exception:
        return ""


def build_impl_index():
    """Fn names defined across tos-api + kernel, plus the trap numbers each
    subsystem dispatcher (``tos-api/src/<sub>/mod.rs``) actually wires — the
    authoritative "is this routed" signal (const ids / ``t[0xNN]`` table)."""
    fns: set[str] = set()
    for d in IMPL_DIRS:
        for f in d.rglob("*.rs"):
            for m in re.finditer(r"\bfn\s+([a-z_][a-z0-9_]*)\s*[(<]", read(f)):
                fns.add(m.group(1))
    disp: dict[str, set[int]] = {}
    for sub in SUBSYS:
        txt = read(ROOT / "tos-api" / "src" / sub / "mod.rs")
        disp[sub] = {int(x, 16) for x in re.findall(r"0x([0-9A-Fa-f]+)", txt)}
    return fns, disp


def xbios_triage_stubs():
    """XBIOS _triage.md: numbers flagged stub / 🟡 (out-of-scope, no hardware)."""
    stubs: set[int] = set()
    for line in read(API / "xbios" / "_triage.md").splitlines():
        m = re.match(r"\|\s*0x([0-9A-Fa-f]+)\s*\|", line)
        if m and ("🟡" in line or re.search(r"\bstub\b", line, re.IGNORECASE)):
            stubs.add(int(m.group(1), 16))
    return stubs


def spec_functions(sub: str):
    base = API / sub
    if not base.exists():
        return
    for f in sorted(base.rglob("*.md")):
        if f.stem.startswith("_"):
            continue
        m = re.match(r"^([0-9a-fA-F]{1,3})-(.+)$", f.stem)
        if not m:
            continue
        yield int(m.group(1), 16), m.group(2), re.sub(r"[^a-z0-9_]", "", m.group(2).lower()), f


def main():
    fns, disp = build_impl_index()
    xstubs = xbios_triage_stubs()
    grand = {"impl": 0, "open": 0, "disc": 0}
    disc_all = []
    for sub, phase in SUBSYS.items():
        impl, opn, disc = [], [], []
        for num, name, handler, docfile in spec_functions(sub):
            wired = (num in disp.get(sub, set())
                     or handler in fns
                     or ALIAS.get(handler, "\0") in fns)
            if wired:
                impl.append(name)
            elif (sub == "xbios" and num in xstubs) or OOS_DOC.search(read(docfile)):
                opn.append(name)
            else:
                disc.append((f"0x{num:02X}", name))
                disc_all.append((sub, f"0x{num:02X}", name))
        grand["impl"] += len(impl); grand["open"] += len(opn); grand["disc"] += len(disc)
        print(f"\n### {sub.upper()} ({phase}) — "
              f"{len(impl)} implementiert · {len(opn)} offen(doku) · {len(disc)} DISKREPANZ")
        if opn:
            print(f"    offen (out-of-scope/stub/deferred laut Spec): {', '.join(sorted(opn))}")
        if disc:
            print("    ⚠ DISKREPANZ (spec'd, Phase done, kein Handler, nicht als out-of-scope markiert):")
            for hexn, name in disc:
                print(f"        {hexn}  {name}")

    print("\n" + "=" * 64)
    print(f"GESAMT: {grand['impl']} implementiert · {grand['open']} offen(doku) · "
          f"{grand['disc']} DISKREPANZ")
    print("\nHinweis: name-basiert — jede DISKREPANZ vor dem Nachkorrigieren am "
          "Dispatcher/Spec verifizieren (Handler evtl. anders benannt).")
    return 1 if disc_all else 0


if __name__ == "__main__":
    sys.exit(main())
