# FIX-05 — die geschriebenen Wörter am Ende der Rechteckliste

Status: **DONE** (2026-09-10) · Stufe 5 der Fix-Reihenfolge aus
[A23](A23.md), Abschnitt 8.2 · deckt **F-A15-1** ab
([A15-STATUS-UNFINISHED.MD](A15-STATUS-UNFINISHED.MD), A23 Abschnitt 6)

Kein Commit. Die Änderung liegt uncommittet im gemeinsamen Arbeitsbaum, auf
FIX-01 bis FIX-04 aufgesetzt.

## Auftrag

`w_owns` hat **zwei** verschiedene Enden, und sie schreiben nicht gleich viele
Wörter (`aes/gemwmlib.c:1230-1241`, selbst nachgelesen am gepinnten Stand
`1a7484aba1bfba23e2c0899e243b6d8177fe41df`):

```c
static void w_owns(WINDOW *pwin, ORECT *po, GRECT *pt, GRECT *poutwds)
{
    while (po)
    {
        rc_copy(&po->o_gr, poutwds);
        pwin->w_rnext = po = po->o_link;
        if (rc_intersect(pt, poutwds))
            return;
    }

    poutwds->g_w = poutwds->g_h = 0;
}
```

- **P1 — die Schleife wird gar nicht betreten** (`po` ist beim Eintritt NULL).
  `:1240` schreibt **nur** `g_w`/`g_h`; `g_x`/`g_y` bleiben unbeschrieben und
  tragen den Inhalt des dispatch-lokalen, **nicht initialisierten**
  `WORD int_out[O_SIZE]` aus `xif` (`aes/gemsuper.c:432`, unbedingt
  zurückkopiert `:444-445`). A15 hat über alle 15 Records belegt, dass die am TT
  gemessenen 31/63 bzw. 0/19 aus der Aufrufgeschichte des **Aufrufers** stammen
  und nicht aus dem Fensterzustand. **Kein Soll. Hier war nichts zu reparieren.**
- **P2 — die Schleife läuft, aber kein Stück schneidet die Arbeitsfläche.**
  `w_owns` kopiert bei **jedem** durchlaufenen Stück dessen Rohgeometrie, stellt
  den Cursor weiter (`:1235`, **vor** dem Test) und ruft `rc_intersect(WORK,
  pout)` (`:1236`). `rc_intersect` schreibt alle vier Komponenten **auch bei
  FALSE** (`util/rectfunc.c:61-78`: die vier Zuweisungen stehen vor dem
  `return`-Ausdruck), mit `x = max(x_Stück, x_work)`, `y = max(y_Stück,
  y_work)`. Am Ende setzt `:1240` nur `w`/`h` auf 0 — `x`/`y` tragen also den
  Wert des **letzten tatsächlich geprüften** Stücks. **Deterministisches Soll.**

Diesen zweiten Pfad hatten wir gar nicht: `compute_rlist` speicherte die schon
vorgefilterte Liste aus `work_rects`, das nicht schneidende Stücke bereits beim
Bauen wegwarf (`intersect_grect` → `None`), und `wind_get` legte am Listenende
vier Nullen ab.

Selbst nachgeprüfte Fundstellen vor dem Fix (Zeilen des Arbeitsbaums nach
FIX-01…04):

- `tos-api/src/aes/window.rs:1330-1339` — `compute_rlist` speicherte
  `work_rects(handle, …)`, also nur die positiven Schnitte.
- `:1303-1324` (`work_rects`, `:1311` der Filter) — verwirft jedes Stück, dessen `intersect_grect`
  `None` liefert; die dort lokal schon berechneten x0/y0 gehen verloren.
- `:1345-1356` (`rlist_next`) — lieferte bei erschöpfter Liste
  `Grect::default()`, also `(0,0,0,0)`, ohne zwischen „nichts geprüft“ und „nur
  Fehlversuche“ zu unterscheiden.
- `:2302-2306` (`wind_get`, FIRST/NEXT-Arm) — schrieb daraufhin unbedingt alle
  vier Wörter.

Ausdrücklich **nicht** Gegenstand und **nicht** angefasst: `w_redraw` /
`visible_bound` / `send_redraw_in` (Stufe 6), die Kapazitätsgrenze `RCAP`
(A21), Reihenfolge mehrerer brechender Fenster (A22), Schatten und
Stückreihenfolge (Stufen 2 und 4, liegen), `work_rects` selbst.

## Was geändert wurde

Eine Quelldatei und eine Testdatei.

`tos-api/src/aes/window.rs`: **+143 / −23 Zeilen** (gemessen gegen den
rekonstruierten Ausgangsstand derselben Datei, nicht gegen `git HEAD` — im
Arbeitsbaum liegen FIX-01…04 und fremde Änderungen uncommittet; die
Rekonstruktion trifft die vor der Arbeit gemessene Zeilenzahl 3652 exakt). Davon
sind 92 hinzugefügte Zeilen Kommentar mit Quellenbeleg und 8 entfernte; der
reine Code samt Leerzeilen umfasst 51 hinzugefügte und 15 entfernte Zeilen.

### 1. `rc_intersect` — die Referenzprimitive mit beiden Hälften ihres Ergebnisses (`:1242-1277`)

**Vorher:** nicht vorhanden. Es gab nur `intersect_grect(a, b) -> Option<Grect>`,
das das fehlgeschlagene Rechteck wegwirft.

**Nachher** — direkt hinter `intersect_grect`, damit beide Formen nebeneinander
stehen:

```rust
fn rc_intersect(p1: Grect, p2: &mut Grect) -> bool {
    let tw = (p2.x + p2.w).min(p1.x + p1.w);
    let th = (p2.y + p2.h).min(p1.y + p1.h);
    let tx = p2.x.max(p1.x);
    let ty = p2.y.max(p1.y);
    p2.x = tx;
    p2.y = ty;
    p2.w = tw - tx;
    p2.h = th - ty;
    tw > tx && th > ty
}
```

Zeile für Zeile `util/rectfunc.c:61-78`, einschließlich der Reihenfolge: die
vier Zuweisungen **vor** dem Rückgabeausdruck. Genau dieses „schreibt auch bei
FALSE“ ist der ganze Inhalt von P2.

`intersect_grect` wurde **nicht** umgebaut. Es an die neue Primitive zu
delegieren wäre verhaltensgleich gewesen, aber es hat drei fremde Leser
(`work_rects`, `visible_bound`/`send_redraw_in`, `is_broken`) und Stufe 6 fasst
zwei davon ohnehin an; die vier Zeilen doppelter min/max sind der kleinere
Eingriff. Der Doc-Kommentar an `rc_intersect` sagt ausdrücklich, wofür die
`Option`-Form weiter da ist.

### 2. `compute_rlist` speichert die **rohe** Besitzerliste (`:1363-1392`, Rumpf ab `:1384`)

**Vorher:**

```rust
    let n = work_rects(handle, &mut list);
```

**Nachher:**

```rust
    let n = border_pieces(handle, &mut list);
```

Was `newrect` in `w_rlist` einhängt, sind die `WS_TRUE`-Reststücke selbst
(`aes/gemwrect.c:165-205`); der Arbeitsflächenschnitt passiert **auf dem Weg
hinaus**, Stück für Stück, in `w_owns` (`w_getsize(WS_WORK, …)` und dann
`rc_intersect`, `gemwmlib.c:1556-1561,1230-1240`). Nur die überlebenden Stücke
zu speichern wirft zwei beobachtbare Dinge weg: die reinen Randstücke, über die
der Lauf hinwegsteigen muss, und die Koordinaten, die der letzte fehlgeschlagene
`rc_intersect` hinterlässt.

**`work_rects` blieb unverändert.** Es hat mit `visible_bound` (`:1492`) einen
zweiten Leser, der die `WM_REDRAW`-Rechtecke treibt; dessen Semantik ist Stufe 6
und wurde hier nicht beiläufig mitgeändert. `visible_bound` ruft weiterhin
dasselbe `work_rects` mit demselben Ergebnis — die Änderung liegt eine Ebene
darüber, im Verbraucher `compute_rlist`, nicht im geteilten Helfer.

### 3. `Owns` — die drei Zustände als Typ (`:1395-1435`, `enum` ab `:1408`)

**Vorher:** `rlist_next(handle) -> Grect`, ein einziger Rückgabewert für alle
drei Fälle.

**Nachher:**

```rust
enum Owns {
    Visible(Grect),               // Schleife hat mit `return` verlassen
    Exhausted { x: i16, y: i16 }, // gelaufen, nichts überlebt: P2
    Empty,                        // nie betreten: P1
}
```

Der Typ ist die Unterscheidung. Ein `Grect` mit `w = h = 0` kann P1 und P2 nicht
auseinanderhalten, und genau das war der Fehler: `wind_get` konnte gar nicht
wissen, wie viele Wörter es schreiben darf.

### 4. `rlist_owns` — der Lauf (`:1428-1455`)

```rust
fn rlist_owns(handle: i16) -> Owns {
    let Some(w) = win(handle) else { return Owns::Empty };
    let work = w.work;
    let mut last: Option<(i16, i16)> = None;
    while (w.rnext as usize) < (w.rcount as usize) {
        let mut t = w.rlist[w.rnext as usize];
        w.rnext += 1;
        if rc_intersect(work, &mut t) {
            return Owns::Visible(t);
        }
        last = Some((t.x, t.y));
    }
    match last {
        Some((x, y)) => Owns::Exhausted { x, y },
        None => Owns::Empty,
    }
}
```

Der Cursor wird **vor** dem Test weitergestellt und damit auch bei einem
Fehlversuch — bei EmuTOS steht `pwin->w_rnext = po = po->o_link;` über dem
`if`. Ein reines Randstück wird also übersprungen und nicht beim nächsten Aufruf
erneut angeboten. `last` merkt sich, was `rc_intersect` trotz FALSE geschrieben
hat.

### 5. `wind_get`, FIRST/NEXT-Arm (`:2402-2428`)

**Vorher:** vier unbedingte `set_intout`.

**Nachher:**

```rust
match rlist_owns(handle) {
    Owns::Visible(r)          => { 1←r.x; 2←r.y; 3←r.w; 4←r.h }
    Owns::Exhausted { x, y }  => { 1←x;   2←y;   3←0;   4←0   }
    Owns::Empty               => {               3←0;   4←0   }
}
pb.set_ret(1);
```

`poutwds` ist `&int_out[1]` (`aes/gemsuper.c:343` mit `WM_OX`,
`include/aesdefs.h:509`) — Wort *i* des Rechtecks ist also `int_out[1+i]`, wie
gehabt. Im `Empty`-Zweig bleiben `int_out[1]`/`[2]` **unberührt**; sie lesen
sich beim Aufrufer als die definierte Null des Dispatchers
(`tos-api/src/aes/mod.rs:263`, `:132-140`). Das ist **kein** Nachbau von
EmuTOS' Stackresten — uninitialisierten Rust-Speicher einzuführen wäre keine
Lösung — sondern die Feststellung, dass es dort nichts zu treffen gibt.

### 6. Zwei Doc-Korrekturen ohne Codewirkung

- `Window::rlist` (`:386-417`, der neue Absatz `:403-410`): hält jetzt ausdrücklich fest, dass die
  gespeicherten Stücke die **rohen** `WS_TRUE`-Reste sind und warum.
- Die Lesertabelle an `DROP_SHADOW_SIZE` (`:266`): der FIRST/NEXT-Eintrag nennt
  nicht mehr `work_rects`, sondern `compute_rlist` über `border_pieces` mit dem
  Schnitt pro Stück in `rlist_owns`.

### Was bewusst unverändert blieb

`work_rects`, `visible_bound`, `send_redraw_in`, `broadcast_redraw`,
`intersect_grect`, `border_pieces`, `subtract`, `put`, `RCAP`, `walk_rects`,
`draw_frame_pieces`, `is_broken`, `save_windows`/`restore_windows`,
`wind_delete`s Listenlöschung — alle nur gelesen.

## EmuTOS-Beleg

| Aussage | Quelle |
|---|---|
| zwei Enden, unterschiedlich viele Wörter | `aes/gemwmlib.c:1230-1241` |
| Cursor wird vor dem Schnitt weitergestellt | `:1235` gegen `:1236` |
| `rc_intersect` schreibt auch bei FALSE, mit max/min | `util/rectfunc.c:61-78` |
| `pt` ist `WS_WORK` des angefragten Fensters, `po` ist `w_rlist` bzw. `w_rnext` | `aes/gemwmlib.c:1556-1561` |
| `poutwds` ist `&int_out[1]` | `aes/gemsuper.c:343`, `include/aesdefs.h:509` |
| Rückkopie unbedingt über `OUT_LEN` Wörter | `aes/gemsuper.c:443-445` |
| das lokale `int_out` ist uninitialisiert | `aes/gemsuper.c:432` |
| `w_rlist` enthält die rohen `WS_TRUE`-Reste | `aes/gemwrect.c:165-205`, insbesondere `:203` |
| Stückreihenfolge TOP/LEFT/RIGHT/BOTTOM | `aes/gemwrect.c:30-33,113-125` |
| Ursprung des BOTTOM-Stücks: `old.x`, `new.y + new.h` | `mkpiece`, `aes/gemwrect.c:73,89-92` |
| das brechende Rechteck ist `WS_TRUE`, nicht `curr` | `aes/gemwrect.c:190` |
| `ret` bleibt TRUE, egal wie der Lauf endet | `aes/gemwmlib.c:1513-1524,1589-1592` |

## Der neue Test

`tos-api/tests/aes_desk_background.rs:1567-1757`, neuer Abschnitt „10. the words
written at the end of the rectangle list“ (+192 Zeilen, reiner Anhang; die zehn
vorhandenen Tests der Datei wurden nicht berührt). Unterbringung wie bei FIX-01,
FIX-02 und FIX-04 in dieser Datei, weil sie den Aufbau fertig hat (`SERIAL`,
`rig()`/`fresh()`, `open_window`, `wind_get_rect`, `wind_get_ret_rect`,
`WF_FIRSTXYWH`/`WF_NEXTXYWH`, die Konstante `S`).

`the_end_of_the_rectangle_list_writes_the_last_piece_it_examined` (`:1631`) — **ein**
Test, der beide Enden in **einem** Lauf zeigt.

Aufbau: ein Fenster `END_UNDER = (40,60,400,300)` und darüber ein zweites, das

- **denselben linken oberen Eckpunkt** hat, damit `have_piece[TOP]` und
  `have_piece[LEFT]` falsch sind (`aes/gemwrect.c:107-108`) — und damit BOTTOM
  in `brkrct`s Konstantenordnung das **letzte** Stück ist;
- dessen `WS_TRUE`-Kanten rechts und unten **auf der Mitte** zwischen der
  Arbeitsflächenkante des unteren Fensters und dessen eigener `WS_TRUE`-Kante
  liegen: weit genug, um die ganze Arbeitsfläche zu überdecken, kurz genug, um
  ein RIGHT- und ein BOTTOM-Stück übrig zu lassen.

Beide Geometrien werden über `wind_get(WF_CXYWH)`/`(WF_WXYWH)` **zurückgelesen**
und die Vorbedingungen dann in `brkrct`s eigenen Begriffen geprüft: die vier
`have_piece[]`-Vergleiche und die Enthaltensein-Bedingung
`WS_TRUE(oben) ⊇ WORK(unten)`.

Warum diese Enthaltensein-Bedingung genügt, um P2 zu beweisen, ohne die Stücke
aufzuzählen: `brkrct` schneidet das **Komplement** des überdeckenden Rechtecks
aus dem unteren heraus (`aes/gemwrect.c:100-135`), jedes Reststück ist also
disjunkt zum überdeckenden Rechteck. Enthält dieses die ganze Arbeitsfläche,
kann kein Reststück sie treffen.

Geprüft wird dann:

1. **P2** — `wind_get(unten, WF_FIRSTXYWH)` liefert `ret = 1` und alle vier
   Wörter: `x = max(old.x, work.x)`, `y = max(WS_TRUE(oben).y + .h, work.y)`,
   `w = h = 0`. Die beiden Koordinatenformeln sind die von `mkpiece` BOTTOM
   (`:73,89-92`) und von `rc_intersect` (`rectfunc.c:67-70`), auf die
   zurückgelesenen Werte angewandt.
2. **P1** — der unmittelbar folgende `WF_NEXTXYWH` (Cursor steht hinter dem
   letzten Stück): `ret = 1`, `w = h = 0`. **Über x/y wird ausdrücklich nichts
   behauptet**, mit Begründung im Test: auf EmuTOS ist das der uninitialisierte
   Dispatcherpuffer, und eine Zahl dort festzuschreiben würde einen Vertrag
   erfinden, den es nicht gibt.

**Keine eingefrorene Zahl.** Die Schattengröße geht als
`S = tos_api::aes::window::DROP_SHADOW_SIZE` ein, nie als Literal; beide
Fenstergeometrien kommen aus demselben Lauf; die Erwartung ist als die zwei
Quellformeln über diesen Werten geschrieben. Nachgefahren: mit `DROP_SHADOW_SIZE
= 4` statt `2` bleibt der Test grün (die Konstante wurde dafür kurz geändert und
von Hand zurückgesetzt, `diff` gegen eine vorher abgelegte Kopie: identisch).

### Beleg, dass der Test den Defekt fängt

Der Test wurde **vor** dem Fix geschrieben und gefahren. Er war rot, mit der
Diagnose im Klartext:

    thread '...' panicked at tos-api/tests/aes_desk_background.rs:1727:5:
    assertion `left == right` failed: the list of the window below is not empty
    — it holds the RIGHT piece at x = 429 and the BOTTOM piece at y = 360 — but
    none of them meets its work area (41, 83, 376, 276). `w_owns` walks them all
    and every `rc_intersect` writes its four components even on FALSE
    (util/rectfunc.c:63-78), so the words left standing are those of the last
    piece examined, x = max(40, 41) and y = max(360, 83), with w/h zeroed by
    aes/gemwmlib.c:1240. Got (0, 0, 0, 0). All four zeroes mean the pieces that
    fail the clip were dropped when the list was built instead of being walked.
      left: (0, 0, 0, 0)
     right: (41, 360, 0, 0)

Also exakt der in A23 Abschnitt 6 beschriebene Befund. Nach dem Fix grün.

## Wirkung auf die anderen Leser — geprüft, nicht angenommen

- **Positive Stücke:** unverändert. `work_rects` hat dieselben Stücke in
  derselben Reihenfolge mit demselben `intersect_grect` geschnitten, das
  `rlist_owns` jetzt mit `rc_intersect` schneidet. Die von FIRST/NEXT
  ausgegebene Folge der **sichtbaren** Rechtecke ist identisch — deshalb wurde
  laut A23 7.1 kein vorhandener Test rot, und es wurde keiner.
- **`visible_bound` / `WM_REDRAW`:** unberührt, `work_rects` unverändert (`:1492` ruft es weiter).
- **Zeichenpfad:** unberührt, `border_pieces` wird nur zusätzlich gelesen.
- **Kapazität:** unverändert. `border_pieces` kappt selbst bei `RCAP`
  (`put`, `:1283-1288`); vorher wurde aus derselben gekappten Liste eine
  Teilmenge gespeichert, jetzt die Liste selbst. Kein neues Kapazitätsverhalten,
  A21 nicht berührt.
- **Desktopfenster (Handle 0):** `border_pieces(0)` startet bei
  `ws_true(gl_rscreen)`, `rlist_owns` schneidet gegen `w.work` = `gl_rfull` —
  dasselbe Ergebnis wie vorher über `work_rects`.
- **Kernel-Desktop:** `kernel/src/desk/redraw.rs:84-90` bricht bei
  `t.w == 0 || t.h == 0` ab (wie `desk/desksupp.c:223-229`), also auch bei einem
  P2-Ende mit von Null verschiedenem x/y. Nur gelesen, nicht geändert.
- **`save_windows`/`restore_windows`, `wind_delete`, `wm_new`:** kopieren bzw.
  löschen den ganzen Slot; Liste, Zähler und Cursor bleiben zusammen.

## Tatsächlich gesehene Zahlen

Alle Läufe am 2026-09-10 im gemeinsamen Arbeitsbaum (`70caee2` plus die fremden
uncommitteten Änderungen sowie FIX-01…FIX-04), Host macOS/arm64.

`cargo test -p tos-api --tests`, Ausgangsstand nach Stufe 4 plus Farbänderung —
selbst nachgefahren, bevor eine Zeile geändert wurde:

    13 binaries, 433 passed, 0 failed, 0 ignored

Nach Test **und** Fix, derselbe Befehl, tatsächlich gesehen:

    13 binaries, 434 passed, 0 failed, 0 ignored

Aufschlüsselung: `tos_api` (lib) 10 · `aes_appl_init` 1 · `aes_button_dispatch`
1 · `aes_desk_background` **11** (vorher 10) · `aes_dialog_undraw` 2 ·
`aes_object_font` 6 · `aes_reference` 177 · `aes_window_content` 1 ·
`aes_window_drag` 4 · `gemdos_drive_login` 2 · `gemdos_fs_paths` 2 ·
`vdi_raster_reference` 59 · `vdi_reference` 158.

Die eine zusätzliche Bestehende ist der neue Test. **Kein vorhandener Test wurde
rot** — genau die Vorhersage aus A23 7.1 für Stufe 5. Die Wächter W1 bis W7 aus
A23 7.2 liegen sämtlich in diesem Lauf und sind grün: die 177 Referenzfälle in
`aes_reference` (W1, W2, W3), `aes_object_font` + `aes_dialog_undraw` (W4), die
drei Ziehtests in `aes_window_drag` (W5) und der Umzugstest `:1146-1198` (W7),
die drei Hintergrundtests in `aes_desk_background` (W6). Auch T1 und T2 in
`aes_window_content` bleiben grün — Stufe 6 ist nicht eingeschleust worden.

Build, beide Boards, je mit erzwungener Neuübersetzung (`touch` auf die
geänderte Quelle, weil ein zweiter Board-Build sonst fälschlich „up to date“
melden kann):

    cargo xtask build --board qemu-virt
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
    cargo xtask build --board rpi4
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
      -> objcopy -> kernel8.img (gitignoriertes Bauartefakt, keine Hardwareaktion)

Warnungszahlen identisch zum Stand nach FIX-04.

Clippy, `cargo clippy -p tos-api --all-targets`:

| Ziel | nach dem Fix |
|---|---|
| `tos-api` (lib) | 81 |
| `tos-api` (lib test) | 82 |
| `tos-api` (test `vdi_raster_reference`) | 2 |
| `tos-api` (test `vdi_reference`) | 3 |

**Delta: 0** gegenüber der in FIX-04 aufgenommenen Basis. Belegt statt behauptet:
`cargo clippy -p tos-api --lib --message-format=short` nennt für `window.rs`
**genau elf** Fundstellen, und alle elf sind vorbestehend und liegen außerhalb
der geänderten Bereiche — zehn `private_interfaces` an je einer öffentlichen
`wind_*`-Funktion (`:2187, 2227, 2282, 2346, 2380, 2564, 2787, 2802, 2879,
2896`) und ein `unused_assignments` an `top` in `draw_frame` (`:1825`); alle elf
stehen wörtlich schon in der Ausgangsmessung vor der ersten Codezeile.
`aes_desk_background` erscheint in keiner Summenzeile, erzeugt also weiterhin
keine Warnung. Kein clippy-eigener Lint an `window.rs`.

Rauchprobe QEMU (`cargo xtask run --board qemu-virt`, nach 45 s beendet):
sauberer Hochlauf bis `[shell] sh_ldapp: starting NEWDESK` und `[desk] desktop
up (0 drives)`, keine Panik, kein Ausnahmefehler in den 52 Laufzeilen. Das ist
eine Rauchprobe, **kein** Abnahmelauf — der gehört dem Tester.

## Textvorschlag für docs/STATUS.md

Die Datei ist fremd modifiziert und wurde **nicht** angefasst. Vorschlag zum
Eintragen beim Commit, unter *Recent Decisions*:

> **2026-09-10 — Rechteckliste: die zwei Enden von `w_owns` auseinandergehalten**
> (Stufe 5 der Fix-Reihenfolge aus `TASK/A23.md` 8.2; schließt F-A15-1).
> `wind_get(WF_FIRSTXYWH/WF_NEXTXYWH)` schrieb am Listenende immer vier Nullen.
> EmuTOS kennt dort zwei verschiedene Enden (`w_owns`,
> `aes/gemwmlib.c:1230-1241`): wird die Schleife gar nicht erst betreten,
> schreibt `:1240` **nur** `g_w`/`g_h` — x/y bleiben der uninitialisierte
> Dispatcherpuffer (`aes/gemsuper.c:432,444-445`) und haben kein Soll; läuft die
> Schleife dagegen, ohne dass ein Stück die Arbeitsfläche schneidet, sind alle
> vier Wörter geschrieben, weil `rc_intersect` sein Ergebnis auch bei FALSE
> ablegt (`util/rectfunc.c:61-78`) — x/y sind dann die des **letzten geprüften**
> Stücks. Diesen zweiten Fall gab es bei uns nicht: die Liste speicherte nur die
> schon work-geschnittenen, positiven Stücke. Jetzt hält `Window::rlist` die
> **rohe** Besitzerliste (wie EmuTOS' `w_rlist`, `aes/gemwrect.c:203`), der
> Arbeitsflächenschnitt passiert pro Stück beim Weitergehen, der Cursor rückt
> auch bei einem Fehlversuch vor, und `wind_get` schreibt je nach Ausgang vier,
> vier oder zwei Wörter (`tos-api/src/aes/window.rs`: neue Primitive
> `rc_intersect`, `compute_rlist`, `enum Owns`, `rlist_owns`). Der geteilte
> Helfer `work_rects` und damit der `WM_REDRAW`-Pfad blieben unangetastet
> (Stufe 6). Neuer Regressionstest
> `aes_desk_background::the_end_of_the_rectangle_list_writes_the_last_piece_it_examined`
> — zeigt beide Enden in einem Lauf und lässt x/y im ersten Fall ausdrücklich
> unbestimmt. Suite `cargo test -p tos-api --tests`: 434 passed, 0 failed
> (vorher 433).

## Braucht dieser Schritt eine reale Gegenmessung?

**Ja für P2 — und es gibt heute keinen Fall dafür.**

- **P1** braucht keine. Das Soll ist „w = h = 0, ret = TRUE, x/y unbestimmt“ und
  folgt allein aus `gemwmlib.c:1240` und `gemsuper.c:432`. Die drei gemessenen
  Fälle **127, 133 und 204** sind laut A15 sämtlich P1; ihre TT-Werte (31/63
  bzw. 0/19) sind über alle 15 Records auf die Aufrufgeschichte des Aufrufers
  zurückgeführt und damit ausdrücklich **kein** Soll. Sie ändern sich durch
  diesen Fix nicht und bleiben die „hinnehmbare Abweichung“, als die A15 sie
  eingeordnet hat.
- **P2** ist **quellenbelegt, nicht messbelegt.** Die eingefrorene Tabelle
  `tosdump/cases/aes-r2b-rlist.tsv` (CRC `B4CC4A5A`) enthält **keinen** Fall, in
  dem die Rechteckliste eines Fensters nichtleer ist und trotzdem kein Stück die
  Arbeitsfläche schneidet. A15 hält das unter „Offen“ ausdrücklich fest.

**Nötiger neuer Messfall** — nicht angelegt, hier nur beschrieben:

- **Wohin:** eine **neue** Falltabelle unter `tosdump/cases/`, nicht in
  `aes-r2b-rlist.tsv` (eingefroren, CRC-gebunden). Nach der Regel aus
  `TASK/README.md` bekommt sie einen eigenen Lauf in einem eigenen Verzeichnis;
  die zugehörige TT-Referenz wäre ein neues `EMUTOS14-AES-*.BIN` neben den
  vorhandenen. Sinnvoll gebündelt mit den zwei anderen offenen Messlücken
  derselben Familie: A22 (drei und mehr überlappende Fenster) und A24
  (verschachtelte FIRST/NEXT über zwei Fenster, die Abnahme von Stufe 3).
- **Was der Fall tun muss:** ein Fenster öffnen, ein zweites darüber, dessen
  `WS_TRUE`-Fläche die **ganze Arbeitsfläche** des ersten überdeckt, ohne
  dessen `WS_TRUE`-Rechteck vollständig zu überdecken; dann `wind_get(1,
  WF_FIRSTXYWH)` mit `OUT_LEN 5` und vergiftetem `int_out`. Erwartet werden
  `ret = 1`, `int_out[3] = int_out[4] = 0` und zwei **geschriebene**
  (nicht-POISON) Wörter `int_out[1]`/`[2]` mit den Koordinaten des letzten
  geprüften Stücks. Der Poison-Test ist hier der eigentliche Schiedsrichter: er
  trennt P2 von P1, ohne dass man die Zahlen vorher kennen muss.
- **Was er nicht kann:** die Zahlen selbst sind geräteabhängig (Fontmetrik →
  Arbeitsfläche, `DROP_SHADOW_SIZE`). Der TT-Wert ist also nicht 1:1 auf den Pi
  übertragbar; verglichen werden muss die **Beziehung** — Koordinaten des
  letzten Stücks nach `mkpiece` + `rc_intersect` —, so wie der Hosttest es tut.

Zusätzlich fehlt für die **ganze** Kette weiterhin die Abnahme aus A23 8.3:
Stufe 2 und Stufe 4 sind erst zusammen an den Fällen 166/174/183/213 messbar,
und das nur, solange das Pi-Profil `s = 2` behält. Das ist nicht dieser Task.

## Was offen bleibt

- P2 auf realer Hardware, siehe oben. Blockiert diesen Fix nicht.
- **Stufe 6** (`w_redraw` aus der rohen Besitzerliste) ist bewusst **nicht**
  eingeschleust; T1 in `aes_window_content.rs:828-832` ist deshalb grün
  geblieben und bleibt der Detektor dafür.
- A18/A26: die Spezifikation `docs/api/aes/window-graphics.md:341,848-851` sagt
  weiterhin nur „a rect with w=h=0“ und verschweigt die Zwei-Wort-Regel und den
  Unterschied P1/P2. **Nicht geändert** — Spec-Korrekturen liegen bei A18/A26.

## Zusätzliche Befunde — nur gemeldet, nicht angefasst, kein Task angelegt

1. **A23 Abschnitt 8.3, Zeile „Stufe 5“, widerspricht A15 und Abschnitt 6
   derselben Datei.** Dort steht, für das P2-x/y gebe es „ein echtes Orakel: die
   gemessenen Fälle **127, 133 und 204**“. A15 hat diese drei Fälle einzeln und
   über alle 15 Records als **P1** nachgewiesen (A15, „Welcher Mechanismus
   greift“: „In allen drei Fällen P1, nicht P2“), und A23 Abschnitt 6 übernimmt
   das ausdrücklich. Die drei Fälle werden durch diesen Fix nicht grün und
   sollen es auch nicht — ihre TT-Werte sind unbestimmt. Die Spalte „Vorhandenes
   Messorakel“ für Stufe 5 ist damit leer, nicht gefüllt. Auftragsseitig war das
   bereits richtiggestellt; hier festgehalten, damit die Tabelle nicht später
   als Beleg zitiert wird.
2. **`work_rects` hat nach diesem Fix nur noch einen Leser** (`visible_bound`).
   Solange Stufe 6 aussteht, ist das richtig so; danach kann die Funktion
   entweder ganz in `visible_bound` aufgehen oder wegfallen. Nicht angefasst.
