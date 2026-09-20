# A99 — STATUS: DONE (Hostbeleg rot/gruen + QEMU-Selbstbeleg vorher/nachher) / Pi-Sichtabnahme offen

`objc_change` toggelt SELECTED jetzt per XOR wie EmuTOS' `ob_change`
(`aes/gemoblib.c:1253-1320`): aendert sich nur das SELECTED-Bit und
`xor_is_ok(type, flags, spec)` erlaubt es, wird das Objektrechteck — um die
`ob_sst`-Randdicke eingerueckt, negativ auf 0 geklemmt — mit **einem**
`bb_fill(MD_XOR, FIS_SOLID, IP_SOLID, …)` komplementiert und `just_draw`
uebersprungen; `G_IMAGE`-Deselektion XOR **und** Redraw (D-093a); `G_USERDEF`
→ `ob_user(prev, new)` ohne Redraw; `G_ICON` nie XOR; FL3DOBJ → Redraw.
Bearbeitet 2026-09-20/21, Branch `desk-rebuild`, Basis HEAD `30a7156`,
**nicht committet**. Spec: `docs/api/aes/object-form.md` § `objc_change` (47)
inkl. D-093a (unveraendert).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemoblib.c:1253-1320` (`ob_change`: `:1263-1264` Early-out, `:1270-1271` `!redraw`, `:1275`/`:1319` `gsx_moff`/`gsx_mon`, `:1277-1278` `th < 0 → 0`, `:1282-1284` `G_USERDEF` → `ob_user`, `redraw = FALSE`, `:1286-1287` `G_ICON` nie XOR, `:1295-1304` `bb_fill(MD_XOR, FIS_SOLID, IP_SOLID, t.g_x+th, t.g_y+th, t.g_w-2th, t.g_h-2th)` wenn `ok \|\| (G_IMAGE && !(new & SELECTED))`, `redraw = !ok`, `:1315-1316` `just_draw`); `:107-148` (`xor_is_ok`; `:114` FALSE fuer `G_IMAGE` oder `FL3DOBJ`); `:433-448` (`ob_user`); `aes/gemobjop.c:25-70` (`ob_sst`; `:40` Typmaske, `:44-46` `G_TITLE` `th = 1`); `aes/gemmnlib.c:170` (`do_chg` → `ob_change`, der Menue-Hilite-Pfad); `aes/gemfmlib.c:243` (`fm_button`-Radio-Sweep → `ob_change`) |
| NewTOS (vorher) | `tos-api/src/aes/object.rs` `change_clip_t`: nach dem Zustandswechsel immer `draw_tree(obj, 0)` = Redraw (nur `G_USERDEF` → `ob_user`). Mit A85s nachlaufendem XOR in `just_draw` bleibt bei fuellungslosen Objekten nach `objc_change(NORMAL, redraw)` der Balken stehen. Kompensationen: Flaechenfuellung des unselektierten `G_TITLE` in `just_draw`, weisse Vorfuellung in `menu.rs` `rehilite`/`popup_shown`, `redraw_bar_separator` in `close_box`/`menu_tnormal` |
| NewTOS (nachher) | `tos-api/src/aes/object.rs` (`change_clip_t`: der `ob_change`-Typ-Switch; `just_draw` `G_TITLE`-Zweig ohne Fuellung), `tos-api/src/aes/menu.rs` (Kompensationen entfernt) |

## Was gebaut ist

* **`tos-api/src/aes/object.rs`** (`measurement-2026-09-20-a99/method/product-a99-object-menu.diff`):
  * `change_clip_t` (Koerper von `objc_change` / `change` / `change_t`): nach
    Zustandsschreiben und `redraw`-Tor: Typ (`& 0x00ff`), Flags,
    `th = sst_th(obj).max(0)`; Cursor-Klammer unveraendert (A65); dann der
    `ob_change`-Switch mit der C-Variablen `redraw`:
    `G_USERDEF` (`spec != 0`) → bestehendes `ob_user(t, obj, x, y, w, h, spec,
    clip, curr, new)`, `redraw = false` (EL0-Upcall-Regeln unveraendert);
    `G_ICON` → Redraw (ein `G_CICON` nimmt wie in EmuTOS den `default`-Zweig);
    sonst bei `(new ^ curr) & SELECTED`: `ok = xor_is_ok(…)`; wenn `ok ||
    (G_IMAGE && !(new & SELECTED))` → `fill_pattern(x+th, y+th, w-2th, h-2th,
    {BLACK, WHITE, IP_SOLID, WriteMode::Xor})` (Schutz `w, h > 0` wie A85s
    nachlaufender Block), `redraw = !ok`. Danach `draw_tree(obj, 0)` nur bei
    `redraw` (= `just_draw`).
  * `just_draw` `G_TITLE`-Zweig: die Flaechenfuellung des *unselektierten*
    Titels (WHITE / GRAY_LT) ist weg — A85s dokumentierte
    Nicht-EmuTOS-Kompensation fuer das Redraw-Deselect.
* **`tos-api/src/aes/menu.rs`** (nur Kompensationen):
  * `rehilite` + `popup_shown`: die weisse `fill_rect` ueber den alten Eintrag
    vor `set_state_bit(old, SELECTED, false)` entfernt — mit dem XOR-Toggle
    zwingend (ein XOR ueber Weiss liesse den Eintrag schwarz). Der
    Rahmen-Repaint danach **bleibt**: der Kernel-Desktop-Dropdown hat einen
    *inneren* Rand (`th = +1`, `kernel/src/desk/menu.rs` `build_menu`
    `(1<<16)|0x1070`), den der zeilenbreite Eintrags-XOR kreuzt; EmuTOS'
    Desktop-Box hat einen aeusseren (`0xFF1100`, `desk/desk_rsc.c:294`) —
    desk/, nicht dieses Ticket.
  * `close_box`: kein `redraw_bar_separator`; `menu_tnormal`: kein
    `title_touches_separator`/`redraw_bar_separator`; beide Helfer geloescht —
    der Titel-XOR ist um `th = 1` eingerueckt und erreicht die Trennlinie nie;
    die einzige Loeschung dieser Zeile war die entfernte Fuellung.
* **`tos-api/tests/aes_objc_change_toggle.rs`** (neu, 12 Tests): Harness von
  `aes_object_selected_xor.rs` (modusbewusster XOR, Kernel-555-Palettendecode)
  plus **Zeichenoperations-Log** (`FillRect`/`Line`/`FillPattern{x,y,w,h,mode,
  solid}`/`Glyph`/`BlitMono`/`CopyRect`/`PutDevicePixel`). Baum = 2 Objekte
  (Elternbox `(200,90,70,40)`: FL3DBAK `0x00ff1100` = LWHITE-Flaeche oder
  hohl-weiss; Kind `(220,100,24,16)`); der AES schreibt `ob_state` durch
  `as_mut_ptr()` (ein `as_ptr()` eines nicht-`mut`-Arrays ist UB und liess den
  Optimierer den alten Zustand lesen — vor dem archivierten Rotlauf behoben).
* **`tos-api/tests/aes_screen_buffer_save_under.rs`** — nur Harness:
  `BufBackend::fill_pattern` ist jetzt modusbewusst (XOR komplementiert das
  Ziel, wie sein eigenes `abline`); es malte jedes Muster opak, was mit dem
  XOR-Toggle den un-hervorgehobenen File-Titel schwarz liess (A85 hatte das
  Harness-Artefakt unter „nicht angefasst“ gelistet). Erwartung (bytegleicher
  Schirm nach dem Dropdown) unveraendert
  (`method/test-harness-save-under-fill-pattern-mode.diff`).
* **`docs/STATUS.md`** — Phase-13-Fragment + eine Zeile „Recent Decisions“
  (2026-09-20). **`TASK/A99.md`** — Status + Abschluss.
* **`measurement-2026-09-20-a99/`** — RUNINFO, 30 Screendumps (3 Szenarien ×
  vorher/nachher), 6 Serial-Logs, Build-/Clippy-Logs, Hosttest-Quittungen,
  Methode (Skripte, Aktionen, Produkt-/Harness-Diff), SHA-256-Manifest. Keine
  `*.img`.

## Tests: rot vor, gruen nach

| Test (`aes_objc_change_toggle.rs`) | vor (HEAD `30a7156`) | nach |
|---|---|---|
| (a) `a99_unboxed_g_ftext_deselect_restores_the_parent_ground_and_glyphs` | rot: `(220,100)` war LWHITE `0xBDBDBD`, ist `0x424242` = `~LWHITE` (Balken bleibt) | gruen |
| (b) `a99_g_ibox_deselect_restores_the_ground` | rot: dito bei `(221,101)` | gruen |
| (c) `a99_g_string_select_deselect_round_trip_is_pixel_identical` | rot: `(220,100)` war WEISS, ist SCHWARZ | gruen |
| (d) `a99_selected_toggle_on_an_xor_able_object_is_exactly_one_xor_fill` | rot: Ops `[Glyph, Glyph, FillPattern XOR]` statt `[FillPattern XOR]` | gruen |
| (d) `a99_g_string_toggle_is_exactly_one_xor_fill` | rot: dito | gruen |
| (d) `a99_inside_border_insets_the_xor_by_th` (`G_BOX` `th = 2` → XOR `(222,102,20,12)`) | rot: 4 Rand-`FillRect` + Flaechen-`FillRect` + XOR | gruen |
| (e) `a99_g_image_deselect_xors_and_redraws` (D-093a) | rot: Ops `[BlitMono]`, kein XOR vor dem Redraw | gruen |
| (f) `a99_fl3dact_button_toggle_redraws_without_a_bare_xor` (Waechter) | gruen | gruen |
| (g) `a99_g_icon_toggle_redraws_without_xor` (Waechter) | gruen | gruen |
| (h) `a99_redraw_zero_stores_the_state_and_draws_nothing` (Waechter) | gruen | gruen |
| (i) `a99_unchanged_state_is_a_no_op` (auch SELECTED→SELECTED kein Doppel-XOR) | gruen | gruen |
| (j) `a99_g_userdef_toggle_issues_no_generic_draw` (Waechter) | gruen | gruen |

`cargo run -p xtask -- test` **748/0/2 → 760/0/2** (Basis 748 + 12;
`host-tests/xtask-test-BASELINE-HEAD-30a7156.log`, `xtask-test-AFTER.log`,
`cargo-test-no-fail-fast-AFTER.log`). Keine bestehende Erwartung geaendert:
Menue-Schirmpuffer (`drop_down_over_a_truecolor_pattern_restores_it_byte_for_byte…`
verlangt weiter den bytegleichen Schirm nach dem Dropdown — der Titel wird
per XOR normal), Cursor-Klammer, A85-XOR-Matrix, 3D-Box/Button, CICON, Font,
Nicelines — alle gruen.

## Build / Clippy

| | Ergebnis |
|---|---|
| `cargo xtask build --board rpi4` (tos-api + kernel neu uebersetzt) | rustc **tos-api 64 / kernel 94**, rc 0 — Delta 0 (`build/xtask-build-rpi4.log`) |
| `cargo clippy -p tos-api` | **80 → 80** (A85-Basis 80; `object.rs` 16 → 16, `menu.rs` 10 → 10 Treffer); `--test aes_objc_change_toggle`/`aes_screen_buffer_save_under`: 0 Warnungen |
| qemu-virt | nicht gebaut (Lead) |

## QEMU raspi4b (`measurement-2026-09-20-a99/`)

A85/A79-Methode (`method/make_scratch_a99.sh` = rsync der Crates +
`a79_patch.py` + Injektor mit `a99_scripts.rs`/`a99_scripts2.rs` +
`a78_patch2.py`; `tools/make-sd-image.sh 64` → sd-base; `sd-put.sh` mit
`newdesk-tera.inf`/`teradesk-window.inf` → sd-tera; `run.sh`, 640×480,
`-display none`, QMP 0.3 s nach jeder Marke). **Je Szenario zwei Kerne**:
vorher = Scratch mit `object.rs` + `menu.rs` = HEAD `30a7156` (`git show`,
kein Checkout), nachher = Baum; gleiche Skripte, gleiche Images. Zeiger vor
jedem Dump unten rechts geparkt (ausser gehaltenes OK). 0 EXC/PANIC in allen
6 Serial-Logs.

| Lauf | Differenz vorher/nachher |
|---|---|
| TeraDesk „View → Set mask…“, Klick Zeile 1 (`tera-*-D-row1.png`), Zeile 2 (`E-row2`), Zeile 3 (`F-row3`) | **0 px** in allen 6 Paaren (A–F). Pixelklassen der vier Zeilen (`320..415 × 209..224/225..240/241..256/257..272`), in beiden Kernen gleich: nach dem Dialogaufbau Zeile 1 = 1340 px `(66,66,66)` = `~LWHITE` + 196 weisse Glyphpixel, Zeilen 2–4 LWHITE `(189,189,189)` + schwarze Glyphen; nach Klick 2: Zeile 1 LWHITE + 196 schwarze, Zeile 2 1404 px `~LWHITE` + 132 weisse; nach Klick 3: Zeile 3 1329 px `~LWHITE` + 207 weisse, Rest LWHITE — **genau eine Zeile invertiert, auch vorher** |
| Kernel-Desktop File-Menue (`desk-*-A..D`) | **0 px** in allen 4 Paaren; `D-closed == A-idle` (0 px) in beiden Kernen: Titel nach dem Schliessen normal, Trennlinie intakt **ohne** `redraw_bar_separator` |
| Desktop-Info-Alarm, OK gehalten (`about-*-D-ok-held.png`), losgelassen (`E`) | **0 px** in allen 5 Paaren; `D` gegen `C` (nachher): 388 px = gedrueckter FL3DACT-OK + Zeiger — Zeile y=277: `lllll###.` → `lllll####` (linke Bevel-Linie WEISS → SCHWARZ), `d###` → `.###` (rechte LBLACK → WEISS), Spalte x=300 analog = gesenkter Bevel per Redraw (`add_3d_effect` `:252/:270-272`), kein nackter XOR; `E` gegen `A`: 51 px = nur die zwei Zeigerpositionen (Alarm zu, Desk-Titel normal). Der Injektor **kann** halten (`Dn … Mark … Up`) |

**Gegenbefund zur Ticket-Praemisse** („Klick auf eine andere Zeile laesst
den ersten Balken stehen“): das ist per Geste in TeraDesk **nicht**
sichtbar, auch nicht auf dem Vorher-Kern. Der Serial-Trace (`arm` vor den
Zeilenklicks) zeigt den Pfad `evnt_button (25) → objc_find (43, out 6/7/8)
→ form_button (56 — der AES-Radio-Sweep, NewTOS `form.rs` → object::change
auf FTYPE1 deselect / FTYPEn select) → wind_update → objc_draw(21 FILETYPE,
0) → objc_draw(5 FTPARENT, 8) → objc_draw(12 Slider, 8) ×2`: TeraDesk
zeichnet nach jedem TOUCHEXIT-Zeilenklick die ganze FL3DBAK-Box neu
(`sdk/apps/teradesk/lists.c:849-869` `case FTYPE1..4` → `redraw = TRUE` →
`:902-908` `set_lselector(TRUE)` → `xd_drawdeep(FTPARENT)`), was Flaeche
und Zeilen im neuen Zustand malt. Der Zustand „beide Balken“ lebt nur
zwischen den zwei `change()`-Aufrufen des Sweeps und diesem Redraw
(Millisekunden). A85s Aussage war eine ungemessene Vorhersage
(A85-Quittung: „nicht gemessen“). Ein Scan von `desktop.rsc` nach
waehlbaren fuellungslosen Objekten findet nur diese vier Zeilen
(`SELECTABLE|RBUTTON|TOUCHEXIT`) und eine FL3DBAK-`G_IBOX` in Baum 4 —
TeraDesk kann den Defekt per Geste nicht zeigen. Der rohe
`objc_change`-Pfad (EmuTOS `fm_button` `gemfmlib.c:243`; jede App mit
`form_do`-Radio auf fuellungslosen Zeilen) ist im Hosttest (a)–(d) rot/gruen
belegt; die QEMU-Dumps sind der Regressionsbeleg (bytegleich).

Nicht gelaufen: QED; ein Szenario, das den Zwischenzustand dumpen koennte,
gibt es mit den vorhandenen Apps nicht.

## Nicht angefasst / offen

* **Pi-Sichtabnahme offen** (Tester/Lead; nichts geflasht).
* Kernel-Desktop-Dropdown mit innerem Rand (`th = +1`) gegen EmuTOS' aeusseren
  (`0xFF1100`): Grund fuer den verbleibenden Rahmen-Repaint in
  `rehilite`/`popup_shown`. desk/, eigenes Ticket, falls gewuenscht.
* EmuTOS' `bb_fill` mit `w - 2th <= 0` fuellt ein normalisiertes Rechteck
  (`arb_corner`); `change_clip_t` schuetzt wie A85s nachlaufender Block.
  Degeneriert (Objekt schmaler als der doppelte Innenrand).
* Der XOR laeuft vor `just_draw`s HIDETREE-/`spec == -1`-Reject, wie
  EmuTOS' `ob_change` (`:1301` vor `:491`): `objc_change(SELECTED, redraw)`
  auf einem HIDETREE-Objekt invertiert dessen Rechteck. 1:1.
* Schwester-Harnesse (`aes_object_3d_box_border.rs`, `aes_window_color_words.rs`,
  …) behalten den 6-Bit-Gruen-Decode / das opake `fill_pattern` (A85 (5)/(6));
  `aes_object_cursor_bracket.rs` liest die SELECTED-Hohlbox weiter ueber
  `fg = BLACK` des XOR-Fills SCHWARZ — gruen, nicht angefasst.
* `ob_user`s EL0-Upcall-/Kernel-built-Trennung unveraendert.
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
