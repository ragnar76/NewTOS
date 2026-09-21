# V01, V02, V04–V10, V13–V15 — Quittung (2026-09-21)

**Was getan wurde:** Auditor-Messung `measurement-2026-09-21-vdi-qemu/` —
drei frische Dumps der Tabelle `tosdump/cases/vdi-all.tsv` (CRC `D135ADAF`):
EmuTOS 1.4 auf Hatari-TT (bitgleich zur echten TT-Referenz), EmuTOS 1.4 auf
Hatari-Falcon030 in VIDEL-Truecolor (Release-ROM und gepinnter Quellstand:
0/182 verschieden), NewTOS auf QEMU raspi4b 640x480.

**Ergebnis:** EmuTOS selbst weicht zwischen TT und Falcon-TC in exakt den 13
Zellen ab, die diese Tickets nennen — Geraeteparameter nach EmuTOS' eigener
Arithmetik, je mit Quellzeile (`docs/analysis/vdi-v-series-classification-2026-09-21.md`
§ 1/§ 4). Gegen die Falcon-TC-Referenz blieben 3 Faelle = ein Defekt
(V11/V12, OVERLAY_BIT), in `2d6b3cc` behoben: danach 0/182.

**Artefakte:** `tosdump/reference/EMUTOS14-VDI-all-falcontc.{BIN,LOG}` (neu),
`tos-api/tests/vdi_reference/cases_v_series.rs` (15 Tests gruen),
`measurement-2026-09-21-vdi-qemu/` (Dumps, fuenf `tosdiff`-Laeufe, Zellen-
und Handle-Auswertung, Methode `run-falcon-tc.sh`).

**Offen:** echte Pi 4 (1920x1080): die Geometriezellen (V01, V06, V13, V14)
folgen derselben Formel (1919/1079, 240/67) — Formel getestet, Zahl seit dem
Pi-Lauf vom 2026-09-09 nicht neu gemessen.
