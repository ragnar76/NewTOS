# A78 — STATUS: DONE (Hostbeleg + QEMU-Beleg) / Tester- und HW-Abnahme offen

`wind_set(WF_CXYWH)` folgt jetzt EmuTOS' `draw_change` Zweig fuer Zweig: die
reine Verschiebung des **obersten** Fensters wird **geblittet** (`w_move`,
`bb_screen` = Backend-`copy_rect` unter der Zeigerklammer), der Verschobene
bekommt **kein** `WM_REDRAW` (`moved`), die reine Verkleinerung ebenso, und
der Fan-out (`w_update`) laeuft **von oben nach unten** in z-Reihenfolge und
nur ueber den Bereich `[stop .. start]`. Developer-Selbstbeleg 2026-09-20,
Branch `desk-rebuild`, Basis HEAD `b1c90fa`, **uncommitted** im Baum.
Messlauf: [`measurement-2026-09-20-a78/`](../measurement-2026-09-20-a78/RUNINFO.TXT).

## Quelle → Code, Zweig fuer Zweig (`references/emutos` @ `1a7484ab`, `aes/gemwmlib.c`)

| Zweig | EmuTOS | NewTOS (`tos-api/src/aes/window.rs`) |
|---|---|---|
| Geometrie speichern, nicht offen → fertig | `:1069-1077`, `:1085-1086` | `WF_CXYWH`-Arm: `w.prev = w.curr; set_curr`, `if open { draw_change_open }` |
| `pt == c` (Top-Request, der keiner ist) | `:1108-1116` | `draw_change_open`: `return` — nichts gezeichnet, nichts gesendet |
| reine Verkleinerung (Subset) | `:1141-1146` `stop = w_handle; w_cpwalk(gl_wtop); moved = TRUE` | `stop = handle; draw_frame_pieces(gl_wtop, true, None); moved = true` |
| Shrink → `start = DESKWH`; `c = max + S` | `:1148-1154` | dito (zweiter Schatten im Tail `:1213-1215` = `plus_shadow`) |
| Superset-Grow | `:1160-1167` `c = pt`, `start` bleibt `w_handle` | `c = pt` |
| reine Verschiebung von `gl_wtop` | `:1173-1178` → `w_move :938-996` | `w_move(handle, backend)` → `(moved, stop, c)`, `start = DESKWH` |
| `w_move`: Quelle rechts/unten ueber den Rand **und** Bewegung dorthin → kein Blit | `:951-957` | `stop = DESKWH`, `c = s ∪ d` |
| `w_move`: `w_mvfix` (Clip auf `gl_rfull`, `-1`-Saum) | `:915-930`, `:963-964` | `w_mvfix` 1:1 |
| `w_move`: `bb_screen` unter `gsx_sclip(&gl_rfull)` | `:969-970`, `gemgraf.c:328-363` | `bb_screen`: `cursor_hide; copy_rect(S_ONLY=3, clip gl_rfull); cursor_show` |
| `w_move`: linker Saum nachzeichnen | `:972-982` `w_cpwalk(gl_wtop, 0, 0, FALSE)` | `draw_frame_pieces(handle, true, Some(d))` (1-px-Spalte) |
| andere Moves/Shrinks | `:1180-1190` | `union_grect`, `start = DESKWH` |
| Tail: Schatten, `w_drawdesk`, `w_update` | `:1213-1222` | `plus_shadow`; `compose_except(c, moved.then_some(gl_wtop))`; `w_update(start, c, stop, moved)` |
| `w_update`: oben→unten, Bereich, `moved`-Sprung | `:1004-1048`, `:1025` | `w_update` ueber `open_by_z_desc`, bricht bei `bottom` ab, ueberspringt `gl_wtop` bei `moved` |

Der alte Kommentar (`:2969-2972`, „unlike EmuTOS, which blits … nothing carried
its pixels to the new place") und der unbedingte `send_redraw(handle, work)`
sind weg; `broadcast_redraw` (Handle-Reihenfolge 1..15) ist durch `w_update`
ersetzt — **ein** Fan-out, auch fuer `wind_close` und `form_dial(FMD_FINISH)`
(beide `DESKWH..DESKWH, moved = false`, jetzt ebenfalls oben→unten wie EmuTOS).

Spec zuerst: `docs/api/aes/window-graphics.md` § `wind_set`, Tabellenzeile
`WF_CXYWH` + neue Notiz „`WF_CXYWH` on an open window — `draw_change` branch by
branch" (Faelle, Blit-Bedingungen, `moved`-Ueberspringen, Reihenfolge,
Rechtecke, alle mit Zeilen).

## Test: rot vor, gruen nach

`tos-api/tests/aes_window_move_blit.rs` (Muster `aes_window_content.rs`,
BufBackend mit `copy_rect`-Mitschnitt + Nachrichtenmitschnitt, 640×480):

| Fall | Erwartung (EmuTOS) | HEAD `b1c90fa` | nach dem Fix |
|---|---|---|---|
| T1 reine Verschiebung Top (30,40,320,240)→(60,80) | 1 `copy_rect (30,40,322,242)→(60,80)`, Inhalt vollstaendig an neuer Stelle, freigewordener Streifen Desktop, **0** `WM_REDRAW` | **rot** (0 Blit, 1 `WM_REDRAW`) | gruen |
| T2 reine Verkleinerung | 0 Blit, **0** `WM_REDRAW`, Rest-Arbeitsflaeche unangetastet | **rot** (1 `WM_REDRAW`) | gruen |
| T3 Vergroesserung (Regression) | 1 `WM_REDRAW(h, 30,40,402,302)` | gruen | gruen |
| T4 h1 unter h2, h1 verschoben | `[h2 (200,150,172,152)] [h1 (50,60,322,242)]` | **rot** (`[h1] [h2]`) | gruen |
| T5 Quelle ueber den rechten Rand, Bewegung nach links | kein Blit, 1 `WM_REDRAW(h, 300,100,322,242)` | gruen | gruen |
| T5b dieselbe, Bewegung nach rechts | `copy_rect (400,100,240,242)→(420,100)`, 0 `WM_REDRAW` | **rot** | gruen |
| T6 gleiches Rechteck | 0 Pixel, 0 Nachrichten (`:1115-1116`) | **rot** (1 `WM_REDRAW`) | gruen |

Rohausgaben: `measurement-2026-09-20-a78/host-tests/aes_window_move_blit-RED-before.txt`
/ `-GREEN-after.txt`.

## Pruefungen — selbst gefahren

| | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **660 / 0 / 1** (Sollstand 653 + 7 neue) |
| bestehende `aes_window_*`, `aes_desk_background`, `aes_reference` | unveraendert gruen, keine Aenderung noetig (keiner friert Handle-Reihenfolge oder den Redraw an den Verschobenen ein) |
| rustc-Warnungen, beide Boards | tos-api **64**, kernel **94** — Delta 0 |
| clippy-Warnungen, beide Boards | tos-api **81**, kernel **125** — Delta 0; Testdatei clippy-sauber |
| QEMU raspi4b, TeraDesk per `#Z`, Injektor: Titelbalken 80 px nach unten | **nachher:** `op 105 in=(1,5,303,222,640,449)` ohne `post_msg [20,…]` — Inhalt steht an der neuen Stelle, TeraDesk zeichnet nichts nach; **vorher:** zwei `post_msg [20,…]` (Work-Rect + Damage∩Hull), TeraDesk zeichnet neu. Screendumps vorher/nachher an allen 6 Marken **0 Pixel** verschieden |
| dito, zweiter Zug 300 px nach links (Quelle ueber den rechten Rand) | beide Kernel `post_msg [20,1,0,1,7,222,633,258]` = `((s ∪ d) ∩ gl_rfull + S) ∩ hull` — EmuTOS' Wert fuer den verweigerten Blit (`:951-957`) |
| QEMU raspi4b, unser Desktop (`desk/` `WM_MOVED` → `WF_CXYWH`), Fenster (+30,+40) | **nachher:** `op 105 in=(1,5,80,83,544,400)` ohne `post_msg [20,…]`; **vorher:** zwei. Screendumps vorher/nachher **0 Pixel** an allen 3 Marken |
| Idle-Screendumps gegen `measurement-2026-09-19-a46/screendumps/` | **nicht** bytegleich: 194 (Tera) bzw. 104 (Desk) Pixel, bbox nur der Zeiger-Sprite (bei uns noch an (319,239), bei A46 schon unterwegs nach (300,300) — Dump-gegen-`park`-Race der Methode); Delta fuer den **HEAD-Kernel identisch**, also kein A78-Effekt. Alles andere bytegleich |
| `git status --short` | `M docs/api/aes/window-graphics.md`, `M tos-api/src/aes/window.rs`, `?? tos-api/tests/aes_window_move_blit.rs`, `?? measurement-2026-09-20-a78/`, `?? TASK/A78-STATUS-DONE.MD` (+ `M TASK/A78.md`) |

## Was offen bleibt

- **Tester-Lauf** (voller Harness beide Boards) und **echte Pi 4**: TeraDesk-/
  Desktop-Fenster am Titelbalken ziehen — Inhalt muss ohne Neuzeichnen
  mitkommen (kein Flackern), der Zeiger darf im Blit-Bereich keine Spur
  hinterlassen (Klammer in `bb_screen`).
- `docs/STATUS.md` (Phasenzeile + Recent Decisions) beim Commit durch den Lead.

## Aus dem Skopus, nicht angefasst

- `w_mvfix` richtet nur `x = -1` aus; ein Fenster weiter links vom Rand (`x < -1`)
  oder ueber `gl_rfull.y` wird von EmuTOS beim Blit **versetzt** kopiert
  (`:915-930` clippt nur die Quelle). 1:1 uebernommen; der Desktop-Drag erlaubt
  solche Positionen nicht (`gemctrl.c:232-237`).
- `wind_open` schickt weiter `WM_REDRAW(work)` statt `WS_TRUE ∩ gl_rfull` (A59 § 7).
- `aes_window_content.rs:748-752` Doc-Kommentar zu `WF_TOP` veraltet (A59 § 7).
