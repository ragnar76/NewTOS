# A89 — STATUS: DONE (Hostbeleg + QEMU-Regressionsbeleg) / Pi-Abnahme offen

Menuetafel, zwei Nachbarn von A73, nach `references/emutos/aes/gemmnlib.c`
`mn_do` nachgezogen: (1) aus dem Untermenue ins Niemandsland bleiben
Elternitem und Untermenue stehen, ein Druck waehlt das Elternitem; (2) ein
DISABLED-Titel bekommt weder Hilite noch Kasten. Bearbeitet 2026-09-21,
Branch `desk-rebuild`, Basis HEAD `85c591e`, **nicht committet**. Ein Paket
mit A90/A92 (`TASK/A90-STATUS-DONE.MD`, `TASK/A92-STATUS-DONE.MD`),
gemeinsames Messverzeichnis `measurement-2026-09-21-a89-a92/`.

## Der Sachverhalt, beidseitig

| Punkt | EmuTOS (Soll) | NewTOS (vorher) | NewTOS (nachher) |
|---|---|---|---|
| 1 Untermenue → Niemandsland | Hit-Test faellt durch; `if (menu_state == SUBMENU_STATE) cur_item = last_item;` (`:437-438`) → `OUTSIDE_STATE`; `item_changed` falsch → kein `undisplay_submenu`, Elternitem bleibt selektiert (`:452-459`); nur das Untermenue-Item wird enthellt (`:446-448`); Warten auf `MU_BUTTON \| M1 \| M2 \| M3` (`:293-305`). Druck → `break` (`:373-374`) → Cleanup: `undisplay_submenu` (`:486-490`), `menu_sr(FALSE)` (`:491`), `cur_item != NIL` und enabled → `*pitem = cur_item` (Elternitem, `menu_state != SUBMENU_STATE`, `:492-500`) → `TRUE`. Druck auf das Elternitem mit sichtbarem Untermenue (`INITEM_STATE`, `cur_submenu == NIL`) → dasselbe Cleanup → Elternitem. | Verlassen des Untermenues: `it == NIL != cur_item` → Untermenue zu, Elternitem enthellt, `cur_item = NIL`. Druck auf das Elternitem mit sichtbarem Untermenue: Kantenblock verlangte `sub_item != NIL` (Untermenue-Item) oder `sub_tree == 0` (Elternbox ohne Untermenue) → **ignoriert**, Track lief weiter. | `keeps_parent = it == NIL && t == NIL && sub_tree != 0 && (was_in_sub \|\| was_outside)` — Zustand bleibt, solange der Zeiger draussen ist; Kantenblock: `sub_item != NIL` → Untermenue-Item (gleicher Baum) bzw. nichts (Fremdbaum, vorbestehend), sonst `cur_item != NIL && !in_sub_box` → Untermenue schliessen, `(cur_title, cur_item)`; sonst `outside` → Abbruch. |
| 2 DISABLED-Titel | `menu_down` (`:229-244`): `if (do_chg(tree, ititle, SELECTED, TRUE, TRUE, TRUE)) { menu_sr(TRUE); ob_draw }` — `do_chg` verweigert DISABLED (`:158-160`); `mn_do` ruft `menu_down` nur ueber dasselbe `do_chg` (`menu_select`, `:466-467`). Druck auf dem Titel: `buparm ^= 1` (`:375`). Verlassen nach unten: `cur_menu == NIL` → `cur_title = NIL` → `done` (`:406-411`) bzw. DISABLED-Ausstieg (`:429-434`). | `open_box` sicherte, hilitete und zeichnete den Kasten; A73's DISABLED-Zweig nahm ihn beim Verlassen wieder weg. | `open_box` gibt `NIL` fuer DISABLED; `cur_box == NIL`, kein Item-Tracking, Druck ohne Wirkung, Verlassen → `None` (`close_box` mit `NIL` nur Cursor-Klammer). |

## Was gebaut ist

* **`tos-api/src/aes/menu.rs`** — `track_owned`: `was_in_sub`/`was_outside`,
  `keeps_parent` im Kastenblock, Kantenblock umgestellt (Untermenue-Item /
  Elternitem inkl. Untermenue-Schliessen vor dem Kasten-Restore / Abbruch),
  DISABLED-Titel-Kommentar; `open_box`: DISABLED-Gatter; Doc von `track`.
* **`tos-api/src/aes/gsxif.rs`** — `popup_blit_save`: ohne jeden BDOS-Pool
  (`!is_initialised()`, nur im Hosttest) Heap-Kopie (`Backing::Heap`), wie
  `bb_save` ohne Schirmpuffer. Nur so geht auf dem Host ein Untermenue auf;
  D-086 („Pool knapp → kein Popup/Untermenue“) bleibt, wo ein Pool existiert;
  auf dem Board unerreichbar (`bring_up_bdos_pools` vor AUTO/Desktop,
  `kernel/src/lib.rs:276`). Nebeneffekt: der A73-Popup-Fall
  `menu_popup_waits_for_the_selecting_press_to_be_released` laeuft jetzt
  (rc=1, `mn_item` 2, Taste oben, naechster Aufruf nur Timer) — `#[ignore]`
  entfernt, Kommentar angepasst. **Ausserhalb der Dateiliste des Leads**
  (`menu.rs`/`event.rs`/`input.rs`); Rueckbau = 10 Zeilen + die drei
  Untermenue-Tests und der Popup-Fall wieder auf `#[ignore]`.
* **`tos-api/tests/aes_a89_a92_menu_event_neighbours.rs`** (neu) — A73-
  Harness + **Sonden-Pakete** (`PROBE`: im Pump des Trackers gelesen, liefern
  `ob_state` zweier Objekte und zwei Pixel-Pens), Untermenue ueber
  `menu_attach` (37) in eigenem Baum, Strings auf dem Heap (der Marker
  `gemmnext.c:62-63` schreibt hinein).
* **`docs/api/aes/menu.md`** — § „Mouse ownership over the track“ Punkte 6
  (Untermenue/Elternitem) und 7 (DISABLED-Titel), mit Zeilen.
* **`docs/STATUS.md`** — Phase-13-Fragment + Recent-Decisions-Zeile
  (2026-09-21, gemeinsam mit A90/A92).

## Tests: rot vor, gruen nach (`host-tests/`)

| Test | vor | nach |
|---|---|---|
| `press_on_the_parent_item_with_its_submenu_showing_selects_the_parent` | Haenger (IDLE-Kappe 20 000 Pumps, Zeiger (84,47)) | `fired=0x0010 msg=[10,1,0,4,12,…]` |
| `leaving_the_submenu_into_no_mans_land_keeps_the_parent_and_a_press_selects_it` | Haenger (Zeiger (300,66): der Druck draussen war ein Abbruch, nichts kam) | Sonde `state_a=0x0001` (Open SELECTED), `pen_a=0` (Untermenue steht), `at=(300,66)`; `fired=0x0010 msg=[10,1,0,4,12,…]`; Untermenue-Pixel nach der Auswahl wieder Desktop |
| `leaving_the_submenu_and_returning_to_the_parent_item_a_press_selects_the_parent` | Haenger | `fired=0x0010 msg=[10,1,0,4,12,…]` bei (84,47) |
| `hovering_a_disabled_title_drops_no_box_and_hilites_nothing` | Sonden `state 0x0009` (DISABLED\|SELECTED), `pen 0` (Kasten gezeichnet) | Sonden `state 0x0008`, `pen 3` (Desktop-Dither), `fired=0x0020` |

`cargo run -p xtask -- test` **770 → 779 / 0 / 1** (8 neue Faelle im Paket +
der A73-Popup-Fall aus `ignored` heraus). `cargo xtask build --board rpi4`
tos-api 64 / kernel 94 (Delta 0). Clippy `-p tos-api` 80 (Delta 0; in
`menu.rs` nur die vorbestehenden `private_interfaces`-Treffer, `gsxif.rs`/
`event.rs`/Testdatei 0).

## QEMU raspi4b (`measurement-2026-09-21-a89-a92/`)

TeraDesk per `#Z`, H02-run3 (C:-Fenster oeffnen, HELLO.TXT waehlen, File
hovern, Info… hovern, Klick), Park unten rechts (A87 `parkbr`), vorher = HEAD
`input/event/menu/gsxif.rs`, nachher = Baum: **5/5 Dumps bytegleich**,
`[a73] track -> Some((4, 20)) at (150,43) mb=0x01`, `evnt_multi fired=0x0010`
(nur `MU_MESAG`), C1/D = „File Information“-Dialog fuer HELLO.TXT; 0
`EXC`/`PANIC`. **TeraDesk hat keinen DISABLED-Titel** (`desktop.rsc` Baum 0:
Titel 3–7 `ob_state` 0; kein `menu_ienable` auf einen Titel im Quelltext)
**und kein Untermenue** (kein `menu_attach`) — (a)/(b) des Leads sind auf dem
Board nicht darstellbar, Hosttest ist der Beleg.

## Nicht angefasst / offen

* Pi-Sichtabnahme offen (Tester/Lead; nichts geflasht). Sichtbar aendert
  sich fuer TeraDesk/QED nichts (kein DISABLED-Titel, kein Untermenue).
* Nicht nachgebaut (EmuTOS-Artefakte ohne App-Wirkung): `cur_menu` bleibt in
  EmuTOS nach einem DISABLED-Titel veraltet, `ob_find` trifft dann gegen einen
  nicht gezeigten Kasten (`:427`) und `do_chg` hilitet ein unsichtbares Item;
  ein Druck auf den Rand eines Untermenues liefert `*pitem = smroot`
  (`:494-496`). Beides gemeldet, nicht implementiert.
* Vorbestehend, unveraendert: ein DISABLED-Item mit angehaengtem Untermenue
  oeffnet es bei uns (`open_submenu` ohne DISABLED-Gatter; EmuTOS ueber
  `menu_select` → `do_chg` nicht, `:470-475`); Untermenue in einem Fremdbaum
  liefert keine Auswahl (msg[5..7]-Pfad, Modul-Doku).
* Kein Commit, kein `git checkout/stash/reset`, kein `*.img` im
  Messverzeichnis.
