# A84 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Abnahme offen

`wind_set(WF_HSLIDE/WF_VSLIDE/WF_HSLSIZ/WF_VSLSIZ)` speichert und zeichnet
den Slider nur noch, wenn sich der Wert aendert — wie EmuTOS' `wm_set`.
Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis HEAD `a8d3163`, **nicht
committet**. Spec (Researcher, uncommitted, unveraendert gelassen):
`docs/api/aes/window-graphics.md` § `wind_set` (105) Feldtabelle („only if
changed“), Blockzitat „Redraw scope“, Randfall 5.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemwmlib.c:1690-1716`: `if (pwin->w_hslsiz != pinwds[0]) { pwin->w_hslsiz = pinwds[0]; gadget = W_HSLIDE; }` (`:1691-1694`), ebenso `WF_VSLSIZ :1698`, `WF_HSLIDE :1705`, `WF_VSLIDE :1712`; unveraendert bleibt `gadget = -1` → `do_cpwalk` bleibt `FALSE` → kein `w_cpwalk` (`:1739-1755`). Verglichen wird der **validierte** Wert (Clamp 1..1000 bzw. −1 vor dem `switch`, `:1643-1656`). |
| NewTOS (vorher) | `tos-api/src/aes/window.rs` `wind_set`: die vier Arme speicherten und riefen `redraw_vslider`/`redraw_hslider` bedingungslos — 46 Zeichenoperationen fuer das zweite `WF_VSLIDE 500` (Literal-Rahmen nach A79; das Ticket mass 32 auf dem Vor-A79-Rahmen). |
| NewTOS (nachher) | `store_if_changed(&mut w.vslide, v)` in den vier Armen; Redraw nur bei `true`. 0 Zeichenoperationen fuer den zweiten Aufruf. |

## Was gebaut ist

* **`tos-api/src/aes/window.rs`** — `wind_set`: die Arme `WF_VSLIDE`,
  `WF_VSLSIZ`, `WF_HSLIDE`, `WF_HSLSIZ` validieren zuerst (`clamp(1, 1000)`
  bzw. `slider_size`), dann `win(handle).is_some_and(|w|
  store_if_changed(&mut w.vslide, v))` → `redraw_vslider(handle)` nur bei
  Aenderung. Neu `fn store_if_changed(field: &mut i16, v: i16) -> bool`
  mit EmuTOS-Zitat. Kommentar ueber den Armen erweitert. `redraw_vslider`/
  `redraw_hslider` unveraendert (der Pfeil-Ausschluss ist nicht Teil des
  Tickets, siehe unten).
* **`tos-api/tests/aes_window_slider_guard.rs`** (neu, 8 Faelle): eigenes
  `CountingBackend` (jede Zeichen-Eintrittsstelle des `RasterBackend`
  protokolliert eine Zeile), Host + Parameterblock nach
  `aes_window_color_words.rs`. Faelle: T8 (`WF_VSLIDE 500` zweimal → zweiter
  Aufruf 0 Ops, Pixelhash unveraendert, `wind_get` liest 500), dasselbe fuer
  `WF_HSLIDE`/`WF_VSLSIZ`/`WF_HSLSIZ`; Vergleich nach dem Clamp (`1000` dann
  `1500` = unveraendert); `WF_VSLSIZ −1` auf frischem Fenster = keine
  Aenderung (`w_setup`-Default −1); spaetere Aenderung zeichnet und speichert;
  ungeoeffnetes Fenster speichert ohne zu zeichnen (Regressionswaechter,
  war schon gruen).
* **`docs/STATUS.md`** — Phase-13-Fragment (A84/A82/A83) + eine Zeile
  „Recent Decisions“ 2026-09-20.
* **`measurements/measurement-2026-09-20-a83-a84/`** — RUNINFO, Screendumps, Serial,
  Hosttest-Quittungen, Build-/Clippy-Logs, Methode (Scratch-Diff),
  SHA-256-Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach

Gegenprobe in einer Scratch-Kopie der Crates mit `window.rs` = HEAD `a8d3163`
(`measurements/measurement-2026-09-20-a83-a84/host-tests/aes_window_slider_guard-RED-before-HEAD-a8d3163.txt`,
`…-GREEN-after.txt`):

| Test | vor | nach |
|---|---|---|
| `t8_vslide_set_twice_draws_only_once` | „WF_VSLIDE 500 again issued 46 draw operations, EmuTOS issues 0 … first op: fill_rect x=327 y=63 w=23 h=1“ | gruen |
| `hslide_set_twice_draws_only_once` | 46 Ops (`fill_rect x=31 y=257 w=23 h=1`) | gruen |
| `vslsiz_set_twice_draws_only_once` / `hslsiz_…` | 46 Ops | gruen |
| `the_guard_compares_after_clamping` (1000 → 1500) | 46 Ops | gruen |
| `default_size_on_a_fresh_window_is_not_a_change` | „WF_VSLSIZ -1 on a fresh window drew 46 ops“ | gruen |
| `a_later_change_still_draws_and_stores` | rot (zweiter Aufruf zeichnete) | gruen |
| `an_unopened_window_stores_without_drawing` | gruen | gruen |

`cargo run -p xtask -- test` **718/0/2 → 727/0/4** (+8 dieser Datei, +1
aktiver A83-NULL-Fall, +2 A83-Faelle `#[ignore]`;
`host-tests/xtask-test.log`). Frischer `cargo xtask build --board rpi4`
(nach `cargo clean -p kernel -p tos-api --target …`): rustc **tos-api 64 /
kernel 94**, rc 0 — Delta 0 (`build/xtask-build-rpi4.log`). Clippy
`-p tos-api` **81 → 81** (die elf `window.rs`-Treffer sind die bekannten:
`top` never read, `Aespb`-Sichtbarkeit); `--test aes_window_slider_guard
--test aes_window_name_pointer`: 0 Treffer in den beiden Testdateien
(`build/clippy-*.txt`). Kein qemu-virt-Build (Lead).

## QEMU raspi4b (`measurements/measurement-2026-09-20-a83-a84/`)

A79-Methode unveraendert (`measurements/measurement-2026-09-20-a79/method/`:
`make_scratch_final.sh` → Scratch-Kern mit H02/A46/A78-Injektor, sha256
`26b76a51…`; `tools/make-sd-image.sh 64` → sd-base; `sd-put.sh` mit
`newdesk-tera.inf`/`teradesk-window.inf` → sd-tera; `run.sh` mit
`desk.actions` bzw. `tera-window.actions`, 640x480, `-display none`, QMP
0.3 s nach jeder Marke). Zeiger unten rechts geparkt.

| Dump | sha256 | A79-Referenz |
|---|---|---|
| `desk-A-idle.png` | `9cd7377f…` | = `final-desk-A-idle.png` |
| `desk-B-two.png` / `desk-C-two-2s.png` | `9638ccf0…` | = `final-desk-B-two.png` / `final-desk-C-two-2s.png` |
| `tera-A-idle.png` | `76965702…` | = `final-tera-A-idle.png` |
| `tera-B-window.png` / `tera-C-window-2s.png` | `2280f56d…` | = `final-tera-B-window.png` / `final-tera-C-window-2s.png` |

Alle sechs **bytegleich** mit der A79-Referenz (erwartet: der Waechter
aendert kein Pixel); `grep -c "EXC\|PANIC\|panic"` = 0 in beiden
Serial-Logs. QED nicht gefahren (Dateidialog per Injektor ist nicht
„billig“; A83 aendert an `WF_NAME` heute nichts).

## Nicht angefasst / offen

* **Pfeil-Ausschluss:** EmuTOS' `w_cpwalk(w_handle, W_VSLIDE, …)` zeichnet
  nur den `W_VSLIDE`/`W_HSLIDE`-Teilbaum (Spur + Elevator), `draw_vslider`/
  `draw_hslider` malen die Pfeile mit. Pixelunbeobachtbar; Spec-Randfall 5
  nennt es weiter als offen (Spec nicht angefasst — Researcher/Lead).
* **Pi-Abnahme offen** (Tester/Lead; nichts geflasht).
* Kein Commit, kein `git checkout/stash/reset`.
