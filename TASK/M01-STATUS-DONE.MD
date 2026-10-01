# M01 — Quittung

**Status: DONE** · 2026-09-09 · Aktuellen Pi-Fensterlauf archivieren

## Auftrag

Nur `aes-r2-wind.tsv` auf dem aktuellen Pi messen, vorhandene r1-Dateien vorher
unter neuer Laufkennung sichern. Keine Fensterreparatur.

## Was ich gemacht habe

1. Karte `/Volumes/BOOT` gegen `RUNINFO.TXT` per SHA-256 geprüft — Kernel
   `2fbf8462…`, `DUMPAES.NAP` `5889e9bb…`, beide identisch. Nicht über
   Dateinamen, sondern über Hashes.
2. Die r1-Tabelle als `AES-R1.TSV` auf der Karte gesichert, dann `AES.TSV` durch
   `AES-R2.TSV` (`5419a709…` = `cases/aes-r2-wind.tsv`) ersetzt. Der Dumper
   liest nur den festen Namen, daher der Tabellentausch.
3. Durchgang-1-Ergebnisse auf der Karte gegen `run1-pi4/` verglichen: alle sechs
   Dateien bitgleich, nichts ging beim Überschreiben verloren.
4. Lauf auf realer Pi 4 durch den Anwender; Ergebnis abgeholt.
5. Nach `measurements/measurement-2026-09-09/run2-pi4-r2wind/` archiviert, getrennt von r1.
6. Validiert und gegen die TT-Referenz verglichen.

## Artefakte

- `measurements/measurement-2026-09-09/run2-pi4-r2wind/AES.BIN` — 78 Records, 9392 Bytes,
  SHA-256 `680c03b9…`
- `measurements/measurement-2026-09-09/run2-pi4-r2wind/AES.LOG` — SHA-256 `d18c4e32…`
- `measurements/measurement-2026-09-09/diff/03-validate-lauf2.txt`
- `measurements/measurement-2026-09-09/diff/04-aes-r2wind-full.txt`
- `measurements/measurement-2026-09-09/diff/05-aes-r2wind-ohne-globals.txt`

## Ausgeführte Prüfung

`tosdiff --validate run2-pi4-r2wind/AES.BIN cases/aes-r2-wind.tsv` → accepted,
78 Records à 120 Byte, `build 70ca`, CRC `AFB11C9F`.

## Ergebnis

Abnahme erfüllt. Roh 78/78 abweichend (die sechs `global[]`-Wörter je Fall),
nach deren Trennung **2 von 78**: Fälle 244 und 247, beide `WF_OWNER`. Klasse
A05, hier nicht entschieden.

Vorhersage widerlegt: die `NUM_WIN`-Randfälle weichen **nicht** ab.
`MAX_WINDOWS` steht auf 15 (`tos-api/src/aes/window.rs:210`), gesetzt am
06.09. in `752b988`. Vorhersage in `MESSRUN.TXT` und Fallkommentar in
`aes-r2-wind.tsv:105-108` sind veraltet, nicht der Lauf.

## Offen

Keine Hardwareabnahme offen. A10, A11, A12 sind entsperrt.
