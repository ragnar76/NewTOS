# A34 — Quittung

**Status: DONE** · 2026-09-12 (fortgeschrieben 2026-09-12 im A33-Arbeitsgang)
· Der Defekt ist belegt, repariert und gegengemessen. **Die Abnahme „340 und
344 fallen zusammen“ ist mit [A33](A33.md) erreicht** — siehe den Nachtrag am
Ende. Der Bericht darunter ist der Stand bei Abschluss von A34 und bleibt
unveraendert stehen.

## Der Befund, nachgeprueft

Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(`git -C references/emutos log -1` bestaetigt), Zielprofil Falcon030/TOS4.

`wm_create` (`aes/gemwmlib.c:1370-1385`) schreibt drei Slots und hoert auf:

```c
w_setup(rlr, i, kind);
w_setsize(WS_CURR, i, &gl_rzero);
w_setsize(WS_PREV, i, &gl_rzero);
w_setsize(WS_FULL, i, pt);
```

`w_setup` (`:205-228`) weist `w_owner`, `w_flags`, `w_kind`, `w_pname`,
`w_pinfo`, die vier Sliderfelder und (unter `CONF_WITH_WINDOW_COLOURS`, im
512er-Falcon-Bau gesetzt) `w_tcolor`/`w_bcolor` zu — `w_work` ist nicht dabei.
`grep -n "w_work\|WS_WORK" references/emutos/aes/*.c` ergibt genau drei
Schreibstellen: `wm_init` fuer den Desktop (`:1294`, `= gl_rfull`), `wm_delete`
(`:1480`, `= gl_rfull`) und `draw_change`/`w_move` (`:1073-1077`,
`= wm_calc(WC_WORK, kind, WS_CURR)`). `wm_get(WF_WXYWH)` ist ein reines
`w_getsize(WS_WORK, …)` (`:1529`, `:236-262`) — kein Rechenweg.

Die Ortsangabe aus dem Ticket stimmt, nur die Zeilennummer ist inzwischen
verschoben: `tos-api/src/aes/window.rs:2312` (nicht `:2267`, das File ist durch
A22 laenger geworden).

## Was geaendert wurde

**Eine Zeile gestrichen**, `tos-api/src/aes/window.rs::wind_create`:

```rust
w.work = calc(WC_WORK, kind, full);
```

An ihre Stelle tritt ein Kommentarblock, der die drei EmuTOS-Schreibstellen
nennt, die Messfaelle 340/344 als Beleg fuehrt und die zweite, **nicht**
reparierte Haelfte derselben Naht ausweist (siehe unten). Sonst nichts.

## Was `w.work` sonst noch liest — die Frage vor dem Loeschen

Alle Lesestellen im Baum, `grep -n "\.work\b"`:

| Stelle | Wer | Erreichbar fuer ein erzeugtes, **nicht geoeffnetes** Fenster? |
|---|---|---|
| `window.rs:2639` | `wind_get(WF_WXYWH)` | **ja** — das ist der gemessene Wert selbst |
| `window.rs:1473` | `rlist_owns` (`w_owns`, `WF_FIRSTXYWH/NEXTXYWH`) | nein, wirkungslos: `border_pieces` liefert fuer ein nicht offenes Fenster `0` Stuecke (`window.rs:804-808`, EmuTOS `newrect` `gemwrect.c:189-192`), die Schleife wird nie betreten und `work` nie angefasst |
| `window.rs:1597` | `send_redraw_in` (`w_update`→`w_redraw`) | nein: `broadcast_redraw` filtert auf `w.open`; der einzige andere Weg ist `top_window` (`wind_set(WF_TOP)`), und dort liefert `w_union` fuer ein nicht offenes Fenster `None`, bevor irgendetwas gesendet wird |
| `window.rs:2778` | `wind_set(WF_CXYWH)` | schreibt `work` per `set_curr` **bevor** es liest |
| `window.rs:2382` | `wind_open` | liest nach `set_curr`, also den neu gerechneten Wert |
| `window.rs:3430-3433` | `drag_size` (`hctl_window` `W_SIZER`) | nein: interaktiver Pfad auf dem getoppten, offenen Fenster |
| `window.rs:3671` | `chk_ctrl` (`w_setactive`s `ctrl`) | nein: `get_top()` liefert nur offene Fenster oder `DESKWH` |
| `window.rs:512` | `init_desk_window` | schreibt, liest nicht |

Ausserhalb von `window.rs` gibt es keinen direkten Leser: `kernel/src/desk/`
holt die Arbeitsflaeche ausschliesslich ueber `wind_get(WF_WXYWH)`. Der
Desktop erzeugt Fenster an genau **einer** Stelle — `open_dir_window`
(`desk/main.rs:1565`) — und ruft dort `wind_get_work` erst in `:1611`, also
**nach** dem `wind_open` in `:1605`; dazwischen stehen nur `wind_set(WF_NAME)`
und `wind_set(WF_INFO)`. Alle uebrigen `wind_get_work`-Aufrufe im Desktop
arbeiten auf Handles aus Fensternachrichten bzw. `win_slot`, also auf
geoeffneten Fenstern.

**Ergebnis: kein Pfad zwischen `wind_create` und dem ersten `wind_open` bekommt
durch die Streichung einen anderen Wert.** Der einzige Beobachter ist
`WF_WXYWH` selbst. Es gab nichts vorzulegen, was ueber A34 hinausgegangen waere.
Dass es so ist, steht jetzt auch als Test da
(`a_created_but_unopened_window_owns_no_rectangle_whatever_its_work_rect_holds`),
statt nur behauptet zu sein.

## Bestehende Tests

**Keiner musste geaendert werden.** Kein bestehender Test fragte `WF_WXYWH` vor
dem `wind_open`; die WF_WXYWH-Faelle der Referenzsuite (`cases_r2_wind.rs` 256
und 262, `aes_reference.rs:231,270`, `aes_window_rectlist_zorder.rs:221,269`)
oeffnen ihr Fenster alle zuerst, und `cases_r1.rs:1297` fragt ein **nie
erzeugtes** Handle, das schon am `get_pwin` scheitert.

## Neue Tests

Neue Datei `tos-api/tests/aes_window_create_work.rs` (vorher auf Existenz
geprueft), sechs Faelle:

* `the_work_rect_of_a_created_window_does_not_depend_on_kind_or_maximum_extent`
  — zwei Erzeugungen aus **derselben** Slotvorgeschichte mit verschiedenem
  `kind` und verschiedener Maximalausdehnung muessen dasselbe antworten. **Vor
  dem Fix rot** (`11,43,276,154` gegen `41,83,198,96`).
* `a_created_window_does_not_answer_wind_calc_of_its_maximum_extent` — die
  Antwort darf nicht `wind_calc(WC_WORK, kind, full)` sein, also nicht der
  Orakelfall 337 der Tabelle. **Vor dem Fix rot** (genau `11,43,276,154`).
* `closing_a_never_opened_window_leaves_its_work_rect_where_the_create_found_it`
  — `wm_close`s frueher Rueckweg (`:1444-1447`), der Fall-344-Teil.
* `an_opened_window_still_answers_wind_calc_of_the_rectangle_it_was_opened_at`
  — die Gegenprobe gegen Zuviel-Streichen: `w_move` schreibt `WS_WORK` sehr wohl
  (`:1073-1077`), und `WS_FULL`/`WS_CURR` stehen weiter richtig.
* `a_created_but_unopened_window_owns_no_rectangle_whatever_its_work_rect_holds`
  — die Kompensationsfrage oben, als Test.
* `created_after_a_sweep_answers_gl_rfull_like_the_dump` — **`#[ignore]`**, mit
  Begruendung im Attribut: das ist der gemessene Sollwert `0,19,640,461` der
  Faelle 340/344, und er haengt an A33. Nicht geloescht, nicht gruen geredet.

Die beiden roten Faelle wurden nach dem Fix noch einmal gegengeprueft, indem die
gestrichene Zeile voruebergehend wieder eingesetzt wurde: beide werden wieder
rot, die Datei danach aus der Handkopie zurueckgeschrieben und per `shasum`
byteidentisch bestaetigt (**kein** `git checkout`/`restore`/`stash`).

## Gegenmessung

`measurement-2026-09-12-qemu-r3-a34/` (AES.BIN, AES.LOG, BOOT-SERIAL.txt,
DIFF-emutos-vs-newtos-640x480.txt, RUNINFO.TXT). QEMU `raspi4b`, headless,
640x480, `#Z 01 C:\DUMPAES.NAP@`, ohne Karte und ohne Klick;
`tosdump/cases/aes-r3-wind-state.tsv` (39 Faelle, CRC `979FAC60`) gegen
`tosdump/reference/EMUTOS14-AES-r3.BIN`, verglichen mit
`tosdump/host/build/tosdiff`. Vorher-Stand:
`measurement-2026-09-12-qemu-r3-a22/`.

```
case  EmuTOS          NewTOS vorher    NewTOS nachher
 340  0,19,640,461    11,43,276,154    0,0,0,0    verbessert, NICHT deckungsgleich
 344  0,19,640,461    11,43,276,154    0,0,0,0    verbessert, NICHT deckungsgleich
```

Feldweise (`int_out`-Nutzdaten):

* weggefallen: 340 `int_out[1..4]`, 344 `int_out[1..4]`
* neu aufgebrochen: 340 `int_out[2..4]`, 344 `int_out[2..4]`
* also: `int_out[1]` faellt zusammen, der gerechnete Wert ist weg, **kein
  anderer Fall und kein anderes Feld aendert sich in irgendeine Richtung**
  (22 abweichende `int_out`-Zeilen vorher, 20 nachher).

Unveraendert richtig: 431, 439, 448, 458, 469, 502, 511, 519, **528**, 538,
**549**, **561** — die A22-Gruppen sind nicht aufgebrochen. Unveraendert ohne
Soll: die sieben Dispatcher-Stapelreste 261, 280, 400, 481, 494, 574, 588
(`aes/gemsuper.c:430-432,444-445`), weder besser noch schlechter. Die
`global[]`-Unterschiede (6 Felder x 39 Faelle: `global[2..4]`, `global[10..12]`)
sind in beiden Laeufen zahlengleich.

## Warum die Abnahme nicht erreicht ist — und was genau fehlt

EmuTOS antwortet in den Faellen 340/344 `gl_rfull`, **weil der Slot recycelt
ist**: die Tabelle deklariert `!reset wind_new` (Zeile 215), `wm_new` schickt
jedes benutzte Handle durch `wm_close` + `wm_delete`
(`aes/gemwmlib.c:1884-1888`), und `wm_delete` schreibt `WS_CURR`/`WS_PREV` auf
`gl_rscreen` und `WS_FULL`/`WS_WORK` auf `gl_rfull` (`:1477-1480`). `wm_create`
laesst `w_work` danach in Ruhe — daher `0,19,640,461`. Auf einem nie belegten
Handle waere es `0,0,0,0` (BSS).

NewTOS' `wind_delete` (`window.rs:2501-2525`) und `wm_new`
(`window.rs:3069-3078`, `slots()[1..].fill(Window::EMPTY)`) schreiben **kein**
Rechteck, sondern nullen den Slot. Das ist wortwoertlich der Auftrag
[A33](A33.md), und A34 kann ihn nicht mitnehmen.

**Gemessen, nicht vermutet — und es sind zwei Haelften, nicht eine:** ein
Schmierpatch, der (a) `wm_new` fuer jeden freigegebenen Slot `WS_WORK =
gl_rfull` schreiben liess **und** (b) `wind_create`s `*w = Window::EMPTY` das
Feld nicht mehr loeschen liess, macht den `#[ignore]`-Fall
`created_after_a_sweep_answers_gl_rfull_like_the_dump` gruen — `0,19,640,461`.
**(a) allein bliebe unsichtbar**, weil der Create-Pfad das Feld sofort wieder
nullt. Der Patch ist per Handkopie vollstaendig zurueckgebaut (shasum geprueft,
keine `SCRATCH`-Marke mehr im Baum).

Damit ist die Handlungsempfehlung fuer A33 genau: `wind_delete` und `wm_new` auf
EmuTOS' vier Werte setzen **und** die `work`-Zeile aus dem Slot-Reset in
`wind_create` nehmen. Danach — und erst danach — fallen 340 und 344 zusammen,
und die `#[ignore]`-Marke faellt weg.

## Abnahmestand

| Punkt | Stand |
|---|---|
| `cargo run -p xtask -- test` | **457 → 462 gruen / 0 rot / 1 → 2 ignoriert** (das zweite Ignorierte ist der A33-Fall oben) |
| `cargo run -p xtask -- build --board rpi4` | sauber, frisch gebaut (`touch` davor), `tos-api` 64 / `kernel` 94 Warnungen — Basisstand unveraendert |
| `cargo run -p xtask -- build --board qemu-virt` | sauber, frisch gebaut, dieselben Zahlen |
| `cargo clippy -p tos-api --tests` | 81 Warnungen (Basisstand), die neue Testdatei erzeugt **null** |
| 340/344 fallen zusammen | **nein** — `int_out[1]` ja, `int_out[2..4]` offen, blockiert auf A33 |
| Endzustand „nur noch die sieben Faelle“ | **nein** — es sind die sieben **plus** 340/344 mit je drei Worten |
| QEMU/Hatari-Prozesse | keiner laeuft (`pgrep -f 'qemu-system-aarch64\|Hatari'` leer) |

## Nicht angefasst

`border_pieces` und die Subtraktionsreihenfolge (A22), `docs/api/aes/window-graphics.md`
(die Spezifikation nannte `WS_WORK` unter „`wind_create` (100) / Globals
written“ ohnehin nie und ist damit bereits richtig), `wind_delete`/`wm_new`
(A33), alle CRC-gebundenen Tabellen unter `tosdump/cases/`, alle
`.BIN`/`.LOG` unter `tosdump/reference/`, alle bestehenden
`measurement-*`-Verzeichnisse. Nichts committet, keine Hardware beruehrt.


---

# Nachtrag 2026-09-12 — die Abnahme ist erreicht

[A33](A33.md) hat die zweite Haelfte nachgeliefert: `wm_delete` bestueckt den
freigegebenen Slot (`aes/gemwmlib.c:1476-1482` — `newrect`,
`WS_CURR`/`WS_PREV` = `gl_rscreen`, `WS_FULL`/`WS_WORK` = `gl_rfull`,
`w_flags = 0`, `w_owner = NULL`), `wind_delete` ist dessen Opcode und `wm_new`
faehrt EmuTOS' `wm_close`+`wm_delete`-Schleife (`:1886-1890`) statt die Tabelle
zu nullen; dazu traegt `wind_create` `WS_WORK` ueber seinen Slot-Reset.

Gegenmessung `measurement-2026-09-12-qemu-r3-a33/` (dieselbe Tabelle, dasselbe
Profil):

```
case  EmuTOS          nach A34    nach A33
 340  0,19,640,461    0,0,0,0     0,19,640,461   DECKUNGSGLEICH
 344  0,19,640,461    0,0,0,0     0,19,640,461   DECKUNGSGLEICH
```

`int_out`-Abweichungszeilen 20 → 14; neu aufgebrochen ist nichts. Uebrig
bleiben nur die sieben Dispatcher-Stapelreste 261/280/400/481/494/574/588
(`aes/gemsuper.c:430-432,444-445`), kein Soll.

Der `#[ignore]`-Fall
`tos-api/tests/aes_window_create_work.rs :: created_after_a_sweep_answers_gl_rfull_like_the_dump`
laeuft ohne Marke gruen. Die oben beschriebene Streichung in `wind_create` ist
unveraendert in Kraft; A33 hat sie nicht angefasst, sondern nur den Slot-Reset
derselben Funktion um `work` erleichtert.

Quittung: [A33-STATUS-DONE.MD](A33-STATUS-DONE.MD).
