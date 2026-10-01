# FIX-04 — der Schlagschatten als ganze Beziehung

Status: **DONE** (2026-09-10) · Stufe 4 der Fix-Reihenfolge aus
[A23](A23.md), Abschnitt 8.2 · deckt **D-A13-3** ab
([A13-STATUS-UNFINISHED.MD](A13-STATUS-UNFINISHED.MD), A23 Abschnitt 5)

Kein Commit. Die Änderung liegt uncommittet im gemeinsamen Arbeitsbaum.

## Auftrag

Der Schlagschatten war bei uns **auf beiden Seiten** abwesend: die
Rechteckliste rechnete mit dem nackten `curr`, und gezeichnet wurde er
überhaupt nicht. Ein Fix an nur einer der beiden Seiten erzeugt den Fehler, den
er zu beheben scheint:

- **nur Geometrie** — wir melden `s` Pixel als verdeckt, die niemand übermalt.
  Eine Anwendung überspringt dann einen sichtbaren Streifen. Der Zahlenvergleich
  gegen den TT wäre trotzdem grün.
- **nur Zeichnung** — der Desktop und die Fenster darunter zeichnen in den
  Schatten zurück, weil die Fläche in keiner Besitzerliste steht.

Deshalb in **einem** Schritt: Geometrie, Zeichnung, Zeichenclip, die
Schadensrechtecke und das Überdeckungsflag.

Selbst nachgeprüfte Fundstellen vor dem Fix (Zeilen des Arbeitsbaums nach
FIX-01/02/03):

- `tos-api/src/aes/window.rs:640` — `border_pieces` startet mit `w.curr`
  (Zeilen `:637-638`) und zieht in `:648` das nackte `ow.curr` ab.
- `:601-605` — `is_broken` vergleicht `ow.curr` gegen `curr`.
- `:1494-1499` — `draw_frame` beginnt mit `rect_outline(b, …)`; danach folgt
  sofort die Titelzeile. Kein Schatten.
- `:735` — `draw_frame_pieces` reicht `pc` unverändert an `walk_rects` durch.
- `:1930`, `:1994`, `:2284` — Schadensrechteck bei Open, Close/Delete und
  Move/Size ohne jeden Zuschlag.
- `DROP_SHADOW_SIZE` kam in `tos-api/` und `kernel/src/` überhaupt nicht vor
  (`grep`, selbst nachgefahren).

Ausdrücklich **nicht** Gegenstand und **nicht** angefasst: `w_redraw` /
`visible_bound` / `send_redraw_in` in ihrer Beziehung zur rohen Besitzerliste
(Stufe 6), P1/P2-x/y am Listenende (Stufe 5), Kapazitätsgrenze `RCAP` (A21),
Reihenfolge mehrerer brechender Fenster (A22), Objekt- und Dialogschatten in
`tos-api/src/aes/object.rs` (Wächter W4). `visible_bound` **sieht** die neue
Größe — es liest über `work_rects` → `border_pieces` —, seine Logik ist
unverändert.

## Was geändert wurde

Eine Quelldatei und zwei Testdateien.

`tos-api/src/aes/window.rs`: **250 Zeilen hinzu, 20 entfernt** (gemessen gegen
eine vor der Arbeit beiseitegelegte Kopie desselben Standes, nicht gegen
`git HEAD` — im Arbeitsbaum liegen FIX-01…03 und fremde Änderungen
uncommittet). Davon sind 191 hinzugefügte Zeilen Kommentar mit Quellenbeleg;
der reine Code umfasst 59 hinzugefügte und 20 entfernte Zeilen.

### 1. Die Konstante und ihre zwei Helfer (`:238-331`)

**Vorher:** nicht vorhanden.

**Nachher** — ein eigener Abschnitt direkt hinter `struct Grect`, weil beide
Helfer Grect-Operationen sind und die Konstante damit **vor** allen sechs
Lesern steht:

```rust
pub const DROP_SHADOW_SIZE: i16 = 2;          // :269

const SHADOW_TH: i16 = DROP_SHADOW_SIZE / 2;  // :278
const _: () = assert!(
    2 * SHADOW_TH == DROP_SHADOW_SIZE,
    "DROP_SHADOW_SIZE must stay even: the drawn bands are 2*th wide, so an odd \
     size would report more area as covered than the frame paints"
);

fn ws_true(curr: Grect) -> Grect {            // :306
    if curr.w != 0 && curr.h != 0 {
        Grect { w: curr.w + DROP_SHADOW_SIZE, h: curr.h + DROP_SHADOW_SIZE, ..curr }
    } else {
        curr
    }
}

fn plus_shadow(r: Grect) -> Grect {           // :330
    Grect { w: r.w + DROP_SHADOW_SIZE, h: r.h + DROP_SHADOW_SIZE, ..r }
}
```

Zwei Helfer, weil EmuTOS zwei verschiedene Regeln hat und sie nicht dasselbe
tun: `w_getsize(WS_TRUE, …)` addiert **nur bei nichtleeren Maßen**
(`gemwmlib.c:257`), der Zuschlag am Ende von `draw_change` addiert
**bedingungslos** (`:1213-1215`, auf das Nullrechteck, das ein Close
hereinreicht). Die beiden zusammenzulegen wäre eine stille Abweichung.

`SHADOW_TH` ist **kein zweiter Zahlenwert**, sondern die Ableitung
`s = 2 · th`, die in der Referenz zwischen Geometrie (`DROP_SHADOW_SIZE = 2`)
und Zeichnung (`th = 1` aus `W_BOX`s `ob_spec`) gilt. Der `const`-Assert setzt
sie durch, statt sie zu kommentieren: eine ungerade Größe bricht den Build,
nicht erst den Bildschirm.

**Der Doc-Kommentar an `DROP_SHADOW_SIZE` (`:240-268`) führt alle acht Leser
mit Quellenbeleg in einer Tabelle.** Das ist die einzige Quelle der Wahrheit;
im Baum steht kein zweiter Zahlenwert (nachgefahren: `grep -n
"DROP_SHADOW_SIZE\|SHADOW_TH"` findet außerhalb dieses Blocks nur Verwender).

### 2. Geometrie — `border_pieces` (`:749-758`)

**Vorher:**

```rust
        Some(w) if handle == DESKWH => (w.curr, i16::MIN),
        Some(w) if w.open => (w.curr, w.z),
        ...
            n = subtract(out, n, ow.curr);
```

**Nachher:**

```rust
        Some(w) if handle == DESKWH => (ws_true(w.curr), i16::MIN),
        Some(w) if w.open => (ws_true(w.curr), w.z),
        ...
            n = subtract(out, n, ws_true(ow.curr));
```

**Beide** Rollen, wie `newrect`: das brechende Rechteck (`gemwrect.c:190`) und
das eigene Startrechteck (`:203`). Nur eine von beiden zu vergrößern wäre
schlechter als keine — nimmt man dem Brecher den Schatten, gehört der Streifen
unter dem Schatten des Nachbarn zwei Fenstern gleichzeitig; nimmt man ihn dem
eigenen Startrechteck, fällt der Streifen, den der eigene Rahmen bemalt, aus
genau der Liste, die diesen Anstrich clippt.

Der Desktop bekommt den Zuschlag ebenfalls (EmuTOS' `newrect` läuft über
`everyobj` für jedes Fenster, `gemwmlib.c:1088`); er ist folgenlos, weil
`walk_rects` mit `gl_rfull` und `work_rects` mit `WS_WORK` wieder schneidet.

### 3. Überdeckungsflag — `is_broken` (`:702-707`)

`ow.curr` gegen `curr` → `ws_true(ow.curr)` gegen `ws_true(w.curr)`. `mkrect`
setzt `VF_BROKEN`, wenn `brkrct` ein Stück der Liste bricht
(`gemwrect.c:136-162`); die Liste besteht aus `WS_TRUE`-Stücken und das
brechende Rechteck ist `WS_TRUE`. **Ein Fenster, das ein anderes nur mit seinem
Schatten überdeckt, bricht es damit schon** — und genau dieses Flag entscheidet
in `draw_change`, ob das getoppte Fenster überhaupt ein `WM_REDRAW` braucht
(`gemwmlib.c:1066,1121-1135`).

### 4. Zeichnung — `draw_shadow` (`:1551-1594`), gerufen aus `draw_frame` (`:1628`)

Neue Funktion, unmittelbar hinter `rect_outline` im Rahmenanstrich:

```rust
fn draw_shadow(b: Grect, backend: &dyn RasterBackend, clip: ClipRect) {
    if SHADOW_TH == 0 || b.w <= 0 || b.h <= 0 {
        return;
    }
    backend.fill_rect(b.x + SHADOW_TH, b.y + b.h, b.w - SHADOW_TH, DROP_SHADOW_SIZE, BLACK, clip);
    backend.fill_rect(b.x + b.w, b.y + SHADOW_TH, DROP_SHADOW_SIZE, b.h + SHADOW_TH, BLACK, clip);
}
```

Das ist `ob_draw`s `SHADOWED`-Block (`gemoblib.c:889-901`), eingesetzt auf das
Rechteck, das `gr_inside(&t, th)` hinterlässt (`t = (x+th, y+th, w-2th,
h-2th)`), mit `2·th = DROP_SHADOW_SIZE` — die Herleitung steht Zeile für Zeile
im Doc-Kommentar. Die Farbe ist `BLACK`, weil `gr_crack` die Bits 15..12 des
niederwertigen Worts von `W_BOX`s `ob_spec 0x00011101` nimmt
(`gemgraf.c:612-627`) und das `1` ist.

`draw_frame` ruft sie direkt hinter `rect_outline`, weil `ob_draw` den
`W_BOX`-Rahmen und seine `SHADOWED`-Dekoration im selben Besuch malt, bevor es
in Titelzeile und Gadgets absteigt (`gemoblib.c:640-901`).

Bei uns ist die Zeichenseite **nicht geschenkt**: `draw_frame` ist eine eigene
Rasterroutine, kein `ob_draw` über einen `W_ACTIVE`-Baum. Die vorhandene Formel
in `tos-api/src/aes/object.rs:2602-2614` ist die Vorlage — sie wurde **gelesen
und übernommen, nicht neu erfunden und nicht angefasst** (Wächter W4: der
Objektschatten zieht sein `th` aus dem `ob_spec` des jeweiligen Objekts, der
Fensterschatten aus der Konstante; das sind zwei Metriken, nicht eine).

### 5. Zeichenclip — `draw_frame_pieces` (`:845`)

`walk_rects(handle, pc, …)` → `walk_rects(handle, pc.map(plus_shadow), …)`.

`w_cpwalk` hat zwei Zweige und sie unterscheiden sich genau hierin
(`gemwmlib.c:365-375`): bei `usetrue == TRUE` ist der Clip `WS_TRUE(wh)`, bei
`usetrue == FALSE` der geerbte globale Clip **plus** `DROP_SHADOW_SIZE`. Unser
`Some(damage)` ist der FALSE-Zweig aus `w_update` (`:1029-1031`), unser `None`
der TRUE-Zweig.

Der TRUE-Zweig braucht **nichts**: jedes Stück aus `border_pieces` beginnt bei
`ws_true(curr)` und wird nur noch verkleinert, ein erneuter Schnitt mit
`WS_TRUE` kann also beweisbar nichts ändern. Das steht als Begründung im
Kommentar, statt einen Schnitt einzubauen, der immer wirkungslos ist.

`walk_rects` selbst (`do_walk`) bleibt unverändert und bekommt einen Absatz
(`:775-779`), damit es später niemand „nachbessert": `w_drawdesk` reicht das
Rechteck seines Aufrufers unverändert durch (`gemwmlib.c:340-359`), nur
`w_cpwalk` vergrößert. Der Schatten erreicht den Desktopanstrich trotzdem —
über die `WS_TRUE`-Stücke.

### 6. Schadensrechteck bei Open (`:2139-2140`)

**Vorher:** `redraw_clip(r.x, r.y, r.w, r.h);`

**Nachher:**

```rust
        let d = plus_shadow(r);
        redraw_clip(d.x, d.y, d.w, d.h);
```

`draw_change` nimmt für ein Open `c = pt` (der „grow that is a superset or an
open"-Zweig, `gemwmlib.c:1163-1167`, erreicht weil das erzeugte, noch nicht
geöffnete Fenster ein Nullrechteck als `WS_CURR` hat) und der Tail addiert den
Schatten (`:1213-1215`).

### 7. Schadensrechteck bei Close/Delete (`:2209`)

Eine Zeile vor `redraw_clip`/`broadcast_redraw`:

```rust
    let damage = plus_shadow(damage);
```

Für einen Close ist das der eigentliche Punkt: der Schatten des
verschwindenden Fensters liegt **außerhalb** seines Rahmenrechtecks; ohne
diesen Zuschlag bliebe sein schwarzes Band auf dem Schirm stehen. Der Weg über
`wind_delete` ist derselbe (`close_window`), also mit abgedeckt. EmuTOS unioniert
hier das **`WS_CURR`**-Rechteck des neuen Topfensters (`:1198-1201`), nicht
dessen `WS_TRUE`; die eine Addition danach deckt beides.

### 8. Schadensrechteck bei Move/Size (`:2516-2522`)

**Vorher:** `let d = union_grect(prev, r);`

**Nachher:**

```rust
            let same_corner = (r.w != 0 || r.h != 0) && r.x == prev.x && r.y == prev.y;
            let resized = same_corner && (r.w != prev.w || r.h != prev.h);
            let d = if resized {
                plus_shadow(Grect { x: prev.x, y: prev.y, w: r.w.max(prev.w), h: r.h.max(prev.h) })
            } else {
                union_grect(prev, r)
            };
            let d = plus_shadow(d);
```

`draw_change` verrechnet den Schatten **nicht einmal für alles**, und A23
Abschnitt 5 warnt ausdrücklich davor, das zu verkürzen. Eine reine
Größenänderung — gleiche linke obere Ecke, andere Ausdehnung — bekommt im
Größenzweig einen eigenen Zuschlag (`gemwmlib.c:1152-1154`)

```c
c.g_w = max(pt->g_w, c.g_w) + DROP_SHADOW_SIZE;
c.g_h = max(pt->g_h, c.g_h) + DROP_SHADOW_SIZE;
```

**bevor** der gemeinsame Tail einen zweiten addiert (`:1213-1215`). Jeder andere
Weg (Move, Grow, Open, Close) erreicht den Tail mit der schlichten Vereinigung
und wird einmal belastet. Mit nur einer Addition bliebe beim Verkleinern ein
schattenbreiter Streifen der alten Geometrie unbemalt.

Der Move-Zweig behält bewusst `union_grect(prev, r)`: EmuTOS blittet dort das
Topfenster (`w_move`, `:938-1000`) und braucht deshalb nur das Quellrechteck;
wir zeichnen stattdessen neu und brauchen beide. Das war schon vorher eine
ausgewiesene Abweichung und wurde nicht angefasst.

### Was bewusst unverändert blieb

| Stelle | Warum |
|---|---|
| `WF_CXYWH`, `WF_WXYWH`, `WF_PXYWH`, `WF_FXYWH`, `wind_calc`, Gadget-/Treffgeometrie | EmuTOS trennt `WS_CURR` und `WS_TRUE` in `w_getxptr`/`w_getsize` ausdrücklich (`gemwmlib.c:235-261`); nur `WS_TRUE` bekommt den Zuschlag. Wächter W2/W3. |
| `top_window`s `send_redraw_in(handle, w.curr)` | EmuTOS reicht dort `TRUE(curr) ∩ gl_rfull` weiter (`:1198-1215`), aber `w_redraw` schneidet als Erstes mit `WS_WORK` (`:895-897`), und `WS_WORK ⊂ WS_CURR ⊂ WS_TRUE`. Das Ergebnis ist in **jedem** Fall identisch, die Änderung wäre also nicht beobachtbar — und `top_window` steht nicht in A23s Stufe-4-Liste. Gemeldet, nicht angefasst. |
| `walk_rects` (`do_walk`) | siehe Punkt 5. |
| `visible_bound`, `send_redraw_in` | Stufe 6. Sie **sehen** die neue Größe über `work_rects`, ihre Logik ist unverändert. |
| `redraw_vslider`/`redraw_hslider` | Das ist `w_cpwalk(wh, W_VSLIDE, …, TRUE)` — es zeichnet den Schieber-Teilbaum, nicht `W_BOX`, also keinen Schatten. Die Stücke sind jetzt `WS_TRUE`-basiert, was folgenlos ist: `draw_vslider` malt nur innerhalb der Leiste. |
| `redraw_dialog_area`, `redraw`, `redraw_all`, `redraw_open` | Eigener Aufrufervertrag; den Schatten liefert der `w_cpwalk`-Clip aus Punkt 5 (A23 Abschnitt 5, Zeile „Wiederherstellung und Dialoge"). |
| `tos-api/src/aes/object.rs` | Objekt-/Dialogmetrik, Wächter W4. Null Zeilen geändert. |

## EmuTOS-Beleg

Gepinnter Stand `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(selbst geprüft, `git rev-parse HEAD`). Jede Fundstelle unten selbst gelesen,
nicht aus A23 übernommen.

- `aes/gemwmlib.c:49` — `#define DROP_SHADOW_SIZE    2   /* size of drop shadow on windows */`
- `aes/gemwmlib.c:253-261` — `w_getsize`:

      rc_copy(w_getxptr(which, w_handle), pt);
      if ((which == WS_TRUE) && pt->g_w && pt->g_h)
      {
          pt->g_w += DROP_SHADOW_SIZE;
          pt->g_h += DROP_SHADOW_SIZE;
      }

  `w_getxptr:230-249` liefert für `WS_CURR` **und** `WS_TRUE` denselben
  Speicher — der Unterschied ist ausschließlich dieser Zuschlag.
- `aes/gemwrect.c:190` und `:203` — `newrect` benutzt `WS_TRUE` **zweimal**:
  einmal als brechendes Rechteck (`gl_mkrect.o_gr`, gleich darauf
  `everyobj(tree, ROOT, wh, mkrect, …)`) und einmal als eigenes Startrechteck
  (`new->o_gr`). Dazwischen `:189-192` der Frühausstieg bei Nullgröße, den
  `ws_true` durch Unverändertlassen nachbildet.
- `aes/gemwrect.c:136-162` — `mkrect` setzt `pwin->w_flags |= VF_BROKEN`, wenn
  `brkrct` ein Stück bricht; `brkrct:100-135` prüft dafür `new` gegen `r`, also
  `WS_TRUE` gegen `WS_TRUE`.
- `aes/gemwrect.c:64-98` — `mkpiece`: RIGHT bekommt `x = new.x + new.w`, BOTTOM
  `y = new.y + new.h`; TOP `h = new.y - old.y`, LEFT `w = new.x - old.x`. Das ist
  der Grund, warum der Schatten nur zwei der vier Kanten verschiebt.
- `aes/gemwmlib.c:1284` — `W_ACTIVE[ROOT].ob_state = SHADOWED;` in `wm_init`.
  Der Schatten ist ein **gezeichnetes Attribut**, keine Rechengröße.
- `aes/gemwmlib.c:62,84` — `W_BOX` ist `G_IBOX` mit `ob_spec 0x00011101`;
  `ob_sst` (`aes/gemobjop.c:54-57`) nimmt für `G_BOX/G_BOXCHAR/G_IBOX` das
  Byte 1 heraus → `th = 1`. Damit gilt `DROP_SHADOW_SIZE == 2*th`.
- `aes/gemoblib.c:889-901` — der `SHADOWED`-Block:

      if (th > 0) gr_inside(&t, th); else th = -th;
      if ((state & SHADOWED) && th)
      {
          vsf_color(bcol);
          bb_fill(MD_REPLACE, FIS_SOLID, 0, t.g_x, t.g_y+t.g_h+th, t.g_w + th, 2*th);
          bb_fill(MD_REPLACE, FIS_SOLID, 0, t.g_x+t.g_w+th, t.g_y, 2*th, t.g_h+(3*th));
      }

- `aes/gemgraf.c:612-627` — `gr_crack`: `*pbc = (color >> 12) & 0x000f`, auf
  `0x1101` also `1` = BLACK.
- `aes/gemwmlib.c:365-375` — `w_cpwalk`s zwei Clipzweige (`WS_TRUE` gegen
  `gsx_gclip` + `DROP_SHADOW_SIZE`).
- `aes/gemwmlib.c:286-312` — `do_walk` addiert **nichts**; es schneidet nur
  `gl_rfull` mit `pc` und die Stücke damit.
- `aes/gemwmlib.c:1152-1154` (Größenzweig) und `:1213-1215` (gemeinsamer Tail,
  `/* account for drop shadow (BUGFIX in 2.1) */`) — die zwei getrennten
  Zuschläge am Schadensrechteck.
- `aes/gemwmlib.c:1163-1167`, `:1198-1201` — Open- beziehungsweise
  Close-Zweig, aus denen `c` in den Tail geht.

## Wo die Konstante liegt und wer sie liest

`tos-api/src/aes/window.rs:269` — `pub const DROP_SHADOW_SIZE: i16 = 2;`

`pub`, damit die Tests sie als Parameter lesen können statt eine Zahl
einzubauen. Alle Leser, selbst nachgefahren:

| Leser | Zeile | über |
|---|---|---|
| `is_broken` | `:702`, `:706` | `ws_true` |
| `border_pieces` (eigenes Startrechteck) | `:749`, `:750` | `ws_true` |
| `border_pieces` (brechendes Rechteck) | `:758` | `ws_true` |
| `walk_rects` (Zeichnung: Rahmen, Schieber, Desktophintergrund) | — | über `border_pieces` |
| `work_rects` → `compute_rlist` (`WF_FIRSTXYWH`/`WF_NEXTXYWH`) | — | über `border_pieces` |
| `visible_bound` (`WM_REDRAW`-Hülle) | — | über `work_rects` |
| `draw_frame_pieces` (Clip des FALSE-Zweigs) | `:845` | `plus_shadow` |
| `draw_shadow` (die Pixel) | `:1592`, `:1593` | `DROP_SHADOW_SIZE`, `SHADOW_TH` |
| `wind_open` (Schaden) | `:2139` | `plus_shadow` |
| `close_window` (Schaden, auch für `wind_delete`) | `:2209` | `plus_shadow` |
| `wind_set(WF_CXYWH)` (Schaden) | `:2518`, `:2522` | `plus_shadow` |
| Test `aes_window_content.rs` | `:578` | `tos_api::aes::window::DROP_SHADOW_SIZE` |
| Test `aes_desk_background.rs` | `:1428` | dieselbe Konstante |

Zweiter Zahlenwert im Baum: **keiner.** `SHADOW_TH` (`:278`) ist die abgeleitete
Rahmenstärke `s/2` mit `const`-Assert auf `2·th == s`.

**Warum 2 und nicht 4.** Die Anwenderentscheidung lautet 4 für das Pi-Profil,
mit dem Nachtrag: Verifikationslauf **zuerst** mit 2, damit die am TT
gemessenen Fälle 166/174/183/213 eine echte Identität liefern statt einer
Modellvorhersage. Die Umstellung auf 4 ist danach die Änderung genau dieser
einen Zeile. Geprüft: die Sonde in `aes_desk_background.rs:840` sitzt 5 px unter
der Unterkante, bei 4 bleibt 1 px Luft, ab 6 kippt sie (A23 7.4).

## Der neue Test (Geometrieseite)

`tos-api/tests/aes_desk_background.rs:1418-1565`, Abschnitt „9. the breaking
rectangle includes the drop shadow", genau **ein** `#[test]`:
`the_breaking_rectangle_of_a_covering_window_includes_its_drop_shadow`.

Untergebracht wie FIX-01/02/03 in dieser Datei, weil ihr Aufbau schon steht
(`SERIAL`-Lock, `rig()`/`fresh()`, `open_window`, `wind_get_rect`,
`walk_owner_list` aus FIX-03, `piece_kind`/`P_TOP…P_BOTTOM` aus FIX-02). Die
zehn vorhandenen Tests der Datei wurden nicht berührt.

Aufbau: ein Fenster auf `SHD_UNDER = (40, 60, 440, 360)`, darüber ein zweites
**streng innerhalb dessen Arbeitsfläche** — Position und Größe werden zur
Laufzeit aus der vom AES zurückgemeldeten Arbeitsfläche gerechnet
(`wu.0 + 60 + S`, …), nicht aus Fontmetriken geraten. Damit existieren alle
**vier** Reststücke, und keine der vier geprüften Kanten ist die, die der
Work-Schnitt abschneidet.

Geprüft wird die **Beziehung**, in `mkpiece`s eigenen Begriffen:

| Stück | `mkpiece`-Kante | Soll |
|---|---|---|
| TOP | `h = new.y - old.y` | endet bei `over.y` — **unverändert** |
| LEFT | `w = new.x - old.x` | endet bei `over.x` — **unverändert** |
| RIGHT | `x = new.x + new.w` | beginnt bei `over.x + over.w + S` |
| BOTTOM | `y = new.y + new.h` | beginnt bei `over.y + over.h + S` |

Alle vier zu prüfen ist der Punkt: ein Fix, der einfach jedes Rechteck
vergrößert, verschöbe auch TOP und LEFT und fiele hier durch. `w_getsize`
addiert nur auf `g_w`/`g_h` (`gemwmlib.c:257`), also genau zwei Kanten.

**Keine eingefrorene Zahl.** Die Stücke werden über `piece_kind` nach der Seite
des verdeckenden Fensters benannt (FIX-02s Technik), die Geometrie wird vom AES
zurückgelesen statt angenommen, `S` ist ein Parameter aus der einen Konstante,
und es werden nur Stücke **vor** dem Listenende gelesen — die Wörter am
Listenende sind Stufe 5 und bleiben außen vor.

### Beleg, dass der Test den Defekt fängt

Die Geometriehälfte wurde probeweise von Hand zurückgebaut (`subtract(out, n,
ow.curr)` statt `ws_true(ow.curr)`), der Test gefahren, danach von Hand
zurückgeholt (kein `git`-Eingriff; SHA-256 der Datei danach identisch zur
Kopie). Rot mit:

    the RIGHT piece (395, 145, 62, 212) starts at 395, so the rectangle that
    broke the list was 292 wide instead of 294 — the covering window's bare
    `WS_CURR` rect (103, 145, 292, 212) rather than its `WS_TRUE` one. …
      left: 395
     right: 397

## Die Neuableitung von T2

`tos-api/tests/aes_window_content.rs`, im Test
`clicking_a_window_never_erases_its_content`. Vorher **eine** Zeile
(`:823-824`), nachher ein Block mit zwei Zusicherungen (`:872-928`); dazu vier
Hilfsdefinitionen (`:562-601`: `overlap`, `S`, `TH`, `shadow_bands`).

**Vorher** behauptete der Test: nach dem Topping von A dürfe in
`(400, 222, 157, 207)` — dem Teil von Bs Arbeitsfläche rechts von A — kein
Pixel verändert sein. Gefahren, tatsächlich gesehen:

    assertion `left == right` failed: untopping B erased 180 pixels of its
    content (first Some((400, 222, 4478310, 0)))
      left: 180
     right: 0

`4478310 = 0x445566` ist `MARK_B`, `0` ist Schwarz — also exakt die 180 px, die
A23 Abschnitt 7.1 für `s = 2` vorhergesagt hatte.

**Warum er zu Recht rot wird.** Er hielt fest, dass wir keinen Schatten malen.
Nach `wm_init` trägt die Wurzel des Fensterelementbaums `SHADOWED`
(`gemwmlib.c:1284`); jedes `w_cpwalk`→`do_walk`→`ob_draw` eines Rahmens malt
damit die zwei Bänder aus `gemoblib.c:889-901`, und die Besitzerliste lässt sie
durch, weil `newrect` von `w_getsize(WS_TRUE, …)` ausgeht
(`gemwrect.c:190,203`). Beim Topping von A wird As Rahmen mit **ungebrochener**
Liste gezeichnet, sein rechtes Band `(x+w, y+th, 2th, h+th) = (400, 61, 2, 251)`
landet also mitten in dem geprüften Teil von B. Ein `WM_REDRAW` bekommt B dafür
auch bei EmuTOS nicht: der Top-Zweig lässt `w_update` genau einen Durchgang für
das getoppte Fenster laufen (`:1109-1136,1004-1048`).

**Das neue Soll — beide Hälften, nicht ein verschobener Bereich.** Der Auftrag
warnt davor, T2 durch Wegrücken des geprüften Bereichs grün zu machen; das
allein würde die Zeichenhälfte wieder ungeprüft lassen. Der Bereich wird
deshalb an der Bandkante **geteilt** und beide Teile bekommen eine Aussage:

1. **Der Schatten ist wirklich gemalt** — jedes Pixel von
   `rechtes Band ∩ Prüfbereich` liest `Pel 1` (BLACK, aus `gr_crack` auf
   `W_BOX`s `ob_spec`). Das ist der Detektor für den Halbfix: eine Fläche, die
   die Besitzerliste für A beansprucht und die deshalb sonst niemand bemalt.
2. **Und er hört dort auf** — rechts des Bandes ist kein Pixel von B verändert
   (`diff_in == 0`), wie bisher.

Dazu eine Rig-Prüfung, dass das **untere** Band den Bereich nicht erreicht (es
endet bei Spalte `x+w-1 = 399`) und dass das rechte ihn überhaupt kreuzt — sonst
bewiese Teil 1 nichts.

Alle drei Rechtecke kommen aus `shadow_bands(RA)`, das die Formel aus
`gemoblib.c:889-901` mit `S` und `TH = S/2` führt. **Keine Zahl eingefroren:**
bei einer anderen Schattengröße verschiebt sich die Trennlinie mit.

### Beleg, dass die Neuableitung den Halbfix fängt

Die Zeichenhälfte wurde probeweise von Hand ausgebaut (Aufruf `draw_shadow(b,
backend, clip);` auskommentiert, Geometrie unverändert), der Test gefahren,
danach von Hand zurückgeholt (SHA-256 danach identisch zur Kopie). Rot mit:

    180 of 180 pixels of A's drop shadow in (400, 222, 2, 90) were left
    unpainted (first Some((400, 222, 0))). The owner rectangle list claims that
    strip for A (`w_getsize(WS_TRUE)`, aes/gemwmlib.c:254-261), so nobody else
    paints it — if A's frame does not paint it either, a strip of stale pixels
    stays at the window edge. …

Also genau der Fehler, den der A23-Auftrag verhindern soll — und der Test
erkennt ihn.

## Nachgemessen: die gezeichneten Bänder gegen die Quelle

Ein temporärer Messaufsatz in `aes_desk_background.rs` (ein `#[test]`, das nur
`read_pixel` ausgibt) hat ein Fenster `curr = (100, 100, 200, 150)` geöffnet und
die Umgebung der Kanten ausgelesen; danach von Hand entfernt (SHA-256 der Datei
danach identisch zur Kopie vor dem Aufsatz). `1` = BLACK, `0`/`3` = die beiden
Farben des Desktopmusters. Lauf am 2026-09-10:

    FIX04-PROBE curr=(100, 100, 200, 150) S=2
    FIX04-PROBE y=249 x[99..102]+x[298..303] = [3, 1, 1, 1, 1, 1, 1, 1, 0, 3]
    FIX04-PROBE y=250 x[99..102]+x[298..303] = [0, 3, 1, 1, 1, 1, 1, 1, 3, 0]
    FIX04-PROBE y=251 x[99..102]+x[298..303] = [3, 0, 1, 1, 1, 1, 1, 1, 0, 3]
    FIX04-PROBE y=252 x[99..102]+x[298..303] = [0, 3, 0, 3, 3, 0, 3, 0, 3, 0]
    FIX04-PROBE top y=100 x[299..303] = [1, 3, 0, 3, 0]
    FIX04-PROBE top y=101 x[299..303] = [1, 1, 1, 0, 3]

Soll aus `gemoblib.c:889-901` mit `th = 1`, `S = 2`: unteres Band
`(101, 250, 199, 2)`, rechtes Band `(300, 101, 2, 151)`. Abgelesen:

- `y = 250/251`, `x = 101…301` schwarz → unteres Band **und** die
  Bandüberschneidung des rechten in der unteren rechten Ecke. ✓
- `y = 250/251`, `x = 100` zeigt Desktopmuster → die **linke untere Kerbe**
  (`th` breit), von niemandem bemalt. ✓ EmuTOS-Verhalten (A23 7.3).
- `y = 100`, `x = 300/301` zeigt Desktopmuster, ab `y = 101` schwarz → die
  **rechte obere Kerbe**. ✓
- `y = 252` und `x = 302` zeigen wieder Desktopmuster → der Schatten endet
  exakt an der `WS_TRUE`-Kante `(301, 251)`. ✓

Beide Bänder und beide Kerben stimmen pixelgenau mit der Quelle überein.

## Tatsächlich gesehene Zahlen

Alle Läufe am 2026-09-10 im gemeinsamen Arbeitsbaum (`70caee2` plus die fremden
uncommitteten Änderungen sowie FIX-01, FIX-02 und FIX-03), Host macOS/arm64.

`cargo test -p tos-api --tests`, Ausgangsstand nach Stufe 3 — selbst
nachgefahren, bevor eine Zeile geändert wurde:

    13 binaries, 432 passed, 0 failed, 0 ignored

Zwischenstand nach der reinen Codeänderung, vor jeder Teständerung:

    aes_window_content: FAILED, 0 passed; 1 failed
    alle übrigen 12 Binaries grün

Das ist **T2 und sonst nichts** — genau die eine Rotfärbung, die A23 7.1 für
Stufe 4 vorhergesagt hatte. **T1 ist grün geblieben**, wie gefordert.

Nach Neuableitung von T2 und dem neuen Test, derselbe Befehl, tatsächlich
gesehen:

    13 binaries, 433 passed, 0 failed, 0 ignored

Aufschlüsselung: `tos_api` (lib) 10 · `aes_appl_init` 1 · `aes_button_dispatch`
1 · `aes_desk_background` **10** (vorher 9) · `aes_dialog_undraw` 2 ·
`aes_object_font` 6 · `aes_reference` 177 · `aes_window_content` 1 ·
`aes_window_drag` 4 · `gemdos_drive_login` 2 · `gemdos_fs_paths` 2 ·
`vdi_raster_reference` 59 · `vdi_reference` 158.

Die eine zusätzliche Bestehende ist der neue Geometrietest. Die Wächter W1 bis
W7 aus A23 7.2 liegen sämtlich in diesem Lauf und sind grün: die 177
Referenzfälle in `aes_reference` (W1, W2, W3), `aes_object_font` +
`aes_dialog_undraw` (W4), die drei Ziehtests in `aes_window_drag` (W5) und der
Umzugstest `:1146-1198` (W7), die drei Hintergrundtests in
`aes_desk_background` (W6) einschließlich der latenten Sonde `:840`.

Build, beide Boards, je mit erzwungener Neuübersetzung (`touch` auf die
geänderte Quelle, weil ein zweiter Board-Build sonst fälschlich „up to date"
melden kann):

    cargo xtask build --board qemu-virt
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
    cargo xtask build --board rpi4
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
      -> objcopy -> kernel8.img (gitignoriertes Bauartefakt, keine Hardwareaktion)

Warnungszahlen identisch zum Stand nach FIX-03.

Clippy, `cargo clippy -p tos-api --all-targets`, einmal **vor** der ersten
Codezeile und einmal danach gemessen (kein Dateitausch nötig, weil die Basis
vor der Arbeit aufgenommen wurde):

| Ziel | vor dem Fix | nach dem Fix |
|---|---|---|
| `tos-api` (lib) | 81 | 81 |
| `tos-api` (lib, 2. Ziel) | 81 | 81 |
| `tos-api` (lib test) | 82 | 82 |
| `tos-api` (test `vdi_raster_reference`) | 2 | 2 |
| `tos-api` (test `vdi_reference`) | 3 | 3 |

**Delta: 0.** Die Testziele `aes_desk_background` und `aes_window_content`
erscheinen in beiden Läufen nicht in den Summenzeilen, erzeugen also weiterhin
keine Warnung. Die verbliebenen Warnungen an `window.rs` sind sämtlich
vorbestehend (`private_interfaces` an jeder öffentlichen `wind_*`-Funktion wegen
`pub(crate) struct Aespb`, plus ein `unused_assignments`); der Fix fügt keine
hinzu und entfernt keine.

## Textvorschlag für docs/STATUS.md

Die Datei ist fremd modifiziert und wurde **nicht** angefasst. Vorschlag zum
Eintragen beim Commit, unter *Recent Decisions*:

> **2026-09-10 — Fensterschlagschatten: Geometrie und Zeichnung in einem
> Schritt** (Stufe 4 der Fix-Reihenfolge aus `TASK/A23.md` 8.2; schließt
> D-A13-3). NewTOS kannte den Schlagschatten auf **beiden** Seiten nicht: die
> Besitzer-Rechteckliste rechnete mit dem nackten `curr`, und gezeichnet wurde
> er gar nicht. EmuTOS führt beides über dieselbe Größe — `w_getsize(WS_TRUE,…)`
> addiert `DROP_SHADOW_SIZE` auf `g_w`/`g_h` (`aes/gemwmlib.c:49,253-261`),
> `newrect` benutzt `WS_TRUE` sowohl als brechendes Rechteck wie als eigenes
> Startrechteck (`aes/gemwrect.c:190,203`), `wm_init` setzt
> `W_ACTIVE[ROOT].ob_state = SHADOWED` (`:1284`) und `ob_draw` malt die zwei
> Bänder (`aes/gemoblib.c:889-901`). Ein Fix an nur einer Seite erzeugt den
> Fehler, den er zu beheben scheint: gemeldet-aber-unbemalt lässt einen
> Streifen alter Pixel an jeder Fensterkante, gemalt-aber-ungemeldet lässt den
> Desktop wieder hineinzeichnen. Umgesetzt wurden deshalb zusammen:
> `border_pieces` (beide Rollen), `is_broken`, der `w_cpwalk`-Clip in
> `draw_frame_pieces`, die Schadensrechtecke von Open/Close/Delete/Move/Size —
> einschließlich der zwei getrennten Zuschläge, die `draw_change` im
> Größenzweig und im gemeinsamen Tail vergibt — und die neue `draw_shadow` in
> `draw_frame` (`tos-api/src/aes/window.rs`). Die Größe ist **eine** benannte
> Konstante `DROP_SHADOW_SIZE` mit `const`-Assert auf `s = 2·th`; sie steht
> bewusst zunächst auf **2**, damit der Pi-Verifikationslauf gegen die
> TT-Fälle 166/174/183/213 bitgleich sein muss, und wird erst danach auf die
> vom Anwender gewählten 4 gesetzt. Neuer Regressionstest
> `aes_desk_background::the_breaking_rectangle_of_a_covering_window_includes_its_drop_shadow`
> (prüft alle vier `mkpiece`-Kanten, damit auch die zwei **un**verschobenen);
> `aes_window_content::clicking_a_window_never_erases_its_content` wurde aus der
> Quelle neu abgeleitet und prüft jetzt beide Hälften — Schatten gemalt **und**
> rechts davon nichts verändert. Suite `cargo test -p tos-api --tests`: 433
> passed, 0 failed (vorher 432).

## Braucht dieser Schritt eine reale Gegenmessung?

**Ja, und zwar genau jetzt — und sie hat zwei getrennte Teile.**

### 1. Die Geometriehälfte: die vier Fälle 166, 174, 183, 213

Das ist die Abnahme, auf die A23 8.3 hinarbeitet, und dieser Schritt ist der
Grund, dass sie überhaupt möglich wird. A14 hat über sechs Modellvarianten
gezeigt, dass **weder** die Zerlegungsreihenfolge (Stufe 2) **noch** der
Schatten (Stufe 4) allein reicht; erst zusammen ergeben sie die TT-Zahlen.

- Orakel: `tosdump/cases/aes-r2b-rlist.tsv` gegen
  `tosdump/reference/EMUTOS14-AES-r2brlist.BIN`.
- Erwartung: die vier Fälle müssen **bitgleich** zur TT-Referenz sein — nicht
  „im Rahmen des Modells", sondern identisch. Genau dafür steht die Konstante
  in diesem Schritt auf **2** statt auf den vom Anwender gewählten 4.
- Was der Lauf beweist: Stufe 2 **und** Stufe 4 zusammen. Für Stufe 2 ist es
  die bisher fehlende Messabnahme (FIX-02 war quellen- und hostbelegt, nicht
  messbelegt).
- Was er **nicht** beweist: die Zeichenhälfte. Kein Record der Tabelle enthält
  Pixel.

**Erst nach diesem Lauf** die Konstante auf 4 setzen (eine Zeile,
`window.rs:269`). Ein optionaler zweiter Lauf zeigt dann nur noch die erwarteten
Deltas; jede andere Abweichung wäre ein Fehler. Ab 4 verlieren die vier Fälle
ihre Schiedsrichterrolle und bräuchten eine neue Falltabelle (A23 7.3) — das ist
kein Mangel, sondern die angekündigte Folge der Entscheidung.

### 2. Die Zeichenhälfte: Pixel auf dem Schirm

Kein eingefrorener Record enthält Pixel, und der Hosttest fährt einen
Speicher-Backend, keinen Framebuffer. Nötig ist ein Screendump beziehungsweise
ein realer Pi-Lauf mit **zwei überlappenden Fenstern**, geprüft auf:

1. sichtbarer schwarzer Schatten unten und rechts an jedem Fenster;
2. **kein** unbemalter Streifen an der Fensterkante — der Halbfix-Detektor;
3. die zwei quellenrichtigen Kerben (unten links, oben rechts, je `th` breit);
4. nach `wind_close` beziehungsweise nach einem Verschieben: **kein**
   stehengebliebenes schwarzes Band an der alten Position (das ist der Zuschlag
   aus den Punkten 6–8 oben, den nur ein echter Lauf prüft);
5. der Schatten des oberen Fensters liegt **über** dem Inhalt des unteren und
   wird vom Desktopanstrich nicht wieder überschrieben.

Punkt 1 bis 3 sind auf dem Host schon pixelgenau gegen die Quelle abgeglichen
(Abschnitt „Nachgemessen"), aber auf einem Testbackend, nicht auf Hardware.
Punkt 4 und 5 sind auf dem Host **ungeprüft** — es gibt dafür keinen Test
(Deckungslücke A23 7.5/4).

Eine eigene Hardwareaktion war für diesen Schritt weder nötig noch
durchgeführt; es wurde nichts geflasht und nichts auf ein Volume geschrieben.

## Was offen bleibt

- **Die reale Gegenmessung** aus dem vorigen Abschnitt, beide Teile. Bis dahin
  ist D-A13-3 quellen- und hostbelegt, **nicht** messbelegt.
- **Die Konstante steht auf 2, die Anwenderentscheidung lautet 4.** Das ist
  Absicht und kein vergessener Schritt; die Umstellung ist ausdrücklich für
  *nach* dem Verifikationslauf vorgesehen.
- **Deckungslücke A23 7.5/4 — ungemalte alte Schattenstreifen** bei
  Close/Delete/Move/Size haben weiterhin keinen Test. Der Code dafür ist
  eingebaut (Punkte 6–8) und quellenbelegt, aber nur durch Codelesen abgedeckt.
  Der Auftrag hat auf zwei Teständerungen begrenzt; das gehört als eigener
  kleiner Testauftrag nachgezogen.
- **Deckungslücke A23 7.5/5 — ein Fenster, das ein anderes nur mit seinem
  Schatten überdeckt.** `is_broken` behandelt den Fall jetzt quellenrichtig,
  ein Test dafür fehlt weiter.
- **Stufe 5 (F-A15-1)** und **Stufe 6 (`w_redraw`)** sind unberührt. Stufe 5
  kann ihre Zahlen jetzt aus der endgültigen Geometrie ableiten — das war der
  Grund, warum sie nach dieser Stufe kommt (A23 8.1).
- **A21** (`RCAP`) und **A22** (mehrere brechende Fenster) unverändert.
- Kein Commit gesetzt; `docs/STATUS.md` bewusst nicht angefasst.

## Zusätzlicher Befund — nur gemeldet, nicht angefasst, kein Task angelegt

`top_window` (`tos-api/src/aes/window.rs:680`) reicht `w.curr` als Schaden an
`send_redraw_in`; EmuTOS reicht an derselben Stelle `TRUE(curr) ∩ gl_rfull`
weiter (`gemwmlib.c:1198-1215`). Beobachtbar ist der Unterschied **nicht**:
`w_redraw` schneidet als Erstes mit `WS_WORK` (`:895-897`), und
`WS_WORK ⊂ WS_CURR ⊂ WS_TRUE`, das Ergebnis ist in jedem Fall dasselbe
Rechteck. Die Stelle steht auch nicht in A23s Stufe-4-Liste. Deshalb
unverändert gelassen und hier nur festgehalten, damit sie bei einer späteren
`w_redraw`-Überarbeitung (Stufe 6) nicht als neuer Befund durchgeht.

---

## Nachtrag 2026-09-10 — Schattenfarbe hellgrau (Anwenderentscheidung)

Nach Sicht des Screendumps: der schwarze Schatten wirkt zu hart. Auf Wunsch des
Anwenders auf **hellgrau** geändert.

- Neue Konstante `DROP_SHADOW_COLOR = GRAY_LT` (`tos-api/src/aes/window.rs:206`),
  gelesen von `draw_shadow`. `GRAY_LT` ist EmuTOS' LWHITE und lag bereits als
  3D-Fläche im Baum.
- Der Test nimmt die Farbe jetzt aus dieser Konstante statt selbst BLACK zu
  behaupten; die lokale `const BLACK: Pel = 1` in
  `tos-api/tests/aes_window_content.rs` ist entfallen. **Die Umstellung von
  Schwarz auf Grau brauchte deshalb keine Teständerung** — 433 passed, 0 failed,
  unverändert. Das war der Zweck der Parametrisierung.
- Gemessen am Screendump, sechs Stellen, alle gleich: 1 px schwarzer Rahmen,
  dahinter 2 px `(189,189,189)`. Beleg:
  `measurements/measurement-2026-09-10-qemu/shadow-drawing/desktop-grey-shadow-640x480.png`.

**Abweichung von EmuTOS, bewusst.** EmuTOS nimmt `W_BOX`s Rahmenfarbe, also
BLACK (`gr_crack` auf `ob_spec` `0x1101`, `aes/gemgraf.c:612-627`). Das ist eine
Optik-, keine Konformitätsentscheidung: der Dump zeichnet Rechtecke, keine
Pixel, der Vergleich gegen den TT bleibt unberührt. Begründung im Doc-Kommentar
an der Konstante.

Nebenbefund, vom Anwender bemerkt und nachgemessen: zwischen Fenster und
Schatten liegt jetzt eine sichtbare schwarze 1-px-Linie. Das ist der
Fensterrahmen selbst (`rect_outline`), der vorher im schwarzen Schatten
unsichtbar war. Strukturell richtig, so belassen.

**Die Größe bleibt bei 2**, bis die Gegenmessung durch ist — siehe
`TASK/A23.md`, Nachtrag „wie der Schatten gemessen wird".
