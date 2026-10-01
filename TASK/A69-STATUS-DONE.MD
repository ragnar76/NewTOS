# A69 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen

Im Textzweig von `just_draw` (`tos-api/src/aes/object.rs`) bleibt die
Ueberschreibung `ipat = IP_SOLID; icol = backgrcol` fuer einen hohlweissen
TEDINFO unter jedem 3D-Flag unbedingt (`aes/gemoblib.c:582-585`); die
Befoerderung `G_TEXT→G_BOXTEXT` / `G_FTEXT→G_FBOXTEXT` mit `th = 0` haengt
jetzt am `MD_REPLACE`-Gatter (`if (tmode == MD_REPLACE)`, `:586-599`) —
`gr_crack`s Bit 7 (`mode_replace`), das der Zweig bisher als `_mode`
verwarf. Folgen wie bei EmuTOS: ein TRANSPARENT-`G_TEXT` bleibt Text ohne
Kasten (der zweite Typ-Switch hat kein `case G_TEXT`, `:609-696`), der Grund
neben den Glyphen bleibt sichtbar; ein TRANSPARENT-`G_BOXTEXT` wird
`backgrcol` gefuellt und behaelt seinen Rand. Bearbeitet 2026-09-20 mit
A68/A70, Branch `desk-rebuild`, Basis HEAD `23c6dbe`, **nicht committet**.
Spec (Lead, uncommitted, unveraendert): `object-form.md` „3D handling in
`just_draw`“ (b).

## Tests: rot vor, gruen nach (`measurements/measurement-2026-09-20-a68-a70/host-tests/`)

`tos-api/tests/aes_object_3d_text_button.rs`:

| Test | vor | nach |
|---|---|---|
| `a69_transparent_fl3dbak_g_text_stays_unboxed_over_the_background` (`G_TEXT` FL3DBAK, `te_color 0x1100`, Glyph `X` ueber Schiefer) | `y=101 ours \|___llllllllllllllllllllllll___\| want \|___********___________________\|` — `backgrcol`-Flaeche neben dem Glyph | gruen |
| `a69_replace_fl3dbak_g_text_is_promoted_and_filled_backgrcol_without_border` (`0x1180`, `te_thickness -1`: befoerdert, `th` genullt) | gruen | gruen |
| `a69_transparent_fl3dbak_g_boxtext_is_filled_backgrcol_and_keeps_its_border` (`0x1100`, `-1`) | gruen | gruen |

`xtask test` 680 → 688 / 0 / 1; rpi4 rustc 64/94 (Delta 0); Clippy ohne neuen Treffer.

## QEMU raspi4b

TeraDesks „Disk Volume Information“-Dialog (per Injektor geoeffnet) traegt
genau den Fall: FL3DBAK-`G_FTEXT`-Beschriftungen mit TRANSPARENT-hohlweissem
`te_color` (`desktop.rsc`: 7 solche TEDINFOs, 76 FL3DBAK-Textobjekte).
Vorher/nachher **0 Pixel Differenz** — erwartet, weil die Beschriftungen auf
einer `backgrcol`-Tafel liegen und die alte Fuellung dieselbe Farbe hatte;
sichtbar wird A69 erst ueber einem andersfarbigen/gemusterten Grund (Hosttest).
Kernel-Desktop-Dialoge (`dlg.rs`/`inf.rs`/`insapp.rs`: `te_color 0x0100`, kein
3D-Flag) und `form_alert` bleiben bytegleich. `measurements/measurement-2026-09-20-a68-a70/RUNINFO.TXT`.

## Offen

* Pi-Sichtabnahme (Tester/Lead; nichts geflasht).
* Nebenbefund, nicht angefasst: EmuTOS' `tmode = MD_TRANS`-Umschaltung im
  selben Block hat kein Gegenstueck — `gsx_tblt` schreibt immer transparent
  (der `MD_REPLACE`-Zellhintergrund fehlt generell, vor wie nach A69).
