#!/usr/bin/env python3
"""Build a native NewTOS ``.KBD`` keyboard-layout file from an EmuTOS
``bios/keyb_<lang>.h`` table.

The native ``.KBD`` format (little-endian bytes) matches
``kernel/src/keymap.rs`` ``load_kbd``::

    magic "KBD1"                      (4 bytes)
    norm[128] shft[128] caps[128]     (3 x 128)
    altnorm_n (1), altnorm_n x (scancode, ascii)
    altshft_n (1), altshft_n x (scancode, ascii)
    altcaps_n (1), altcaps_n x (scancode, ascii)

Usage::

    tools/mkkbd.py de  > german.kbd          # parse references/emutos/bios/keyb_de.h
    tools/mkkbd.py us  > us.kbd
    tools/mkkbd.py de --emutos /path/to/emutos > german.kbd

Copy the result to the SD card as ``C:\\KEYBOARD.KBD`` to override the
built-in layout, or add a ``#K de`` line to ``NEWDESK.INF`` to pick a
built-in one instead.
"""
import re
import sys
from pathlib import Path

MAGIC = b"KBD1"


def _tokenize(body: str):
    toks, i, n = [], 0, len(body)
    while i < n:
        c = body[i]
        if c.isspace() or c == ",":
            i += 1
            continue
        if c == "/" and i + 1 < n and body[i + 1] == "*":
            i = body.find("*/", i) + 2
            continue
        if c == "'":
            tok = body[i:i + 4] if body[i + 1] == "\\" else body[i:i + 3]
            toks.append(tok)
            i += len(tok)
            continue
        j = i
        while j < n and not body[j].isspace() and body[j] != ",":
            j += 1
        toks.append(body[i:j])
        i = j
    return toks


def _val(tok: str) -> int:
    tok = tok.strip()
    if tok.lower().startswith("0x"):
        return int(tok, 16)
    if re.fullmatch(r"\d+", tok):
        return int(tok)
    if tok[0] == "'" and tok[-1] == "'":
        inner = tok[1:-1]
        if inner and inner[0] == "\\":
            return {"'": 0x27, '"': 0x22, "\\": 0x5C, "n": 0x0A,
                    "t": 0x09, "r": 0x0D, "0": 0}[inner[1]]
        return ord(inner)
    raise ValueError(f"unparsable token {tok!r}")


def _table(text: str, name: str):
    m = re.search(name + r"\[\]\s*=\s*\{(.*?)\};", text, re.S)
    if not m:
        raise SystemExit(f"table {name} not found")
    return [_val(t) for t in _tokenize(m.group(1))]


def build(lang: str, emutos: Path) -> bytes:
    hdr = emutos / "bios" / f"keyb_{lang}.h"
    text = hdr.read_text(encoding="utf-8", errors="replace")
    pre = f"keytbl_{lang}"

    out = bytearray(MAGIC)
    for sub in ("norm", "shft", "caps"):
        tbl = _table(text, f"{pre}_{sub}")
        if len(tbl) != 128:
            raise SystemExit(f"{lang} {sub}: expected 128 entries, got {len(tbl)}")
        out += bytes(v & 0xFF for v in tbl)

    for sub in ("altnorm", "altshft", "altcaps"):
        pairs = _table(text, f"{pre}_{sub}")
        while pairs and pairs[-1] == 0:  # drop the terminating 0
            pairs.pop()
        if len(pairs) % 2 != 0:
            raise SystemExit(f"{lang} {sub}: odd pair count")
        n = len(pairs) // 2
        out.append(n)
        out += bytes(v & 0xFF for v in pairs)
    return bytes(out)


def main() -> None:
    args = sys.argv[1:]
    if not args:
        raise SystemExit(__doc__)
    lang = args[0]
    emutos = Path("references/emutos")
    if "--emutos" in args:
        emutos = Path(args[args.index("--emutos") + 1])
    blob = build(lang, emutos)
    sys.stdout.buffer.write(blob)


if __name__ == "__main__":
    main()
