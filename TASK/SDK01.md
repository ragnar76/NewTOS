# SDK01 — SDK fuer Fremde benutzbar machen: libc-Luecken und Portierungs-Doku

Status: **IN ARBEIT** · Prioritaet: **P2** · Typ: SDK / Doku — Anwender 2026-10-20 („es gibt Leute, die Apps portieren wollen, dazu braucht es eine Doku“; GitHub-Issue #4)

Grundlage: `docs/analysis/sdk-inventory-2026-10-20.md` (`a36b3280`).

## Pakete (Reihenfolge)

| # | Paket | Groesse | Stand |
|---|---|---|---|
| A | libc-Luecken: `pwdgrp/` in `DIRS` (tmpfile/mkstemp/glob/getlogin …), `__sys_fstat` (ttyname/ctermid), `SIZE_MAX`/`PTRDIFF_MAX`/`SSIZE_MAX` fuer LP64, `ucontext.c`-Testprogramm und Generator-Objekte aus `libc.a`, `.stabs`-Dateien (gets/getwd/siggetmask), `newtos-arch/nf_ops.c` — dazu `tmpfile()` auf NewTOS in QEMU pruefen | S–M | offen |
| B | Doku ehrlich machen: Patch-Aussage („App-Quellen unberuehrt“) in `docs/sdk/README.md`, Kap. 04, 08, 09 §5, `sdk/README.md`, Kopf von `build-qed.sh`/`build-teradesk.sh`; ~12 veraltete Fakten (08, 09, Zahlen in 01/04/07, TeraDesk-`main.c`-Overlay fehlt) | S–M | offen |
| C | Schnellstart Mac **und** Linux von null: Installation, Bezugsquellen der App-Quellen (freemint/qed `f12055f`, freemint/teradesk `b5532b8`), eigenes Programm ohne Kernelbau auf ein Release-Abbild, Liste nicht linkender Funktionen | M | offen |
| D | Compiler-Wrapper `newtos-cc` bzw. `newtos.mk` + Vorlagenprojekt | M | offen |
| E | libm: compiler-rt-Builtins fuer 128-Bit-Gleitkomma und komplexe Multiplikation (`powf`, `remainder` …) | M | offen |
| F | `stat`/`fstat`-Zeitstempel aus der DTA wie Upstream-mintlib | M | offen |
| G | Linux-Weg fuer das SD-Abbild (`make-sd-image.sh` nur macOS) | M | offen |
| — | spaeter/bei Bedarf: `-Wno-error=int-conversion`-Entscheidung, gemeinsamer LP64-Header, socket/crypt/shadow/argp, C++-Laufzeit | — | — |

[Gemeinsame Regeln](README.md) gelten.
