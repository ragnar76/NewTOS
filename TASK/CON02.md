# CON02 — `Cconout`/`Cconws` folgen nicht der Umlenkung von Handle 1

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Nebenfund LIBC-FSTAT (2026-10-17)

NewTOS-`Cconws` schreibt immer auf CON, auch wenn Handle 1 per `Fforce`
umgelenkt ist (`measurements/measurement-2026-10-17-libc-fstat/`). EmuTOS
leitet die Konsolenfunktionen ueber die Standard-Handles des Prozesses:
`run->p_uft[1]` fuer Ausgabe, `run->p_uft[0]` fuer Eingabe
(`bdos/console.c:161,170,281,377,401`). Researcher: genaues Verhalten belegen
(Ziel Geraet vs. Datei, was `HXFORM` bei einem Datei-Handle tut, welche
C-Funktionen betroffen sind), Spec `docs/api/gemdos/console/*` ergaenzen,
dann Developer. Beruehrt FS11 (Geraetetabelle) und CON01.

[Gemeinsame Regeln](README.md) gelten.
