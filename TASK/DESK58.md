# DESK58 — `BLKDEVNUM` im Desktop ist 16 statt 26

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — gefunden in der DESK43-Pruefung, gelistet von U5a (2026-10-14)

`kernel/src/desk/main.rs:1471` setzt `BLKDEVNUM = 16` und zitiert dafuer
`bdosdefs.h`; EmuTOS hat `#define BLKDEVNUM 26` (`include/sysconf.h:43`,
A: … Z:). Der Konstantenabgleich (`tools/xtask/emutos-consts-deviations.txt`)
fuehrt ihn als `defect`; nach der Korrektur muss der Eintrag dort entfallen
(der Hosttest erzwingt das). Vorher pruefen, welche Desktop-Pfade die Grenze
benutzen (`main.rs:1454`) und ob Laufwerke Q:–Z: dadurch sichtbar werden.

[Gemeinsame Regeln](README.md) gelten.
