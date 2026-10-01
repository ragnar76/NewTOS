# R02, R03, R04, R05 — Quittung (2026-09-21)

**Was getan wurde:** Auditor-Messdesign und -Messung
`measurements/measurement-2026-09-21-raster-screen/` (Auswertung
`docs/analysis/raster-screen-r02-r05-2026-09-21.md`): neue Falltabelle
`tosdump/cases/vdi-raster-screen.tsv` (62 Faelle, 41 Preludes, CRC `9780EC39`)
mit einem begrenzten Runner-Delta (`!screen w,h,planes`, `S` als Screen-MFDB,
Preludes, Readback per `v_get_pixel`; `tosdump/core/cases.h` „THE SCREEN",
Host-Suite gruen, alte Tabelle `vdi-raster.tsv` byte-identisch validiert).
Drei Dumps: EmuTOS 1.4 auf Hatari Falcon030 VIDEL-Truecolor 640x480/16 Planes
(Release-ROM und gepinnte Quelle, 0/62 verschieden; Referenz
`tosdump/reference/EMUTOS14-RASTER-screen-falcontc.BIN`) und NewTOS auf QEMU
raspi4b @ `c1f42ff`. Danach der Entwickler-Fix des einen Defekts und die
Gegenmessung `measurements/measurement-2026-09-21-r05/`.

**Ergebnis:** NewTOS stimmt mit EmuTOS auf dem Falcon in **61 von 62** Faellen
Wort fuer Wort ueberein (512 Woerter je Puffer, beide Puffer, beide MFDBs,
alle Arrays): Screen→Memory (R02, 15/15), Memory→Screen mit Readback (R03,
15/15), ueberlappend Screen→Screen in acht Richtungen plus XOR und auf sich
selbst (R04, 15/15), `vrt_cpyfm` Mono→Screen mit allen vier `MD_*`-Modi,
ungueltigen/erweiterten Stiften und 16-Plane-Speicherziel (R05, 16/17). Das
unabhaengige Modell der EmuTOS-Regeln (`method/oracle.py`) stimmt 62/62 mit
dem Falcon ueberein. Der eine Unterschied war der vorhergesagte Defekt
**D-VDI-23, `vrt`-Haelfte** (Fall 215, `INTIN[0] = 0x11` = `PAT_FLAG |
MD_REPLACE` auf dem 16-Bit-Pfad): EmuTOS zeichnet nichts (`cpy_raster` strippt
das Flag in sein lokales `mode`, `vdi/vdi_raster.c:1347`, ruft
`vrt_cpyfm16(info)` ohne es, `:1432-1437`; `vrt_cpyfm16` liest `INTIN[0]` neu,
`:1232`, `default: return`, `:1323-1324`), NewTOS zeichnete die
REPLACE-Expansion. **Behoben:** `tos-api/src/vdi/raster.rs::vrt_cpyfm` nimmt
denselben Fruehausstieg wie `vro_cpyfm` (Ziel-Planes > `PALETTE_PLANES_MAX`
und Bit 4 gesetzt → return, nichts gezeichnet, kein Speicherziel). Roter
Hosttest `case_0215_op121_h12_…` (61/1) → 62/62 gruen; `cargo run -p xtask --
test` 85 Binaere, 903 gruen / 0 rot / 1 ignored; `cargo xtask build --board
rpi4` tos-api 64 / kernel 94 Warnungen (unveraendert), clippy tos-api 80
(unveraendert). Gegenmessung: frischer `--release`-Kernel, gleicher Dumper
(`DUMPRAST.NAP` sha256 `5c48715c…`), gleiche Tabelle, QEMU raspi4b headless
per `#Z`-Autorun: `tosdiff` gegen den Falcon **0 von 62** (vorher 1), gegen
den gepinnten ROM-Dump 0/62, Modell 0/62, gegen den Vorher-Dump genau Fall 215.

**Spec:** `docs/api/vdi/raster/_raster.md` (Kasten „truecolor path", Edge
Case 10, Discrepancies 2026-09-06, Implementation notes) und
`docs/api/vdi/_discrepancies.md` D-VDI-23 tragen jetzt die gemessene Regel fuer
beide Opcodes („Unmeasured twin"/„Open half" gestrichen). Zeilenzitate auf
den gepinnten Stand `1a7484ab` berichtigt: `vro_cpyfm16:980` → `:981`,
`:993` → `:995` (`src < dst`), `:995-999` → `:995-1001`, `:1008-1219` →
`:1008-1220`, `vrt_cpyfm16:1236` → `:1232`, `:1233` → `:1234` (`palette`),
`:1325-1326` → `:1323-1324`, Dispatch opaque `:1363-1368` → `:1367-1372`;
im mitberuehrten D-VDI-25-Kasten von `_raster.md` (und Edge Case 12) ausserdem
`dont_clip:897-898` → `:898-899` (`d_xmax`/`d_ymax`) und `bit_blt` `:672-674`
→ `:674-675` (`d_ymax`). `:1347`, `:1341-1342`, `:1345-1349`, `:1360-1361`,
`:1384-1385`, `:1390-1395`, `:1432-1437`, `vdi_control.c:450-452` und die
uebrigen `vdi_raster.c`-Zitate in `_raster.md` gegen `1a7484ab` geprueft und
richtig. **Nicht angefasst** (nicht beruehrt, R01 bleibt zurueckgestellt):
die D-VDI-25-Zeile in `docs/api/vdi/_discrepancies.md` traegt noch
`:897-898`, `:995-999`, `:672-674`, `:993` — richtig waeren `:898-899`,
`:995-1001`, `:674-675`, `:995`; der Kommentar in `raster.rs::vro_cpyfm`
wurde mitberichtigt (`:980` → `:981`, `:1008-1219` → `:1008-1220`). Die
Analyse `docs/analysis/raster-screen-r02-r05-2026-09-21.md` zitiert bereits
die richtigen Zeilen.

**Artefakte:** `tosdump/cases/vdi-raster-screen.tsv`,
`tosdump/reference/EMUTOS14-RASTER-screen-falcontc.{BIN,LOG}`,
`tos-api/tests/vdi_raster_reference/cases_screen.rs` (Binaer
`vdi_reference_raster_screen`, 62 Tests), `measurements/measurement-2026-09-21-raster-screen/`
(Messdesign, drei Dumps, Diffs, `method/oracle.py`, `RUNINFO.TXT`),
`measurements/measurement-2026-09-21-r05/` (Gegenmessung: `RUNINFO.TXT`, Dump, drei
`tosdiff`-Laeufe, Modell, `manifest.sha256`; `newtos-qemu-raspi4b/RASTER.BIN`
ist per Wurzel-`.gitignore` `*.bin` ignoriert — `git add -f`; kein `*.img`
im Ordner), `docs/STATUS.md` (VDI-Zeile + Recent Decisions 2026-09-21),
HW-Checkliste Punkt 31 (`docs/api/desktop/_hw-test-2026-09-17.md`).

**Offen:** der Lauf auf der echten Pi 4 (1920x1080 besteht den
`!screen`-Guard; Erwartung Record fuer Record identisch zum QEMU-Dump; HW-
Checkliste 31). **R01 / D-VDI-25** bleibt zurueckgestellt und ist hier nicht
entschieden — seine Vorbedingung „erst die Screen-Pfade messen" ist mit dieser
Quittung erfuellt; die Entscheidung liegt beim Lead.
