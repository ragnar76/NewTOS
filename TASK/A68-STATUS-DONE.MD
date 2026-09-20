# A68 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen

Der Textzweig von `just_draw` (`tos-api/src/aes/object.rs`) holt `th` jetzt
ueber `ObjTree::sst_th(obj)` — die eine Stelle mit EmuTOS' Faltung
`if (th > 128) th -= 256` (`aes/gemobjop.c:68-69`), die der Kastenzweig und
`form_center` schon benutzten — statt `ted.te_thickness` roh zu lesen. Ein
`te_thickness = 255` ist damit ein 1-px-Aussenrand (EmuTOS holt `th` fuer
jeden Typ einmal ueber `ob_sst`, `aes/gemoblib.c:489`); `effect_th` (Einzug
des Bevels) ist ebenfalls der gefaltete Wert. Bearbeitet 2026-09-20 zusammen
mit A69/A70 (ein Paket, eine Messung), Branch `desk-rebuild`, Basis HEAD
`23c6dbe`, **nicht committet**. Spec (vom Lead, uncommitted, unveraendert):
`docs/api/aes/object-form.md` „3D handling in `just_draw`“ (a).

`tools/mkrsc.py` reicht das Feld ungefaltet durch (Spec (a), vom Researcher
belegt; `struct.unpack(">IIIhhhhhhhh")` → `TED_FMT`, keine Arithmetik) — 255
erreicht den Renderer. Kein ausgelieferter Baum traegt heute 255 (Kernelbaeume
`-1` als i16, TeraDesk `desktop.rsc` nur `-1`/`0`), daher keine sichtbare
Aenderung in den Screendumps.

## Tests: rot vor, gruen nach (`measurement-2026-09-20-a68-a70/host-tests/`)

`tos-api/tests/aes_object_3d_text_button.rs` (Rig von `aes_object_3d_box_border.rs`):

| Test | vor | nach |
|---|---|---|
| `a68_boxtext_te_thickness_255_is_a_one_pixel_outside_border` (`G_BOXTEXT`, kein 3D, hohlweiss REPLACE) | `y=100 ours \|##############################\| want \|__#........................#__\|` — 255-px-Innenrand, Schwarz bis y=354 | gruen |
| `a68_fl3dind_boxtext_te_thickness_255_draws_like_minus_one` (Referenzzeilen des A67-Tests fuer `-1`) | `y=98 ours \|##############################\| want \|#...........................l#\|` | gruen |

`cargo run -p xtask -- test` 680 → 688 / 0 / 1; `cargo xtask build --board
rpi4` rustc tos-api 64 / kernel 94 (Delta 0); Clippy ohne neuen Treffer.

## QEMU raspi4b

Kernel-Desktop (Idle, Desk-Menue, Desktop-info-`form_alert`) und TeraDesk per
`#Z` (sechs Zustaende bis zum Alarm) vorher (HEAD-`object.rs`) gegen nachher:
**alle neun Paare 0 Pixel Differenz**, 0 `EXC`/`PANIC`. Einzelheiten, Hashes,
Methode: `measurement-2026-09-20-a68-a70/RUNINFO.TXT`.

## Offen

* Pi-Sichtabnahme (Tester/Lead; nichts geflasht).
* Nebenbefund, nicht angefasst: `gsx_tblt` schreibt Glyphen immer transparent;
  EmuTOS' `MD_REPLACE`-Zellhintergrund hat bei uns kein Gegenstueck.
