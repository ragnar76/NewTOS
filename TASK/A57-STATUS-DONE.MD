# A57 — STATUS: DONE (Hostbeleg + QEMU-Selbstbeleg) / Tester-Verdikt offen

Der eine persistente AES-Kratzpuffer ist gebaut: Alarm **und** Menuetafel
sichern in denselben Block, `wind_get(WF_SCREEN)` gibt Adresse und Laenge
als zwei LONGs heraus, eine EL0-Anwendung schreibt durch den Zeiger, ohne
zu faulten. Bearbeitet 2026-09-20, Branch `desk-rebuild`, Basis HEAD
`172bca0` (der Entwurf), **nicht committet**. Ersetzt
`A57-STATUS-UNFINISHED.MD`. Loest **A71** mit
([`A71-STATUS-DONE.MD`](A71-STATUS-DONE.MD)).

Entwurf und Entscheidungen: [`docs/analysis/a57-aes-screen-buffer-design-2026-09-20.md`](../docs/analysis/a57-aes-screen-buffer-design-2026-09-20.md)
— Pool (a), lazy `expose`, Groesse = ein Schirm in Geraetepixeln,
Untermenue/Popup = transienter Pool-Block je Oeffnung (EmuTOS `popup_blit`).

## Der Sachverhalt, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `aes/gemgsxif.c:80-83` (`gl_tmp`/`gl_mlen`), `:98-114` (`gsx_mcalc`/`gsx_malloc` = `dos_alloc_anyram`), `:125-129` (`gsx_mret`), `:320-380` (`bb_set`); `aes/gemwmlib.c:1563-1565` (`WF_SCREEN`); `aes/gemshlib.c:236` (`sh_tographic`); `aes/gemfmalt.c:363,390`; `aes/gemmnlib.c:208-223` (`menu_sr`); `aes/gemmnext.c:136-215` (`popup_blit`) |
| NewTOS (vorher) | kein Puffer: `form.rs` `BbSave` (Vec je Alarm), `menu.rs` `MBACK`/`SMBACK` (`read_pixel`-Stiftpfad), `window.rs` `WF_SCREEN` → FALSE (A54) |
| NewTOS (nachher) | `tos-api/src/aes/gsxif.rs` (neu), `form.rs`, `menu.rs`, `window.rs`, `mod.rs` (`set_intout_long`), `kernel/src/shell_loop.rs` (`sh_tographic` → `gsx_malloc`) |

## Was gebaut ist

* **`gsxif.rs`** — `gsx_mcalc` = `bb_save_words(W,H)*4` (614 400 @640×480,
  4 147 200 @1080p); `gsx_malloc` einmalig aus `sh_tographic`, `mxalloc(page_round(len)+PAGE, MX_PREFTTRAM)`
  vor dem ersten Prozess (kernel-eigen, `Pterm`-fest), Basis seitenausgerichtet,
  nie freigegeben; `gsx_mret`; `install_screen_buffer` (Hostfixture);
  `bb_save`/`Saved::restore` in den einen Puffer mit `bb_set`s Hoehenwaechter
  (`clipped_saves`-Zaehler statt `KINFO`); `popup_blit_save` = frischer
  `mxalloc(.., MX_STRAM)`-Block je Oeffnung, `mfree` beim Zurueckschreiben oder Drop;
  ohne Puffer Heap-Kopie je Sicherung (Rueckfall, `WF_SCREEN` dann FALSE).
* **`window.rs`** — EL0: lazy `usermem::expose(slot, kva, len, PROT_G)` (ADR-023, wie
  `Getbpb`/`Physbase`, idempotent je Slot); EL1: PA (ADR-021); zwei native
  unausgerichtete 32-Bit-Stores an `&int_out[1]`/`&int_out[3]` (`Aespb::set_intout_long`,
  EmuTOS `*(LONG*)poutwds`); `ret 1`.
* **`form.rs`** — Alarm ueber `gsxif::bb_save` (Klemmung `rc_intersect(gl_rscreen)` bleibt).
* **`menu.rs`** — Tafel ueber `gsxif::bb_save` (Device-Pixel; A71), Untermenue und Popup
  ueber `popup_blit_save`; `MBACK`/`SMBACK`/`save_under`/`restore_under` entfernt;
  `pub fn close` als kopfloses Gegenstueck zu `open`.
* **Spec/Doku** — `window-graphics.md` § `WF_SCREEN`, `_raster.md` „AES consumer",
  `menu.md`, `object-form.md`, `docs/COMPATIBILITY.md` (A54-Eintrag auf den Rueckfall
  verengt, neuer Eintrag zur Groesse), `docs/STATUS.md`.
* **`sdk/examples/wfscreen.c`** — EL0-Schreibprobe (`.NAP`, `#Z`-Autorun).

## Tests: rot vor, gruen nach

Gegenprobe in einem Scratch-Worktree auf HEAD `172bca0` mit nur `gsxif.rs`,
`mod.rs`, `harness.rs` und den Testdateien (`measurements/measurement-2026-09-20-a57/host-tests/red-before.txt`):

| Test | vor | nach |
|---|---|---|
| `aes_reference` Fall 166 (`SANCTIONED` leer, direkt verglichen) | `int_out[0] NewTOS = 0 reference = 1` | gruen |
| `aes_window_screen_buffer.rs` (umgedreht: ret 1, Adresse = Fixture-PA auf 331/332/439, Laenge = Formel, zwei native LONGs) | 2 rot | 3 gruen |
| `aes_screen_buffer_save_under.rs` Alarm ueber Truecolor-Muster | „fixture is still all zero" | gruen |
| dito Tafel (A71) | „first at (55,19): was 0x639642, is 0xFFFBFF" | gruen |
| dito Waechter (Spielzeugpuffer 10 Zeilen, 100 verlangt) | gruen | gruen |
| `aes_window_screen_buffer_absent.rs` (Rueckfall FALSE) | gruen | gruen |

`cargo run -p xtask -- test` **670 → 674 / 0 / 1**. Beide Boards rustc 64/94,
Clippy 81/125 (Baseline aus HEAD-Worktree gemessen) — Delta 0.

## QEMU raspi4b (`measurements/measurement-2026-09-20-a57/`)

* `WFSCREEN.NAP` per `#Z`: `ret=1 addr=0x00400000 len=614400`, 614 400 Bytes
  durch den Zeiger geschrieben und zurueckgelesen, zweiter Aufruf gleiche
  Adresse, `PASS`, kein Data Abort; Bootlog `[aes] gsx_malloc: screen buffer pa=0x01000000 len=614400`.
* TeraDesk per Injektor, Scratch-Kern vor/nach: View-Tafel, File-Tafel,
  Alarm „Drive does not exist." auf/zu — **alle neun Zustaende 0 Pixel**
  vor/nach; Tafel- und Alarmflaeche exakt zurueckgeschrieben (Differenz zum
  Ausgangsbild = nur der Zeiger); File-Tafel 0 Pixel zur A46-Referenz; Idle
  0 Pixel zu A41/A46/A78. Trace: TeraDesk erhaelt `(1, addr 0x0040a000, len 614400)`
  statt `(0,0,0,0,0)`.

## Vorhersage des Entwurfs — widerlegt

§1.5 des Entwurfs folgerte, TeraDesk loesche vor A57 dreizehn Menueeintraege.
**Gemessen: nein.** Die View-Tafel ist vor/nach pixelgleich (Show as icons,
Auto arrange, Sort by name/extension/date/size, View unsorted, Reverse order,
Show size/date/time/attributes, Set mask...), die File-Tafel ebenso (Delete...,
Copy disk..., drei Trenner). Grund aus dem Quelltext: TeraDesk liest `buffer.size`
als 8-Byte-`long` (LP64) aus einer Union, von der `wind_get` nur die unteren
4 Bytes fuellt (`resource.c:233-241,307-315`); die oberen 4 sind Stapelinhalt —
App-seitige LP64-Falle (T04), kein Kernthema. A57 aendert an TeraDesks Bild
nichts; was sich aendert, ist die Antwort, die TeraDesk bekommt.

## Nicht angefasst / offen

* `sr_rect` waechst nach `popup_blit`-Form statt `menu_sr` (`gemmnlib.c:215-217`)
  — Nebenbefund des Entwurfs, andere Zeile, gemeldet, nicht korrigiert.
* Untermenue/Popup-Pfad (`popup_blit_save`) ist nur zielseitig pruefbar (kein
  Pool im Hostprozess); kein Programm auf der Karte nutzt `menu_popup`/`menu_attach`.
* `tools/make-sd-image.sh` installiert `WFSCREEN.NAP` nicht (ausserhalb des Scopes);
  Bau: `sdk/build.sh sdk/examples/wfscreen.c <out>/WFSCREEN.NAP`, Start per
  `#Z 01 C:\WFSCREEN.NAP@`.
* Keine Hardware, nichts geflasht, kein Commit, kein `git checkout/stash/reset`.
  Das Tester-Verdikt (Boot + Selftests + Screendumps + fsck, beide Boards) steht aus.
