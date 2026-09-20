# A77 — STATUS: DONE (Hostbeleg; kein HW-Punkt noetig)

`WM_CLOSED` und `WM_FULLED` tragen in `msg[4..7]` jetzt das `WS_CURR`-Rechteck
des angeklickten Fensters — in allen drei Armen (`HOTCLOSE`-Druck, Closer per
`gr_watchbox`, Fuller). Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis
HEAD `312e49a`, **nicht committet**.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemctrl.c:196-197` `w_getsize(WS_CURR, w_handle, &t); r_get(&t, &x, &y, &w, &h);` einmal vor dem Gadget-Switch; `:314` `ct_msgup(message, pwin->w_owner, w_handle, x, y, w, h)`. `W_CLOSER` (`:208-214` HOTCLOSE, `:216-220` `gr_watchbox`) und `W_FULLER` (`:217-220`) fassen `x,y,w,h` nicht an |
| NewTOS (vorher) | `tos-api/src/aes/window.rs::intercept`: `msg[4..7]` in den drei Armen 0; `docs/api/aes/window-graphics.md` Nachrichtentabelle behauptete `0 \| 0 \| 0 \| 0` |
| NewTOS (nachher) | `window.rs` (`intercept` + neuer Helfer `put_rect`), Spec-Zeilen korrigiert |

## Was gebaut ist

* **`docs/api/aes/window-graphics.md`** — Tabellenzeilen `WM_CLOSED`/`WM_FULLED`:
  `msg[4..7]` = `WS_CURR` x/y/w/h mit `gemctrl.c:196-197`/`:314`-Zitat,
  „corrected from 0 (A77, 2026-09-20)“. `event.md` hat keine Kopie der
  Behauptung (`/usr/bin/grep -n "WM_CLOSED\|WM_FULLED" docs/api/aes/*.md`);
  die einzigen Nullen standen in diesen zwei Zeilen.
* **`tos-api/src/aes/window.rs`** — `intercept`: `let ws_curr = w.curr;` vor
  dem `match` (Kommentar mit `gemctrl.c:196-197`, `:314`, `:208-221`);
  `put_rect(&mut msg, ws_curr)` im HOTCLOSE-Arm, im Closer-Arm nach
  `watchbox` und im Fuller-Arm nach `watchbox`. Neuer Helfer `put_rect`
  (vier Woerter) neben `post_to_owner`. Mover/Sizer/Arrow/Elevator-Arme
  unveraendert.
* **`tos-api/tests/aes_window_gadget_rect.rs`** (neu, Harness aus
  `aes_window_hotclose.rs`): drei Proben; das Fenster wird nach `wind_open`
  per `wind_set(WF_CURRXYWH)` auf `(176,118,296,206)` verschoben, damit das
  Soll weder das Erzeugungsrechteck noch 0 ist; Orakel ist
  `wind_get(WF_CURRXYWH)`.
* **`docs/STATUS.md`** — Phase-13-Zeile (datierter Eintrag) + Zeile in
  „Recent Decisions“ (2026-09-20).

## Tests: rot vor, gruen nach

| Test | vor | nach |
|---|---|---|
| `hotclose_wm_closed_carries_ws_curr` (`NAME\|CLOSER\|HOTCLOSE`, Druck) | `msg = [22, 1, 0, 1, 0, 0, 0, 0]`, `left: (0, 0, 0, 0) right: (176, 118, 296, 206)` | gruen |
| `plain_closer_wm_closed_carries_ws_curr` (`NAME\|CLOSER`, Druck + Loslassen innen) | `msg = [22, 1, 0, 1, 0, 0, 0, 0]`, dito | gruen |
| `fuller_wm_fulled_carries_ws_curr` (`NAME\|CLOSER\|FULLER`, Druck + Loslassen innen) | `msg = [23, 1, 0, 1, 0, 0, 0, 0]`, dito | gruen |
| `aes_window_hotclose.rs` (A49, 3 Faelle) | gruen | gruen |
| `aes_window_drag.rs` (4 Faelle) | gruen | gruen |

`cargo run -p xtask -- test` **705 → 708 / 0 / 2**. Frischer
`cargo xtask build --board rpi4`: rustc tos-api 64 / kernel 94 — Delta 0.
Clippy (`-p tos-api --tests`): lib 81 (unveraendert); alle elf
`window.rs`-Treffer liegen bei Zeilen 2384–3563 (vorbestehend, `Aespb`-
Sichtbarkeit + ein `unused_assignments`), keiner in `intercept`/`put_rect`
oder in der neuen Testdatei. Kein qemu-virt-Build (Auftrag).

## Aufrufer-Umfrage (kein HW-Punkt)

`/usr/bin/grep -n "WM_CLOSED\|WM_FULLED" -A 6 sdk/apps/teradesk/window.c
sdk/apps/teradesk/library/xdialog/*.c sdk/apps/qed/src/*.c | /usr/bin/grep -n
"msg\[4\]\|msg\[5\]\|message\[4\]\|\[4\]\|\[5\]"` liefert genau einen
Treffer: QED `src/event.c:396` `long amount = msg[4] >> 8;` — das ist der
`WM_ARROWED`-Arm im `-A 6`-Fenster von `WM_FULLED`, nicht CLOSED/FULLED.
TeraDesk: `library/xdialog/xwindow.c:918-934` ruft `wd_closed(w, 0)` /
`wd_fulled(w, xe_mbshift)` ohne `m4`; `xdialog.c:1667` prueft nur
`ev_mmgpbuf[0]`/`[3]`. QED: `src/event.c:388-393` `do_icon(window->handle,
DO_DELETE)` / `full_window(window)`. Niemand liest die Woerter — kein
Vorher/Nachher-Dump, keine Pi-Abnahme.

## Weitere `hctl_window`-Abweichungen (gefunden, nicht gefixt)

* **`WM_HSLID`/`WM_VSLID`**: EmuTOS ueberschreibt nur `x` mit der
  Sliderposition (`x = gr_slidebox(...)`, `gemctrl.c:305`), `msg[5..7]`
  bleiben `WS_CURR` y/w/h aus `:196-197`. NewTOS (`Gadget::VElev`/`HElev`)
  sendet dort 0; die Spec-Zeilen in `window-graphics.md` behaupten ebenfalls
  0. Eigener Task.
* Stimmen: `WM_TOPPED`/`WM_UNTOPPED` (`0,0,0,0`, `:151,178`), `WM_ARROWED`
  (`action,0,0,0`, `handle_arrow_msg` `:125`), `WM_MOVED` (neues x/y aus
  `gr_dragbox`, altes w/h, `:237-238`), `WM_SIZED` (altes x/y, neues w/h aus
  `gr_rubwind`, `:258-259`).

## Nicht angefasst / offen

* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
* Spec-Zeilen `WM_HSLID`/`WM_VSLID` unveraendert (ausserhalb des Tickets).
