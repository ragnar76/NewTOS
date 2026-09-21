# A98 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Abnahme offen

`wind_set(WF_HSLIDE/WF_VSLIDE/WF_HSLSIZ/WF_VSLSIZ)` zeichnet nur noch den
`W_HSLIDE`/`W_VSLIDE`-Teilbaum (Spur + Elevator), nicht mehr die Pfeile —
wie EmuTOS' `w_cpwalk(w_handle, W_?SLIDE, MAX_DEPTH, TRUE)`.
Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis HEAD `2ac0f9a`, **nicht
committet**, gemeinsam mit A93 in einem Paket.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemwmlib.c:1690-1716` setzen `gadget = W_HSLIDE`/`W_VSLIDE`, `:1754-1755` `w_cpwalk(w_handle, gadget, MAX_DEPTH, TRUE)`; `aes/gemoblib.c:959-977` `ob_draw`: `last = tree[obj].ob_next; everyobj(tree, obj, last, just_draw, …)` — nur der Teilbaum von `obj`. Baum: `w_bldvbar` haengt `W_UPARROW`/`W_DNARROW` (`:412-424`) und `W_VSLIDE` (`:427-430`) unter `W_VBAR`, nur `W_VELEV` unter `W_VSLIDE` (`:436`); `w_bldhbar` ebenso (`:450-462`, `:465-474`). |
| NewTOS (vorher) | `tos-api/src/aes/window.rs` `redraw_vslider`/`redraw_hslider` riefen `draw_vslider`/`draw_hslider`, die Pfeile + Spur + Elevator zeichnen: `WF_VSLIDE 500` schrieb 1035 von 1058 Pixeln der Up/Down-Zellen (identische Werte — deshalb pixelunbeobachtbar). |
| NewTOS (nachher) | `draw_?slider` = `draw_?arrows` + `draw_?slide`; `redraw_?slider` ruft `draw_?slide`. Kein Pfeilpixel geschrieben, Spur + Elevator vollstaendig. |

## Was gebaut ist

* **`tos-api/src/aes/window.rs`** — neu `draw_varrows`/`draw_harrows`
  (Pfeilzellen) und `draw_vslide`/`draw_hslide` (Spur + Elevator, der
  `W_?SLIDE`-Teilbaum); `draw_vslider`/`draw_hslider` rufen beide in
  `W_?BAR`-Kindreihenfolge (fuer `draw_frame` und die Drag-Rueckmeldung,
  unveraendertes Bild); `redraw_vslider`/`redraw_hslider` rufen nur
  `draw_?slide`. Doc-Kommentare mit `gemwmlib.c:403-438,441-476,1690-1755`,
  `gemoblib.c:959-977`. Die Zeichenlogik selbst ist unveraendert (nur
  aufgeteilt).
* **`tos-api/tests/aes_window_slider_subtree.rs`** (neu, 4 Faelle):
  A84-Harness (`CountingBackend`) plus **Schreibmaske pro Pixel** (`touched`,
  gesetzt in jedem `put` nach dem Clip und in `put_device_pixel`).
  Pfeilzellen (`"up"`/`"dn"` bzw. `"lf"`/`"rt"`) und Spur (`"vuppage"`/
  `"vdnpage"`/`"velev"` bzw. `"hlfpage"`/`"hrtpage"`/`"helev"`) ueber
  `gadget_name_at` bestimmt. Je Feld: die Aenderung zeichnet; **jedes**
  Spurpixel geschrieben; **kein** Pfeilpixel geschrieben; alle Pfeilpixel
  wertgleich (Pixelsonde vor/nach, zusaetzlich zur Maske); `wind_get` liest
  den Wert. Hinweis in der Datei zur geteilten Zeile/Spalte (EmuTOS
  `y+h+ADJ3DSTD-1` / `x+w-1+ADJ3DSTD`: der 3D-Slot des Down-/Right-Pfeils
  ueberlappt die letzte Spurzeile/-spalte um eine Linie, die zum
  `W_?SLIDE`-Rechteck gehoert; die Hit-Test-Zelle beginnt eine Linie
  weiter innen).
* **`docs/api/aes/window-graphics.md`** — Blockzitat „Redraw scope“ (Schluss)
  und Randfall 5: Waechter (A84) ja, Teilbaum (A98) ja, mit den beiden
  Testdateien; „reproduces neither“ entfernt.
* **`docs/STATUS.md`**, **`measurement-2026-09-21-a93-a98/`** — siehe A93.

## Tests: rot vor, gruen nach

`measurement-2026-09-21-a93-a98/host-tests/aes_window_slider_subtree-RED-before.txt`
/ `…-GREEN-after.txt`:

| Test | vor | nach |
|---|---|---|
| `vslide_redraw_is_the_vslide_subtree` | „WF_VSLIDE 500 wrote 1035 of 1058 ["up", "dn"] pixels … first Some((327, 63)); 46 draw ops: fill_rect x=327 y=63 w=23 h=1“ | gruen |
| `vslsiz_redraw_is_the_vslide_subtree` | 1035 von 1058 | gruen |
| `hslide_redraw_is_the_hslide_subtree` | „WF_HSLIDE 500 wrote 1035 of 1081 ["lf", "rt"] pixels … first Some((31, 257))“ | gruen |
| `hslsiz_redraw_is_the_hslide_subtree` | 1035 von 1081 | gruen |

(1081 statt 1058 bei „lf“: die Hit-Test-Zelle des linken Pfeils schliesst
die linke Rahmenspalte ein — vorbestehende Hit-Test-Eigenheit, siehe
Beobachtungen im Bericht.)

`cargo run -p xtask -- test` **779/0/1 → 785/0/1**; rpi4 64/94 (Delta 0);
Clippy 80 (Delta 0), `--tests` kein Treffer in der neuen Datei.

## QEMU raspi4b (Selbstbeleg, kein Testerurteil)

Wie A93 (ein Lauf fuer beide): TeraDesk-Elevator-Drag im C:-Fenster,
TeraDesk bestaetigt die Position per `wind_set(WF_VSLIDE)` → der geaenderte
Pfad zeichnet Spur + Elevator neu; **4/4 Dump-Paare vorher/nachher
bytegleich** — erwartungsgemaess, die Pfeile wurden vorher mit identischen
Pixeln uebermalt. 0 Exceptions in beiden Serial-Logs.

## Offen

Pi-Abnahme (Tester/Lead; nichts geflasht). Kein Commit.
