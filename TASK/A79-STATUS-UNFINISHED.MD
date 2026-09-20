# A79 — Quittung

**Zustand: UNFINISHED** · 2026-09-20 · kein Commit (Arbeit liegt im Baum)

**UNFINISHED heißt hier: die Umsetzung steht und ist auf dem Host und in QEMU
belegt; offen ist die Look-Entscheidung des Anwenders („Erst Bild sehen“) und
die Pi-Abnahme.** Basis `desk-rebuild` @ `21c9a3c`, gepinnte EmuTOS-Quelle
`references/emutos` @ `1a7484ab`. Belege:
[measurement-2026-09-20-a79/](../measurement-2026-09-20-a79/RUNINFO.TXT),
Bildpaare: [screendumps/LOOK-COMPARE.md](../measurement-2026-09-20-a79/screendumps/LOOK-COMPARE.md).

## 1. Was tatsächlich getan wurde

| Datei | Was |
|---|---|
| `tos-api/src/aes/window.rs` | `FrameLook { Literal, Tos4Gray }`, `FRAME_LOOK: AtomicU8` (Default `Tos4Gray`), `set_frame_look`/`frame_look`; `Ink` + `gadget_ink(w, g, istop) -> Option<Ink>` (Wort `istop ? tcolor : bcolor` per `object::gr_crack`, `just_draw`s 3D-Substitution hohl-WEISS → `actbutcol`/`backgrcol`; `None` = Tos4Gray-Konstante, solange das Wort dem kompilierten Default gleicht); `draw_box_ink` (1-px-Rand in `bc` + `gr_rect` innen, `IP_HOLLOW` = weiß), `grow_3d` (`gemoblib.c:503-508`), `inside_1`, `draw_elevator`, `vbar_box`/`hbar_box` (EmuTOS' `W_VBAR`/`W_HBAR`-Objektrechtecke, nur zum Zeichnen); `rect_outline(r, pel, …)`, `gadget_sel`/`gadget` (Ink), `draw_label` (`fg`, `mode`), `draw_vslider`/`draw_hslider` (`istop`), `redraw_*slider` berechnen `istop`, `drag_elevator`/`watchbox` mit Top-Ink; `draw_frame` verdrahtet jede Region (W_BOX-Rand, Titelbox, Closer/Fuller, Name inkl. `MOVER`-ohne-`NAME`-Regel `te_color = 0`, Infobox, Bar-Spalte, Pfeile, Tracks, Elevatoren, Bar-Band, Ecke/Sizer). Drop-Shadow bleibt `DROP_SHADOW_COLOR`. `W_*_IDX`-Konstanten. Kommentare `Window.tcolor`, `WF_DCOLOR`, `WF_COLOR` neu. Geometrie unverändert. |
| `tos-api/src/aes/object.rs` | Nur Sichtbarkeit: `gr_crack`, `gr_rect`, `IP_HOLLOW`, `IP_SOLID`, `ACT_BUTCOL`, `BACKGR_COL` → `pub(crate)`. |
| `tos-api/tests/aes_window_color_words.rs` | neu, 7 Fälle (Harness aus `aes_window_move_blit.rs`; `draw_glyph_sized` beachtet `Replace`). |
| `sdk/examples/a79color.c` | neu: Probe-`.NAP` für die Screendumps (Fenster `(30,40,320,240)` Kind `0x0FEF`, nach 7 s T7 + `W_VSLIDE 0x1171`, druckt nichts). |
| `docs/STATUS.md` | Phase-13-Eintrag + Recent-Decisions-Zeile 2026-09-20 (A79). |
| `TASK/A79.md` | Status + „Zwischenstand“. |
| `measurement-2026-09-20-a79/` | Methode (Scratch-Builder mit Look-Schalter, Injektor-Skripte mit `parkbr`), Dumps, Serial, Build-/Test-Logs, Manifest, `LOOK-COMPARE.md`. |

Nicht angefasst: `docs/api/*` (Lead-Design), `docs/COMPATIBILITY.md`,
Geometrie (`vbar_rect`/`hbar_rect`/`vtrack`/`gadget_at`/Rechtecklisten).

## 2. Welche Prüfung lief

* **Hosttest rot-vor/grün-nach** (`host-tests/`): auf HEAD `21c9a3c`
  (Scratch-Kopie, Schalter gestubbt) 6/7 rot — T7 „no pixel of the name band
  changed after WF_COLOR (was 0)“ in beiden Looks, `W_VSLIDE`-Track grau statt
  schwarz, `WF_DCOLOR`-Fenster ohne Rand, Literal-Tabelle ohne Closer-Rand;
  der Hash-Wächter grün (er *ist* der HEAD-Hash `0x1f60a8cef23370da`,
  dreimal/gemischt gemessen). Nachher 7/7 grün.
* `cargo run -p xtask -- test`: **719 / 0 / 2** (Baseline 712 + 7).
* `cargo xtask build --board rpi4` frisch: tos-api 64 / kernel 94, rc 0;
  Clippy tos-api 81 → 81, neue Testdatei 0 Treffer. Kein qemu-virt-Build
  (Vorgabe).
* **QEMU raspi4b** (Scratch-Kerne, je Look ein Kern, Unterschied genau die
  `FRAME_LOOK`-Initialisierung): Kernel-Desktop mit zwei Fenstern
  (Doppelklick auf das Laufwerkssymbol, Zeiger unten rechts geparkt — nie
  durch die Menüleiste) und Probe vor/nach T7. Pixeldiffs in `RUNINFO.TXT`;
  Gray-Idle-Desktop = A78-`desk-after-A-idle` bis auf den Mauszeiger (51 px).
  T7: Namensband rot, Rand schwarz, Text schwarz mit Rot zwischen den
  Strichen; Track schwarz. Alle 2-s-Nachdumps bytegleich.

## 3. Was offen bleibt

* **Look-Entscheidung des Anwenders** anhand der Paare in `LOOK-COMPARE.md`
  (Tos4Gray = heutiges Bild; Literal = EmuTOS-Worte 1:1). Danach fällt der
  unterlegene Arm samt Schalter.
* **Pi-Abnahme** (Tester/Lead; nichts geflasht).
* **Nebenbefunde, nicht gefixt** (nicht farbwortgetrieben bzw. Geometrie):
  (1) `bevel` 2 px vs. `add_3d_effect` 1 px (`gemoblib.c:238-276`);
  (2) `W_NAME` ist `FL3DACT` → bei EmuTOS Bevel um den Namen, ungetoppt dazu
  der `DISABLED`-Dither (`:917-930`); heute nicht gezeichnet;
  (3) `vbar_rect`/`vtrack` enden bei HBAR eine Zeile über EmuTOS' `W_VBAR`
  (`gemwmlib.c:585-616`) → im Literal-Look eine weiße Zeile zwischen Pfeil und
  geteiltem Rand; (4) `MOVER` ohne `NAME`: EmuTOS `te_color = 0`
  (`gemwmlib.c:540-550`), Gray-Arm behält heutigen Stipple; (5) nach
  `wind_open` eines zweiten Fensters behält das alte Top seinen Stipple
  außerhalb des neuen Rechtecks — EmuTOS `w_update` clippt ebenso
  (`:1004-1048`); (6) das Laufwerkssymbol bleibt nach dem Doppelklick
  selektiert (desk/, nicht bewertet).

## 4. Aufräumen

Scratch-Kopien und Images liegen nur im Sitzungs-Scratchpad; im Baum bleiben
die Produktänderung, der Test, das Beispiel, STATUS/Ticket/Quittung und das
Messverzeichnis. Kein `git checkout`/`stash`/`reset`.
