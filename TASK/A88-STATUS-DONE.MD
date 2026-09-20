# A88 — STATUS: DONE (Hostbelege rot/gruen; kein SDK-Aufrufer, kein Injektor-Lauf)

`menu_popup`s drei Nachbarn gegen EmuTOS `mn_popup` (`aes/gemmnext.c:304-356`):
`BEG_MCTRL`/`END_MCTRL`-Klammer, `mn_keystate`, `clamp_ypos`. Bearbeitet
2026-09-21, Branch `desk-rebuild`, Basis HEAD `40be5fe`, **nicht committet**.
Spec: `docs/api/aes/menu.md` § `menu_popup` (Klammer und `mn_keystate` standen
schon; `clamp_ypos`-Regel + ungeklemmtes `ob_x` datiert ergaenzt).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemmnext.c:312` (`wm_update(BEG_MCTRL)`), `:353` (`END_MCTRL`) → `aes/gemwmlib.c:1778-1797` (`wm_update`: `beg_update -= 2; fm_own(beg_update)`) → `aes/gemfmlib.c:82-107` (`fm_own`: `BEG_UPDATE`, `ml_mnhold = gl_mntree; gl_mntree = NULL`, `get_ctrl`/`get_mown`, `ct_chgown(rlr, &gl_rscreen)`; Rueckweg spiegelbildlich); `:114-127` (`clamp_ypos`), `:320-324` (`ob_x = xpos; ob_y = ypos - tree[mn_item].ob_y; clamp_ypos`); `:268-272` (`handle_popup`: `keystate = rets[3]` nach jedem `ev_multi`), `:336-349` (`mdata`: `mn_tree`/`mn_menu`/`mn_item`/`mn_scroll` aus der Eingabe, `mn_keystate = keystate`; Nicht-Wahl `mn_keystate = 0`); `aes/gemevlib.c:56` (`ev_rets`: `rets[3] = kstate`) |
| NewTOS (vorher) | `tos-api/src/aes/menu.rs` `menu_popup` (keine Klammer; `x`/`y` per `clamp(0, …)`; `mn_keystate: 0`, `mn_scroll: 0`), `popup_track`, `popup_shown -> i16` |
| NewTOS (nachher) | `menu.rs`: `menu_popup` (Parameterblock + `mdata`), `mn_popup` (Klammer via `window::fm_own`), `mn_popup_owned` (Warten, Platzierung, `clamp_ypos`, Sicherung, Track), `clamp_ypos`, `popup_shown -> (i16, i16)` |

## Was gebaut ist

1. **Klammer.** `mn_popup` = `super::window::fm_own(true)`; Koerper;
   `fm_own(false)`. Das ist die vorhandene `mctrl_own`-Implementierung
   (`window.rs:3436-3465`), die auch `wind_update(BEG/END_MCTRL)` und
   `form_do` treiben: `UPDATE_LOCK` (`BEG_UPDATE`), `menu::hold_menu`/
   `release_menu` (`gl_mntree` geparkt/zurueck), `MCTRL_CNT` (`ml_ocnt`;
   `chk_ctrl` liest daraus „Kontrollrechteck = Schirm“, `intercept` setzt
   aus). Nichts dupliziert. Nicht darin: EmuTOS' `set_mown(rlr)`/
   `set_mown(ml_pmown)` — `mctrl_own` ruehrt `GL_MOWNER_APP` nicht an
   (dokumentiert als Single-App-No-op; siehe Nebenbefund unten).
2. **`mn_keystate`.** `popup_shown` sampelt beim waehlenden Druck
   `h.keyboard_shift_state()` (der Host-Wert, den auch `graf_mkstate`/
   `ev_rets` liefern) und gibt `(item, keystate)` zurueck; `menu_popup`
   schreibt es bei Wahl, sonst 0. `mn_scroll` wird aus der Eingabe kopiert
   (`:341`; vorher 0 — im selben `mdata`-Block, deshalb hier mitgenommen).
3. **`clamp_ypos`.** 1:1 die beiden `while`-Schleifen mit `gl_hchar`
   (`graf::gl_hchar`), `gl_rscreen.g_h` = `backend.height()`, `gl_rfull.g_y`
   = `gl_hbox`; `ob_x = xpos` ungeklemmt (das Sicherungsrechteck klemmt sich
   selbst, `sr_rect`).

## Hosttests

Modultests `tos-api/src/aes/menu.rs` `popup_wait_tests` (Host + Raster
werden uebergeben, nichts prozessweit installiert; jetzt mit `SERIAL`):

| Fall | Fixes zurueckgesetzt | nachher |
|---|---|---|
| `mn_popup_parks_the_menu_tree_and_keeps_the_mouse_for_its_whole_call` (Leiste installiert, Oeffnungsdruck gehalten; waehrend des ersten Wartens Zeiger in `THEACTIVE` gezogen und dort mit Bewegung losgelassen) | rot: „the pop-up kept the mouse“ — `own_mouse()` false, `gl_mntree` bei jedem Pump non-NULL | gruen: 11 Pumps mit `gl_mntree == NULL`, Maus bei der App, Baum nach `END_MCTRL` zurueck |
| `popup_shown_reports_the_shift_state_at_the_press` (Shift 0x02 nur beim Druck) | rot: `left: 0 right: 2` | gruen: 0x02, Host-Zustand bei Rueckkehr 0 |
| `clamp_ypos_moves_the_box_in_hchar_steps_off_the_bar_and_onto_the_screen` | rot: `left: 0 right: 29` | gruen: −19 → 29, 460 → 412, 200/19 bleiben, 18 → 34, 500 px hoher Kasten → 32 |
| `popup_shown_returns_with_the_picking_button_up` (A73, Guard) | gruen | gruen |

Integrationsfall `tests/aes_a87_a88_menu_ctwait_popup.rs`
`a88_menu_popup_clamps_the_box_below_the_bar_and_onto_the_screen` durch den
AES-Aufruf (pool-loser D-086-Zweig): HEAD Kasten (528,0); nachher (600,29)
bzw. (100,412), `rc = 0`, `mn_keystate = 0`.

**Host-Grenze, ehrlich:** der Track-Pfad von `mn_popup` braucht
`popup_blit_save` → BDOS-Pool → Kernel-High-Half-Alias, den kein Hostprozess
mappt (A73-Befund bleibt; der A73-Integrationsfall bleibt `#[ignore]`).
Neu gelernt: `resource::mfree_tests` installiert im Lib-Testbinary einmalig
einen Buchhaltungs-Pool bei `0x0100_0000` — danach liefert `mxalloc` echte
Pool-Adressen, und ein Modultest, der `popup_blit_save` mit nicht-leerem
Rechteck erreicht, schreibt nach `0xffff000001006000` (SIGSEGV; erster
`xtask test`-Lauf, `host-tests/xtask-test-AFTER-run1-SIGSEGV-pool-alias.log`,
macOS-Crash-Report in `host-tests/` zusammengefasst). Der Klammer-Test fordert
den Kasten deshalb bei x = 700 an (leeres Sicherungsrechteck → `w <= 0`-Tor
vor dem Pool); das Klammer-Verhalten haengt nicht an der Kastenposition.
32-Thread-Stress des Lib-Binaries danach 0/300 (vorher 94/400).

`cargo run -p xtask -- test` 760/0/2 → 770/0/2; rpi4 64/94 (Delta 0);
Clippy lib 80 → 80. Kein SDK-Aufrufer von `menu_popup` (TeraDesk keiner,
QED/cflib eigener Tracker, A73-Audit) — kein Injektor-Lauf, nur Host-Belege.

## Nicht angefasst (Nebenbefunde)

* `fm_own`s `set_mown(rlr)` beim Eintritt / `set_mown(ml_pmown)` beim
  Verlassen (`gemfmlib.c:93,102` → `geminput.c:220-231`, nur bei Taste oben)
  fehlt in `mctrl_own` (dokumentiert als Single-App-No-op). Beobachtbar nur,
  wenn `gl_mowner` beim Eintritt `ctl_pd` ist und die Taste oben — z. B. nach
  einem per Leiste gewaehlten Menuepunkt, der ein Popup oeffnet; unser
  `w_setactive` im naechsten `evnt_multi`-Pass gibt die Maus ohnehin zurueck.
  window.rs, nicht dieses Ticket.
* Bei Nicht-Wahl schreibt `menu_popup` weiterhin alle fuenf `mdata`-Felder
  (EmuTOS nur `mn_keystate = 0`, `:347`) — vorbestehend, nur fuer einen
  Aufrufer sichtbar, der die uebrigen Felder nach `rc == 0` liest.
* `clamp_height` (`menu_settings`) nimmt `backend.height()` als
  `gl_rfull.g_h`; EmuTOS: `gl_height - gl_hbox` (`gemgraf.c:424`,
  `gemmnext.c:90`). Andere Funktion, eigener Punkt.

## Offen

Nichts auf der Pi zu sehen (kein Aufrufer); die Klammer wirkt beim naechsten
Programm, das `menu_popup` nutzt.
