# A92 — STATUS: DONE (Hostbeleg + QEMU-Regressionsbeleg) / Pi-Abnahme offen

`evnt_multi` sampelt die Arm-Zeit-Werte erst nach einem Kopf-Pump — EmuTOS'
`chkkbd(); forker();` (`gemevlib.c:169-170`). Bearbeitet 2026-09-21, Branch
`desk-rebuild`, Basis HEAD `85c591e`, **nicht committet**. Ein Paket mit
A89/A90, Messverzeichnis `measurement-2026-09-21-a89-a92/`.

## Der Sachverhalt, beidseitig

| EmuTOS (Soll) | NewTOS (vorher) | NewTOS (nachher) |
|---|---|---|
| `ev_multi` leert zuerst den Fork-Ring (`chkkbd(); forker();`, `:169-170`): jedes seit dem letzten `dsptch()` (`gemsuper.c:452-464`) eingetroffene Maus-/Tastenpaket ist verarbeitet, bevor die Quick-Checks laufen und `abutton`/`amouse` armen. `ev_mouse` (`:123-128`) und `ev_button` (`:84-94`) haben **keinen** solchen Drain. | `arm_own`/`arm_mb`/`arm_pos` vor dem ersten `pump_input` (der erst in `sched::sleep` nach Pass 0 lief). | `h.pump_input()` am Kopf von `evnt_multi`, dann das Sampeln. `evnt_mouse`/`evnt_button` bewusst unveraendert (siehe A90). |

## Was gebaut ist

* **`tos-api/src/aes/event.rs`** — eine Zeile + Kommentar am Kopf von
  `evnt_multi` (vor der Deadline-Berechnung; S2-Neustart pumpt einmal mehr,
  wie es die Warteschleife taete).
* **`tos-api/tests/aes_a89_a92_menu_event_neighbours.rs`** —
  `a_move_queued_before_the_call_counts_for_the_arm_time_position` (ein Zug
  +20 vor dem Trap eingereiht, `MU_M1` leave 21×21 + Timer 0) und
  `a_press_queued_before_the_call_counts_for_the_arm_time_level` (ein Druck
  eingereiht, `MU_BUTTON` wait-for-press + Timer 0).
* **Bestehende Tests — Skriptanpassung, keine Aussage geaendert:** Skripte,
  die „Eingabe nach dem Arm“ modellieren, bekommen ein Leerpaket
  (`arm_gap`) fuer den Kopf-Drain; ohne es waere das erste Skript-Paket
  Eingabe *vor* dem Aufruf — bei einem Druck der besitzblinde
  `abutton`-Pegeltest (`geminput.c:549-551`, `menu.md` „The one documented
  exception“), der in EmuTOS ebenso sofort `MU_BUTTON` liefert (A73 1b
  meldete genau das: `fired=0x0002`). Betroffen: `aes_a73_event_neighbours`
  (1a/1b/2a/2b), `aes_a87_a88_menu_ctwait_popup` (zwei Klickfaelle),
  `aes_menu_click_ownership` (`menu_item_release_after_drag…`),
  `aes_menu_bar_height` (`press_open_from`), `aes_desk_background`
  (Mover-Drag), `aes_window_drag` (3), `aes_window_content` (Klick ins
  Fenster), `aes_window_hotclose` (`(pumps, left)` = (2, 2) statt (1, 3):
  Kopf-`forker()` + schliessendes `dsptch()`; die Aussage „feuert auf dem
  Druck, das Loslassen bleibt eingereiht“ bleibt). Ausserdem
  `aes_a87_a88…::a88_menu_popup_clamps…`: mit dem Heap-Fallback von A89 wird
  das Popup auf dem Host getrackt — der Fall bekommt je Aufruf einen
  abbrechenden Klick ausserhalb des Kastens (gleiche Asserts: `rc=0`,
  `mn_keystate=0`, Klemmwerte).

## Tests: rot vor, gruen nach

| Test | vor | nach |
|---|---|---|
| `a_move_queued_before_the_call…` | `fired=0x0020 at=(320,300)` (Zug nicht angewendet, M1 verpasst) | `fired=0x0024 at=(340,300)` |
| `a_press_queued_before_the_call…` | `fired=0x0020 mb=0` | `fired=0x0022 mb=1 bret=1` |

`cargo run -p xtask -- test` 770 → 779 / 0 / 1 (Laeufe: run1 = A73 1b rot
durch den Kopf-Pump, run2 = A87/A88, danach die uebrigen Skript-Dateien
einzeln, run4 = alles gruen, run5 (nach dem Un-ignore) = 779; `host-tests/`). rpi4 64/94, Clippy 80.

## Board / offen

* Beobachtbar nur bei Eingang zwischen zwei Traps (USB-HID wird im
  `pump_input` geleert; der QEMU-Injektor speist nur im Pump). QEMU raspi4b
  H02-run3 vorher/nachher: 5/5 Dumps bytegleich, `fired=0x0010`; der
  Druckwarter-Schub des Info-Dialogs zaehlt 44 statt 68 Zeilen im gleichen
  Zeitfenster (zwei Pumps je Aufruf) — Timing, keine Semantik.
* Ticket-Nebensatz „same for `evnt_mouse`/`evnt_button` if they sample“:
  trifft in EmuTOS nicht zu (kein Drain in `ev_mouse`/`ev_button`);
  `evnt_mouse` sampelt seit A90 bewusst **vor** jedem Pump, der Hosttest
  `…crossed_while_the_selecting_press_is_held…` pinnt das mit.
* Vorbestehend, gemeldet: die Interception liest die *lebende* Position,
  nicht die Druck-Momentaufnahme (`pr_xrat`/`pr_yrat`, `ev_rets`
  `gemevlib.c:42-58`); ein Druck plus Zug, die beide vor dem Trap anstehen,
  treffen bei uns die Zugposition (auf dem Board schon vor A92 so, weil der
  Pump alle USB-Reports leert).
* Pi-Sichtabnahme offen; nichts geflasht, nicht committet.
