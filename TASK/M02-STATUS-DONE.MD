# M02 — Quittung

**Status: DONE** · 2026-09-09 · Rechtecklisten-Referenz auf Atari TT aufnehmen

## Auftrag

Nur die vorhandene `aes-r2b-rlist.tsv` mit dem passenden AES-Dumper auf dem TT
ausführen. Keine Änderung der eingefrorenen Tabelle.

## Was ich gemacht habe

1. CF-Karte untersucht: aktiver Arbeitsordner ist `/Volumes/TOOLS/TOSDUMP`
   (`DUMPAES.PRG` `282bf1a9…`, identisch mit `tosdump/deploy-tt/`), nicht der
   Altstand `/Volumes/TOOLS/deploy-tt` vom 31.08.
2. Festgestellt, dass `AES-R2B.TSV` auf der ganzen Karte fehlte.
3. `tosdump/deploy-tt/AES-R2B.TSV` (`a81a7a6c…`) auf die Karte kopiert und
   zusätzlich als `AES.TSV` gesetzt — byteweise dieselbe Datei wie auf der
   Pi-Karte. Die eingefrorene Tabelle blieb unverändert.
4. Lauf auf dem Atari TT durch den Anwender; Ergebnis abgeholt.
5. Als neue Referenz abgelegt, ohne eine bestehende zu überschreiben.

## Artefakte

- `tosdump/reference/EMUTOS14-AES-r2brlist.BIN` — 15 Records, 1832 Bytes,
  SHA-256 `e7ef0a27…`
- `tosdump/reference/EMUTOS14-AES-r2brlist.LOG` — SHA-256 `2a4b1b7c…`
- Arbeitskopien in `measurements/measurement-2026-09-09/reference-tt/`
- Validierung in `measurements/measurement-2026-09-09/diff/03-validate-lauf2.txt`

## Ausgeführte Prüfung

`tosdiff --validate` → accepted, 15 Records à 120 Byte, `build c911`, CRC
`B4CC4A5A`. LOG: `ap_version 0340`, `ap_id 0`, `ap_planes 4`, „all 15 cases
ran". 46 Preludes, jeder NEXT-Fall behält seinen FIRST-Vorlauf.

## Ergebnis

Abnahme erfüllt — mit einer Zahlkorrektur: gemessen sind **1832 Bytes**, nicht
die in `MESSRUN.TXT` und in M03 genannten 1880. 1832 = 15 × 120 + 32 Byte Kopf;
Gegenprobe r1 (11432) und r2wind (9392). 1880 − 32 = 1848 ist kein Vielfaches
von 120, also ein Rechenfehler in der Vorlage. M03 korrigiert, `MESSRUN.TXT` als
Laufbeleg unverändert gelassen.

Erste EmuTOS-Referenz für diese Tabelle überhaupt.

## Offen

Keine Hardwareabnahme offen.
