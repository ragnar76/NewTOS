# MEM06 — Heap als reservierter virtueller Bereich, Speicher beim ersten Zugriff

Status: **IDEE** (Anwender 2026-10-13, aus MEM05) · Prioritaet: **P3** · Typ: Speicherverwaltung — Richtungsnotiz

Abhaengigkeiten: [MEM05](MEM05.md)

## Idee

Jeder Prozess bekommt einen grossen reservierten virtuellen Bereich; echter
Speicher wird erst beim ersten Zugriff (Seitenfehler) zugeteilt. Dann kostet
auch das alte TOS-Muster `Malloc(-1)` → alles holen → `Mshrink` nichts mehr,
und ein Programm, das allen Speicher greift, hungert andere nicht aus.

## Offen

Hat NewTOS die Grundlage fuer seitenfehlergesteuerte Zuteilung (ARCH03,
eigene Adressraeume)? Wie verhaelt sich `Malloc(-1)`/`Mxalloc(-1)` dann
(groesster freier Block = Reserve oder echter Speicher)? Wechselwirkung mit
Speicherschutz und spaeterem m68k-Emulator (24-Bit-Adressraum). Zuerst
Researcher, kein Auftrag ohne Freigabe.

[Gemeinsame Regeln](README.md) gelten.
