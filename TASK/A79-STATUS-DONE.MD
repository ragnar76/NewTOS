# A79 — Quittung

**Zustand: DONE** · 2026-09-20 · Abendarbeit uncommitted im Baum (der Nachmittagsstand ist als `accfe8d` committet) · **Pi-Sichtabnahme offen**

**DONE heißt hier: die Umsetzung steht, die Look-Entscheidung des Anwenders ist
gefallen und umgesetzt (EmuTOS-konform, ein Look), Host- und QEMU-Belege liegen
vor; offen ist allein die Sichtabnahme auf der echten Pi 4 (Tester/Lead).**
Basis `desk-rebuild` @ `fd0eb98` (enthält den Nachmittagsstand `accfe8d`), gepinnte
EmuTOS-Quelle `references/emutos` @ `1a7484ab`. Belege:
[measurements/measurement-2026-09-20-a79/](../measurements/measurement-2026-09-20-a79/RUNINFO.TXT)
(§ FINAL), Referenz
[hatari-falcon-reference/README.md](../measurements/measurement-2026-09-20-a79/hatari-falcon-reference/README.md),
Bilder [screendumps/LOOK-COMPARE.md](../measurements/measurement-2026-09-20-a79/screendumps/LOOK-COMPARE.md)
und `screendumps/final-*.png`. Diese Quittung löst `A79-STATUS-UNFINISHED.MD`
ab (Inhalt übernommen, Abschnitte 1–2 sind der Nachmittagsstand).

## 0. Entscheidung (Anwender, 2026-09-20, Abend)

**EmuTOS-konform: der `Literal`-Look ist der einzige Look.** Gemessen an
EmuTOS selbst — der gepinnte Quellstand als `etos512us.img` auf einem
Hatari-Falcon030 (`--machine falcon --monitor vga`, 640x480), in Truecolor
(`#E … 1C`) und in 256 Farben (`… 1B`): weiße (hohle) Flächen und Bänder,
schwarze 1-px-Gadget-Ränder, Bevel (weiß oben/links, `(140,138,140)`
unten/rechts) auf den `FL3DACT`-Gadgets, getoppter Name `IP_2PATT`,
ungetoppter Name hohl weiß mit `DISABLED`-Text, Tracks `IP_1PATT`. Die
Farbworte sind von EmuTOS 1.3 bis HEAD unverändert (`gemwmlib.c:1349-1357`),
das 1.4-Release-ROM rendert gleich, und der Look hängt ab `ws_ncolors > 8`
nicht von der Farbtiefe ab (`init_3d`, `gemoblib.c:1336-1343`). Der
Website-Showcase mit grauem Chrome ist ein XCONTROL-Farb-CPX-Bild, keine
Referenz (`hatari-falcon-reference/README.md`).

Umsetzung des Entscheids (`tos-api/src/aes/window.rs`): `FrameLook`,
`FRAME_LOOK`, `set_frame_look`, `frame_look` und der `Tos4Gray`-Arm entfernt;
`gadget_ink(w, g, istop) -> Ink` liefert das gecrackte Wort bedingungslos;
`gadget_sel`/`gadget`/`draw_elevator`/`watchbox` nehmen `Ink`; in `draw_frame`
bleiben nur die literalen Arme (Titelbox, Name — `MOVER` ohne `NAME` mit
`te_color = 0`, `gemwmlib.c:540-550` —, Infobox, `vbar_box`/`hbar_box`,
Sizer, Tracks); `TITLE_STIPPLE`, `BLACK` und der `PatternFill`-Import waren
danach tot und sind weg; `GRAY_LT` bleibt für `DROP_SHADOW_COLOR`
(dokumentierte Look-Entscheidung), `GRAY_DK` für den Bevel. Geometrie
unverändert. `DISABLED`-Overlay und 1-px-Bevel bewusst nicht in diesem Ticket
(→ A95, Abschnitt 3).

## 1. Was tatsächlich getan wurde

| Datei | Was |
|---|---|
| `tos-api/src/aes/window.rs` | **Nachmittag:** `Ink` + `gadget_ink(w, g, istop)` (Wort `istop ? tcolor : bcolor` per `object::gr_crack`, `just_draw`s 3D-Substitution hohl-WEISS → `actbutcol`/`backgrcol`); `draw_box_ink` (1-px-Rand in `bc` + `gr_rect` innen, `IP_HOLLOW` = weiß), `grow_3d` (`gemoblib.c:503-508`), `inside_1`, `draw_elevator`, `vbar_box`/`hbar_box` (EmuTOS' `W_VBAR`/`W_HBAR`-Objektrechtecke, nur zum Zeichnen); `rect_outline(r, pel, …)`, `gadget_sel`/`gadget` (Ink), `draw_label` (`fg`, `mode`), `draw_vslider`/`draw_hslider` (`istop`), `redraw_*slider` berechnen `istop`, `drag_elevator`/`watchbox` mit Top-Ink; `draw_frame` verdrahtet jede Region. Drop-Shadow bleibt `DROP_SHADOW_COLOR`. `W_*_IDX`-Konstanten. Kommentare `Window.tcolor`, `WF_DCOLOR`, `WF_COLOR` neu. **Abend:** Schalter und `Tos4Gray`-Arm entfernt (Abschnitt 0); `gadget_ink -> Ink`; tote Konstanten/Import weg. Geometrie unverändert. |
| `tos-api/src/aes/object.rs` | Nur Sichtbarkeit (Nachmittag): `gr_crack`, `gr_rect`, `IP_HOLLOW`, `IP_SOLID`, `ACT_BUTCOL`, `BACKGR_COL` → `pub(crate)`. |
| `tos-api/tests/aes_window_color_words.rs` | Nachmittag neu (7 Fälle, Harness aus `aes_window_move_blit.rs`; `draw_glyph_sized` beachtet `Replace`). Abend: `Tos4Gray`-Fälle (`t7_*_tos4gray`, `vslide_*_tos4gray`) und `tos4gray_default_frames_are_pixel_identical_to_head` (HEAD-Hash `0x1f60a8cef23370da`) gestrichen — ihr Gegenstand existiert nicht mehr; `t7`/`vslide` auf je einen Fall; `default_topped_window_matches_the_gadget_table` (vorher `literal_default_*`) mit den Pixelpositionen aus `falcon-tc-window-c.png` kommentiert (Doppellinie 118/119, Closer-Rechtsrand 38 neben Namens-Linksrand 39, Bevel-Sampling auf der äußersten Zeile/Spalte, damit 1-px- und 2-px-Bevel bestehen; Stipple 3 px innerhalb des Randes wegen des fehlenden Namens-Bevels); **neu** `default_untopped_window_renders_the_untopped_words` (h1 per `wind_set(h2, WF_TOP)` ungetoppt neu gezeichnet: Name hohl weiß ohne Stipple, schwarzer Rand, Glyphen schwarz transparent, Closer mit Rand/Bevel/weißer Fläche, Info weiß, Track `IP_1PATT`; das `DISABLED`-Overlay wird nicht behauptet — A95 — die Aussagen gelten mit und ohne Overlay); **neu** Hash-Wächter `FRAME_HASH = 0x259edc0b844d91a0` (zwei Default-Fenster, allein zweimal, `--shuffle-seed 7`/`99` mit `--test-threads=1`, identisch). |
| `sdk/examples/a79color.c` | Nachmittag neu: Probe-`.NAP` für die Dumppaare (unverändert). |
| `docs/api/aes/window-graphics.md` | A79-Designabschnitt „`WF_COLOR`/`WF_DCOLOR` → pixel mapping“: Entscheid eingetragen (literale Wiedergabe = Kontrakt, Hatari-Referenz = Beleg, Restdifferenzen → A95); Tabelle (3) bleibt als Befundprotokoll „vor dem Fix“. |
| `docs/COMPATIBILITY.md` | Nicht angefasst — erwähnte den grauen Look nie (`grep GRAY_LT|TOS4 look|Tos4Gray`: 0 Treffer). |
| `docs/STATUS.md` | Phase-13-Zeile ergänzt, Recent-Decisions-Zeile 2026-09-20 (A79 Abschluss). |
| `TASK/A79.md` | Status ERLEDIGT, Abschnitt „Abschluss“. |
| `measurements/measurement-2026-09-20-a79/` | `method/make_scratch_final.sh` (ohne Look-Patch), `final_scripts.rs`, `tera-window.actions`, `teradesk-window.inf`, `newdesk-tera.inf`; `screendumps/final-*.png`, `final-compare-ref-vs-newtos.png`; `serial/final-*.log`; `build/*-final.log`; `host-tests/*final*`; `RUNINFO.TXT` § FINAL; `manifest.sha256` ergänzt; `LOOK-COMPARE.md` mit Entscheid-Hinweis. |

## 2. Welche Prüfung lief

* **Nachmittag** (Schalter, beide Looks): Hosttest rot-vor/grün-nach auf HEAD
  `21c9a3c` 6/7 rot (T7 „0 Pixel“), nachher 7/7 grün; 712 → 719; QEMU-Dumppaare
  beider Looks (`LOOK-COMPARE.md`).
* **Abend** (ein Look):
  * `tos-api/tests/aes_window_color_words.rs` 6/6 grün
    (`host-tests/aes_window_color_words-final.txt`).
  * `cargo run -p xtask -- test`: **718 / 0 / 2** (719 − 7 + 6;
    `host-tests/xtask-test-final.log`). Kein anderer Test pinnte den grauen
    Look: die Suite blieb ohne weitere Änderung grün; `aes_reference/`
    vergleicht keine Rahmenpixel; `aes_window_content.rs` liest den Schatten
    aus `DROP_SHADOW_COLOR`.
  * `cargo xtask build --board rpi4` frisch (`cargo clean -p kernel -p tos-api
    --target …`): tos-api 64 / kernel 94, rc 0 — Delta 0 (`BLACK` wäre nach
    dem Rückbau ungenutzt gewesen und ist entfernt, damit keine neue Warnung
    entsteht). Clippy tos-api 81 → 81, Testdatei 0 Treffer. Kein qemu-virt-Build
    (Vorgabe).
  * **QEMU raspi4b** (Scratch-Kern per `make_scratch_final.sh`, nur
    Injektor-Gerüst): `final-desk-{A-idle,B-two,C-two-2s}.png` (unser Desktop,
    Doppelklick aufs Laufwerkssymbol → zwei Fenster; **`final-desk-B-two` =
    `desk-lit-B-two` vom Nachmittag, 0 px** — der Rückbau hat das Bild nicht
    verändert) und `final-tera-{A-idle,B-window,C-window-2s}.png` (TeraDesk
    `#Z`, `TERADESK.INF` mit Fenster 40/40, 44x14 Zellen → Fenster
    `(39,62,376,291)` ganz im Bild). B = C jeweils 0 px. Serial: 0 `EXC|PANIC`.
  * **Vergleich mit der Referenz** (`final-compare-ref-vs-newtos.png`,
    rahmenrelativ, drei Ecken 4x; Klassenkarten in `RUNINFO.TXT` § FINAL):
    gleich sind alle Bandhöhen (23/19/23), alle Randzeilen/-spalten
    (Doppellinie 22/23, Naht −23, Closer/Name 22/23, Name/Fuller −24/−23,
    Up-Pfeil 42..64 + Track 65), Stipple, Tracks, weiße Flächen, Glyphzeilen.

## 3. Was offen bleibt

* **Pi-Sichtabnahme** (Tester/Lead; nichts geflasht).
* **Restdifferenzen zur Referenz → A95** (rahmenrelative Koordinaten; nicht
  farbwortgetrieben, hier nicht gefixt):
  1. Bevel 2 px statt 1 px (`add_3d_effect`, `gemoblib.c:238-276`; die
     Polylines enden 1 px vor den Ecken) — Closer: Referenz LBLACK Spalte 21
     Zeilen 2..21 / Zeile 21 Spalten 2..21, wir Spalten 20..21 / Zeilen 20..21.
  2. Kein Bevel am `FL3DACT`-Namen (Referenz: Zeile 1 weiß über dem
     Namensinneren, Zeile 21 LBLACK, Spalte −25 LBLACK).
  3. Gadget-Glyphen 1 px zu weit links: EmuTOS zentriert in
     `gr_inside(3D-Zelle, th)` mit `(w+1)/2` (`gemgraf.c:565-585`) plus
     `acttxtmove` y−1 (`gemoblib.c:735-757`) → Closer-Zelle `(8,3)`; wir
     `r.x + (r.w-8)/2` auf der ungewachsenen Zelle → `(7,3)`. Zeilen gleich.
  4. Down-Pfeil eine Zeile zu hoch: Referenz −46..−24 (Unterkante direkt über
     der Naht −23), wir −47..−25 mit weißer Zeile −24 (`vbar_rect`/`vtrack`
     enden eine Zeile vor `W_VBAR`, `gemwmlib.c:606-610`).
  5. Rechts-Pfeil eine Spalte zu weit links (−48..−25 statt −46..−24, weiße
     Spalte −24), Links-Pfeil eine Spalte zu weit rechts (1..23 statt 0..22;
     `hbar_rect` sitzt innen).
  6. Nach `wind_open` eines zweiten Fensters behält der alte Top seinen
     getoppten Rahmen außerhalb des neuen Rechtecks (`final-desk-B-two.png`,
     Namensband des hinteren Fensters Zeilen 45..63) — EmuTOS `draw_change`
     zeichnet den alten Top bei Open „covered“ neu
     (`w_cpwalk(oldtop, 0, MAX_DEPTH, TRUE)`, `gemwmlib.c:1202-1209`). Die
     Nachmittagsquittung (Punkt 5) hatte das fälschlich als EmuTOS-gleich
     abgelegt. Per `WF_TOP` wird korrekt ungetoppt neu gezeichnet (Hosttest).
  7. `DISABLED`-Overlay auf dem ungetoppten Namen (`gemwmlib.c:537`,
     `IP_4PATT` weiß transparent, `gemoblib.c:917-930`) fehlt.
* Unbewertet: das Laufwerkssymbol bleibt nach dem Doppelklick selektiert
  (desk/, wie am Nachmittag notiert).

## 4. Aufräumen

Scratch-Kopien und Images liegen nur im Sitzungs-Scratchpad; im Baum bleiben
die Produktänderung, der Test, das Beispiel, Doku/STATUS/Ticket/Quittung und
das Messverzeichnis. Kein `git checkout`/`stash`/`reset`. `A79-STATUS-UNFINISHED.MD` (committet
mit dem Nachmittagsstand) wurde mit `rm` aus dem Arbeitsbaum entfernt — die
Löschung liegt unstaged im Baum, wie die übrigen Änderungen.
