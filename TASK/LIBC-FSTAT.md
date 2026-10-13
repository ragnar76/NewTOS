# LIBC-FSTAT — `fstat` meldet fds 0–2 auch bei Umlenkung als Zeichengeraet

Status: **OFFEN** · Prioritaet: **P3** · Typ: SDK / libc — Anwenderentscheidung 2026-10-13 ([LIBC-TTY](LIBC-TTY.md) Punkt 4)

## Sachverhalt

`sdk/libc/newtos-arch/do_fstat.c:76-79` setzt fuer fds 0–2 immer `S_IFCHR`,
auch wenn sie per `Fforce` auf eine Datei zeigen. Upstream-Verhalten auf TOS
und eine passende Erkennung (z. B. ueber den `isatty`-Status) belegen, dann
korrigieren. Analyse: `docs/analysis/libc-tty-2026-10-13.md`.

[Gemeinsame Regeln](README.md) gelten.
