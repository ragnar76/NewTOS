# LIBC-BUF — erster `printf` auf ungepuffertes stdout legt nur 1 KiB Puffer an

Status: **OFFEN** · Prioritaet: **P3** · Typ: SDK / libc — Nebenfund LIBC-FSTAT (2026-10-17)

Beobachtet in der FSTATP-Sonde: vor der ersten Ausgabe ist
`stdout->__bufsize` 32 KiB; der erste `printf` auf den noch ungepufferten
Strom setzt `__bufsize` auf 0, der danach angelegte Puffer ist 1 KiB — auf der
Konsole wie bei einer Datei. Upstream-stdio-Pfad. Pruefen, ob dasselbe auch
`fopen`-Dateien trifft und damit die FS08-Regel (32 KiB fuer Dateien,
`0abc634`) unterlaeuft; dann entscheiden.

[Gemeinsame Regeln](README.md) gelten.
