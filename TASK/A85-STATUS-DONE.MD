# A85 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg vorher/nachher) / Pi-Sichtabnahme offen

`just_draw` zeichnet SELECTED jetzt nach EmuTOS' Modell: `changecol` aus den
3D-Sysvars (`FL3DOBJ`) bzw. `!xor_is_ok(type, flags, spec)`, die Farbtausche
im Typ-Zweig nur unter `changecol`, danach der nachlaufende XOR ueber `t`
(nach dem Einzug um ein positives `th`) nur fuer `!FL3DOBJ && !changecol`;
`xor_color` ist die `vdi16comp`-Tabelle. Bearbeitet 2026-09-20, Branch
`desk-rebuild`, Basis HEAD `25c72a7`, **nicht committet**. Spec (Researcher,
uncommitted, unveraendert gelassen): `docs/api/aes/object-form.md` Punkt 8 +
D-104-Blockzitat, `docs/api/_discrepancies.md` D-104.

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemoblib.c:504-525` (`changecol`/`movetext`: FL3DOBJ → `actcolchange`/`indcolchange`, sonst `!xor_is_ok`), `:107-148` (`xor_is_ok`), `:83-86`/`:93-102` (`vdi16comp`/`xor_color`), `:681-690` (Innentausch), `:699-706` (Texttausch + `MD_TRANS`), `:790` (Ikone loescht SELECTED), `:793` (`ob_user` gibt den Zustand zurueck), `:805-808` (Button-Label WHITE), `:889-890` (Einzug um `th > 0`), `:932-943` (`if (state & SELECTED) if (!(flags & FL3DOBJ) && !changecol) bb_fill(MD_XOR, …)`), `:950-951` (Bevel); `gemobjop.c:44-46` (`G_TITLE` `th = 1`); `vdi/vdi_line.c:484-495` (`WM_XOR` = `~temp`, Komplement des Geraetepixels) |
| NewTOS (vorher) | `tos-api/src/aes/object.rs` `just_draw`: `changecol = true` fuer alles ausser FL3DOBJ; kein nachlaufender XOR; Inversion je Typ eingefaltet (`G_STRING` schwarze Fuellung + weisser Text, `G_TITLE` volle Fuellung, Text-Zweig bedingungsloser Tausch); `xor_color` = `pen ^ 1` |
| NewTOS (nachher) | `tos-api/src/aes/object.rs` (`xor_is_ok`, `xor_color`, `VDI4COMP`/`VDI16COMP`, Stiftkonstanten, `just_draw` Vorspann + Zweige + nachlaufender Block) |

## Was gebaut ist

* **`tos-api/src/aes/object.rs`** (`measurements/measurement-2026-09-20-a85/method/product-a85-object-rs.diff`):
  * `xor_color(backend, pen)`: ≥ 16 oder ≥ `num_colors()` → WHITE; ≤ 4 Farben
    → `VDI4COMP`; sonst `VDI16COMP` (`:93-102`). `xor_is_ok(t, type, flags,
    spec, backend)` (`:107-148`): TEDINFO-`te_color` an der Baum-ABI gelesen,
    `G_BOX`/`G_BOXCHAR` das Low-Word von `ob_spec`, Innen- **und** Textfarbe ≤
    BLACK; `default:` TRUE. Ein `ob_spec == 0` im Text-Zweig → TRUE (der Zweig
    zeichnet dafuer ohnehin nichts; EmuTOS wuerde den Nullzeiger lesen).
  * Vorspann: `(movetext, changecol) = if d3obj { FL3DACT ? act : ind } else
    { (false, !xor_is_ok(…)) }`; das `(fill, ink)`-Tupel ist weg.
  * Box-Zweig: Innentausch wie gehabt unter `selected && changecol`;
    Button-Label BLACK, WHITE nur unter `selected && changecol` (`:805-808`).
  * `G_STRING`: keine Fuellung, Text/Haekchen BLACK — der nachlaufende XOR
    macht den Balken. `G_TITLE`: keine SELECTED-Fuellung, `deco_th = 1`, Text
    BLACK; die Fuellung des **unselektierten** Titels bleibt als
    **Nicht-EmuTOS-Kompensation** (kommentiert) fuer `object::change`, das
    statt zu XOR-toggeln neu zeichnet (`ob_change` `:1295-1304`, A99) —
    `menu.rs` (`close_box`/`menu_tnormal`/`redraw_bar_separator`) unveraendert.
  * Text-Zweig: Innentausch nur `selected && changecol && is_box`
    (`:669-693`), Texttausch nur `selected && changecol` (`:699-706`).
  * `G_IMAGE`: `xor_color(backend, …)`; `G_ICON`/`G_CICON`: `ob_state &=
    !SELECTED` nach `draw_icon` (`:790`).
  * Nachlaufender Block nach DISABLED: `if ob_state & SELECTED && !d3obj &&
    !changecol` → `fill_pattern(t inset deco_th (wenn > 0), PatternFill { fg
    BLACK, bg WHITE, IP_SOLID, WriteMode::Xor })`; `deco_th` ist das
    endgueltige `th` des Zweigs (Titel 1, promovierter `G_TEXT` 0, Box das
    Spec-Byte, Button negativ). `G_USERDEF`: der zurueckgegebene Zustand
    speist den Block — ein Userdef, der SELECTED zurueckgibt, wird invertiert
    (PLAUSIBLE, faellt natuerlich heraus; nicht gesondert gemessen).
  * `add_3d_effect(…, selected, …)` unveraendert (Lead).
* **`tos-api/tests/aes_object_selected_xor.rs`** (Lead): drei Korrekturen
  (`method/test-file-changes-vs-lead-version.diff`), keine Erwartung
  geschwaecht — (1) Harness-Farbdecode: `device_to_rgb565`/`rgb565_to_device`
  dekodierten das 555-in-565-Palettenwort (`seed_palette` packt gruen `<< 6`)
  als 6-Bit-Gruen, WHITE war `0xFFFBFF`, XOR(BLACK) = `0xFFFFFF` traf keinen
  Stift, XOR(WHITE) = `0x000400` war nicht BLACK — kein S/W-XOR-Feld konnte
  je treffen; jetzt die Kernelkonvention (`VdiScreen::device_to_rgb565`,
  `pal555_to_xrgb`). (2) Titel-Test: die letzte Zeile verlangte x 236..243
  ganz `#`, im Widerspruch zur Zeile davor („Spalte 243 bleibt uninvertiert“);
  EmuTOS `gr_inside(&t, 1)` invertiert 221..242 → `"#######."`. (3)
  `FL3DACT`-`G_BOXTEXT`: der „kein WEISS im Glyphbereich“-Scan lief y 98..117
  und schloss die Bevel-Unterkante y = 117 ein, die EmuTOS WEISS zeichnet
  (LWHITE ginge in der LWHITE-Flaeche unter, `add_3d_effect` `:270-272` —
  dieselbe Rueckfallregel, die der `FL3DACT`-`G_BUTTON`-Waechter festnagelt);
  jetzt die 16-zeilige Glyphzelle y 100..115.
* **`docs/STATUS.md`** — Phase-13-Fragment + eine Zeile „Recent Decisions“
  (2026-09-20). **`TASK/A85.md`** — Status + Abschluss.
* **`measurements/measurement-2026-09-20-a85/`** — RUNINFO, 40 Screendumps (je Lauf
  vorher/nachher), 16 Serial-Logs, Build-/Clippy-Logs, Hosttest-Quittungen,
  Methode (Skripte, Aktionen, fsel-Vorschau-Patch, Produkt-Diff),
  SHA-256-Manifest. Keine `*.img`.

## Tests: rot vor, gruen nach

| Test (`aes_object_selected_xor.rs`) | vor (HEAD `object.rs`, Hand-Tausch) | nach |
|---|---|---|
| `a85_selected_fl3dbak_g_box_is_xor_inverted` (Ticket-Fall 1) | rot: Innenraum `d` (LBLACK-Stift) statt `L` (`~LWHITE`) | gruen |
| `a85_selected_fl3dbak_g_boxtext_is_xor_inverted` | rot | gruen |
| `a85_selected_fl3dind_g_box_changes_colour_without_xor` (Ticket-Fall 2) | gruen | gruen |
| `a85_selected_fl3dind_xor_color_uses_the_complement_table` | rot: `g` (GREEN = `RED ^ 1`) statt `c` (LCYAN) | gruen |
| `a85_selected_fl3dact_g_button_keeps_face_moves_text_sinks_bevel` | gruen | gruen |
| `a85_selected_fl3dact_g_boxtext_keeps_face_and_black_text` | rot: Flaeche getauscht | gruen |
| `a85_selected_non3d_g_ftext_is_xor_inverted_as_a_block` (TeraDesk SETMASK) | rot: `l`-Grund unveraendert | gruen |
| `a85_selected_non3d_g_text_is_xor_inverted_as_a_block` | rot | gruen |
| `a85_selected_fl3dbak_g_string_is_xor_inverted_not_black_on_black` | rot | gruen |
| `a85_selected_fl3dind_g_string_stays_black_on_the_ground` | rot: schwarze Fuellung | gruen |
| `a85_selected_g_title_xor_is_inset_by_th_one` | rot: Zeile 0 invertiert | gruen |
| `a85_selected_non3d_g_ibox_is_xor_inverted` | rot: nichts invertiert | gruen |
| `a85_selected_non3d_patterned_g_box_inverts_the_dither` | rot: solide weiss | gruen |
| `a85_selected_fl3dbak_g_button_reads_inverted` (Waechter) | gruen | gruen |

`cargo test -p tos-api -p tos-types -p xtask --no-fail-fast` **737/11/2 →
748/0/2** (Basis 734 + 14; `cargo run -p xtask -- test` bricht beim ersten
roten Binary ab, daher `--no-fail-fast` fuer die Vorher-Zahl). Keine
bestehende Erwartung geaendert: Menue-Schirmpuffer, Cursor-Klammer, 3D-Box/
Button, CICON, Font, Nicelines — alle Pixeltests unveraendert gruen (die
hohl-weissen S/W-Faelle sind pixelgleich, wie das Ticket verlangt).

## Build / Clippy

| | Ergebnis |
|---|---|
| `cargo xtask build --board rpi4` (tos-api + kernel neu uebersetzt) | rustc **tos-api 64 / kernel 94**, rc 0 — Delta 0 (`build/xtask-build-rpi4.log`) |
| `cargo clippy -p tos-api` | **81 → 80** (Basis per Hand-Tausch der HEAD-`object.rs` gemessen, `build/clippy-tos-api-host-BEFORE-HEAD-object-rs.txt`; `object.rs` 17 → 16: ein vorbestehendes collapsible-if im Ikonen-Zweig ist durch die neue Anweisung dahinter keines mehr); `--test aes_object_selected_xor`: 0 Treffer in der Testdatei |
| qemu-virt | nicht gebaut (Lead) |

## QEMU raspi4b (`measurements/measurement-2026-09-20-a85/`)

A79-Methode (`make_scratch_a85.sh` = rsync der Crates + `a79_patch.py` +
Injektor + `a78_patch2.py`; `tools/make-sd-image.sh 64` → sd-base; `sd-put.sh`
mit `newdesk-tera.inf`/`teradesk-window.inf` → sd-tera; `run.sh`, 640×480,
`-display none`, QMP 0.3 s nach jeder Marke). **Je Lauf zwei Kerne**: vorher
= Scratch mit `object.rs` = HEAD `25c72a7`, nachher = Baum; gleiche Skripte,
gleiche Images, also isoliert der Pixeldiff diese Aenderung. Zeiger fuer die
Titel-Dumps unten rechts geparkt. 0 EXC/PANIC in allen 16 Serial-Logs.

| Lauf | Differenz vorher/nachher |
|---|---|
| Kernel-Desktop File-Menue offen (`desk-*-B-file-open.png`) | **86 px**, bbox (52,0)-(104,18) = nur das Titelrechteck: Zeile y=0 (52 px) und die Spalten x=52/x=103 in y=1..17 BLACK → WHITE; Zeile y=18 (Trennlinie der Leiste) bleibt BLACK (vorher von der Vollfuellung gemalt, jetzt vom Einzug verschont) = EmuTOS' `(x+1, y+1, w-2, h-2)` |
| … Eintrag „Search…“ gehovert (`desk2-*-C-item-hover.png`) | 86 px = derselbe Titel; der `G_STRING`-Balken **bytegleich** |
| … nach Klick ausserhalb (`*-D-closed.png`) | **0 px** (Titel wieder normal, Trennlinie intakt) |
| TeraDesk File-Menue offen (`tera-*-B-file-open.png`) | **82 px**, Titel 64..111 (48 + 2×17); gegen `measurements/measurement-2026-09-17-h03/screendumps/run2-shipped-a-file-menu-open.png`: 118 = 82 + 36 Zeigerpixel — Zeile 0 und die Spalten 64/111 in Zeilen 1..17 jetzt weiss, Zeile 18 in allen drei die schwarze Trennlinie |
| … „Open…“ gehovert | 82 px = Titel; der Eintrag bytegleich |
| TeraDesk „View → Set mask…“ (`tera2-*-C-setmask.png`) | 1422 px = 82 (View-Titel, unter dem modalen Dialog noch SELECTED) + **1340 in der Listenzeile FTYPE1** (320..415 × 209..224; non-3D `G_FTEXT` 0x1100 auf der FL3DBAK-Box): vorher LWHITE (189,189,189) mit 196 weissen Glyphpixeln, nachher `~LWHITE` (66,66,66) mit denselben 196 weissen Glyphpixeln = der XOR-Balken (`vdi_line.c:494`) |
| Desktop-Info-Alarm (`about-*-C-about-alert.png`, A68-A70-Skript) | 86 px = nur der Desk-Titel; der Alarm (FL3DACT-OK, FL3DBAK-Flaeche) **bytegleich** (Waechter) |
| fsel-Vorschau (`fsel-*.png`, `fsel::preview()` vor `sh_main`, Scratch-Patch) | **0 px** (Laufwerk C SELECTED-FL3DIND: Farbwechsel + gesenkter Bevel, OK/Cancel FL3DACT) |

Nicht gelaufen: QED (Datei-Oeffnen per Injektor nicht billig; keine
QED-Objektklasse aendert sich ueber die allgemeine Regel hinaus); der
Kernel-Desktop-Dialog „Set file mask…“ (Editfeld, keine waehlbare Liste).

## Nicht angefasst / offen

* **Pi-Sichtabnahme offen** (Tester/Lead; nichts geflasht).
* **A99 — `object::change` zeichnet statt zu XOR-toggeln** (`ob_change`
  `:1295-1304`), nicht Teil dieses Tickets. Mit dem nachlaufenden XOR wird
  das fuer fuellungslose Objekte **sichtbar**: ein unboxed `G_TEXT`/`G_FTEXT`,
  `G_IBOX` oder `G_USERDEF`, per `objc_change(SELECTED, redraw)` richtig als
  Balken gezeichnet, behaelt nach `objc_change(unselected, redraw)` den
  Balken (der Text wird ueber den invertierten Grund gemalt statt
  zurueckgetoggelt), bis das Elternobjekt neu gezeichnet wird. TeraDesks
  SETMASK-Zeilen sind genau diese Klasse: der erste Aufbau stimmt
  (gemessen), ein zweiter Klick auf eine andere Zeile laesst nach Erwartung
  den ersten Balken stehen (nicht gemessen). `G_STRING` (`menu.rs`
  `rehilite` malt vorher WEISS) und `G_TITLE` (Fuellung im Zweig behalten)
  sind wie bisher kompensiert. `xor_is_ok` liegt jetzt in `object.rs` und
  ist der Baustein fuer A99.
* `G_USERDEF`-SELECTED-XOR: faellt natuerlich heraus (der Zweig gibt den
  Zustand zurueck, der Block wertet ihn aus); PLAUSIBLE, kein eigener Test.
* `add_3d_effect` bekommt weiter `selected` vom Anfang von `just_draw`;
  EmuTOS uebergibt den vom Zweig hinterlassenen Zustand (`:951`). Lead: unveraendert.
* CROSSED/DISABLED nutzen das uneingezogene Rechteck; EmuTOS zieht `t`
  einmal fuer den ganzen Zustandsblock ein (`:889-890`). Vorbestehend.
* `gsx_tblt` schreibt immer transparent; EmuTOS' `tmode = MD_TRANS` unter
  `changecol` hat keinen Gegenpart. Vorbestehend.
* Die Schwester-Harnesse (`aes_object_3d_box_border.rs`,
  `aes_window_color_words.rs`, …) tragen denselben 6-Bit-Gruen-Decode;
  dort ohne Wirkung (kein XOR), nicht angefasst. `aes_object_cursor_bracket.rs`
  malt jedes `fill_pattern` opak (modusblind) und trifft das richtige Bild
  ueber `fg = BLACK` des XOR-Fills — nicht angefasst.
* Kein Commit, kein `git checkout/stash/reset`, nichts geflasht.
