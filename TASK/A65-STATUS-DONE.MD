# A65 — STATUS: DONE

`objc_draw` und `objc_change` nehmen den Mauszeiger jetzt vom Schirm, solange sie
zeichnen. Bearbeitet 2026-09-17, Branch `desk-rebuild`, Basis HEAD `52a969b`.
Messlauf: [`measurement-2026-09-17-a65/`](../measurement-2026-09-17-a65/RUNINFO.TXT).

## Der Fehler, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll), `ob_draw` | `references/emutos/aes/gemoblib.c:974` `gsx_moff()` … `:976` `gsx_mon()` |
| EmuTOS (Soll), `ob_change` | `references/emutos/aes/gemoblib.c:1275` … `:1319` |
| Zähler | `references/emutos/aes/gemgsxif.c:440-455` (`gl_moff`, zählende Semaphore) |
| NewTOS (vorher) | `tos-api/src/aes/object.rs` — **keine einzige Klammerung** in der ganzen Datei |
| NewTOS (nachher) | `tos-api/src/aes/object.rs` — `hide_cursor_for_draw`/`show_cursor_after_draw` an fünf Stellen |

Gepinnte Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.

Der Zeiger ist ein Save-under-Sprite (`tos-api/src/vdi/mouse.rs`, `draw`/`erase`;
EmuTOS `cur_display`/`cur_replace`, `vdi/vdi_mouse.c:975-985,1099`). Wird zwischen
Sichern und Zurückschreiben in den 16×16-Kasten gezeichnet, schreibt der nächste
Zeigerschritt den **Stand vor** der Zeichnung zurück.

## Wo die Klammer sitzt — und warum genau dort

EmuTOS klammert **je Einsprung**, nicht in der Baumwanderung (`everyobj`) und
nicht im Einzelobjektmaler (`just_draw`). Die Rollenzuordnung im Baum:

| NewTOS | EmuTOS-Rolle | geklammert |
|---|---|---|
| `render_tree_t` (Rumpf von `objc_draw`, `render_tree`, `render_tree_lp64`, `render_tree_app`) | `ob_draw` | **ja** |
| `draw_obj_t` (Rumpf von `draw_obj`) | `ob_draw(tree, obj, depth)` | **ja** |
| `draw_obj_clip` | `ob_draw` mit gesetztem Clip | **ja** |
| `draw_root` | `ob_draw(tree, ROOT, depth)` | **ja** |
| `change_clip_t` (Rumpf von `objc_change`, `change`, `change_t`) | `ob_change` | **ja** |
| `edit_field_t` (Rumpf von `objc_edit`, `edit_field`) | `curfld` | **ja**, eine Klammer über beide Zeichnungen |
| `set_state_bit_t` | Aufrufer von `ob_change` | **nein**, mit Begründung im Kommentar |
| `draw_tree` / `just_draw` | `everyobj` / `just_draw` | **nein** |

Drei Entscheidungen, die der Auftrag selbst klären ließ:

**1 — Die Ebene.** Eine Klammer in `draw_tree` wäre zu tief: die Routine ruft sich
für jedes Kind selbst auf, also gäbe es ein Hide/Show-Paar je Knoten. Der Zähler
trüge es, EmuTOS' Form ist es nicht. Der nächsthöhere gemeinsame Punkt ist
`render_tree_t` — und der deckt mit **einer** Klammer alle vier öffentlichen
Zeicheneinsprünge ab (`objc_draw` über den Trap, `render_tree*` für den Kernel),
genau wie EmuTOS' `ob_draw` alle seine Aufrufer abdeckt.

**2 — Verschachtelung ist EmuTOS' eigene Form, nicht ein Nebeneffekt.**
`w_update` klammert (`aes/gemwmlib.c:1011`) und erreicht dann über
`w_cpwalk` → `do_walk` → `ob_draw` (`:1029`, `:378`, `:309`) eine zweite Klammer.
Das geht, weil `gl_moff` zählt: nur der Übergang 0→1 löscht das Sprite, nur 1→0
zeichnet es neu (`aes/gemgsxif.c:440-455`). `crate::vdi::cursor_hide`/`cursor_show`
sind derselbe Zähler (`HIDE_COUNT`, `vdi/mouse.rs:638-651`), also bleiben die
Klammern, die `menu.rs`, `form.rs`, `window.rs`, `shell.rs` und der Desktop
(`kernel/src/desk/redraw.rs`, `graf_mouse(M_OFF/M_ON)`) schon setzen, richtig und
kosten nichts. Gemessen, nicht behauptet: die drei Maschinenläufe zeigen null
abweichende Bildpunkte.

**3 — `edit_field_t`.** EmuTOS' Texteingabe redraw't über `curfld`
(`aes/gemobed.c:204-218`) und hat dort **zwei sich ausschließende Zweige**: das
Feld über `ob_draw` (`:214`) und den Cursorbalken über `gsx_cline` (`:216`).
Beide sind geklammert — `ob_draw` durch sich selbst, `gsx_cline` durch sich
selbst („draw a clipped polyline, **hiding the mouse as you do it**",
`aes/gemgraf.c:186-192`). Bei uns laufen beide **nacheinander**, nicht alternativ.
Deshalb **eine** Klammer über beide: dieselbe Zusage wie EmuTOS, ohne das Sprite
zwischen Feld und Balken einmal überflüssig aufblitzen zu lassen.

## Der Test: rot, dann grün

Neu: `tos-api/tests/aes_object_cursor_bracket.rs`, drei Fälle, 640×480 Falcon-
Truecolor (das Profil, auf dem fotografiert wurde). Die Vier-Ebenen-TT-Fassung aus
`aes_reference/` taugt hier nicht: ihre Pseudopalette wird nie gefüllt (`init_wk`
sät sie nur für `planes > 8`), WEISS und SCHWARZ lösen auf dasselbe Gerätewort auf
und eine Invertierung wäre unsichtbar. Das war der erste Anlauf und ist im
Kopfkommentar der Testdatei festgehalten.

Rot vor dem Fix (`measurement-2026-09-17-a65/build/newtest-RED.txt`):

```
Change: the AES drew through the cursor's save-under box.
176 pixels differ; damage (120,105)..(135,115) = 16x11;
predicted (120,105)..(135,115) = 16x11 = 176 px (16 columns by 16-k = 16-5 rows)
y=104  A |####################|  B |####################|
y=105  A |####################|  B |##                ##|
...
y=115  A |####################|  B |##                ##|
y=116  A |....................|  B |....................|
```

Die Prüfung ist die **Signatur**, nicht bloß Ungleichheit: der Kasten muss genau
16 Spalten ab dem Sprite-Ursprung breit und genau `16 − k` Zeilen hoch sein
(`k = 5` im Fall), 176 Punkte, und jeder davon muss die Farbe von **vor** der
Änderung tragen. Der dritte Fall misst das Sprite statt der AES — er invertiert
die Zeile mit einem rohen Backend-Schreibzugriff hinter dem Rücken des Zeigers,
also kann keine Klammer ihn grün machen; er hält die Geometrie auch nach dem Fix
fest.

Grün danach (`build/newtest-GREEN.txt`): `3 passed; 0 failed`.

## Prüfungen

| | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | **551 bestanden / 0 gefallen / 1 übersprungen** (Sollstand 548/0/1 + 3 neue) |
| rustc-Warnungen, beide Boards | tos-api **64**, kernel **94** — Sollstand, Delta 0 |
| clippy-Warnungen, beide Boards | tos-api **81**, kernel **125** — Sollstand, Delta 0 |
| Neubau belegt | beide Protokolle zeigen `Compiling tos-api` **und** `Compiling kernel` |
| QEMU raspi4b, Desktop / TeraDesk / QED | **0 von 307200** Bildpunkten verschieden, je Fall; PNGs byteweise gleich |
| QEMU raspi4b, serielle Protokolle | Desktop und TeraDesk identisch; QED identisch auf frischer Karte (die 48 Zeilen des ersten Vergleichs waren `C:\CLIPBRD`, das QED beim ersten Start anlegt) |
| qemu-virt-Rauchprobe | bootet, EmuCON-Rückfall mangels Bildschirm — wie in X01 |

Verfahren und Rohdaten: `measurement-2026-09-17-a65/`, Verfahrensdateien aus
`measurement-2026-09-14-x01/method/` unverändert übernommen.

## Was die echte Pi 4 zeigen muss — Vorhersagen, nicht Messungen

QEMU `raspi4b` kann den Fehler nicht zeigen: `[pcie] RC not present … USB
disabled`, es gibt dort keinen Eingabeweg, der Zeiger bewegt sich nie. Die drei
Abnahmepunkte des Tickets bleiben deshalb **Vorhersagen**, am Gerät in einem Zug
abzuarbeiten:

1. Im Dateiauswähler einen Dateinamen **an der Oberkante** seiner Zeile anklicken,
   Maus zwei Bildpunkte bewegen. **Vorher:** helle Kerbe, zwei Zeichenzellen breit,
   fast volle Zeilenhöhe, linke Kante an der Pfeilspitze, sie bleibt stehen.
   An der **Unterkante** geklickt dieselbe Kerbe, nur ein bis zwei Punkte hoch —
   das Foto. **Nachher:** keine Kerbe.
2. **Trennprobe:** dieselbe Geste auf der Menüleiste → vorher wie nachher nichts
   Liegengebliebenes (`menu.rs` klammerte schon).
3. **Gegenprobe:** ohne Klick über die Liste fahren → nichts bleibt liegen.
   Träte dabei etwas auf, wäre die Diagnose falsch.

Foto und Referenz: `docs/analysis/fsel-reference-2026-09-17/`.

## Beobachtungen außerhalb des Auftrags — gemeldet, nicht angefasst

- **Die Nebenbefunde `menu.rs:262`, `:415`, `:428` und `:190` sind kein eigener
  Fehler.** Sie sind derselbe, von der Aufruferseite gesehen: `set_state_bit` →
  `change_t` → `change_clip_t` und `draw_obj` → `draw_obj_t`. Beide Ziele sind
  jetzt geklammert, also sind alle vier Stellen richtig, **ohne dass `menu.rs`
  angefasst wurde** — und zwar an der Stelle, an der EmuTOS auch klammert
  (`gemmnlib.c:170` und `:719` verlassen sich beide auf `ob_change`/`ob_draw`).
- **`draw_obj_clip` fehlte in der Liste des Auftrags** (sechs Einsprünge genannt,
  `:1891` nicht darunter). Es ist dieselbe Klasse wie `draw_obj_t` und ist
  mitgeklammert; ein Loch in derselben Datei stehen zu lassen wäre die schlechtere
  Wahl gewesen.
- **`set_state_bit_t` ist bewusst NICHT geklammert**, anders als im Auftrag
  vorgesehen. Es ist kein `ob_*`-Einsprung, sondern ein Aufrufer von `change_t`;
  eine Klammer dort wäre eine überflüssige Verschachtelungsebene. Die Begründung
  steht im Kommentar an der Funktion.
- **Textcursor als `fill_rect`** (`object.rs`, `edit_field_t`) statt XOR-Linie
  (EmuTOS `aes/gemobed.c:203,216`) — unverändert, eigener Befund.
- **`objc_change` hat keinen XOR-Pfad** (EmuTOS `gemoblib.c:1290-1304`
  `bb_fill(MD_XOR,…)` mit `redraw = !xor_is_ok()`) — unverändert, eigener Befund.
- **`resource-fileselector.md` erwähnt den Zeiger nicht** — unverändert.
- **Ein Bildpunkt Versatz am geparkten Mauspfeil** gegenüber
  `measurement-2026-09-14-x01/screendumps/apitest-after.png`: der Pfeil steht heute
  bei (320,240) statt (321,241). Der Versatz liegt in **beiden** Läufen dieses
  Tickets gleich vor, gehört also in die drei Tage zwischen dem 14.09. und heute,
  nicht hierher. Wer `vdi_mouse_park` liest, erwartet bei 640×480 `(639/2, 479/2)`
  = (319,239) — keiner der beiden Abzüge zeigt das. Eigener Befund.

## Was am Spec-Stand geändert wurde — und warum das im Auftrag lag

`docs/api/aes/object-form.md`, Eintrag `objc_change`: die fehlende Zeile über die
`gsx_moff`-Klammer nachgetragen (`gemoblib.c:1275` / `:1319`). Der Auftrag führte
die Spec-Lücke unter „melden, nicht mitändern"; die Regel „Spec vor Code"
(`CLAUDE.md` §5) verlangt aber, dass eine implementierte Zusage eine Spec-Zeile
hat. Das ist genau die Zeile zu dieser Änderung, nicht eine Nachbarschaftspflege —
der ebenfalls genannte Teil der Lücke (`resource-fileselector.md`) ist unberührt.

## Nicht committet

Der Arbeitsbaum wird geteilt und trägt fremde, unversionierte Arbeit. Die
Änderung liegt im Baum; Commit und der Hash für `docs/STATUS.md` sind Sache des
Leads. Der `Recent Decisions`-Eintrag ist mit dem Stand „Arbeitsbaum, noch nicht
committet" geschrieben und muss beim Commit um den Hash ergänzt werden.

## Berührte Dateien

```
tos-api/src/aes/object.rs                    (Fix)
tos-api/tests/aes_object_cursor_bracket.rs   (neu, 3 Dauertests)
docs/api/aes/object-form.md                  (Spec-Zeile zu objc_change)
docs/STATUS.md                               (Phasenzeile + Recent Decisions)
TASK/A65.md                                  (Kopf)
TASK/A65-STATUS-DONE.MD                      (diese Quittung)
measurement-2026-09-17-a65/                  (Messlauf)
```
