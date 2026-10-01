# AES-R3 `aes-r3-wind-state.tsv` — Einordnung der zwölf Abweichungen

**Datum:** 2026-09-12 · **Rolle:** Audit (kein Produktcode geändert)
**Gepinnte EmuTOS-Quelle:** `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
**Referenzdump:** `tosdump/reference/EMUTOS14-AES-r3.BIN` (EmuTOS 1.4, `etos512de.img`, Hatari TT/ttmed/VGA, 640×480)
**NewTOS-Dump:** `measurements/measurement-2026-09-11-qemu-r3/AES.BIN` (QEMU raspi4b, 640×480, Build `70ca` = HEAD `70caee2`)
**Falltabelle:** `tosdump/cases/aes-r3-wind-state.tsv`, CRC `979FAC60`, 39 Fälle

---

## 0. Die Auswahl der Zwölf — selbst nachgeprüft

Ich habe die Dumps mit einem eigenen Dekoder (Record-Layout aus
`tosdump/core/tosdump.h:114-125`) **feldweise** verglichen — `opcode`, `contrl`,
`global`, `int_in`, `int_out`, `addr_in`, `addr_out`, `ret` — statt mich auf
`DIFF-emutos-vs-newtos-640x480.txt` zu verlassen.

Ergebnis: genau **261, 280, 340, 344, 400, 481, 494, 528, 549, 561, 574, 588**
weichen in etwas anderem als `global[]` ab. Kein Fall übersehen, keiner zu viel.
Die übrigen 27 unterscheiden sich ausschliesslich im `AESGLOBAL`, und zwar in
allen 39 Fällen identisch (je Dump genau **ein** distinkter `global`-Vektor):

    EmuTOS : [832, 1, 0, 0, 4419, 0, 0, 0, 0, 0,  4,     0, -16892, 0, 0]
    NewTOS : [832, 1, 1, 4419, 0, 0, 0, 0, 0, 0, 16, 13152,     18, 0, 0]

Pi 4 gegen QEMU: 38 Fälle byteidentisch, **einzige** Abweichung Fall 603
(`640,480` gegen `1920,1080`) — reine Bildschirmgrösse, Schublade 3. Der
Vergleich gegen den QEMU-Lauf ist damit belegt zulässig.

---

## 1. Was ich ausgeführt habe

### 1.1 Eigener EmuTOS-Lauf mit einer eigenen Falltabelle

Zwei Fragen liessen sich aus der R3-Messung allein nicht entscheiden. Ich habe
dafür eine **neue** 16-Fall-Tabelle gebaut (CRC `8AF4E2C1`, von `casecheck --aes`
angenommen, exit 0) und in Hatari gegen `etos512de.img` gefahren — 23 s, Lauf
sauber, „all 16 cases ran", keine Prozesse zurückgelassen. Die Tabelle liegt im
Scratch (`…/scratchpad/x1/AES.TSV`), **keine** bestehende Tabelle und **kein**
eingefrorenes Artefakt wurde angefasst.

**S1 — hängt EmuTOS' `WF_WXYWH` auf einem erzeugten, nie geöffneten Fenster von
den `wind_create`-Argumenten ab?**

| Fall | Vorlauf | EmuTOS `int_out[1..4]` |
|---|---|---|
| 16 | `create(0x0FEF, 10,20,300,200)` — **erster Fall des Laufs** | `0, 0, 0, 0` |
| 19 | `create(0x0FEF, **100,120,200,150**)` | `0, 19, 640, 461` |
| 22 | `create(**0x0001**, 10,20,300,200)` | `0, 19, 640, 461` |
| 25 | `create(**0**, 10,20,300,200)` | `0, 19, 640, 461` |
| 36 | create, open, close, **delete**, create neu mit anderem FULL | `0, 19, 640, 461` |
| 40 | create 1, create 2 → `WF_WXYWH(**2**)`, Handle 2 im Lauf nie zuvor benutzt | `0, 0, 0, 0` |
| 27 | Orakel `wind_calc(WC_WORK, 0x0FEF, 10,20,300,200)` | `11, 43, 276, 154` |
| 28 | Orakel `wind_calc(WC_WORK, 0x0FEF, 100,120,200,150)` | `101, 143, 176, 104` |
| 29 | Orakel `wind_calc(WC_WORK, 0, 10,20,300,200)` | `11, 21, 298, 198` |

Damit ist bewiesen: die Antwort ist **unabhängig von `kind` und von `WS_FULL`**
(19/22/25 liefern denselben Wert bei drei verschiedenen Argumentkombinationen).
Sie ist der **stehengebliebene `WS_WORK`-Slot**: `0,0,0,0` solange der Handle im
Lauf noch nie belegt war (BSS, Fälle 16 und 40), `gl_rfull` = `0,19,640,461`
sobald ihn ein `wm_delete` einmal durchlaufen hat. Das ist exakt das, was die
Quelle sagt — `wm_create` schreibt `WS_WORK` **nie**, `wm_delete` schreibt dort
`gl_rfull` (`aes/gemwmlib.c:1370-1385` gegen `:1480`).

**S2 — woher kommen `x`/`y`, wenn ein `NEXT` auf eine bereits erschöpfte Liste
trifft (P1-Pfad)?**

| Fall | letzter Aufruf vor der Messung (dessen `int_out[1..4]`) | gemessenes `int_out[1..4]` |
|---|---|---|
| 53 | `FIRST(1)` → `31,63,296,194` | `31, 63, 0, 0` |
| 59 | `WF_FXYWH` → `10,20,300,200` | **`10, 20`**, 0, 0 |
| 65 | `WF_CXYWH` → `30,40,320,240` | **`30, 40`**, 0, 0 |
| 71 | `wind_set(WF_HSLIDE)`, `n_intout = 1` (davor `FIRST` → `31,63,…`) | `31, 63, 0, 0` |
| 76 | `FIRST(1)` → `31,63,296,194`, gemessen **`WF_TOP` mit `n_intout = 5`** | `1`, **`63, 296, 194`** |
| 81 | `WF_CXYWH` → `30,40,320,240`, gemessen **`WF_TOP` mit `n_intout = 5`** | `1`, **`40, 320, 240`** |

Das entscheidet die Frage restlos, und zwar **gegen** jede Fenster-Semantik:
Fall 59 und 65 zeigen, dass `x`/`y` schlicht die Wörter des **vorherigen
AES-Aufrufs** an derselben Indexposition sind. Fall 76 und 81 zeigen dasselbe
auf einem Pfad, der mit `w_owns` nichts zu tun hat: `wm_get(WF_TOP)` schreibt nur
`poutwds[0]`, und mit `n_intout = 5` kommen `int_out[2..4]` als *die drei
restlichen Wörter des Vorgängeraufrufs* heraus.

Die Ursache steht im Dispatcher, nicht im Fenstermanager: `xif` legt
`WORD int_out[O_SIZE]` als **uninitialisierte** lokale Variable an und kopiert
danach **bedingungslos** `OUT_LEN` Wörter davon in das Array des Aufrufers
(`aes/gemsuper.c:430-432`, `:444-445`). Weil jeder AES-Aufruf denselben
Stapelrahmen belegt, trägt die Zelle den Wert des Vorgängeraufrufs. Der
Dumper poisont sein eigenes `int_out` **vor jedem einzelnen Aufruf**, auch vor
jedem Vorlauf (`tosdump/core/runner_aes.c:160-183`) — der Wert kann also
nicht aus dem Anwendungsarray stammen.

### 1.2 Was ich aus der Quelle nachgerechnet und gegen die Messung geprüft habe

Ich habe die Zerlegung beider Drei-Fenster-Gruppen aus `newrect`/`mkrect`/
`brkrct`/`mkpiece` (`aes/gemwrect.c:64-205`) von Hand durchgerechnet und gegen
`w_owns`' Arbeitsflächenschnitt (`aes/gemwmlib.c:1230-1241`,
`util/rectfunc.c:63-78`) geclippt:

- **Gruppe I** (Öffnungsfolge 1, 2, 3) → 4 Stücke:
  `31,63,296,17` · `31,80,29,162` · `262,80,65,80` · `31,242,149,15`
- **Gruppe II** (Öffnungsfolge 1, 3, 2) → 5 Stücke:
  `31,63,296,17` · `31,80,29,80` · `262,80,65,80` · `31,160,29,82` · `31,242,149,15`

Beide Reihen stimmen **Wert für Wert** mit dem EmuTOS-Dump überein (Fälle
439/448/458/469/481 bzw. 519/528/538/549/561/574). Die Hatari-Referenz ist damit
nicht nur gegen echte TT-Dumps, sondern zusätzlich gegen die Quelle validiert.

### 1.3 Versionsfrage — abschliessend geklärt, nicht behauptet

Zwischen `VERSION_1_4` und dem gepinnten Stand hat sich in `aes/` **genau
zweierlei** geändert:

    $ git -C references/emutos log --oneline VERSION_1_4..HEAD -- aes/
    acb7f3b4 Add partial implementation of WF_OWNER to wind_get
    4b0d9518 Refine 'nicelines' check
    $ git -C references/emutos diff --stat VERSION_1_4..HEAD -- aes/
     aes/gemoblib.c |  2 +-
     aes/gemwmlib.c | 13 +++++++++++++

`aes/gemwrect.c` ist **unverändert**; in `gemwmlib.c` ist ausschliesslich der
`WF_OWNER`-Zweig hinzugekommen (Feld 20, in dieser Tabelle nie abgefragt).
`wm_create`, `wm_open`, `wm_close`, `wm_delete`, `w_getsize`, `w_owns`,
`draw_change` sind identisch. **Schublade 2 trifft auf keinen der zwölf Fälle
zu.** Das ist geprüft, nicht angenommen.

---

## 2. Die zwölf Fälle

Drei Ursachen, mehr nicht.

| Fall | Was geprüft wird | EmuTOS | NewTOS | Schublade |
|---|---|---|---|---|
| 261 | `NEXT(2)` am Listenende (P1) | `31,63,0,0` | `0,0,0,0` | **3** — B |
| 280 | `NEXT(1)` nach 4 Stücken (P1) | `31,242,0,0` | `0,0,0,0` | **3** — B |
| 340 | `WF_WXYWH`, erzeugt, nie geöffnet | `0,19,640,461` | `11,43,276,154` | **1** — A |
| 344 | dasselbe nach `wind_close` | `0,19,640,461` | `11,43,276,154` | **1** — A |
| 400 | `NEXT(1)` nach dem P2-Ende (P1) | `31,280,0,0` | `0,0,0,0` | **3** — B |
| 481 | Gruppe I, 5. Aufruf = Ende (P1) | `31,242,0,0` | `0,0,0,0` | **3** — B |
| 494 | Gruppe I, 6. Aufruf = Ende (P1) | `31,242,0,0` | `0,0,0,0` | **3** — B |
| 528 | Gruppe II, Stück 2 | `31,80,29,80` | `31,80,29,162` | **1** — C |
| 549 | Gruppe II, Stück 4 | `31,160,29,82` | `31,242,149,15` | **1** — C |
| 561 | Gruppe II, Stück 5 | `31,242,149,15` | `0,0,0,0` | **1** — C (+ B für x/y) |
| 574 | Gruppe II, 6. Aufruf = Ende (P1) | `31,242,0,0` | `0,0,0,0` | **3** — B |
| 588 | Gruppe II, 7. Aufruf = Ende (P1) | `31,242,0,0` | `0,0,0,0` | **3** — B |

---

## Befund A — `wind_create` schreibt `WS_WORK`, EmuTOS nicht

**Fälle 340, 344 · Schublade 1 · CONFIRMED · Schwere: niedrig–mittel**

**EmuTOS-Beleg.** `aes/gemwmlib.c:1370-1385` (`wm_create`) schreibt genau drei
Slots — `WS_CURR = gl_rzero`, `WS_PREV = gl_rzero`, `WS_FULL = pt` — und `w_setup`
(`:205-228`) rührt `w_work` ebenfalls nicht an. `WS_WORK` wird in der ganzen
AES nur an drei Stellen geschrieben: `wm_init` für den Desktop (`:1294`),
`wm_delete` (`:1480`, `= gl_rfull`) und `draw_change` (`:1073-1077`).
`wm_get(WF_WXYWH)` ist ein reines `w_getsize(WS_WORK, …)` (`:1529`, `:254-262`)
— kein Rechenweg.

**NewTOS-Beleg.** `tos-api/src/aes/window.rs:2267`:

```rust
w.work = calc(WC_WORK, kind, full);
```

in `wind_create`, direkt nach `w.full = full; w.curr = …; w.prev = …`. Der
Kommentar darüber zitiert `gemwmlib.c:1377-1380` korrekt für CURR/PREV/FULL —
und die Zeile darunter tut etwas, das dort nicht steht.

**Beleg, dass es genau diese Zeile ist.** NewTOS liefert `11,43,276,154`. Mein
Orakelfall 27 zeigt: das ist `wind_calc(WC_WORK, 0x0FEF, 10,20,300,200)`, also
`calc` von `WS_FULL` — auf die Stelle genau. Gegenprobe: EmuTOS' Wert ist von
`kind` und `full` **nachweislich unabhängig** (Fälle 19/22/25), NewTOS' Wert ist
es nachweislich nicht.

**Wie sich der Fehler über die dokumentierte API zeigt.** Ein Programm, das
`wind_create(kind, …)` und danach — vor dem `wind_open` — `wind_get(h,
WF_WXYWH)` aufruft, bekommt unter EmuTOS den Bildschirm ohne Menüzeile
(`gl_rfull`, bzw. `0,0,0,0` bei einem noch nie belegten Handle) und unter NewTOS
die gerechnete Arbeitsfläche der Maximalausdehnung. Die Folge ist
**auflösungsabhängig invertiert**: unter EmuTOS wächst der Wert mit dem
Bildschirm (auf der Pi 4 wäre er `0,19,1920,1061`), unter NewTOS nicht — der
Pi-Dump liefert dieselben `11,43,276,154` wie QEMU. Ein Programm, das seine
Dokumentgrösse daraus ableitet, rechnet auf beiden Maschinen mit verschiedenen
Zahlen.

**Kein Geräteparameter.** `gl_rfull` ist zwar geräteabhängig, aber die
Abweichung ist es nicht: NewTOS' Antwort ist auf beiden Auflösungen dieselbe
Konstante und damit weder `gl_rfull` noch irgendeine Funktion davon.

**Nebenbefund an derselben Naht.** NewTOS' `wind_delete`
(`tos-api/src/aes/window.rs:2452-2462`) setzt `rlist`/`rcount`/`rnext`/`in_use`/
`open` zurück, schreibt aber **nicht** `WS_CURR = WS_PREV = gl_rscreen` und
`WS_FULL = WS_WORK = gl_rfull` wie `wm_delete` (`gemwmlib.c:1477-1480`). Heute
ist das unbeobachtbar, weil `wind_create` alle vier Slots neu setzt. Sobald die
Zeile `:2267` fällt, wird es beobachtbar — die beiden gehören in **eine**
Korrektur, sonst liefert das reparierte `wind_create` `0,0,0,0` statt `gl_rfull`
und Fall 340 kippt von einem falschen Wert auf einen anderen falschen Wert.

**Was A16 davon berührt:** nichts. A16 hat den `wind_close`-Pfad in Ordnung
gebracht, und die Messung bestätigt das (Abschnitt 4). Der Create-Pfad war
ausdrücklich nicht Teil von A16's Auftrag.

---

## Befund B — das Listenende schreibt `x`/`y` nicht: kein Soll

**Fälle 261, 280, 400, 481, 494, 574, 588 und die `x`/`y`-Hälfte von 561 ·
Schublade 3 · CONFIRMED als „kein Soll" · keine Reparatur**

**EmuTOS-Beleg.** `aes/gemwmlib.c:1230-1241` (`w_owns`):

```c
while (po) {
    rc_copy(&po->o_gr, poutwds);
    pwin->w_rnext = po = po->o_link;
    if (rc_intersect(pt, poutwds))
        return;
}
poutwds->g_w = poutwds->g_h = 0;
```

Wird die Schleife **gar nicht betreten** (P1), schreibt `:1240` nur `g_w`/`g_h`.
`poutwds` ist `&int_out[1]` **des dispatch-lokalen, uninitialisierten**
`WORD int_out[O_SIZE]` aus `xif` (`aes/gemsuper.c:430-432`), das anschliessend
bedingungslos mit `OUT_LEN` Wörtern in das Anwendungsarray kopiert wird
(`:444-445`).

**Beleg, dass es Stapelinhalt ist und keine Fenstersemantik.** Meine Fälle 59
und 65: dasselbe Fenster, dieselbe erschöpfte Liste, nur ein anderer
Zwischenaufruf — und `x`/`y` folgen dem Zwischenaufruf (`10,20` bzw. `30,40`)
statt dem Fensterzustand. Meine Fälle 76 und 81: `WF_TOP` mit `n_intout = 5`
gibt `int_out[2..4]` des Vorgängeraufrufs zurück, auf einem Pfad ohne jede
Berührung mit `w_owns`. Damit ist die Erscheinung **dispatcherweit** und nicht
Teil des Fenstervertrags.

**Warum das keine Reparatur begründet.** Der Wert ist keine Aussage der AES,
sondern der Inhalt einer nicht initialisierten lokalen Variablen. Ein
konformes Programm liest nach `w == h == 0` keine Koordinaten mehr — der
dokumentierte Abbruchvertrag ist genau `w == 0 && h == 0`
(`docs/api/aes/window-graphics.md:339-342`). NewTOS' Null ist ein definierter
Platzhalter für einen undefinierten Wert; das exakte Nachbilden wäre das
Nachbilden einer fremden Stapelbelegung und auf einer anderen Architektur nicht
einmal wohldefiniert.

Das deckt sich mit dem, was `FIX-05-STATUS-DONE.MD` bereits aus der Quelle
abgeleitet hatte („**Kein Soll. Hier war nichts zu reparieren.**"). Neu ist,
dass es jetzt **gemessen** ist statt nur gelesen.

**Die Gegenseite stimmt.** Der P2-Pfad — Liste nicht leer, kein Stück trifft die
Arbeitsfläche — hat sehr wohl ein Soll, und NewTOS trifft es: Fall 393 liefert
auf **beiden** Maschinen `31,280,0,0`. Das ist FIX-05's eigentlicher Inhalt und
er ist damit messbelegt.

**Ehrliche Einschränkung.** Ich habe nicht geprüft, ob die Kommentarbehauptung in
`tos-api/src/aes/window.rs:1410-1412` („`Owns::Empty` … Leaving x/y untouched is
the point") mit dem Code übereinstimmt — der Code schreibt dort tatsächlich nur
`int_out[3]`/`[4]`, die Nullen in `[1]`/`[2]` kommen aus dem Dispatcher. Der
Kommentar ist also inhaltlich richtig, aber der Effekt nach aussen ist Null statt
unverändert. Das ist hier folgenlos; es wäre nur dann eine Frage, wenn NewTOS
irgendwo einen undeklarierten Ausgabewort-Inhalt garantieren wollte, was es
nicht tut.

---

## Befund C — die verdeckenden Rechtecke werden in Handlereihenfolge abgezogen, nicht in z-Reihenfolge

**Fälle 528, 549, 561 · Schublade 1 · CONFIRMED · Schwere: hoch**

Das ist der Befund, für den die Tabelle gebaut wurde, und er ist positiv
ausgefallen.

**Die Messung.** Gruppe I und Gruppe II zeigen **dieselben drei Rechtecke**, nur
in unterschiedlicher Öffnungsreihenfolge (1,2,3 gegen 1,3,2). EmuTOS antwortet
verschieden, NewTOS zweimal gleich:

| Aufruf | EmuTOS Gr. I | EmuTOS Gr. II | NewTOS Gr. I | NewTOS Gr. II |
|---|---|---|---|---|
| FIRST | `31,63,296,17` | `31,63,296,17` | `31,63,296,17` | `31,63,296,17` |
| NEXT 1 | `31,80,29,162` | `31,80,29,**80**` | `31,80,29,162` | `31,80,29,**162**` |
| NEXT 2 | `262,80,65,80` | `262,80,65,80` | `262,80,65,80` | `262,80,65,80` |
| NEXT 3 | `31,242,149,15` | `31,**160,29,82**` | `31,242,149,15` | `31,**242,149,15**` |
| NEXT 4 | *Ende* | `31,242,149,15` | *Ende* | *Ende* |
| NEXT 5 | *Ende* | *Ende* | *Ende* | *Ende* |

**EmuTOS: 4 Stücke in Gruppe I, 5 in Gruppe II. NewTOS: 4 und 4 — und zwar
zweimal exakt die Liste von Gruppe I.** Eine Umsetzung, die die Öffnungs- bzw.
z-Reihenfolge nicht kennt, kann die beiden Gruppen nicht unterscheiden, und
genau das ist eingetreten.

**EmuTOS-Beleg für die Regel.** `draw_change` baut bei **jeder** Änderung alle
Listen neu, in Baumreihenfolge:

```c
/* update rectangle lists */
everyobj(gl_wtree, ROOT, NIL, (EVERYOBJ_CALLBACK)newrect, 0, 0, MAX_DEPTH);
```
— `aes/gemwmlib.c:1088`.

`newrect(tree, wh)` (`aes/gemwrect.c:165-205`) verwirft die eigene Liste, bricht
mit `everyobj(tree, ROOT, wh, mkrect, …)` (`:198`) die Listen **aller vor `wh`
besuchten** Fenster (Abbruch bei `this == last`, `aes/gemobjop.c:93-94`) und legt
dann die eigene neu an. Die Geschwisterliste läuft von `ob_head` nach `ob_tail`,
und `gl_wtop = W_TREE[ROOT].ob_tail` (`gemwmlib.c:1092`) — das **letzte** Kind ist
das oberste Fenster. `w_obadd` hängt ein neu geöffnetes Fenster als letztes Kind
an (`:182-202`), `wm_mktop` verschiebt per `ob_order(…, NIL)` ebenfalls ans Ende
(`:1605`). Also: **jede Liste wird von den verdeckenden Fenstern in aufsteigender
z-Reihenfolge gebrochen**, und `brkrct` zerschneidet dabei jedes *überlebende*
Stück erneut — die Reihenfolge ändert das Ergebnis.

**NewTOS-Beleg.** `tos-api/src/aes/window.rs:788-792`:

```rust
for ow in slots().iter() {
    if ow.in_use && ow.open && ow.z > myz {
        n = subtract(out, n, ws_true(ow.curr));
    }
}
```

Die **Menge** ist richtig (`ow.z > myz`), die **Reihenfolge** ist die
Slot-/Handlereihenfolge. `subtract` selbst ist eine getreue `brkrct`-Umsetzung
(TOP/LEFT/RIGHT/BOTTOM, In-Place-Ersetzung) — der Fehler sitzt allein in der
Schleifenreihenfolge. Dass NewTOS die z-Ordnung kennt, zeigt dieselbe Datei an
anderer Stelle: `:1185-1195` sortiert fürs Zeichnen per Auswahlsortierung nach
aufsteigendem `z`, und `bump_z()` (`:536-538`) wird sowohl bei `wind_open`
(`:2304`) als auch beim Toppen (`:675`) gezogen. Die Information ist da; die
Rechteckliste benutzt sie nicht.

**Wie sich der Fehler über die dokumentierte API zeigt.** Drei überlappende
Fenster, deren Öffnungsreihenfolge von der Handlereihenfolge abweicht — z. B.
Fenster 3 geöffnet, dann Fenster 2 darüber. Das unterste Fenster bekommt in
seiner Redraw-Schleife **vier statt fünf** Stücke, und zwei davon haben falsche
Höhen (`…,29,162` statt `…,29,80`; das Stück `31,160,29,82` fehlt ganz). Die
*überdeckte Gesamtfläche* stimmt zwar, aber:

1. Die Zerlegung überdeckt in NewTOS Gruppe II Fläche, die EmuTOS in zwei
   Stücke teilt — die Stückgrenzen liegen anders. Jede `G_USERDEF`-Zeichenroutine
   und jeder Anwendungs-Redraw, der pro Stück einmal gerufen wird, wird eine
   andere Anzahl Male mit anderen Rechtecken gerufen.
2. Die Liste steuert nicht nur `WF_FIRSTXYWH`, sondern in NewTOS auch
   `walk_rects` (`:818-840`, der Zeichenpfad) und `w_union` (den
   `WM_REDRAW`-Hüllvergleich). Ein Zerlegungsfehler ist damit **kein reiner
   Abfragefehler**, sondern erreicht die Fensterdarstellung.
3. Der Sonderfall, in dem die falsche Reihenfolge nicht nur andere Stücke,
   sondern *zu wenige* Stücke ergibt, ist hier real eingetreten (4 statt 5).
   Ob es eine Geometrie gibt, in der dadurch Fläche **verloren** geht statt nur
   anders geschnitten zu werden, habe ich nicht geprüft — siehe Abschnitt 6.

**Schwere.** Hoch, weil der Pfad in der Darstellung landet und weil drei
überlappende Fenster mit gemischter Öffnungsreihenfolge auf einem echten
Desktop der Normalfall und nicht die Ausnahme sind.

**Vermutete Richtung der Reparatur** (Entscheidung liegt beim Entwickler):
die verdeckenden Fenster vor dem Abziehen nach **aufsteigendem `z`** sortieren,
wie `:1185-1195` es für den Zeichenlauf bereits tut. Die Regel „Baumreihenfolge"
ist quellenbelegt und deckungsgleich mit „aufsteigendes z", weil `w_obadd` und
`ob_order(…, NIL)` beide ans Ende hängen.

---

## 3. Spezifikationsbefund — an den Forscher

`docs/api/aes/window-graphics.md:347-358` beschreibt die Rechteckliste und
schliesst mit:

> **Contract:** the *sequence of visible pieces* a redraw handler receives …
> The `ORECT` free-list, the four-way split, and `VF_BROKEN` are **reference**;
> NewTOS **may compute the visible-region decomposition any way it likes as long
> as `WF_FIRSTXYWH`/`WF_NEXTXYWH` yield the same set of pieces.**

**Diese Freigabe ist durch die Messung widerlegt.** Es gibt keine
implementierungsfreie „gleiche Stückmenge": die Zerlegung ist eine
deterministische Funktion der Bruchreihenfolge, und EmuTOS liefert für dieselbe
sichtbare Fläche nachweisbar fünf Stücke, wo eine flächengleiche Zerlegung mit
vieren auskommt. Der Absatz ist genau die Lizenz, unter der die heutige
NewTOS-Umsetzung gebaut wurde. **Befund C ist damit zu einem guten Teil ein
Spezifikationsfehler, nicht nur ein Codefehler** — und dass der zugehörige
Stelle im Code (`window.rs:1356-1360`: „for a stable arrangement … is the same
list") dieselbe Annahme wiederholt, ist die Folge, nicht die Ursache.

Zwei kleinere Lücken derselben Datei:

- `:224-232` (`wind_create`) zählt CURR/PREV/FULL auf. Korrekt, aber es steht
  nicht da, dass `WS_WORK` **nicht** geschrieben wird — und genau dort ist NewTOS
  darüber hinausgegangen. Ein ausdrücklicher Negativsatz gehört hin.
- `:276-282` (`wind_delete`) sagt „resets its rects to screen/full". Dass
  `WS_WORK = gl_rfull` darin enthalten ist, ist über ein späteres
  `wind_create` + `WF_WXYWH` **beobachtbar** und sollte benannt werden.
- `:334-338` (`w_owns`) sagt „When the list is exhausted it returns a rect with
  `w=h=0`" — der P2-Fall (vier geschriebene Wörter, `x`/`y` vom letzten
  geprüften Stück) fehlt, obwohl FIX-05 ihn im Code umgesetzt hat und Fall 393
  ihn bestätigt.

---

## 4. Die vier Messlücken — schliesst die Messung sie?

| Ticket | Frage | Schliesst? | Richtung |
|---|---|---|---|
| **A24** verschachtelte FIRST/NEXT | Suchzeiger pro Fenster oder global? | **ja** | **NewTOS ist richtig.** Fall 253 (`F1, F2 → N1`) liefert auf beiden Maschinen `31,80,29,162`, also Fenster 1's zweites Stück — das `FIRST(2)` hat den Zeiger von Fenster 1 nicht verstellt. Der Spiegelfall 261 liefert auf beiden das Ende (`w = h = 0`); nur die undefinierten `x`/`y` weichen ab (Befund B). Fall 270 (zweimal verschränkt) und 280 (Massstab) stimmen. Der in A24 beschriebene Defekt D-A14-1 ist behoben und jetzt **messbelegt**, nicht nur hostgefahren. |
| **A16** die drei Rechtecke nach `wind_close` | schreibt der Close CURR/PREV/WORK? | **ja** | **NewTOS ist richtig; die Implementierung wird bestätigt, nicht widerlegt.** 301/306/311 (vor dem Close) und 317/323/329/335 (danach) stimmen alle sieben. 317 = `1,23,-24,-46` deckt sich mit dem Orakel 337 (`wind_calc(WC_WORK, 0x0FEF, 0,0,0,0)`) auf **derselben** Maschine, ist also nicht zufällig getroffen. 335 zeigt `WS_FULL` unverändert. 344 == 340 auf **beiden** Maschinen: die frühe Absage `:1446-1447` für ein nie geöffnetes Fenster wird eingehalten. `A16-STATUS-UNFINISHED.MD` kann auf DONE. **Aber:** 340/344 decken einen *benachbarten* Defekt auf, den A16 auftragsgemäss nicht berührt hat (Befund A) — der braucht ein eigenes Ticket. |
| **P2 / FIX-05** vier Wörter am nichtleeren Listenende | | **ja** | **NewTOS ist richtig.** Fall 393 stimmt auf beiden Maschinen exakt (`31,280,0,0`), inklusive der aus der Geometrie hergeleiteten `y = 40 + 238 + DROP_SHADOW_SIZE = 280`. Die daneben gemessene P1-Seite (Fall 400) ist **kein Soll** — jetzt gemessen statt nur hergeleitet (Befund B). |
| **A22** drei und mehr überlappende Fenster | dieselbe Stückmenge und Reihenfolge? | **ja** | **NewTOS ist falsch.** Befund C. Die in A22 als Verdacht formulierte Frage („NewTOS subtrahiert … in Handlereihenfolge") ist bestätigt und der Defekt reproduziert. |

**Bilanz:** drei von vier Lücken schliessen zugunsten von NewTOS, eine dagegen.
Zusätzlich ein Defekt, den keines der vier Tickets gesucht hatte (Befund A).

---

## 5. Fall 603 und das `AESGLOBAL`

- **603** (`wind_close(0)` → `WF_CXYWH(0)`): EmuTOS `0,0,640,480`, NewTOS QEMU
  ebenso, NewTOS Pi 4 `0,0,1920,1080`. Die frühe Absage in `wm_close` greift auf
  beiden Maschinen; der Unterschied ist die Bildschirmgrösse. **Schublade 3.**
- Das `AESGLOBAL`-Rauschen (`ap_id` 0 gegen 1, `ap_private` in umgekehrter
  Wortlage, `ap_planes` 4 gegen 16, `ap_3resv`) ist in allen 39 Fällen identisch,
  also nicht fallabhängig, und war ausdrücklich nicht Gegenstand dieses
  Auftrags. Eine Beobachtung dazu unten.

---

## 6. Was offen bleibt

Ehrlich benannt, nicht verschwiegen:

1. **Ob Befund C jemals Fläche verliert.** Gemessen ist eine andere Zerlegung
   derselben Fläche plus eine falsche Stückzahl. Ob es eine Geometrie gibt, in
   der die Handlereihenfolge sichtbare Fläche ganz aus der Liste fallen lässt
   (statt sie nur anders zu schneiden), habe ich **nicht** geprüft. Für die
   Reparaturentscheidung ist es unerheblich — die Abweichung steht fest —, für die
   Schwereeinschätzung nicht.
2. **Ob die Bruchregel „Baumreihenfolge" von „aufsteigendes z" trennbar ist.**
   In beiden Gruppen fallen sie zusammen, weil die Anordnung nur durch `wind_open`
   entstanden ist. Trennen liesse sich das nur über `wind_set(WF_TOP)`, und die
   R3-Tabelle hat den Pfad bewusst gemieden (`WM_REDRAW` in die nie geleerte
   eigene Queue). Aus der Quelle sind sie deckungsgleich (`w_obadd` und
   `ob_order(…, NIL)` hängen beide ans Ende); **gemessen** ist nur der
   Öffnungsfall. Wer die Reparatur abnimmt, sollte eine WF_TOP-Tabelle
   nachziehen.
3. **Kein NewTOS-Lauf meiner Probetabelle.** Die beiden Fragen, die sie
   beantwortet, liegen auf der EmuTOS-Seite; NewTOS' Antworten auf die
   äquivalenten Fälle stehen bereits im R3-Dump (Fall 340 = `calc(WS_FULL)`, sieben
   P1-Fälle = Null). Ein Gegenlauf hätte keine Einstufung geändert. Ich habe ihn
   **nicht** gefahren und behaupte auch nicht, ihn gefahren zu haben.
4. **`RCAP = 16`** in `tos-api/src/aes/window.rs:1290`: `put` verwirft stumm
   jedes Stück jenseits des sechzehnten. Ab wie vielen Fenstern das greift und
   was EmuTOS' `get_orect()` bei erschöpftem `NUM_ORECT`-Pool tut (ein
   NULL-Zugriff in `mkpiece`, `gemwrect.c:68-69`), habe ich nicht untersucht.

---

## 7. Ausserhalb des Auftrags beobachtet — nicht verfolgt

- `ap_private` steht bei EmuTOS in `global[3..4]` als `0x0000,0x1143`, bei NewTOS
  als `0x1143,0x0000` — die beiden Wörter des LONG liegen vertauscht; ein Programm,
  das `AESGLOBAL.ap_private` als LONG liest, bekommt `0x11430000` statt `0x00001143`.
  Vom Auftrag als bekanntes Rauschen ausgenommen, wirkt aber wie eine Wortlage im
  `AESGLOBAL` und nicht wie ein Geräteparameter. Eigenes Ticket wert.
- `tos-api/src/aes/window.rs:2452-2462` (`wind_delete`) lässt `wm_delete`'s vier
  Slot-Rücksetzungen aus; heute unbeobachtbar, wird mit der Reparatur von Befund A
  beobachtbar. Gehört in dieselbe Korrektur (oben unter Befund A schon vermerkt).
- `docs/api/aes/window-graphics.md:948` bezeichnet `w_rlist`/`w_rnext` als
  „**Reference** — NewTOS reimplements the storage" — dieselbe Lizenz wie in
  Abschnitt 3, an einer zweiten Stelle.

---

## 8. Aufräumbestätigung

- Kein Produktcode angefasst. Keine `.rs`-Datei geändert.
- Keine bestehende Falltabelle, kein `*.BIN`, kein `*.LOG`, kein
  `measurement-*`-Verzeichnis geändert.
- Meine Probetabelle, ihr Dump und der Dekoder liegen ausschliesslich im
  Scratch-Verzeichnis der Sitzung, nicht im Arbeitsbaum.
- Einzige neue Datei im Repository: **dieses Dokument**, exklusiv angelegt
  (vorher auf Existenz geprüft).
- Kein `git checkout` / `clean` / `stash` / `restore` / `reset`, kein Commit.
- Nach dem Hatari-Lauf `pgrep -f 'qemu-system-aarch64|Hatari'` geprüft: keine
  Prozesse übrig. Keine Hardware berührt, nichts geflasht, nichts ausgehängt.
