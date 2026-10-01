# A70 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen

Im Kastenzweig von `just_draw` (`tos-api/src/aes/object.rs`) bekommt ein
hohlweisser `G_BUTTON` die solide 3D-Flaechenfarbe nur noch unter `FL3DOBJ`
(`d3obj`: FL3DIND → `indbutcol`, FL3DACT → `actbutcol`); ein FL3DBAK-Knopf
bleibt `IP_HOLLOW`/`WHITE` mit schwarzem Rand — EmuTOS `case G_BUTTON:`
(`aes/gemoblib.c:639-658`, `if (flags & FL3DOBJ) … else { IP_HOLLOW; WHITE }`;
FL3DBAK 0x0400 traegt das FL3DOBJ-Bit 0x0200 nicht, D-103). `G_BOX`/`G_IBOX`/
`G_BOXCHAR` behalten die bisherige Abbildung FL3DBAK → `backgrcol`
(`:619-636`). Bearbeitet 2026-09-20 mit A68/A69, Branch `desk-rebuild`, Basis
HEAD `23c6dbe`, **nicht committet**. Spec (Lead, uncommitted, unveraendert):
`object-form.md` Punkt 2 + „3D handling in `just_draw`“ (c), `_discrepancies.md` D-103.

## Sichtbare Aenderung: keine — belegt, nicht angenommen

Kein ausgelieferter Baum traegt einen FL3DBAK-`G_BUTTON`: `form_alert` und
`fsel` setzen FL3DACT (`gemfmalt.c`, `gemfslib.c:73-146`), die desk/-Dialoge
setzen keine 3D-Flags, TeraDesks `desktop.rsc` hat 180 `G_BUTTON`s und 0 mit
FL3DBAK (Scan in `measurements/measurement-2026-09-20-a68-a70/RUNINFO.TXT`). A70 aendert
heute keinen Dialog; die Screendumps (Kernel-Desktop-Alarm, TeraDesk-Alarm
„Drive does not exist.“ mit FL3DACT-OK) sind vorher/nachher **0 Pixel**
verschieden.

## Tests: rot vor, gruen nach (`measurements/measurement-2026-09-20-a68-a70/host-tests/`)

`tos-api/tests/aes_object_3d_text_button.rs`:

| Test | vor | nach |
|---|---|---|
| `a70_fl3dbak_g_button_stays_hollow_white_with_a_black_border` (`th = -1`) | `y=100..115 ours \|__#llllllllllllllllllllllll#__\| want \|__#........................#__\|` — Innen `backgrcol` | gruen |
| `a70_fl3dact_g_button_takes_actbutcol` (`actbutcol` per `objc_sysvar` auf RED, danach zurueck; Ring 217/246 × 97/118, Bevel) | gruen | gruen |
| `a70_fl3dbak_g_box_still_takes_backgrcol` (`ob_spec 0x00ff_1100`) | gruen | gruen |

`xtask test` 680 → 688 / 0 / 1; rpi4 rustc 64/94 (Delta 0); Clippy ohne neuen Treffer.

## Offen

* Pi-Sichtabnahme (Tester/Lead; nichts geflasht).
* Der SELECTED-XOR-Pfad (`changecol`-Block, `:681-690`) ist unberuehrt — Ticket A85.
