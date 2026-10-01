# A90 — STATUS: DONE (Hostbeleg) / Pi-Abnahme offen

`evnt_mouse` (22) auf das `amouse`-Modell gebracht, das A73 fuer
`evnt_multi`'s `MU_M1`/`MU_M2` eingefuehrt hat. Bearbeitet 2026-09-21,
Branch `desk-rebuild`, Basis HEAD `85c591e`, **nicht committet**. Ein Paket
mit A89/A92 (`TASK/A89-STATUS-DONE.MD`, `TASK/A92-STATUS-DONE.MD`),
Messverzeichnis `measurements/measurement-2026-09-21-a89-a92/`.

## Der Sachverhalt, beidseitig

| EmuTOS (Soll) | NewTOS (vorher) | NewTOS (nachher) |
|---|---|---|
| `ev_mouse` (`gemevlib.c:123-128`) = `ev_block(MU_M1, pmo)` → `mwait(iasync(MU_M1, pmo))` (`:64-69`) → `amouse` (`gemasync.c:157-162`, `geminput.c:568-587`): `in_mrect` **einmal zur Arm-Zeit**, besitzblind (`azombie`, `:575-576`); sonst auf `c_msleep` geparkt, das nur `post_mouse(gl_mowner, …)` (`:446-456`) bedient — `mchange` bei Besitz, `set_mown` beim Rueckfall (`:220-231`). **Kein** `chkkbd(); forker();` vor dem Arm (nur `ev_multi`, `:169-170`). Waehrend `ct_msgup` (`gemctrl.c:100-101`) besitzt `ctl_pd` die Maus: ein dann durchquertes Rechteck wird erst nach dem Loslassen (`w_setactive` → `set_mown(app)` → `post_mouse(app, xrat, yrat)`, `:441-442`) gegen die dann aktuelle Position vollendet. `rets[2]` = lebende `button` (`:127`). | `while !rect_event(live) { sleep }` — lebend, besitzblind, kein `w_setactive`; kehrte mit gehaltenem Druck zurueck (`EV_MB = 1`). | `arm_pos` vor allem Pumpen; Runde: `if !ct_msgup_busy() { w_setactive() }`, `fired = (pass == 0 && rect_event_at(arm_pos)) \|\| (own_mouse() && rect_event(live))`. Interception (Leiste/Rahmen) bewusst **nicht** in `evnt_mouse` (Inline-Modell laeuft nur fuer Nachrichten-Waits, AUD-P2-003); nur die Rueckgabe des Besitzes. |

## Was gebaut ist

* **`tos-api/src/aes/event.rs`** — `evnt_mouse` (Doc + Schleife wie oben).
* **`tos-api/tests/aes_a89_a92_menu_event_neighbours.rs`** —
  `evnt_mouse_rectangle_crossed_while_the_selecting_press_is_held_is_delivered_at_the_release`
  (Muster A73 1a: Auswahl auf Druck, `ct_msgup`-Sperre; dann `evnt_mouse(leave,
  11×11)`, 30 px raus gehalten, Loslassen) und Waechter
  `evnt_mouse_satisfied_at_arm_time_returns_at_once_regardless_of_ownership`
  (Treffer zur Arm-Zeit ohne einen Pump, `EV_MB` lebend = gehalten).
* **`docs/api/aes/event.md`** — Notiz unter `evnt_mouse` (22): `amouse`,
  kein Drain, Auslieferung erst bei Besitz.

## Tests: rot vor, gruen nach

| Test | vor | nach |
|---|---|---|
| `…crossed_while_the_selecting_press_is_held…` | `rc=1 at=(94,47) EV_MB=0x01` (zurueck beim ersten Zug, Druck gehalten) | `rc=1 at=(114,47) EV_MB=0x00 queued=2` (zurueck beim Loslassen) |
| `…satisfied_at_arm_time…` (Waechter) | gruen | gruen: `at=(84,47) EV_MB=0x01`, Warteschlange 2 → 1 (nur das `dsptch()` des Aufrufs) |

Zaehler (Paket): xtask test 770 → 779 / 0 / 1, rpi4 64/94, Clippy 80.

## Board / offen

* Kein ausgeliefertes Programm ruft `evnt_mouse` (TeraDesk, QED, Beispiele;
  `grep -rl evnt_mouse sdk/` trifft nur gemlib) — kein QEMU-/Pi-Beleg
  moeglich, Hosttest ist der Beleg. Der H02-run3-Regressionslauf des Pakets
  ist unberuehrt (5/5 Dumps bytegleich).
* Inline-Modell-Eigenschaft, vorbestehend und gemeldet: waehrend eines
  `evnt_mouse`-Waits (wie bei `evnt_button`/`evnt_keybd`/`evnt_timer`) laeuft
  keine Leisten-/Rahmen-Interception; in EmuTOS liefe `ctlmgr` als eigener
  Prozess. Ein Zeiger, der waehrend `evnt_mouse` die Leiste betritt, oeffnet
  bei uns nichts, und `w_setactive` gibt den Besitz in der naechsten Runde
  zurueck (kein Track haelt `gl_ctmown`).
* Pi-Sichtabnahme offen; nichts geflasht, nicht committet.
