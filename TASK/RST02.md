# RST02 — Warmstart EmuTOS-treu: Kern-Neueinstieg mit erhaltenen Systemvariablen

Status: **OFFEN** · Prioritaet: **P3** · Typ: Abweichung von EmuTOS — Rest aus [RST01](RST01.md) (Anwenderentscheidung 2026-10-17)

RST01 macht Ctrl+Alt+Del vorerst zum echten Pi-Neustart (wie kalt). EmuTOS'
`warm_reset` ist `jmp _main` mit gueltigen `memvalid`/`memval2`/`memval3`:
Systemvariablen (`resvalid`/`resvector`, `phystop`, `bootdev` …) bleiben, nur
BSS, die ersten 64 Vektoren und Treiber werden neu aufgesetzt, FIRST_BOOT-
Zweige uebersprungen (`bios/panicasm.S:102-107`, `bios/memory.S:117-152,567-616`,
`bios/bios.c:482,492,798,806`; Spec `docs/api/bios/_reset-warm-cold.md`).

Auftrag: Kern-Neueinstieg ohne Firmware (MMU/Caches/Geraete sauber neu),
Systemvariablen-Erhalt nach der Spec, `resvector`-Aufruf; Test in QEMU und auf
der Pi.

[Gemeinsame Regeln](README.md) gelten.
