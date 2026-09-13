# M03 — Quittung

**Status: DONE** · 2026-09-09 · Rechtecklisten auf aktuellem Pi aufnehmen

## Auftrag

Dieselbe Tabelle wie M02 auf dem aktuellen Pi ausführen, separat von M01/r1
archivieren.

## Was ich gemacht habe

1. Erst nach der Archivierung von M01 die Pi-Karte umgestellt: `AES-R2B.TSV`
   über `AES.TSV` (`a81a7a6c…`), damit Durchgang 3 den Lauf 2 nicht
   überschreibt. `AES-R1.TSV`, `AES-R2.TSV`, `AES-R2B.TSV` blieben daneben
   erhalten.
2. Lauf auf realer Pi 4 durch den Anwender; Ergebnis abgeholt.
3. Nach `measurement-2026-09-09/run3-pi4-rlist/` archiviert.
4. Validiert und gegen die frische TT-Referenz aus M02 verglichen.
5. Poison-Muster geprüft, um den Listenende-Befund nicht falsch zu lesen.

## Artefakte

- `measurement-2026-09-09/run3-pi4-rlist/AES.BIN` — 15 Records, 1832 Bytes,
  SHA-256 `e0280b7d…`
- `measurement-2026-09-09/run3-pi4-rlist/AES.LOG` — SHA-256 `9dd6c3a1…`
- `measurement-2026-09-09/diff/06-aes-r2brlist-full.txt`
- `measurement-2026-09-09/diff/07-aes-r2brlist-ohne-globals.txt`

## Ausgeführte Prüfung

`tosdiff --validate run3-pi4-rlist/AES.BIN cases/aes-r2b-rlist.tsv` → accepted,
15 Records à 120 Byte, `build 70ca`, CRC `B4CC4A5A`. 1832 Bytes, also die in
M02 korrigierte Zahl.

## Ergebnis

Abnahme erfüllt. Nach Trennung der `global[]`-Wörter **10 von 15** Fällen
abweichend, in drei Gruppen — als Lesehilfe festgehalten, **nicht** bewertet:

- **Listenende (127, 133, 144, 204):** TT liefert an `int_out[1]`/`[2]` Werte
  ungleich 0 (31/63, bei 204 die 19), Pi jeweils 0. Genau das Feld, auf das
  `MESSRUN.TXT` zeigt: `w_owns` (gemwmlib.c:1229-1240) setzt am Listenende nur
  `g_w`/`g_h` auf 0. Wichtig: 31 und 63 sind **nicht** das Poison-Muster — das
  ist `0xDEAD` (`tosdump/core/tosdump.h:78`). Der TT hat sie geschrieben.
- **Bildschirmgeometrie (201, 208):** 640/1920, 461/1061. Klasse A01–A04.
- **Rechteckzerlegung (166, 174, 183, 213):** `newrect`/`brkrct`
  (gemwrect.c:101-205), unterschiedliche Zerlegung bei 1920×1080 gegen 640×480.

## Offen

Keine Hardwareabnahme offen. Die Bewertung gehört zu A13, A14 und A15 — die
sind durch M02 + M03 entsperrt. Der Listenende-Befund ist der einzige aus dieser
Runde, der nach echter Abweichung aussieht.
