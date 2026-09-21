# A102 — STATUS: DONE (Hostbeleg rot/gruen + QEMU-Selbstbeleg vorher/nachher) / Pi-Sichtabnahme offen

Vier Punkte, ein Paket mit A100 (Messverzeichnis
`measurement-2026-09-21-a100-a102/`). Bearbeitet 2026-09-21, Branch
`desk-rebuild`, Basis HEAD `022d6d0`, **nicht committet**, nichts geflasht.

## 1 — `THEACTIVE` = Summe der Titel (Kernel-Desktop)

| | Datei:Zeile |
|---|---|
| EmuTOS | `desk/deskmain.c:1528-1534` (`title->ob_x = x; title->ob_width = n; … mbar->ob_width = x`), `:1584-1589` (Ueberlauf → `mbar->ob_x = 1`), `desk/desk_rsc.c:255-259` (`THEACTIVE` `{2, 0, 28, 769}`); `aes/gemmnlib.c:717` (`gl_ctwait` = `ob_actxywh(tree, THEACTIVE)`) |
| NewTOS vorher | `build_menu`: `o[2] = (0, 0, sw, MENU_H)` |
| NewTOS nachher | `MenuGeom.active_x/active_w`; `o[2] = (16, 0, Σ title_w, MENU_H)` = `(16, 0, 232, 19)` |

Hosttests (Replik des Kernel-Baums, `aes_a100_a102_desk_menu_geometry.rs`;
beidseits gruen — der Kernel-Crate ist forced-target, sein Beleg ist der
Serial-Dump): `a102_the_bar_row_right_of_options_stays_with_the_application`
(Zeiger (400,8), 300 Pakete Ruhe → `MU_TIMER` bei 101 ms, nichts gezeichnet;
HELP `0x6200` waehrend der Ruhe → `MU_KEYBD` sofort, Zeiger in der Zeile),
`a102_the_last_column_of_options_starts_the_track_and_the_next_does_not`
(x = 247: Options-Box unten, Snapshot zeigt den Aussenrand bei Spalte 171;
x = 248: Timer puenktlich, Snapshot = Leiste unveraendert),
`a102_a_title_inside_theactive_still_starts_the_track` (`MN_SELECTED (4, 12)`).
QEMU Kernel-Desktop: `[a100] THEACTIVE=(16,0,232,19)` (vorher `(0,0,640,19)`);
BIG.NCP gewaehlt, Zeiger nach (400,8), CR: nachher `[aes] op 25
out=(1,400,8,0,0,7181,0)` sofort und Dump D ohne Auswahl; vorher kein
`op 25` bis zum Parken (`out=(1,527,135,…)`), Dump D noch mit Auswahl
(Diff 2666 px, bbox bis y = 169). Der Kernel-Desktop behandelt HELP nicht
(`hndl_kbd`, wie EmuTOS), daher CR als sichtbare Taste.

## 2 — `mctrl_own` mit `set_mown`

| | Datei:Zeile |
|---|---|
| EmuTOS | `aes/gemfmlib.c:86-96` (`fm_own(TRUE)`: `ml_mnhold`, `get_ctrl`, `get_mown(&ml_pmown)`, `ct_chgown(rlr, &gl_rscreen)`), `:98-106` (`ct_chgown(ml_pmown, &ml_ctrl); gl_mntree = ml_mnhold`); `aes/gemctrl.c:392-397` (`ct_chgown`: `set_ctrl; if (!gl_ctmown) set_mown`); `aes/geminput.c:209-212` (`get_mown`), `:220-235` (`set_mown`: `if (!button) gl_cowner = gl_mowner = mp; post_mouse; post_button`) |
| NewTOS vorher | `mctrl_own`: `hold_menu`/`release_menu` + Zaehler; `gl_mowner` unberuehrt |
| NewTOS nachher | `window.rs` `ML_PMOWN`; 0→1: `hold_menu(); ML_PMOWN = get_mown(); ct_chgown(true)`; 1→0: `ct_chgown(ML_PMOWN); release_menu()`. `input.rs`: `get_mown()`, `ct_chgown(app)` (`!GL_CTMOWN`), `set_mown(app)` (`BUTTON == 0`); `w_setactive` = `ct_chgown(true)` (verhaltensgleich). Kontrollrechteck weiter live in `chk_ctrl` (`ml_ocnt > 0` → Schirm) |

Modultest `menu.rs::popup_wait_tests::fm_own_gives_the_caller_the_mouse_and_hands_it_back_to_the_previous_owner`:
Leiste registriert, Zeiger mit Taste oben in `THEACTIVE` → `mchange` gibt
`ctl_pd` die Maus; `fm_own(true)` → App (rot an HEAD: „BEG_MCTRL: … the
caller owns the mouse"); `fm_own(false)` → `ctl_pd`; verschachtelt nur die
aeusseren Kanten; Taste gehalten → unveraendert; App als Vorbesitzer bleibt
App. Bestehender Klammer-Test (A88) weiter gruen.

## 3 — `menu_popup` Nichtwahl schreibt nur `mn_keystate`

EmuTOS `aes/gemmnext.c:345-349`: `else { mdata->mn_keystate = 0; rc = FALSE; }`.
Vorher schrieb NewTOS alle fuenf Felder (tree/menu/scroll wie Eingabe,
item −1). Nachher `Menu::write_keystate_abi(mdata, abi, 0)` (LP64 @14 /
ILP32 @10). Test `a102_menu_popup_no_pick_writes_only_mn_keystate`
(Sentinels `0xDEAD_BEEF_CAFE_F00D`/`0x1111`/`0x2222`/`0x3333`/`0x5555`,
Klick ausserhalb): rot an HEAD (vier Felder ueberschrieben), gruen nach.
Spec `docs/api/aes/menu.md` § `menu_popup`: „and nothing else".

## 4 — `clamp_height` mit `gl_rfull.g_h = gl_height − gl_hbox`

EmuTOS `aes/gemmnext.c:90` (`max_height = gl_rfull.g_h / gl_hchar`),
`aes/gemgraf.c:424` (`gl_rfull = (0, gl_hbox, gl_width, gl_height − gl_hbox)`).
Vorher `backend.height() / CELL_H` (30 bei 480 Zeilen), nachher
`(height − gl_hbox()) / gl_hchar()` (28). `aes_menu_settings.rs`: neuer
Abschnitt „clamps height from above" (29, 30, 100, `i16::MAX` → 28; 28
bleibt): rot an HEAD („height 29 must clamp to … 28: left 29"), gruen nach.
Spec § `menu_settings`: `gl_rfull`-Satz.

## Summen

`cargo run -p xtask -- test` **804 / 0 / 1 → 811 / 0 / 1**. rpi4 frisch
tos-api 64 / kernel 94 (Delta 0). `cargo clippy -p tos-api` 80 → 80;
Testdateien 0. Kein qemu-virt-Build (Lead). 0 EXC/PANIC in 4 Serial-Logs.

## Nicht angefasst

* Nach CR-Deselektion bleibt *Open* im File-Menue aktiv (`men_update`;
  Dump G vorher = nachher) — desk/.
* `w_setactive` laeuft inline bei jedem `evnt_multi`-Durchlauf und gibt die
  Maus mit Taste oben sofort der App zurueck; EmuTOS' `set_mown(ctl_pd)` am
  Ende eines `fm_own` bei gehaltenem Eintritts-Druck bleibt deshalb nur bis
  zum naechsten Durchlauf sichtbar. Vorbestehendes Modell (H02/A73).

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht).
