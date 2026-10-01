# A100 — STATUS: DONE (Hostbeleg rot/gruen + QEMU-Selbstbeleg vorher/nachher) / Pi-Sichtabnahme offen

Die Dropdown-Boxen des Kernel-Desktops tragen jetzt EmuTOS' Box-Wort
`0xFF1100` (`desk/desk_rsc.c:294`): 1-px-Rand **aussen** (`th = -1`),
`IP_HOLLOW`-Innenraum (weiss), Rand/Text schwarz. Der zeilenbreite
Eintrags-XOR eines Hovers (`ob_change`, `aes/gemoblib.c:1295-1304`) erreicht
den Rand nicht mehr; der Rahmen-Repaint, den `rehilite`/`popup_shown` als
Kompensation fuer den frueheren Innenrand behielten (A99), ist weg — EmuTOS
zeichnet nach `menu_select`/`do_chg` nichts (`aes/gemmnlib.c:451-475`,
`aes/gemmnext.c:278-287`). Bearbeitet 2026-09-21, Branch `desk-rebuild`,
Basis HEAD `022d6d0`, **nicht committet**, nichts geflasht. Gemeinsam mit
A102 (ein Paket, ein Messverzeichnis).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `desk/desk_rsc.c:291-301` (Box 8: `G_BOX`, `16716032L` = `0xFF1100`, `{2, 0, 20, 8}`; Eintraege `{0, row, 20, 1}`), `:255-259` (`THEACTIVE` `{2, 0, 28, 769}`); `desk/deskmain.c:1537-1541` („the menu bar cannot start at character offset 0 because, in that case, the outline of the Desk dropdown box will not be drawn properly"), `:1617` (`dropbox->ob_x = mbar->ob_x + title->ob_x`); `aes/gemoblib.c:1295-1304` (`ob_change`: XOR des Item-Rechtecks, Inset = `ob_sst` des *Items*, 0 fuer `G_STRING`); `aes/gemmnlib.c:451-475` (`mn_do`: nur `menu_select`, kein Rahmen); `aes/gemmnext.c:278-287` (`handle_popup`: nur `do_chg`); `aes/gemgraf.c:547-559` (`gr_rect`: `IP_HOLLOW` = weisse Flaeche) |
| NewTOS (vorher) | `kernel/src/desk/menu.rs` `build_menu`: Box-Spec `(1<<16)\|0x1070` (Rand innen), `THEACTIVE` bei x = 0; `tos-api/src/aes/menu.rs` `rehilite`/`popup_shown`: vier `fill_rect(BLACK)` entlang der Box-Kanten nach jedem Toggle — auf Aussenrand-Boxen (TeraDesk `desktop.rsc` Baum 0 Objekt 9 `0xFF1100`, QED) ein zweiter, innerer Rahmen |
| NewTOS (nachher) | `kernel/src/desk/menu.rs` (`MenuGeom.active_x`, Box-Spec `0x00FF_1100`, Box-x = `active_x + title_x`), `tos-api/src/aes/menu.rs` (`rehilite(tree, old, new, backend)` ohne Repaint, `popup_shown` ohne Repaint) |

## Was gebaut ist

* **`kernel/src/desk/menu.rs`** — `MenuGeom` +`active_x`/`active_w`;
  `adjust_menu`: `active_x = 2·CHAR_W`, Ueberlauf → `CHAR_W`
  (`deskmain.c:1584-1589`), `box_x = (active_x + title_x).min(sw − box_w)`;
  `build_menu`: `THEACTIVE = (active_x, 0, active_w, MENU_H)`, Box-Spec
  `0x00FF_1100` mit Zitat. Eintraege unveraendert `(0, row·MENU_H, iw, MENU_H)`
  — die RSC-Geometrie.
* **`tos-api/src/aes/menu.rs`** — `rehilite`: Box-Parameter und die vier
  `fill_rect`-Zeilen entfernt, Doc-Kommentar mit der Begruendung (Aussenrand
  vs. Innenrand, EmuTOS repaintet nie); `popup_shown`: dieselben vier Zeilen
  entfernt. Drei Aufrufer angepasst.
* **`docs/api/desktop/deskmain.md`** — `adjust_menu`-Absatz: `THEACTIVE`
  (Breite = Titelsumme, `ob_x` = 2 Zellen, Zelle 0 verboten), Box-Wort
  `0xFF1100`, Eintraege bei `(0, row)`, Box-x (datiert A100/A102).
* **`tos-api/tests/aes_a100_a102_desk_menu_geometry.rs`** (neu, 6 Faelle;
  Harness aus `aes_a87_a88_menu_ctwait_popup.rs` + Schirm-Snapshots und
  Tasten je Pump, modusbewusstes `fill_pattern` wie das A99-Harness, Cursor
  aus): **Replik** des Kernel-Desktop-Baums (gleiche Labels, gleiche
  `adjust_menu`-Arithmetik) — der Kernel-Crate ist `forced-target` AArch64
  und hat keine Hosttests; die Geometrie, die `build_menu` wirklich baut,
  belegt der Serial-Dump des QEMU-Laufs (`[a100] THEACTIVE=… FILEBOX=…
  spec=…`), die Hosttests pinnen das AES-Verhalten auf dieser Geometrie.
* **`measurements/measurement-2026-09-21-a100-a102/`** — RUNINFO, 24 Screendumps
  (Kernel-Desktop 8 Marken, TeraDesk 4 Marken, je vorher/nachher),
  4 Serial-Logs, Build-/Clippy-Logs, Hosttest-Quittungen, Methode
  (Skripte, Actions, Scratch-Patches, Produkt-Diff, HEAD-Kopien),
  `PIXDIFF.TXT`, SHA-256-Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach (`host-tests/`)

| Test | vor (HEAD `022d6d0`) | nach |
|---|---|---|
| `a100_hovering_an_entry_leaves_the_outside_border_and_the_other_rows_untouched` | rot: „no pixel outside entry 0 changes on the hover: 0 inverted, **194 other**" — der Repaint schwaerzte Zeile 303 und Spalten 68/225 | gruen |
| `a100_an_inside_border_box_gets_its_frame_inverted_by_the_item_xor_as_in_emutos` | rot: „row 19 inverted with entry 0 (no frame repaint)": `0` (schwarz) statt `0xFFFFFF` | gruen |

`cargo run -p xtask -- test` **804 / 0 / 1 → 811 / 0 / 1** (mit A102:
+6 Integration, +1 Modultest; `aes_menu_settings` erweitert).

## Build / Clippy (`build/`)

| | Ergebnis |
|---|---|
| `cargo xtask build --board rpi4` (tos-api + kernel frisch) | **tos-api 64 / kernel 94**, rc 0 — Delta 0 |
| `cargo clippy -p tos-api` | **80 → 80** (`menu.rs` 10 vorbestehende `Aespb`-Hinweise, `window.rs` 11 vorbestehend, `input.rs` 0); neue/geaenderte Testdateien 0 |
| qemu-virt | nicht gebaut (Lead) |

## QEMU raspi4b (`screendumps/`, `serial/`, `PIXDIFF.TXT`)

A87-Methode (`method/make_scratch_a100.sh` = rsync der Crates + a79_patch +
Injektor + a78_patch2 + key_step_patch + **tree_dump_patch** (Serial-Dump des
registrierten Baums); vorher = HEAD-Kopien via `git show`, nachher = Baum;
`tools/make-sd-image.sh` 64 MiB, sd-tera = + `#Z`-NEWDESK.INF; 640×480,
`-display none`, QMP 0.3 s nach jeder Marke; Zeiger vor Dumps unten rechts).

* Kernel-Desktop, Serial: vorher `[a100] THEACTIVE=(0,0,640,19)
  FILEBOX#11=(52,19,158,285) spec=0x11070`, nachher `THEACTIVE=(16,0,232,19)
  FILEBOX#11=(68,19,158,285) spec=0xff1100 ENTRY0=(0,0,158,19)`.
* F (File offen) / G (Hover „Close top window"): Umriss vorher Spalten
  52/209, Zeilen 19/303 (innen); nachher Spalten **67/226**, Zeilen
  **18/304** (aussen; Zeile 18 = Trennlinie der Leiste, wie EmuTOS/TeraDesk).
  Hover-Zeile y = 142: vorher `WWKKKK` ab x = 50 (Spalte 52 = vom Repaint
  gehaltener Rand, Inversion ab 53), nachher `WKKKKK` ab x = 66 (Spalte 67 =
  Rand, Inversion ab der Box-Spalte 68); rechts vorher `KKKWW` (Rand 209),
  nachher `KKKKW` (Rand 226). F/G-Diff 7980/8709 px = Box + 16 px Versatz.
* A/B/C/E/H: 1084 px = die Titel 16 px weiter rechts (`THEACTIVE.x`).
* TeraDesk (Aussenrand-Box): A/B/D **0 px**; C (Hover „Select all") 761 px
  = Spalten 64/239 (208/205 px) + Zeilen 19/242 (176/176 px): der alte
  innere Rahmen, weg.
* 0 `EXC`/`PANIC` in allen vier Serial-Logs.

## Nicht angefasst (Nebenbefunde)

* Titelbreite `strlen·8 + 4` und Boxbreite `maxlen·8 + 6`
  (`kernel/src/desk/menu.rs` `adjust_menu`); EmuTOS `strlen·8` und
  `(maxlen + 1)·8`, Desk-Menue mindestens `MIN_DESKMENU_WIDTH` = 20 Zellen
  (`deskmain.c:1519-1522,1605-1636`). desk/, eigener Task.
* `THEBAR` 19 px hoch (`MENU_H`); RSC `{0, 0, 80, 513}` = 1 Zelle + 2 px = 18
  (`desk_rsc.c:249-253`), `THEACTIVE` 769 = 19. Wirkung ungeprueft.
* Nach CR-Deselektion bleibt *Open* im File-Menue aktiv (Dump G, vorher =
  nachher): `men_update`/`count_selected` nach `desk_clear_all`. desk/.

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht).
