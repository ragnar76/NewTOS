# A108 — STATUS: DONE (Replik-Hosttest + Serial-Dump + QEMU-Selbstbeleg) / Pi-Sichtabnahme offen

Der Kernel-Desktop baut sein `ADMENU` jetzt mit `adjust_menu`s Massen: Titel
`strlen` Zellen, Boxen `maxlen + 1` Zellen (Desk-Box mindestens
`MIN_DESKMENU_WIDTH` = 20), `THEBAR` `gl_hchar + 2` = 18 px. Die
Leistenzeile des QEMU-Dumps ist gegen die Hatari-Falcon-Referenz
pixelgleich. Bearbeitet 2026-09-21, Branch `desk-rebuild`, Basis HEAD
`a570123`, **nicht committet**, nichts geflasht. Gemeinsames Paket mit
A103/A105 (`measurement-2026-09-21-a103-a108/`).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `desk/deskmain.c:80` (`MIN_DESKMENU_WIDTH 20`), `:1509` (Einheit `CHAR_WIDTH`), `:1528-1534` (`title->ob_x = x; title->ob_width = n; x += n; mbar->ob_width = x`), `:1602-1603` (`m = MIN_DESKMENU_WIDTH - 1` fuer die erste Box, dann 0), `:1608-1614` (breitestes Item), `:1617` (Box-x), `:1620` (`m++`), `:1626-1631` (Rechtsrand: `ob_x = width - m`, `m = (m-1) \| ((CHAR_WIDTH-1)<<8)`), `:1633-1635` (`item->ob_width = m; dropbox->ob_width = m`), `:1895-1900` (vor `rsrc_obfix`); `desk/desk_rsc.c:249-253` (`THEBAR {0,0,80,513}` = 1 Zelle + 2 px), `:255-289` (`THEACTIVE`/Titel/`THEMENUS` `769`), `:297-341` (Eintraege `{0,row,20,1}`); `aes/gemrslib.c:53-99` (`fix_chpos`: `gl_wchar`/`gl_hchar`, Offsetbyte); `aes/gemmnlib.c:700-731` (`mn_bar`: `ob_draw(THEBAR)`, Trennlinie `gsx_cline(0, gl_hbox-1, …)`); `aes/gemgraf.c:412-415` (`gl_hbox = gl_hchar + 3`) |
| NewTOS (vorher) | `kernel/src/desk/menu.rs` `adjust_menu`: Titel `strlen·8 + 4`, Box `maxlen·8 + 6`, kein Mindestmass, Rechtsrand nur `min(sw − bw)`; `build_menu`: `THEBAR` `MENU_H` = 19 |
| NewTOS (nachher) | `MIN_DESKMENU_WIDTH`, `BAR_BOX_H = CELL_H + 2`; `adjust_menu` in Zellen·8 mit Seed, `m++`, Rechtsrand-Verschiebung + 1-px-Kuerzung; `THEBAR` 18 |

## Was gebaut ist

* **`kernel/src/desk/menu.rs`** — Konstanten `MIN_DESKMENU_WIDTH` (20),
  `BAR_BOX_H` (`CELL_H + 2`, Doc mit `desk_rsc.c:249-253`, `gemrslib.c:76-82`,
  `gemmnlib.c:718-722`); `adjust_menu`: `tw = strlen · CHAR_W`; `m =
  fold(seed, max) + 1`, `box_w = m · CHAR_W`; Rechtsrand `if bx + bw >= sw
  { bx = sw − bw; bw −= 1 }`; `build_menu`: `o[1]` (THEBAR) `BAR_BOX_H` hoch.
  Doc-Kommentar mit allen Zeilen.
* **`tos-api/tests/aes_a100_a102_desk_menu_geometry.rs`** — Replik-Arithmetik
  auf die EmuTOS-Formeln (`BAR_BOX_H`, `MIN_DESKMENU_WIDTH`); Literale der
  A100/A102-Faelle nachgezogen (File-Box `(68,19,158,285) → (64,19,160,285)`,
  `THEACTIVE` 232 → 216, Options-Box `(172,214) → (160,216)`, Kante 248 →
  232); +2 Faelle: `a108_titles_and_boxes_measure_as_adjust_menu_computes`
  (Titel (0,48),(48,48),(96,48),(144,72); `THEACTIVE` (16,0,216,19); Boxen
  160/160/136/216 bei 16/64/112/160; `THEBAR` 18; jeder Eintrag boxbreit) und
  `a108_bar_row_glyphs_sit_where_the_falcon_reference_has_them` (Tinten-Bbox
  und -Zaehler je Titel in den Leistenzeilen 0..17 = `falcon-tc-desktop.png`:
  Desk 25..54/3..14/173, File 73..102/3..14/135, View 121..150/3..14/155,
  Options 169..222/3..16/273; nichts rechts von 232; Zeile 17 weiss, 18
  Trennlinie).
* **`docs/api/desktop/deskmain.md`** — `adjust_menu`-Absatz: Breiten in
  Zellen, Seed, `m++`, Rechtsrand, `THEBAR` 18, Eintragshoehe 16 (Rest).

## Tests

Der Kernel-Crate ist `forced-target` AArch64 ohne Hosttests: der Replik-Test
pinnt die Formelwerte und die AES-Zeichnung der Leistenzeile auf dieser
Geometrie; er ist an HEAD **nicht rot** (die Replik liegt in der Testdatei).
Die Kernel-Geometrie belegt der Serial-Dump unten. `cargo run -p xtask --
test` **811 / 0 / 1 → 821 / 0 / 1** (mit A103/A105). Frischer `cargo xtask
build --board rpi4`: **tos-api 64 / kernel 94**, Warnungsmenge zeilengleich
mit dem A100-Log; Clippy tos-api 80 → 80; kein qemu-virt-Build (Lead).

## QEMU raspi4b (`screendumps/`, `serial/`, `PIXDIFF.TXT`)

A100-Methode (`method/make_scratch_a108.sh`, Injektor + `tree_dump_patch.py`
A108-Variante; vorher = HEAD-Kopien via `git show`, nachher = Baum; 640×480,
Zeiger unten rechts bei A/H; 0 EXC/PANIC).

* Serial vorher `[a108] THEBAR=(0,0,640,19) THEACTIVE=(16,0,232,19)
  titles=(0,52),(52,52),(104,52),(156,76)`, Boxen 150/158/134/214 bei
  16/68/120/172; nachher `THEBAR=(0,0,640,18) THEACTIVE=(16,0,216,19)
  titles=(0,48),(48,48),(96,48),(144,72)`, Boxen 160/160/136/216 bei
  16/64/112/160 (`spec=0xff1100`, `ENTRY0=(0,0,box_w,19)`).
* Leistenzeile (Zeilen 0..18) gegen `falcon-tc-desktop.png`: vorher **840
  px** (Bbox (73,3)-(234,16): File/View/Options 4/8/12 Spalten zu weit
  rechts), nachher **0 px**; Tinten-Bboxen/-Zaehler je Titel identisch mit
  der Referenz; Trennlinie Zeile 18, Zeile 17 weiss.
* F (File offen): Box `(64,19,160,285)`, Umriss Spalten 63/224, Zeilen 18/304
  (vorher `(68,19,158,285)`, 67/226); hilitierter Titel = XOR von
  `gr_inside(Titel, 1)` (Spalten 65..110, Zeilen 1..17; `G_TITLE` `th = 1`,
  `gemobjop.c:44-46`).

## Rest (benannt, nicht angefasst)

* **Dropdown-Eintraege 19 px hoch** (`MENU_H`) statt der RSC-Zelle
  `gl_hchar` = 16 px (`desk_rsc.c:297-341` `{0,row,20,1}`); die File-Box ist
  285 statt 240 px hoch. `build_menu`, eigener Task (kein Referenzbild mit
  offenem Menue im Hatari-Satz). **Erledigt 2026-09-22 als [A109](A109.md)** (1) — `ENTRY_H`, Serial-Dump 240/16.
* `menu::track` prueft `THEBAR` ueber `gl_hbox`, nicht ueber das Objekt
  (EmuTOS `rect_change(THEBAR)` fuer `MU_M2` im `START_STATE`,
  `gemmnlib.c:286-292`) — die 18-Zeilen-Hoehe ist dort unbeobachtbar.

## Offen

Pi-Sichtabnahme (Tester/Lead; nichts geflasht). Kein Commit.
