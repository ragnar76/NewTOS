# A71 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Tester-Verdikt offen

Die Menuetafel sichert den Schirm unter sich nicht mehr ueber `read_pixel`
(Stiftrundung), sondern device-genau ueber `bb_save`/`bb_restore` in den
einen AES-Kratzpuffer — zusammen mit A57 gebaut, 2026-09-20, Branch
`desk-rebuild`, Basis HEAD `172bca0`, **nicht committet**.
Siehe [`A57-STATUS-DONE.MD`](A57-STATUS-DONE.MD).

## Der Fehler, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemmnlib.c:208-223` `menu_sr` → `bb_save`/`bb_restore` (`gemgsxif.c:383-392` → `bb_set`, `vro_cpyfm(S_ONLY)` der Geraetepixel) |
| NewTOS (vorher) | `tos-api/src/aes/menu.rs` `save_under`/`restore_under`: `backend.read_pixel` (Palettensuche → Stift 0 fuer jede Nicht-Stift-Farbe) und `fill_rect(pen)` je Pixel; zwei Ablagen `MBACK`/`SMBACK` |
| NewTOS (nachher) | `menu.rs` `open_box` → `gsxif::bb_save` (der eine Puffer, `MENU_SAVE`), `erase_box_inner` → `Saved::restore`; Untermenue/Popup → `gsxif::popup_blit_save` (transienter Pool-Block, EmuTOS `popup_blit`); `read_pixel`-Pfad entfernt |

## Der Test: rot, dann gruen

`tos-api/tests/aes_screen_buffer_save_under.rs`, Fall
`drop_down_over_a_truecolor_pattern_restores_it_byte_for_byte_through_the_screen_buffer`:
Muster aus allen RGB565-Worten (fast kein Stift darunter) unter der Leiste,
`menu::open(File)` / `menu::close` (neue kopflose Seam), Schirm vorher/nachher
byteweise verglichen und der Fixture-Puffer auf Beschreibung geprueft.

* **vor** (HEAD-Worktree + Seam): `the screen after the drop-down differs ... first at (55,19): was 0x639642, is 0xFFFBFF`
  — der Stiftpfad brachte Weiss (Stift 0) zurueck.
* **nach**: gruen; `xtask test` 674 / 0 / 1.

## QEMU (`measurement-2026-09-20-a57/`)

TeraDesk View- und File-Tafel auf/zu (Injektor): Tafelflaeche exakt
zurueckgeschrieben (Differenz zum Ausgangsbild = nur der Zeiger), File-Tafel
0 Pixel zur A46-Referenz, vor/nach 0 Pixel — ueber Stiftfarben war der alte
Pfad verlustfrei; der Unterschied zeigt sich nur ueber Truecolor-Inhalt, den
der Hosttest liefert.

## Offen

* Nebenbefund `sr_rect` (Rechteckform `popup_blit` statt `menu_sr`) unveraendert.
* Untermenue bei offener Tafel (Verschachtelung ueber Pool-Block) nur zielseitig pruefbar.
* Tester-Verdikt steht aus; keine Hardware.
