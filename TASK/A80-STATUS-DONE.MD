# A80 — STATUS: DONE (Hostbeleg; kein HW-Punkt noetig)

`wind_set(WF_TOP)` auf dem Desktop-Handle 0 und auf einem erzeugten, nicht
geoeffneten Handle zeichnet nichts mehr, sendet nichts und vergibt keinen
Stapelwert; `wind_set(0, WF_TOP)` setzt — Lead-Entscheid, 1:1-Ziel — wie
EmuTOS `WS_WORK(0) := wm_calc(WC_WORK, NONE, WS_CURR(0))`, so dass
`wind_get(0, WF_WXYWH)` danach `(1, 1, W−2, H−2)` antwortet. Bearbeitet
2026-09-20, Branch `desk-rebuild`, Basis HEAD `03f2ba9`, **nicht committet**.

## Der Sachverhalt, beidseitig (jede Zeile selbst nachgelesen)

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemwmlib.c:1679-1683` `if (w_handle != gl_wtop) wm_mktop(w_handle)` — rohes `gl_wtop` (`NIL` ohne offenes Fenster, `:1297`; sonst `W_TREE[ROOT].ob_tail`, `:1093`; nie 0). `wm_mktop` `:1605-1613`: `ob_order(gl_wtree, h, NIL)` verweigert ROOT (`aes/gemoblib.c:1209-1210`); `draw_change(h, WS_CURR)` speichert `WS_PREV := WS_CURR`, `WS_CURR := pt`, `WS_WORK := wm_calc(WC_WORK, w_kind, pt)` (`:1069-1077`) und kehrt fuer ein nicht-`VF_ISOPEN`-Fenster zurueck (`:1085-1086`); das Desktopfenster traegt nur `VF_INUSE` (`w_setup` `:205-213`, `wm_init` `:1290`); `wm_calc` mit `kind = NONE`: jede Kante 1 px (`:1815-1867`); `wm_mktop` restauriert `WS_PREV` (`:1613`). `wm_init` speichert `gl_rfull` (`:1294`), `wm_new` ueberspringt Handle 0 (`:1886`). Nicht geoeffnetes Handle ≥ 1: `get_par` liest `W_TREE[-1]` (`aes/gemobjop.c:145-160`, `ob_next = NIL` aus `w_nilit` `:172-178,1262`) — undefiniert |
| NewTOS (vorher) | `tos-api/src/aes/window.rs` `WF_TOP`-Arm `if handle != get_top() { top_window(handle) }` (`get_top()` = 0 ohne offenes Fenster → `wind_set(0, WF_TOP)` No-op); `top_window` prueft nur `old_top == handle`: `bump_z` auf einen geschlossenen Slot, Rahmen des weiterhin obersten Fensters deaktiviert gezeichnet, fuer Handle 0 `draw_frame_pieces(0, true)` = 1-px-Rahmen um den Desktop-Arbeitsbereich (2775 bzw. 1215 Pixel, A59 T5/T6) |
| NewTOS (nachher) | `window.rs`: neuer `wm_mktop(handle)` — nicht offen → fuer `DESKWH` `set_curr(w, w.curr)` (Geometriespeicherung von `draw_change`), sonst nichts, return; offen → `top_window`. `WF_TOP`-Arm vergleicht gegen `gl_wtop()` |

## Was gebaut ist

* **`docs/api/aes/window-graphics.md`** (Spec zuerst):
  * `wind_set`-Feldtabelle, Zeile `WF_TOP`: rohes `gl_wtop`, `wm_mktop`-Ablauf,
    „kein Pixel, keine Nachricht“ fuer Handle 0 / ungeoeffnetes Handle.
  * Neue Note „`WF_TOP` on the desktop handle and on a created-but-unopened
    handle (A80, 2026-09-20)“ vor der A78-`draw_change`-Note: Zeilenbelege wie
    oben, Netto-Effekt, wo `WS_WORK(0)` sonst noch gelesen wird (`w_owns`
    `:1558`, `w_setactive` `:326-328`), Dauer bis `wm_init`; NewTOS-Vertrag fuer
    das ungeoeffnete Handle (TRUE, kein Repaint, keine Nachricht, keine
    z-Aenderung, Rechtecke unangetastet).
  * Verifikationspunkt nach „`wind_get(WF_TOP)` needs no handle“:
    `wind_get(0, WF_WXYWH)` ist `gl_rfull` bis zum ersten `wind_set(0, WF_TOP)`.
  * Handle-0-Absatz bei `wind_create`: Verweis auf die Ausnahme.
* **`tos-api/src/aes/window.rs`** — `wm_mktop` (neu, vor `top_window`, Doc mit
  allen `file:line`), `WF_TOP`-Arm (`gl_wtop()` statt `get_top()`, Kommentar),
  ein Satz im `WF_WXYWH`-Kommentar von `wind_get`. `top_window`, `bump_z`,
  `set_curr`, `desktop_work` unveraendert. Die gespeicherte `w.work` von
  Handle 0 ist bei uns wie bei EmuTOS der Clip der Desktop-Rechteckliste
  (`rlist_owns`) und ohne offenes Fenster das ctrl-Rechteck (`chk_ctrl`);
  `send_redraw_in` clippt weiter gegen `desktop_work()` = `gl_rfull`
  (EmuTOS `w_update` `:1010` liest die Globale, nicht `WS_WORK(0)`).
* **`tos-api/tests/aes_window_top_unopened.rs`** (neu, Harness aus
  `aes_window_move_blit.rs`; `fresh()` ruft `wm_init`, damit `WS_WORK(0)` je
  Fall auf `gl_rfull` steht — `wind_new` setzt es nicht zurueck).
* **`docs/STATUS.md`** — Phase-13-Zeile (datierter Eintrag) + Zeile in
  „Recent Decisions“ (2026-09-20).

## Tests: rot vor, gruen nach

| Test | vor | nach |
|---|---|---|
| `t5_topping_the_desktop_handle_paints_nothing_and_recomputes_its_work_rect` (h offen, `wind_set(0, WF_TOP)`) | `2775` Pixel geaendert (erstes `(0, 19)`), `WF_WXYWH(0)` = `(0,19,640,461)` | 0 Pixel, `WF_TOP` = h, `z(h)`/`next_z` gleich, keine Nachricht, `WF_WXYWH(0)` = `(1,1,638,478)`, `WF_CXYWH(0)`/`WF_PXYWH(0)` unveraendert |
| `t5b_topping_the_desktop_handle_with_nothing_open_recomputes_its_work_rect` | `left: (0, 19, 640, 461) right: (1, 1, 638, 478)` (alter Arm: `get_top()` = 0 → No-op) | gruen |
| `t6_topping_a_created_but_unopened_handle_does_nothing` (h offen, h2 erzeugt) | `1215` Pixel geaendert (erstes `(55, 43)`) | 0 Pixel, `WF_TOP` = h, `z(h)`/`next_z`/Rechtecke von h2 und Desktop gleich, keine Nachricht, ret 1 |
| `t7_topping_an_open_window_under_the_top_one_still_repaints_and_redraws` (Waechter) | gruen | gruen |
| `aes_window_content.rs::clicking_a_window_never_erases_its_content` | gruen | gruen |
| `aes_window_rectlist_zorder.rs::topping_a_window_moves_it_in_the_subtraction_order_like_a_later_open` | gruen | gruen |
| `aes_window_zorder_counter.rs::the_counter_steps_past_i16_max_without_a_panic_and_keeps_the_order` | gruen | gruen |

`cargo run -p xtask -- test` **708 → 712 / 0 / 2**. Frischer
`cargo xtask build --board rpi4` nach `cargo clean -p tos-api -p kernel`
(beide Bare-Metal-Targets): rustc tos-api 64 / kernel 94 — Delta 0.
Clippy `-p tos-api --target aarch64-unknown-none-softfloat`: 81 mit HEAD-
`window.rs` (Kopie im Job-Scratchpad, von Hand zurueckgespielt) und 81 mit dem
A80-Stand; alle elf `window.rs`-Treffer liegen bei 2434/2784–3616
(vorbestehend: `Aespb`-Sichtbarkeit + ein `unused_assignments`), keiner in
`wm_mktop` (735–751), im `WF_TOP`-Arm (3400–3420) oder in der neuen
Testdatei (`--tests`: 0 Treffer). Kein qemu-virt-Build (Auftrag).

## Aufrufer-Umfrage (kein HW-Punkt)

`/usr/bin/grep -n "WF_TOP" sdk/apps/teradesk/*.c
sdk/apps/teradesk/library/xdialog/*.c sdk/apps/qed/src/*.c` — 16 Treffer:

* TeraDesk `wind_set`: `library/xdialog/xwindow.c:348-370` `xw_set_topbot`
  setzt `w->xw_handle` eines Fensters aus seiner Liste (Aufrufer
  `window.c:2378`, `xdialog.c:2112`, `xdnmdial.c:124`, `xwindow.c:384,924`) —
  offene Fenster; `window.c:506-512` `wd_top_app` mit Handle **−1** (MagiC-
  Erweiterung) steht unter `#if _MINT_` und ist im `_TOS_ONLY`-Build
  (`sdk/apps/build-teradesk.sh:87`, `_MINT_=0`) nicht dabei — Handle −1 wird
  von `wind_set` ohnehin verweigert (`get_pwin`). Uebrige Treffer sind
  `wind_get(0, WF_TOP)` (`startprg.c:136,142`, `window.c:547`,
  `xwindow.c:170`) und der `wf == WF_TOP`-Vergleich (`xwindow.c:353`).
* QED: `src/window.c:546-554` `top_window` kehrt ohne `WI_OPEN|WI_ICONIFIED`
  zurueck, bevor `wind_set(w->handle, WF_TOP)` laeuft; `src/aktion.c:135`
  toppt `akt_handle` (den offenen Dialog) als Antwort auf
  `WM_TOPPED`/`WM_NEWTOP`/`WM_ONTOP`; `src/window.c:130` ist `wind_get`.
* Kernel-Desktop (`kernel/src/desk/main.rs:1439` `top_msg`) toppt nur einen
  Handle, den `win_find` kennt (offenes Desktopfenster).

Niemand toppt Handle 0 oder ein ungeoeffnetes Handle — kein Vorher/Nachher-
Dump, keine Pi-Abnahme; Verhalten sichtbar unveraendert.

## Nicht angefasst / offen

* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
* EmuTOS-Seite von T5 (`wind_get(0, WF_WXYWH)` = `(1,1,W−2,H−2)`) ist aus der
  Quelle hergeleitet, nicht in Hatari gemessen (wie in A59 vermerkt).
* `aes_window_content.rs:748-752` (Doc-Kommentar „WM_REDRAW for its work
  area“ bei `WF_TOP`) ist weiterhin veraltet (A59 § 7) — nicht Teil des
  Tickets.
* Der NewTOS-Vertrag „Rechtecke unangetastet“ fuer das ungeoeffnete Handle
  ist eine Praezisierung ueber den Lead-Wortlaut („kein Repaint, keine
  z-Aenderung, return 1“) hinaus: der Code tut nichts, der Test pinnt es.
