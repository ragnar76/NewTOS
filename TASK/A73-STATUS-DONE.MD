# A73 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Abnahme offen

Drei Nachbarn des H02-Fixes im Ereignis- und Menuemanager, nach EmuTOS
nachgezogen: (2) die Menuetafel bleibt beim Verlassen von Titel *und* Kasten
stehen (`mn_do` `OUTSIDE_STATE`), ein Druck ausserhalb bricht ab und wird vom
Kontrollmanager verschluckt; (1) `MU_M1`/`MU_M2` werden auf Pass 0 von
`evnt_multi` gegen die Arm-Zeit-Position geprueft (`amouse`), lebend nur, wenn
die App die Maus (wieder) besitzt; (3) `menu_popup` wartet nach dem
Zurueckschreiben ein zweites Mal auf das Loslassen und kehrt mit Taste oben
zurueck. Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis HEAD `6ed0bf3`,
**nicht committet**. Spec (vom Lead, uncommitted, unveraendert gelassen):
`docs/api/aes/menu.md` `:37-44` (Popup-Vertrag), `:387-413` (Popup-Besitz +
Druck/Loslassen-Klammer), `:629-635` (Hover-Korrektur), `:704-730` (Punkt 5,
`OUTSIDE_STATE`); `docs/api/aes/event.md` `:221-258` (Arm-Zeit-Regel).
Audit-Tests (vorgefunden): `tos-api/tests/aes_a73_event_neighbours.rs`.

## Der Sachverhalt, je Punkt beidseitig

| Punkt | EmuTOS (Soll) | NewTOS (vorher) | NewTOS (nachher) |
|---|---|---|---|
| 2 `OUTSIDE_STATE` | `aes/gemmnlib.c:415-443`: Zeiger weder ueber Titel (`ob_find(THEACTIVE)`), noch im Kasten (`ob_find(cur_menu)`), noch im Untermenue → `menu_state = OUTSIDE_STATE`; Item enthellt (`item_changed`, `:459`), Titel bleibt (`:463-464` uebersprungen), Warten auf `MU_BUTTON \| M1 \| M2 \| M3` (`:293-305`); Wiedereintritt → `INITEM_STATE` (`:415-426`); `MU_BUTTON` ausserhalb `START`/`INTITLE` → `break` (`:369-374`), Aufraeumen ohne Auswahl (`:482-511`), `return FALSE`; alles unter `gl_ctmown` (`:277,513`; `geminput.c:334-335`). DISABLED-Titel: `cur_title = NIL; done = TRUE` (`:429-434`). | `track_owned`: `if !in_bar && !in_box && !in_sub_box { close_box; return None }` — Tafel zu, Maus zurueck an die App, der naechste Desktop-Klick kam als `MU_BUTTON` (TeraDesk: Dateiauswahl weg). | `outside = cur_title != NIL && t == NIL && !in_box && !in_sub_box`; aussen: bleiben; Tastenwechsel aussen: `close_box` + `None` (Druck verzehrt — `b_click` unter `GL_CTMOWN`, `w_setactive` erst nach dem Loslassen); DISABLED-Titel: `close_box` + `None` (bei uns wurde ein Kasten geoeffnet — `open_box` gated nicht; EmuTOS' `menu_down` tut es, `:158-160,469`). Leistenreihe zaehlt nur im `START_STATE` (`cur_title == NIL`). |
| 1 `MU_M1`/`M2` Pass 0 | `amouse` (`geminput.c:568-587`) testet **einmal zur Arm-Zeit**, besitzblind; danach nur `post_mouse(gl_mowner, …)` (`:446-456`) aus `mchange` (`:388-390`) oder `set_mown` (`:220-231`); `acancel` verwirft das unbefriedigte Arm (`gemevlib.c:264`). Auswahl auf Druck: `ct_msgup` wartet (`gemctrl.c:100-101`), kein `set_mown` → `MU_MESAG` allein; Auswahl auf Loslassen: `w_setactive` → `set_mown(app)` → `post_mouse` lebend → `MU_MESAG \| MU_M1`. | `own_m = own_mouse() \|\| pass == 0` mit **lebender** Position **nach** dem Inline-Kontrollmanager — ein Track im selben Pass lieferte `MU_M1` neben `MN_SELECTED`. | `arm_pos` vor der Interception gesampelt (wie `arm_own`/`arm_mb`); Pass 0: `rect_event_at(arm_pos) \|\| (own_mouse() && rect_event(live))`; Pass ≥ 1 unveraendert (lebend, nur Besitzer). |
| 3 `menu_popup` | `mn_popup` (`gemmnext.c:304-356`): `ev_button(1, 0x00ff, 0x0000)` vor (`:314`), `handle_popup` kehrt auf dem Druck zurueck (`:268,290-291`), `popup_blit`-Restore (`:333`), **`ev_button(1, 0x00ff, 0x0000)` erneut** (`:351`), `END_MCTRL` (`:353`). Keine `ct_mouse`-Klammer — die gibt es fuer Popups nicht (`BEG_MCTRL` = `fm_own`, Kontext der App). | `popup_track`: erstes Warten vorhanden (`:1031-1033`), Schleife bricht auf dem Druck-Pegel, Restore, Rueckgabe mit Taste unten. | `popup_track` = `wait_button_up` (`:314`) + Save + `popup_shown` (zeichnen, tracken, Restore, `wait_button_up` `:351`). |

**Korrektur der Ticketpraemisse (Punkt 3):** das Ticket vermutete eine
fehlende `ct_mouse`-Klammer. EmuTOS hat fuer Popups keine — `grep
'ct_mouse\|gl_ctmown' aes/gemmnext.c` trifft nichts; der auswaehlende Druck
wird durch das **zweite `ev_button`** vor `END_MCTRL` verzehrt, das ist der
Vertrag (`menu.md:387-413`). Genau dieses Warten fehlte.

## Was gebaut ist

* **`tos-api/src/aes/menu.rs`** — `track_owned`: `outside`-Bestimmung vor dem
  Kantenblock, Abbruchzweig im Kantenblock, `START_STATE`-/`OUTSIDE_STATE`-/
  DISABLED-Behandlung statt des Sofortschliessens; Doc-Kommentare von `track`
  und `was_down` angepasst. `popup_track` in `wait_button_up` +
  `popup_shown(tree, box, backend, h, saved)` aufgeteilt, zweites Warten nach
  dem Restore. Neu `#[cfg(test)] mod popup_wait_tests` (1 Fall; Host und
  Raster als Parameter, **nichts prozessweit installiert** — ein
  `install_host` im Lib-Testbinary kippte `resource.rs`'
  `r_object_of_a_g_cicon_leads_to_the_mono_iconblk_head`, der hostlos 4 Planes
  erwartet; daher zurueckgenommen).
* **`tos-api/src/aes/event.rs`** — `rect_event_at(m, pos)`, `arm_pos`,
  `rect_fires`-Closure fuer `MU_M1`/`MU_M2`.
* **`tos-api/tests/aes_a73_event_neighbours.rs`** (Audit-Datei, vorgefunden):
  nur die tote Hilfsfunktion `evnt_multi_poll` entfernt (dead_code-Warnung).
* **`docs/STATUS.md`** — Phase-13-Zeile (datierter Eintrag) + Zeile in
  „Recent Decisions“.
* **`measurements/measurement-2026-09-20-a73/`** — RUNINFO, Screendumps, Serial,
  Hosttest-Quittungen, Build-/Clippy-Logs, Methode (Skripte, Actions,
  Injektor-Diff, Produkt-Diff), SHA-256-Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach

| Test | vor | nach |
|---|---|---|
| `m1_armed_before_press_track_is_not_delivered_with_mn_selected` | `fired=0x0014` (`MU_M1` neben `MN_SELECTED`) | `fired=0x0010`, naechster Aufruf `0x0024` |
| `m1_armed_before_release_track_is_delivered_with_mn_selected` (Waechter) | gruen `0x0014` | gruen `0x0014` |
| `leaving_the_box_keeps_it_open_and_reentry_press_selects` | `fired=0x0002 bret=1 at (84,47)` | `fired=0x0010 msg=[10,1,0,4,12,…]` |
| `press_outside_the_open_box_is_consumed_as_cancel` | `fired=0x0002 bret=1 at (84,200)` | `fired=0x0020` (nur Timer) |
| `menu_popup_waits_for_the_selecting_press_to_be_released` | `#[ignore]` (BDOS-Pool) | `#[ignore]`, Grund unveraendert |
| Modul `menu::popup_wait_tests::popup_shown_returns_with_the_picking_button_up` | `left: 1 right: 0` (zweites Warten per Hand entfernt, per Hand zurueck) | gruen |

`cargo run -p xtask -- test` **688 → 693 / 0 / 2** (der Popup-Integrationsfall
bleibt der zweite ignorierte). `cargo xtask build --board rpi4` rustc tos-api
64 / kernel 94 — Delta 0. Clippy (`-p tos-api --tests`): in `menu.rs` nur die
vorbestehenden `private_interfaces`-Treffer auf `pub fn menu_*(pb: &Aespb)`,
keiner in `event.rs`, im neuen Code oder in der Testdatei.

## QEMU raspi4b (`measurements/measurement-2026-09-20-a73/`)

TeraDesk per `#Z` (INF aus H02: TeraDesk-Standard, Textansicht, das
Layout, das `rowstat.py` misst), 640×480, headless, QMP, Injektor in
Scratch-Kopien; vorher = HEAD-`menu.rs`/`event.rs`, nachher = Baum.
Park **unten links** (neue `parkbl`-Aktion): die (0,0)-Ecke liegt in der
Leiste, der Sweep kreuzt den File-Titel — mit `OUTSIDE_STATE` bliebe die
Tafel bis zum Klick stehen. Zielpunkt **(88,300)** statt (88,200): TeraDesks
File-Tafel reicht bis y≈245 (`run3-B.png`), (88,200) liegt noch darin.

| Lauf | C (aussen) | D (+0,5 s nach dem Klick) |
|---|---|---|
| nachher | Tafel unten, File invertiert (0.779), HELLO-Zeile **0.836**; `track -> None at (88,300) mb=0x01` | Tafel zu, File normal, HELLO-Zeile **0.836 = steht**, Infozeile „32 bytes used in 1 selected item.“; keine `evnt_multi`-Zustellung |
| vorher | Tafel **schon zu** (`track -> None at (88,262)` beim Verlassen); C ≡ nachher-D (sha `7bf08d5b…`) | `evnt_multi fired=0x0002 clicks=1 mouse=(88,300)` → HELLO-Zeile **0.161 = abgewaehlt** |

Regressionslauf (H02 run3, Auswahl ueber die Leiste, Kern nachher):
`track -> Some((4, 20))`, `evnt_multi fired=0x0010` (nur `MU_MESAG`),
„File Information“-Dialog fuer HELLO.TXT, Infozeile unveraendert. Der
anschliessende Schub `fired=0x0002 … mb=0x01 pass=0` ist der Druckwarter des
Dialogs, neu bewaffnet, waehrend der auswaehlende Druck noch gehalten ist —
die dokumentierte besitzblinde Arm-Zeit-Pegelpruefung (`abutton`,
`geminput.c:549`; `menu.md` „The one documented exception“), von A73
unberuehrt und in EmuTOS identisch. A/B aller drei Laeufe bytegleich; 0
`EXC`/`PANIC`.

## Nicht angefasst / offen

* **Pi-Abnahme offen** (Tester/Lead; nichts geflasht): File hovern, Zeiger
  auf den leeren Desktop, Tafel muss stehen bleiben; Klick dort schliesst sie
  und laesst eine Dateiauswahl stehen; Auswahl ueber die Leiste liefert
  weiter den Dialog.
* Punkt 1 hat keinen Board-Beleg (TeraDesks Hauptwarte bewaffnet kein
  `MU_M1`); Punkt 3 auf dem Board braeuchte eine App mit `menu_popup` (keine
  vorhanden; cflib faellt auf den eigenen Tracker zurueck).
* **A88** (nicht meins): `BEG_MCTRL`/`fm_own`-Klammer, `mn_keystate`,
  `clamp_ypos` des Popups.
* Untermenue-Feinheit `gemmnlib.c:437-438` (aus dem Untermenue ins
  Niemandsland: Elternitem bleibt selektiert, Untermenue bleibt offen) nicht
  nachgebaut — der Untermenue-Pfad weicht vorbestehend ab (ein Druck auf das
  Elternitem waehlt bei EmuTOS das Elternitem, bei uns nichts); bei uns
  schliesst das Untermenue beim Verlassen, die Tafel bleibt.
* `evnt_mouse` (22) testet weiter lebend und besitzblind (eigener Pfad, nicht
  Teil des Tickets). `open_box` gated nicht auf DISABLED (vorbestehend).
* Architektonisch: der Kontrollmanager laeuft inline im `evnt_multi` der App;
  im `OUTSIDE_STATE` haelt er die App wie schon im Kasten fest (bei EmuTOS
  liefe ihr Timer weiter). Vorbestehende Eigenschaft des Inline-Modells,
  durch A73 nur laenger sichtbar.
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
