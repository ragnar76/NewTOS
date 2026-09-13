# FIX-02 — Reihenfolge der Reststücke in der Rechteckliste

Status: **DONE** (2026-09-10) · Stufe 2 der Fix-Reihenfolge aus
[A23](A23.md), Abschnitt 8.2 · deckt **D-A13-2** ab
([A13-STATUS-UNFINISHED.MD](A13-STATUS-UNFINISHED.MD), A23 Abschnitt 3)

Kein Commit. Die Änderung liegt uncommittet im gemeinsamen Arbeitsbaum.

## Auftrag

Wenn ein höher liegendes Fenster ein Rechteck der Besitzerliste bricht,
entstehen bis zu vier Reststücke. EmuTOS erzeugt sie in **einer** festen
Reihenfolge: TOP, LEFT, RIGHT, BOTTOM. NewTOS erzeugte sie in der Reihenfolge
TOP, BOTTOM, LEFT, RIGHT. Die Fläche ist dieselbe, die **Reihenfolge** nicht —
und die ist beobachtbar, weil sie die Aufrufreihenfolge aller drei Leser der
Liste bestimmt.

Ausdrücklich **nicht** Gegenstand und **nicht** angefasst: Schatten (Stufe 4),
Cursorbesitz (Stufe 3), P1/P2-Wörter am Listenende (Stufe 5), `w_redraw`
(Stufe 6), Kapazitätsgrenze `RCAP` (A21), Reihenfolge mehrerer brechender
Fenster (A22). Keine Kompensation an irgendeiner Leserseite eingebaut.

## Was geändert wurde

### 1. `tos-api/src/aes/window.rs`, `subtract` (jetzt `:1073-1121`)

Reine Umstellung von zwei Blöcken plus Kommentar; keine Formel, keine
Bedingung, keine Signatur geändert.

**Vorher** (vormals `:1073-1101`) — der BOTTOM-Block stand vor der Berechnung
des Überlappungsbands `oy`/`oh` und damit vor LEFT und RIGHT:

```rust
        if r.y < c.y {                                  // TOP
            put(&mut out, &mut m, Grect { x: r.x, y: r.y, w: r.w, h: c.y - r.y });
        }
        if r.y + r.h > c.y + c.h {                      // BOTTOM
            put(&mut out, &mut m, Grect { x: r.x, y: c.y + c.h, w: r.w, h: (r.y + r.h) - (c.y + c.h) });
        }
        let oy = r.y.max(c.y);
        let oh = (r.y + r.h).min(c.y + c.h) - oy;
        if r.x < c.x {                                  // LEFT
            put(&mut out, &mut m, Grect { x: r.x, y: oy, w: c.x - r.x, h: oh });
        }
        if r.x + r.w > c.x + c.w {                      // RIGHT
            put(&mut out, &mut m, Grect { x: c.x + c.w, y: oy, w: (r.x + r.w) - (c.x + c.w), h: oh });
        }
```

**Nachher** (`:1104-1117`) — der BOTTOM-Block steht am Ende, `oy`/`oh` rücken
vor LEFT:

```rust
        if r.y < c.y {                                  // TOP
            put(&mut out, &mut m, Grect { x: r.x, y: r.y, w: r.w, h: c.y - r.y });
        }
        let oy = r.y.max(c.y);
        let oh = (r.y + r.h).min(c.y + c.h) - oy;
        if r.x < c.x {                                  // LEFT
            put(&mut out, &mut m, Grect { x: r.x, y: oy, w: c.x - r.x, h: oh });
        }
        if r.x + r.w > c.x + c.w {                      // RIGHT
            put(&mut out, &mut m, Grect { x: c.x + c.w, y: oy, w: (r.x + r.w) - (c.x + c.w), h: oh });
        }
        if r.y + r.h > c.y + c.h {                      // BOTTOM
            put(&mut out, &mut m, Grect { x: r.x, y: c.y + c.h, w: r.w, h: (r.y + r.h) - (c.y + c.h) });
        }
```

Gemessener Umfang dieser einen Änderung an `window.rs`: **24 Zeilen hinzu, 4
entfernt, netto +20** — davon sind 21 hinzugefügte und 1 entfernte
Kommentarzeile; der ausführbare Anteil ist der Umzug von drei Zeilen (der
BOTTOM-Block) hinter den RIGHT-Block. Der Doc-Kommentar nennt jetzt `brkrct` als Quelle
und den In-Place-Ersatz des gebrochenen Rechtecks; ein Blockkommentar vor den
vier Stücken nennt die Konstanten, die Einfügeschleife, die Kantenformeln und
die drei Leser. **Netto Codeänderung: ein verschobener Block (BOTTOM ans Ende),
sonst nichts.**

Die vier Kantenformeln und die vier `have_piece`-Bedingungen sind Zeichen für
Zeichen unverändert. Selbst gegen `mkpiece` nachgeprüft, siehe unten.

### 2. `tos-api/tests/aes_desk_background.rs` — +149 Zeilen, nur Anhang

Neuer Abschnitt „7. the order the rectangle list breaks a window into" mit
genau einem Test, den beiden Fenstergeometrien, den vier EmuTOS-Konstanten und
dem Klassifizierer `piece_kind`.

Unterbringung wie bei FIX-01 in dieser Datei, weil sie den Aufbau fertig hat
(`SERIAL`-Lock, `rig()`/`fresh()`, `open_window`, `wind_get_rect`,
`wind_get_ret_rect`, `WF_FIRSTXYWH`/`WF_NEXTXYWH`); ein eigenes Testbinary
hätte rund 600 Zeilen Harness dupliziert. Die sieben vorhandenen Tests der
Datei wurden nicht berührt.

## EmuTOS-Beleg

Gepinnter Stand `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(selbst geprüft, `git rev-parse HEAD`).

- `aes/gemwrect.c:30-33` — die Konstanten:
  `#define TOP 0`, `#define LEFT 1`, `#define RIGHT 2`, `#define BOTTOM 3`.
- `aes/gemwrect.c:113-125` — `brkrct` füllt `have_piece[]` indiziert mit genau
  diesen Konstanten und hängt anschließend an:
  `for (i = 0; i < 4; i++) if (have_piece[i]) p = (p->o_link = mkpiece(i, new, r));`
  Aufsteigende Indexordnung **ist** damit die Ausgabereihenfolge: TOP, LEFT,
  RIGHT, BOTTOM.
- `aes/gemwrect.c:64-98` — `mkpiece`. Die „common calcs" setzen
  `x = old.x`, `w = old.w`, `y = max(old.y, new.y)`,
  `h = min(old.y+old.h, new.y+new.h) - y`; das ist Zeile für Zeile unser
  `oy`/`oh`. Danach überschreibt der `switch`: TOP `y = old.y`,
  `h = new.y - old.y`; LEFT `w = new.x - old.x`; RIGHT `x = new.x + new.w`,
  `w = (old.x+old.w) - (new.x+new.w)`; BOTTOM `y = new.y + new.h`,
  `h = (old.y+old.h) - (new.y+new.h)`. **Alle vier stimmen mit unseren
  Formeln überein** — nur die Reihenfolge war zu ändern, wie der Auftrag sagt.
- `aes/gemwrect.c:136-162` — `mkrect`: die Stücke ersetzen das gebrochene
  Rechteck **an seiner Stelle** in der Liste (`p->o_link = r->o_link`), die
  relative Ordnung der übrigen bleibt. Unser `subtract` baut `out` in genau
  dieser Ordnung neu auf; das war schon vorher richtig.
- Die drei Leser, für die die Reihenfolge sichtbar wird: `do_walk`
  (`aes/gemwmlib.c:286-312`, bei uns `walk_rects`), `w_owns`
  (`:1230-1240`, gelesen aus `wm_get:1556-1561`, bei uns
  `compute_rlist`/`rlist_next`) und `w_union` (`:869-884`, bei uns
  `visible_bound`).

Nichts an diesen Belegen hängt an Auflösung, Font oder Schattengröße. Es ist
Algorithmussemantik, kein Geräteparameter.

## Wirkung auf die drei Leser — geprüft, nicht angenommen

| Leser | NewTOS | Wirkung der Umstellung | Wie geprüft |
|---|---|---|---|
| Zeichnung | `walk_rects:633-654` → `draw_frame_pieces`, `paint_bg_clip`, Slider | Die Menge der Clip-Rechtecke ist unverändert, ihre **Reihenfolge** nicht. Da die Stücke disjunkt sind und je Stück derselbe Baum gezeichnet wird, ist das Pixelergebnis identisch; beobachtbar ist die Aufrufreihenfolge eines `G_USERDEF`-Objekts. | Alle Pixeltests grün: `aes_desk_background` (7 vorhandene), `aes_window_content`, `aes_window_drag`, `aes_dialog_undraw`, `aes_object_font`. Ein Reihenfolge-Soll für `G_USERDEF` existiert im Baum nicht (A23 7.1, selbst nachgegrept: kein Treffer auf `USERDEF` in `tos-api/tests/`). |
| Öffentliche Iteration | `work_rects` → `compute_rlist` → `rlist_next` → `wind_get` | **Das ist der Fix.** FIRST liefert jetzt LEFT statt BOTTOM, wenn kein TOP-Stück existiert. | Der neue Test unten. |
| WM_REDRAW | `work_rects` → `visible_bound:1188-1198` → `send_redraw_in` | Keine. `visible_bound` bildet die **Vereinigung** über alle Stücke (`union_grect`), die ist kommutativ und assoziativ; die Hülle ist reihenfolgeunabhängig. | Gelesen (`window.rs:1188-1198`) und durch die Redraw-Tests bestätigt: `moving_a_window_off_another_redraws_the_one_underneath`, `clicking_a_window_never_erases_its_content`, `the_mover_drag_repaints_only_desktop_it_uncovered` — alle grün. |

Der vierte, außerhalb von `tos-api` liegende Verbraucher —
`kernel/src/desk/redraw.rs:76-88` (`do_wredraw_nocursor`) — leert die Liste
vollständig und zeichnet in jedes Stück denselben Baum. Reihenfolgeunabhängig
im Ergebnis; nicht angefasst.

## Der neue Test und was er prüft

`tos-api/tests/aes_desk_background.rs:1222`,
`the_owner_rectangle_list_is_broken_in_top_left_right_bottom_order`.

Aufbau: `fresh()`, dann das untere Fenster auf `ORD_UNDER = (40,100,400,300)`
und darüber `ORD_OVER = (140,60,200,200)`. Die Lage ist so gewählt, dass genau
drei der vier Stücke existieren — **das TOP-Stück fehlt**, weil die Oberkante
des oberen Fensters über der des unteren liegt. Genau das ist der Fall, in dem
die Reihenfolge überhaupt sichtbar wird: solange ein TOP-Stück da ist, beginnt
jede der beiden Reihenfolgen damit. Ein einzelnes brechendes Fenster, höchstens
vier Stücke — weit unter `RCAP = 16`, also ohne Berührung mit A21/A22.

Der Test liest die tatsächlich vergebene Geometrie beider Fenster über
`wind_get(WF_CXYWH)` und prüft damit zuerst seine eigene Voraussetzung in
`brkrct`s eigenen Begriffen (`have_piece[TOP]` falsch, die anderen drei wahr).
Dann läuft er die Liste über `WF_FIRSTXYWH`/`WF_NEXTXYWH` bis zum Sentinel ab.

**Keine eingefrorene Zahl.** Jedes zurückgegebene Stück wird über `piece_kind`
danach klassifiziert, auf welcher Seite des überdeckenden Fensters es liegt —
nicht durch Vergleich mit einer erwarteten Koordinate. Geprüft wird dann:

1. Es kommt **kein** TOP-Stück (die Voraussetzung des Falls).
2. Die Folge ist `LEFT, RIGHT, BOTTOM`.
3. Die Folge ist in `brkrct`s Konstantenordnung streng aufsteigend.

`piece_kind` verlangt, dass ein Stück auf **genau einer** Seite liegt, und
schlägt sonst mit der Meldung fehl, dass die Geometrie des Aufbaus die vier
`mkpiece`-Fälle nicht mehr trennt. Die Klassifizierung ist gegen die noch
ausstehenden Stufen robust: das Zuschneiden auf die Arbeitsfläche (`w_owns`)
kann ein Stück nur verkleinern, nie auf die andere Seite schieben, und ein
späterer Schlagschatten vergrößert das überdeckende Rechteck nur nach rechts
und unten — er schiebt RIGHT weiter nach rechts und BOTTOM weiter nach unten,
ändert aber keine der vier Seitenzuordnungen und keine der vier
`have_piece`-Vergleiche (beide Rechtecke wachsen um dasselbe `s`).

### Beleg, dass der Test den Defekt fängt

Der Test wurde **vor** dem Fix geschrieben und gefahren. Er war rot, mit
genau der falschen Reihenfolge im Klartext:

    thread '...' panicked at tos-api/tests/aes_desk_background.rs:1268:
    assertion `left == right` failed: the owner rectangle list came out as
    ["BOTTOM", "LEFT", "RIGHT"] ([(41, 260, 376, 139), (41, 123, 99, 137),
    (340, 123, 77, 137)]). `brkrct` appends the pieces it has in ascending
    constant order TOP(0), LEFT(1), RIGHT(2), BOTTOM(3)
    (aes/gemwrect.c:30-33,113-125); with no TOP piece that is LEFT, RIGHT, BOTTOM
      left: [3, 1, 2]
     right: [1, 2, 3]

Also exakt der in A23 Abschnitt 3 beschriebene Befund: ohne TOP-Stück ist bei
uns BOTTOM das erste Rechteck, bei EmuTOS LEFT. Nach dem Fix grün.

## Tatsächlich gesehene Zahlen

Alle Läufe am 2026-09-10 im gemeinsamen Arbeitsbaum (`70caee2` plus die fremden
uncommitteten Änderungen und den gelandeten FIX-01), Host macOS/arm64.

`cargo test -p tos-api --tests`, Ausgangsstand nach Stufe 1 laut
[FIX-01-STATUS-DONE.MD](FIX-01-STATUS-DONE.MD) und vom Auftraggeber
nachgefahren: **13 Binaries, 430 passed, 0 failed, 0 ignored.**

Nach dieser Änderung, derselbe Befehl, tatsächlich gesehen:

    13 binaries, 431 passed, 0 failed, 0 ignored

Aufschlüsselung: `tos_api` (lib) 10 · `aes_appl_init` 1 · `aes_button_dispatch`
1 · `aes_desk_background` **8** (vorher 7) · `aes_dialog_undraw` 2 ·
`aes_object_font` 6 · `aes_reference` 177 · `aes_window_content` 1 ·
`aes_window_drag` 4 · `gemdos_drive_login` 2 · `gemdos_fs_paths` 2 ·
`vdi_raster_reference` 59 · `vdi_reference` 158.

**Die eine zusätzliche Bestehende ist der neue Test. Kein vorhandener Test ist
rot geworden** — genau wie A23 Abschnitt 7.1 es für Stufe 2 vorhergesagt hatte
(„wird rot: nichts"). Die Wächter W1 bis W7 aus A23 7.2 liegen sämtlich in
diesem Lauf und sind grün; insbesondere die 177 Referenzfälle in
`aes_reference` (W1, W2, W3) und die Zeichentests W4, W5, W6, W7.

Build, beide Boards, je mit erzwungener Neuübersetzung (`touch` auf die
geänderte Quelle, weil ein zweiter Board-Build sonst fälschlich „up to date"
melden kann):

    cargo xtask build --board qemu-virt
      -> Compiling tos-api, Compiling kernel, Finished dev profile, 0 errors
    cargo xtask build --board rpi4
      -> Compiling tos-api, Compiling kernel, Finished dev profile, 0 errors
      -> objcopy -> kernel8.img (gitignoriertes Bauartefakt, keine Hardwareaktion)

Warnungen unverändert: `tos-api` (lib) 64, `kernel` (lib) 94 — auf beiden
Boards, identisch zum Stand nach FIX-01.

Clippy, `cargo clippy -p tos-api --all-targets`, einmal mit und einmal ohne die
Änderung gemessen (beide Dateien vorübergehend aus einer beiseitegelegten Kopie
ersetzt und danach aus der Kopie zurückgeholt — kein `git`-Eingriff):

| Ziel | ohne Fix | mit Fix |
|---|---|---|
| `tos-api` (lib) | 81 | 81 |
| `tos-api` (lib, 2. Ziel) | 81 | 81 |
| `tos-api` (lib test) | 82 | 82 |
| `tos-api` (test `vdi_raster_reference`) | 2 | 2 |
| `tos-api` (test `vdi_reference`) | 3 | 3 |

**Delta: 0.** Das Testziel `aes_desk_background` erscheint in beiden Läufen
nicht in den Summenzeilen, erzeugt also weiterhin keine Warnung. Die einzige
Differenz zwischen den beiden Ausgaben ist die Klammer „(N duplicates)" der
Cargo-Zusammenfassung (15/66/81 gegen 19/64/79) — eine Deduplizierungsangabe
des zweiten Laufs, keine geänderte Warnungszahl.

## Textvorschlag für docs/STATUS.md

Die Datei ist fremd modifiziert und wurde **nicht** angefasst. Vorschlag zum
Eintragen beim Commit, unter *Recent Decisions*:

> **2026-09-10 — Die Rechteckliste bricht ein Fenster in EmuTOS' Reihenfolge**
> (Stufe 2 der Fix-Reihenfolge aus `TASK/A23.md` 8.2; schließt D-A13-2).
> `subtract` (`tos-api/src/aes/window.rs`) erzeugte die Reststücke eines
> gebrochenen Rechtecks als TOP, BOTTOM, LEFT, RIGHT; EmuTOS erzeugt sie als
> TOP, LEFT, RIGHT, BOTTOM — `brkrct` indiziert `have_piece[]` mit den
> Konstanten `TOP 0`, `LEFT 1`, `RIGHT 2`, `BOTTOM 3` und hängt sie in einer
> aufsteigenden Schleife an (`aes/gemwrect.c:30-33`, `:113-125`). Die
> Kantenformeln stimmten bereits (`mkpiece`, `:64-98`), geändert wurde nur die
> Reihenfolge. Sie ist beobachtbar, weil sie die Aufrufreihenfolge aller drei
> Leser der Besitzerliste bestimmt: den Zeichenlauf (`walk_rects`, und damit
> die Aufrufreihenfolge eines `G_USERDEF`-Objekts), die öffentliche
> `WF_FIRSTXYWH`/`WF_NEXTXYWH`-Iteration und die Redraw-Vereinigung
> (letztere im Ergebnis reihenfolgeunabhängig). Fehlt das TOP-Stück, lieferte
> FIRST bisher BOTTOM statt LEFT. Neuer Regressionstest
> `aes_desk_background::the_owner_rectangle_list_is_broken_in_top_left_right_bottom_order`,
> der die Stücke über ihre Lage zum überdeckenden Fenster klassifiziert und
> daher keine Koordinate einfriert. Suite `cargo test -p tos-api --tests`:
> 431 passed, 0 failed (vorher 430).

## Braucht dieser Schritt eine reale Gegenmessung?

**Ja, aber nicht jetzt und nicht allein.** A23 Abschnitt 8.3, Zeile „Stufe 2",
ist hier maßgeblich und wird nicht abgeschwächt:

- Die **Reihenfolge selbst** ist mit dem Hosttest und der Quelle belegt. Der
  Beleg ist vollständig: Konstanten, Einfügeschleife, Kantenformeln,
  In-Place-Ersatz.
- Die **Messabnahme** steht aus. Die vier gemessenen TT-Fälle **166, 174, 183
  und 213** (`tosdump/cases/aes-r2b-rlist.tsv` gegen
  `tosdump/reference/EMUTOS14-AES-r2brlist.BIN`) werden von diesem Fix
  **allein nicht grün**: A14 hat über sechs Modellvarianten gezeigt, dass
  weder die Reihenfolge noch der Schatten für sich genügt. Sie werden erst
  zusammen mit **Stufe 4** (D-A13-3, Schatten) grün — und auch das nur unter
  der in A23 7.3/8.3 offenen Bedingung, dass das Pi-Profil `s = 2` behält. Die
  Anwenderentscheidung vom 2026-09-10 setzt `s = 4`; dann verlieren diese vier
  Fälle ihre Schiedsrichterrolle und brauchen eine neue Falltabelle. Das ist
  Gegenstand von Stufe 4, nicht von hier.
- Ein grüner Hostlauf ist deshalb für diesen Schritt **kein** Nachweis des
  Endergebnisses. Er belegt, was er belegt: die Reihenfolge stimmt jetzt mit
  der Quelle überein.

Eine eigene Hardwareaktion war für diesen Schritt weder nötig noch
durchgeführt; es wurde nichts geflasht und nichts auf ein Volume geschrieben.

## Was offen bleibt

- **D-A13-3 (Schatten, Stufe 4)** und **D-A14-1 (Cursorbesitz, Stufe 3)** sind
  unberührt. `A13` bleibt deshalb OFFEN; der Nachtrag dort ist ergänzt, die
  Statuszeile nicht geändert.
- **Die Messabnahme 166/174/183/213** steht aus, siehe oben.
- **Deckungslücke A23 7.5/1** ist weiter nur teilweise geschlossen:
  `WF_FIRSTXYWH`/`WF_NEXTXYWH` haben jetzt zwei dauerhafte Tests (Handleprüfung
  aus FIX-01, Reihenfolge aus diesem Ticket). Verschachtelte Iteration über
  zwei Fenster (7.5/2), `save_windows`/`restore_windows` (7.5/3), RCAP-Überlauf
  (7.5/6, A21) und die Reihenfolge mehrerer brechender Fenster (7.5/7, A22)
  bleiben ungetestet.
- Kein Commit gesetzt; `docs/STATUS.md` bewusst nicht angefasst.

## Zusätzlicher Befund — nur gemeldet, nicht angefasst, kein Task angelegt

Beim Nachprüfen der Kantenformeln gegen `mkpiece` fiel auf, dass unser
`subtract` das gebrochene Rechteck **nicht** in die Freiliste zurückgibt,
sondern in ein neues Array schreibt — das ist eine bewusste, unbeobachtbare
Abweichung von EmuTOS' `ORECT`-Freiliste (`or_start`/`get_orect`,
`aes/gemwrect.c:36-61`) und die Ursache dafür, dass unsere Kapazität eine
feste `RCAP = 16` ist statt eines gemeinsamen `NUM_ORECT`-Vorrats über alle
Fenster. Das ist A21 und wurde hier nicht angefasst.
