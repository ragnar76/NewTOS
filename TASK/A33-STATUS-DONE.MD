# A33 — Quittung

**Status: DONE** · 2026-09-12 · Der Defekt ist belegt, repariert und
gegengemessen. Die Abnahme „340 und 344 fallen zusammen“ ist **erreicht**;
damit faellt auch die von [A34](A34.md) offen gelassene Abnahme.

## Der Befund, selbst nachgeprueft

Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(`git -C references/emutos log -1` bestaetigt), Zielprofil Falcon030/TOS4.

### Was `wm_delete` belegt — vollstaendig

`aes/gemwmlib.c:1466-1485`:

| Zeile | Schreibt | Wert |
|---|---|---|
| `:1473-1474` | — | `if (pwin->w_flags & VF_ISOPEN) wm_close(w_handle);` |
| `:1476` | `w_rlist` | `newrect(gl_wtree, w_handle)` — Liste zurueckgeben, `VF_BROKEN` loeschen |
| `:1477` | `WS_CURR` (= `W_TREE[wh].ob_x/y/width/height`) | `gl_rscreen` = `0,0,640,480` |
| `:1478` | `WS_PREV` (= `pwin->w_prev`) | `gl_rscreen` |
| `:1479` | `WS_FULL` (= `pwin->w_full`) | `gl_rfull` = `0,19,640,461` |
| `:1480` | `WS_WORK` (= `pwin->w_work`) | `gl_rfull` |
| `:1481` | `w_flags` | `0` — `VF_INUSE` **und** `VF_ISOPEN` gehen runter |
| `:1482` | `w_owner` | `NULL` |

`gl_rscreen` / `gl_rfull` / `gl_rzero` werden in `gsx_start` gesetzt
(`aes/gemgraf.c:423-425`): `r_set(&gl_rscreen, 0, 0, gl_width, gl_height)`,
`r_set(&gl_rfull, 0, gl_hbox, gl_width, gl_height - gl_hbox)`.

Die Ticketbehauptung „alle vier Rechtecke“ stimmt also — bestaetigt an der
Quelle, nicht uebernommen. **Nicht** angefasst werden `w_kind`,
`w_pname`/`w_pinfo`, die vier Sliderfelder und die Gadgetfarben; die weist
`w_setup` beim naechsten Erzeugen ohnehin alle neu zu (`:210-226`).

### Was davon beobachtbar ist (Abnahmepunkt 2)

**Genau eins von den vieren, und nur aus dem naechsten Leben des Slots.**

* Solange der Slot frei ist, faellt jedes `wind_get` durch `get_pwin`
  (`:1470-1472`, `:1391-1404`) — `wm_get` gibt FALSE zurueck, bevor der
  Feldschalter laeuft (`:1513-1524`). Keines der vier Rechtecke ist lesbar.
* `wm_create` ueberschreibt `WS_CURR` und `WS_PREV` mit `gl_rzero` und
  `WS_FULL` mit dem uebergebenen Rechteck (`:1377-1379`) — drei der vier sind
  damit auch danach unsichtbar.
* `WS_WORK` schreibt `wm_create` **nicht**, und `w_setup` auch nicht
  (`:205-228`). In der ganzen AES gibt es drei Schreibstellen: `wm_init` fuer
  den Desktop (`:1294`), `wm_delete` (`:1480`) und `draw_change`/`w_move`
  (`:1073-1077`). `wm_get(WF_WXYWH)` ist ein reines `w_getsize(WS_WORK, …)`
  (`:1529`, `:236-262`).

Deshalb antwortet ein erzeugtes, nie geoeffnetes Fenster in einem recycelten
Handle `gl_rfull` — und in einem nie belegten `0,0,0,0` (BSS). Gemessen:
`tosdump/cases/aes-r3-wind-state.tsv` Faelle 340/344 gegen
`tosdump/reference/EMUTOS14-AES-r3.BIN`, beide `0,19,640,461`.

`w_owner = NULL` ist **nicht** beobachtbar (freier Slot faellt durch
`get_pwin`, `wm_create` setzt den Besitzer neu) — trotzdem uebernommen, weil
es das ist, was die Funktion tut.

### Was `wm_new` belegt

`:1875-1893`: erst die Sperren leeren (`while(ml_ocnt > 0) wm_update(END_MCTRL);`
`while(wind_spb.sy_tas > 0) wm_update(END_UPDATE);`), **dann**
`for (wh = 1; wh < NUM_WIN; wh++) { if (VF_ISOPEN) wm_close(wh); if (VF_INUSE) wm_delete(wh); }`.
Der `VF_INUSE`-Test ist der Unterschied zum Nullen der Tabelle: ein Slot, der
gar nicht in Benutzung ist, wird nicht angefasst.

## Der Fehler, beidseitig

| Seite | Stelle |
|---|---|
| EmuTOS (Soll) | `references/emutos/aes/gemwmlib.c:1476-1482` (`wm_delete`), `:1886-1890` (`wm_new`), `:1377-1379` (`wm_create` schreibt `WS_WORK` nicht) |
| NewTOS (Ist, vorher) | `tos-api/src/aes/window.rs::wind_delete` — setzte nur `rlist`/`rcount`/`rnext`/`in_use`/`open`, **kein** Rechteck, **kein** `owner`; `::wm_new` — `slots()[1..].fill(Window::EMPTY)`, also alle Slots genullt statt die benutzten durch `wm_close`+`wm_delete` geschickt; `::wind_create` — `*w = Window::EMPTY` loeschte `work` wieder |

## Was geaendert wurde

Alles in `tos-api/src/aes/window.rs`:

1. **Neu: `fn wm_delete(handle: i16) -> bool`** — EmuTOS' `wm_delete` 1:1:
   `get_pwin`-Ablehnung, `wm_close` falls offen, `newrect`, die vier
   `w_setsize`, `w_flags = 0`, `w_owner = NULL`. Rueckgabe wie EmuTOS.
2. **`wind_delete`** ist jetzt nur noch dessen Opcode (`pb.set_ret(i16::from(wm_delete(pb.intin(0))))`).
   Die TRUE/FALSE-Faelle bleiben wortgleich (Referenzfaelle 195/197/200/201/202).
3. **`wm_new`** faehrt EmuTOS' Schleife statt `fill(Window::EMPTY)`; die
   Sperrzaehler werden jetzt **vor** dem Sweep geleert, wie in der Quelle.
   `next_z` (unser eigener Zaehler, EmuTOS hat keinen) wird danach
   zurueckgesetzt.
4. **`wind_create`** traegt `WS_WORK` ueber den Slot-Reset:
   `let work = w.work; *w = Window::EMPTY; w.work = work;`.

Dazu die Doc-Kommentare an denselben Stellen (u. a. die Behauptung im
`wm_new`-Kommentar, EmuTOS' `wm_new` zeichne nichts — die war falsch, siehe
unten).

## Was Teil (b) sonst noch beruehrt — selbst nachgeprueft

Teil (b) aendert, was ein erzeugtes, **nie geoeffnetes** Fenster in `work`
vorfindet: statt `0,0,0,0` jetzt der Wert des Slots (nach einem `wm_delete`
also `gl_rfull`). Alle Leser von `w.work` im Baum, jeweils daraufhin geprueft,
ob sie fuer ein solches Fenster erreichbar sind **und ob sie bei geaendertem
Wert etwas anderes tun**:

| Stelle | Wer | Wirkung von (b) |
|---|---|---|
| `wind_get(WF_WXYWH)` | der gemessene Wert selbst | **ja, gewollt** |
| `rlist_owns` (`w_owns`) | `WF_FIRSTXYWH`/`NEXTXYWH` | keine: `work` wird nur **im** `while rnext < rcount`-Rumpf gelesen; `compute_rlist` fuellt `rcount` aus `border_pieces`, und das liefert fuer ein nicht offenes, nicht-DESKWH-Fenster `0`. Der Rumpf laeuft nie. |
| `send_redraw_in` (`w_update`→`w_redraw`) | Fanout | **hier war der Wert wirklich im Spiel** und es aendert trotzdem nichts: der Fruehausstieg `let mut d = work; if !rc_intersect(t, &mut d) { return; }` kann jetzt **passiert** statt genommen werden — dahinter steht aber `let Some(d) = w_union(handle) else { return; }`, und `w_union` reduziert dieselben `border_pieces` (0 Stueck) zu `None`. Es wird also weiterhin keine Nachricht gesendet. Erreichbar ist der Pfad ohnehin nur ueber `broadcast_redraw` (filtert `w.open`) und `top_window` (`wind_set(WF_TOP)`). |
| `wind_set(WF_CXYWH)` | `draw_change` | keine: `set_curr` **schreibt** `work` vor dem Lesen |
| `wind_open` | `send_redraw(handle, w.work)` | keine: steht nach `set_curr` |
| `drag_size` (`hctl_window` `W_SIZER`) | interaktiv | keine: nur auf dem getoppten, offenen Fenster |
| `chk_ctrl` (`w_setactive`s `ctrl`) | Maussteuerung | keine: `get_top()` liefert nur offene Fenster oder `DESKWH` |
| `init_desk_window` | schreibt | — |

Ausserhalb von `window.rs` gibt es keinen direkten Leser; `kernel/src/desk/`
holt die Arbeitsflaeche nur ueber `wind_get(WF_WXYWH)` und erzeugt Fenster an
genau einer Stelle (`desk/main.rs::open_dir_window`), wo der erste
`wind_get_work` **nach** dem `wind_open` steht.

**Ergebnis: der einzige Beobachter ist `wind_get(WF_WXYWH)` selbst.** Es gab
nichts vorzulegen. Der eine Pfad, an dem der Wert ueberhaupt eine Verzweigung
erreicht (`send_redraw_in`), ist geprueft und endet gleich — das ist mehr als
der A34-Entwickler feststellen musste, weil er die Streichung geprueft hat und
nicht das Stehenlassen.

## Was Teil (a) sonst noch beruehrt

Ein **freigegebener** Slot traegt jetzt `gl_rscreen` in `curr` statt Null. Jeder
Leser von `w.curr` im Baum wurde daraufhin durchgesehen: alle gehen entweder
ueber `win()` (filtert `in_use`) oder ueber eine Schleife mit
`w.in_use && w.open` (`get_top`, `get_bottom`, `border_pieces`s Cover-Schleife,
`is_broken`, `find_handle`, `redraw_all`s Auswahlsortierung). Die einzige
ungefilterte Stelle ist `find_handle`s `slots()[DESKWH].curr` — Handle 0, das
`wm_new` per Definition nicht anfasst.

`save_windows`/`restore_windows` (Abnahmepunkt 3) kopieren das ganze Array und
fuehren die neuen Werte unveraendert mit; sie fuehren keinen neuen Leser ein.

**Eine Nebenwirkung, die ich ausdruecklich melde und nicht wegargumentiere:**
`wm_new` zeichnet jetzt, was `wm_close` zeichnet. Der bisherige Kommentar
behauptete, EmuTOS' `wm_new` zeichne nichts — das ist falsch: `wm_close`
(`:1453`) ruft `draw_change`, dessen Schwanz `w_drawdesk` + `w_update` ist
(`:1216-1222`). Das Ticket verlangt genau das („statt sie ueber `wm_close` +
`wm_delete` zu schicken“). Praktisch: an beiden Aufrufstellen im Kernel
(`shell_loop.rs:84` `wm_init`, `:313` nach Programmende) folgt unmittelbar ein
ganzflaechiges `sh_draw_clear()` bzw. `redraw_desktop()`, und `post_msg` auf
eine tote PID ist ein sauberes `false` (`by_pid` → `None`). Im
QEMU-Messlauf ist der Desktop danach sauber hochgekommen.

## Bestehende Tests

**Keiner musste geaendert werden**, bis auf die eine `#[ignore]`-Marke, die
aufzuloesen der Auftrag war:

* `tos-api/tests/aes_window_create_work.rs ::
  created_after_a_sweep_answers_gl_rfull_like_the_dump` — `#[ignore]` entfernt,
  Doc-Kommentar auf den erreichten Zustand umgeschrieben. Laeuft gruen mit
  `0,19,640,461`.

Die Referenzfaelle zu `wind_delete` (`cases_r2_wind.rs` 195/197/200/201/202) und
zu `WF_WXYWH` (`cases_r2_wind.rs` 256/262, `aes_window_rectlist_zorder.rs`)
laufen unveraendert — keiner von ihnen friert den alten Zustand ein.

## Neue Tests

Beide Dateien vorher auf Existenz geprueft.

**`tos-api/tests/aes_window_delete_slot.rs`** (5 Faelle) — der `wind_delete`-Weg,
den die create_work-Datei nur ueber `wind_new` erreicht:

* `a_window_created_in_a_deleted_handle_inherits_gl_rfull_as_its_work_rect`
* `deleting_a_never_opened_window_arms_the_slot_just_the_same` (die vier
  `w_setsize` stehen **nicht** unter dem `VF_ISOPEN`-Test)
* `the_freed_slots_screen_rectangles_do_not_survive_the_next_create` — die
  Gegenprobe: `WS_CURR`/`WS_PREV` = `gl_rzero`, `WS_FULL` = das Erzeugungs-
  rechteck; `gl_rscreen` darf nicht durchschlagen
* `a_freed_handle_answers_no_rectangle_at_all` — `get_pwin` lehnt ab, also sind
  drei der vier Werte prinzipiell unsichtbar
* `a_sweep_does_not_disturb_a_slot_that_is_already_free`

**`tos-api/tests/aes_window_virgin_slot.rs`** (1 Fall, **allein in seiner
Datei**, weil die Fensterablage prozessglobal und eine Testdatei ein eigener
Prozess ist) — `a_handle_answers_zero_before_its_first_life_and_gl_rfull_after_it`:
der Unterschied zwischen nie belegt (`0,0,0,0`) und recycelt (`gl_rfull`), in
einem Lauf am selben Handle. Das ist die Pruefung, die eine „Tabelle pauschal
mit `gl_rfull` fuellen“-Reparatur faengt.

Keiner der Tests friert eine Zahl ein, die nicht aus EmuTOS folgt:
`GL_RFULL`/`GL_RSCREEN`/`GL_RZERO` sind die drei Konstanten aus
`gemgraf.c:423-425` bei der Geometrie, die der Harness stellt.

### Beide Haelften einzeln als tragend nachgewiesen

Per Handkopie (`shasum`-geprueft, **kein** `git checkout`/`restore`/`stash`):

| Probe | Rueckbau | Ergebnis |
|---|---|---|
| A | nur (b) — `*w = Window::EMPTY` loescht `work` wieder | 5 Faelle rot, alle mit `(0,0,0,0)` statt `(0,19,640,461)` |
| B | nur (a) — die vier `w_setsize` aus `wm_delete` | 5 Faelle rot, vier davon mit `(1,23,-24,-46)` = `wm_calc(WC_WORK, kind, gl_rzero)`, dem Rest aus `wm_close` |
| C | die naive Lesart: nach dem Sweep pauschal `work = gl_rfull` fuer alle Slots | nur `aes_window_virgin_slot` rot — genau die Pruefung, fuer die sie da ist |

Danach byteidentisch zurueckgebaut (`b0b3370883b40547c9ef16f4912f958e39d22dee`),
keine `SCRATCH-PROBE`-Marke mehr im Baum.

## Gegenmessung

`measurements/measurement-2026-09-12-qemu-r3-a33/` (AES.BIN, AES.LOG, BOOT-SERIAL.txt,
DIFF-emutos-vs-newtos-640x480.txt, RUNINFO.TXT). QEMU `raspi4b`, headless,
640x480, `#Z 01 C:\DUMPAES.NAP@`, ohne Karte und ohne Klick;
`tosdump/cases/aes-r3-wind-state.tsv` (39 Faelle, CRC `979FAC60`) gegen
`tosdump/reference/EMUTOS14-AES-r3.BIN`, verglichen mit
`tosdump/host/build/tosdiff`. Vorher-Stand:
`measurements/measurement-2026-09-12-qemu-r3-a34/`.

```
case  EmuTOS          NewTOS vorher    NewTOS nachher
 340  0,19,640,461    0,0,0,0          0,19,640,461   DECKUNGSGLEICH
 344  0,19,640,461    0,0,0,0          0,19,640,461   DECKUNGSGLEICH
```

Feldweise (`int_out`-Nutzdaten): weggefallen 340 `int_out[2..4]` und 344
`int_out[2..4]`; **neu aufgebrochen: nichts**. 20 abweichende
`int_out`-Zeilen vorher, 14 nachher.

**Endzustand erreicht.** Die verbleibenden 14 Zeilen sind ausschliesslich die
sieben Dispatcher-Stapelreste — 261, 280, 400, 481, 494, 574, 588, je
`int_out[1..2]` (`aes/gemsuper.c:430-432,444-445`), kein Soll. Kein anderer
Fall und kein anderes `int_out`-Feld weicht in der ganzen Tabelle ab;
namentlich stehen 528/549/561 (A22) und die Gruppen 439/448/458/469/481 sowie
519/538/574 unveraendert. Die `global[]`-Unterschiede sind in beiden Laeufen
zahlengleich (6 Felder x 39 Faelle = 234 Zeilen).

## Abnahmestand

| Punkt | Stand |
|---|---|
| `cargo run -p xtask -- test` | **462 → 469 gruen / 0 rot / 2 → 1 ignoriert** (der Rest-Ignore ist der fremde Doctest `tos-api/src/bdos/mem/stats.rs`) |
| `#[ignore]` in `aes_window_create_work.rs` | **aufgeloest** |
| `cargo run -p xtask -- build --board rpi4` | sauber, frisch gebaut, `tos-api` 64 / `kernel` 94 Warnungen — Basisstand unveraendert |
| `cargo run -p xtask -- build --board qemu-virt` | sauber, frisch gebaut, dieselben Zahlen |
| `cargo clippy -p tos-api --tests` | 81 Lib-Warnungen (Basisstand), die zwei neuen Testdateien erzeugen **null** |
| 340/344 fallen zusammen | **ja**, `0,19,640,461` |
| Endzustand „nur noch die sieben Faelle“ | **ja** |
| QEMU/Hatari-Prozesse | keiner laeuft (`pgrep -f 'qemu-system-aarch64\|Hatari'` leer) |
| `docs/STATUS.md` | datierter *Recent-Decisions*-Eintrag + Phase-13-Zeile fortgeschrieben, **ohne** `git stash` |

## Nicht angefasst

`border_pieces` und die Subtraktionsreihenfolge (A22), die gestrichene Zeile in
`wind_create` (A34), `docs/api/aes/window-graphics.md`, `save_windows`/
`restore_windows`, alle CRC-gebundenen Tabellen unter `tosdump/cases/`, alle
`.BIN`/`.LOG` unter `tosdump/reference/`, alle bestehenden
`measurement-*`-Verzeichnisse. Nichts committet, keine Hardware beruehrt,
nichts ausgeworfen.

## Offen (ausserhalb dieses Auftrags, nur gemeldet)

* `save_windows` (`window.rs`) nullt die Live-Tabelle mit `Window::EMPTY`,
  schickt sie also **nicht** durch `wm_delete`. Ein gestartetes GEM-Programm
  findet in `WS_WORK` eines recycelten Handles darum `0,0,0,0` statt
  `gl_rfull`. Durch diese Aenderung **nicht** verschlechtert (der Wert war
  vorher derselbe), aber dieselbe Naht. Ausdruecklich ausserhalb von
  „Nur `wind_delete` und `wm_new`“ gelassen.
* Die verbleibende Hardwareabnahme (echte Pi 4) steht wie immer aus; der
  Messlauf war QEMU.
