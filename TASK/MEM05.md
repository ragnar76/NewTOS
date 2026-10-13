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

## Entscheidung (Anwender 2026-10-13)

Analyse: `docs/analysis/mem05-md-pool-2026-10-13.md`. Grundsatz des
Anwenders: NewTOS laeuft auf groesserer Hardware mit viel mehr Speicher,
interne Grenzen duerfen sich daran anpassen.

1. **Kernel:** kein fester MD-Pool mehr. Der Vorrat waechst seitenweise nach
   Bedarf; statt einer systemweiten Grenze gilt eine grosszuegige Grenze je
   Prozess (bei Erreichen `Malloc` → 0 wie auf TOS); MD-Freigabe in O(1).
   Die an `MD_BLOCK_COUNT` gekoppelten Schutztabellen (`memprot.rs`,
   `region.rs`) wachsen mit.
2. **SDK/mintlib:** Chunkgroesse verdoppelt weiter bis z. B. 16 MB statt bei
   32 KiB stehenzubleiben (kleine Programme bleiben klein); dafuer eine
   kleine dokumentierte Overlay-Funktion, die nur `MAXHUNK` setzt.
3. `docs/COMPATIBILITY.md`: Eintrag „keine feste MD-Grenze“ als erlaubte
   Verbesserung.
4. Reservierter virtueller Heap mit Zuteilung beim ersten Zugriff → eigenes
   Ticket [MEM06](MEM06.md).

Naechster Schritt: Researcher-Entwurf (Tabellenwachstum, Prozessgrenze und
wo sie eingestellt wird, SDK-Anpassung, Tests) zur Freigabe, dann Developer.
