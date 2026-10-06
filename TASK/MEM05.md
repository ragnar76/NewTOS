# MEM05 — viele kleine `malloc` erschoepfen die 254 Speicherdeskriptoren systemweit

Status: **OFFEN** · Prioritaet: **P2** · Typ: Grenzwert — Nebenfund der E9-Nacharbeit ([ARCH03](ARCH03.md))

## Sachverhalt

Seit E9 holt die libc Speicher per GEMDOS `Malloc` (wie mintlib auf TOS).
MD-Sonde (`measurements/measurement-2026-10-06-post-e9/`): 254 freie MDs am
Start; `malloc(32)` × 10 000 belegt 19 MDs, × 100 000 173; nach 147 605
Bloecken (4,7 MB) liefert `malloc` NULL, alle MDs systemweit weg, ~930 MB frei.

## Arbeit

Zuerst belegen, wie EmuTOS MDs verwaltet (`bdos/osmem.c`, Pool-Groesse,
dynamisches Nachfassen) und wie mintlib unter TOS Bloecke anfordert
(`_heapbase`/MINHUNK); dann angleichen. Nicht umbauen ohne diesen Beleg.
