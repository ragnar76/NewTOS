# A93 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Pi-Abnahme offen

`WM_HSLID`/`WM_VSLID` tragen in `msg[5..7]` die `WS_CURR`-Woerter y, w, h
(und in `msg[4]` die 0..1000-Position) — wie EmuTOS' `hctl_window`.
Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis HEAD `2ac0f9a`, **nicht
committet**, gemeinsam mit A98 in einem Paket.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemctrl.c:196-197`: `w_getsize(WS_CURR, w_handle, &t); r_get(&t, &x, &y, &w, &h);` einmal vor dem Gadget-Switch; `:300` `message = (cpt == W_HELEV) ? WM_HSLID : WM_VSLID;`; `:305` `x = gr_slidebox(gl_awind, cpt-1, cpt, (cpt == W_VELEV));` — **nur `x`**; `:314` `ct_msgup(message, pwin->w_owner, w_handle, x, y, w, h);`. |
| NewTOS (vorher) | `tos-api/src/aes/window.rs` `intercept`, Arme `Gadget::VElev`/`HElev`: `msg[0]`, `msg[4] = drag_elevator(…)`; `msg[5..7]` blieben 0. Spec-Tabelle behauptete 0. |
| NewTOS (nachher) | `put_rect(&mut msg, ws_curr)` (das vor dem Switch kopierte `w.curr`, A77) und danach `msg[4] = drag_elevator(…)` — dieselbe Form wie `WM_CLOSED`/`WM_FULLED` (A77). |

## Was gebaut ist

* **`tos-api/src/aes/window.rs`** — `intercept`: die beiden Elevator-Arme
  setzen `put_rect(&mut msg, ws_curr)` vor dem Slider-Wort; Kommentar mit
  `gemctrl.c:196-197,305,314`; der Sammelkommentar ueber `ws_curr` nennt
  jetzt auch `WM_HSLID`/`WM_VSLID`.
* **`tos-api/tests/aes_window_slid_rect.rs`** (neu, 2 Faelle): A77-Harness
  (Rasterpuffer, Skript-Host, Parameterblock). Fenster mit beiden Leisten,
  per `wind_set(WF_CURRXYWH)` auf (176,118,296,206) verschoben (Orakel
  `wind_get`), Elevator ueber `gadget_name_at` (`"velev"`/`"helev"`) gefunden,
  Druck, 10 gehaltene Pakete, Loslassen; `evnt_multi(MU_MESAG|MU_TIMER)`
  liefert die Nachricht. Prueft `msg[0]`, `msg[3]`, `msg[4] ∈ 1..=1000`,
  `(msg[5], msg[6], msg[7]) == (y, w, h)`.
* **`docs/api/aes/window-graphics.md`** — Tabellenzeilen `WM_HSLID`/
  `WM_VSLID`: `msg[5..7]` = `WS_CURR` y/w/h mit Zitat, „corrected from 0
  (A93, 2026-09-21)“; Blockzitat unter der Tabelle ergaenzt.
  **`docs/api/_discrepancies.md`** — die Wortlayout-Kopie (`WM_HSLID`/
  `WM_VSLID`) nachgezogen.
* **`docs/STATUS.md`** — Phase-13-Fragment (A93/A98) + eine Zeile „Recent
  Decisions“ 2026-09-21.
* **`measurements/measurement-2026-09-21-a93-a98/`** — RUNINFO, Hosttest-Quittungen
  (rot/gruen), Build-/Clippy-Logs, Methode (Scratch-Skripte, Aktionen,
  Diffs, Testkopien), Screendumps, Serial, SHA-256-Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach

`measurements/measurement-2026-09-21-a93-a98/host-tests/aes_window_slid_rect-RED-before.txt`
/ `…-GREEN-after.txt`:

| Test | vor | nach |
|---|---|---|
| `vertical_elevator_drag_wm_vslid_carries_ws_curr_ywh` | `msg = [26, 1, 0, 1, 421, 0, 0, 0]`, `WS_CURR = (176, 118, 296, 206)` → `left: (0, 0, 0)` | gruen |
| `horizontal_elevator_drag_wm_hslid_carries_ws_curr_ywh` | `msg = [25, 1, 0, 1, 193, 0, 0, 0]` → `left: (0, 0, 0)` | gruen |

`cargo run -p xtask -- test` **779/0/1 → 785/0/1** (+2 diese Datei, +4 A98),
rc 0. `cargo xtask build --board rpi4`: tos-api 64 / kernel 94 (Delta 0),
`kernel8.img` sha256 `8a50e3d4c709c7da…` (`build/kernel8-tree-rpi4-dev.sha256`).
`cargo clippy -p tos-api` 80 (Delta 0; `window.rs` 11 Notes, alle vorbestehend:
10× `Aespb`-Sichtbarkeit, 1× `top` never read); `--tests`: kein Treffer in der
neuen Datei. Kein qemu-virt-Build (Lead).

## QEMU raspi4b (Selbstbeleg, kein Testerurteil)

A87-Methode (`method/make_scratch_a93.sh`, Injektor, `parkbr`), BEFORE =
Scratch mit `window.rs` = HEAD `2ac0f9a` (`git show`, kein Checkout), AFTER =
Baum. TeraDesk `#Z`, C:-Fenster per Doppelklick (24 Eintraege, 14 sichtbar),
Zeiger auf den Elevator (491,200) → B, Druck, drei gehaltene Schritte bis
(491,290), Loslassen → C, Parken → D. Beide Kerne: `evnt_multi fired=0x0010`
nach dem Loslassen (WM_VSLID an TeraDesk), Liste ans Ende gescrollt
(DTACHILD … VDI.TSV), Elevator unten; **4/4 Dump-Paare bytegleich**
(A `300e96db…`, B `ea9e3323…`, C `7d077fd1…`, D `a62f774e…`), 0 Exceptions.
A93 ist in Pixeln unbeobachtbar — TeraDesk (und QED) lesen nur `msg[4]`.

## Offen

Pi-Abnahme (Tester/Lead; nichts geflasht). Kein Commit.
