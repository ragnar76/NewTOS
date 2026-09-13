# FIX-03 — Rechteckliste und Iteratorcursor pro Fenster

Status: **DONE** (2026-09-10) · Stufe 3 der Fix-Reihenfolge aus
[A23](A23.md), Abschnitt 8.2 · deckt **D-A14-1** ab
([A14-STATUS-UNFINISHED.MD](A14-STATUS-UNFINISHED.MD), A23 Abschnitt 4)

Kein Commit. Die Änderung liegt uncommittet im gemeinsamen Arbeitsbaum.

## Auftrag

Die Rechteckliste eines Fensters und der Cursor, mit dem
`wind_get(WF_FIRSTXYWH)`/`wind_get(WF_NEXTXYWH)` sie abläuft, lagen bei uns in
**einem** prozessweiten Zustand. Ein `FIRST` auf Fenster B überschrieb damit die
Liste **und** den Cursor, in deren Mitte Fenster A gerade steckte. EmuTOS führt
beides als Feld der `WINDOW`-Struktur.

Selbst nachgeprüfte Fundstellen vor dem Fix:

- `tos-api/src/aes/window.rs:1054-1064` — `struct RList` / `static RLIST` mit
  je einem gemeinsamen `rects`/`count`/`cursor` für alle Fenster.
- `:1149-1160` — `compute_rlist(handle)` überschreibt sie.
- `:1162-1175` — `rlist_next()` liest sie und erhöht den Cursor; die Signatur
  kennt das Fenster gar nicht.

Ausdrücklich **nicht** Gegenstand und **nicht** angefasst: Schatten (Stufe 4),
P1/P2-Wörter am Listenende (Stufe 5), `w_redraw`/`visible_bound` (Stufe 6),
Kapazitätsgrenze `RCAP` (A21), Reihenfolge mehrerer brechender Fenster (A22).
Keine Kompensation an irgendeiner Leserseite eingebaut; keine Zeichenroutine
berührt.

## Was geändert wurde

Genau eine Quelldatei, `tos-api/src/aes/window.rs`: **70 Zeilen hinzu, 39
entfernt** (gemessen gegen eine per Hand zurückgebaute Kopie desselben Standes,
nicht gegen `git HEAD` — im Arbeitsbaum liegen FIX-01 und FIX-02 uncommittet).
Davon sind 44 hinzugefügte Zeilen Kommentar mit Quellenbeleg.

### 1. Die Ablage wandert in den Fenster-Slot — `Window` (`:276-299`)

**Vorher:** `struct Window` endete mit `bcolor` (`:275`); Liste und Cursor lagen
außerhalb in `RLIST`.

**Nachher** (`:297-299`, davor 21 Zeilen Doc-Kommentar mit dem EmuTOS-Beleg):

```rust
    rlist: [Grect; RCAP],
    rcount: u16,
    rnext: u16,
```

`Window::EMPTY` (`:321-323`) initialisiert alle drei — leere Liste, Zähler 0,
Cursor 0. Damit ist der Zustand an den Slot gebunden: **Liste, Zähler und
Cursor werden gemeinsam gesetzt und gemeinsam gelöscht**, nie einzeln. Das war
die Auflage aus A23 Abschnitt 4 („nicht einen neuen Cursor pro Fenster mit
altem globalem rects/count kombinieren").

### 2. `struct RList` / `static RLIST` ersatzlos entfernt (vormals `:1052-1064`)

13 Zeilen weg, einschließlich des handgeschriebenen `unsafe impl Sync`. Es gibt
im Baum keinen zweiten Leser dieses Zustands mehr — geprüft per `grep`, der
einzige direkte API-Aufrufer war und ist `wind_get`.

### 3. `compute_rlist` schreibt in *dieses* Fenster (`:1159-1179`)

**Vorher** (`:1149-1160`):

```rust
fn compute_rlist(handle: i16) {
    let (rects, count, cursor) = unsafe { (…RLIST.rects…, …RLIST.count…, …RLIST.cursor…) };
    *cursor = 0;
    let mut list = [Grect::default(); RCAP];
    let n = work_rects(handle, &mut list);
    rects[..n].copy_from_slice(&list[..n]);
    *count = n;
}
```

**Nachher** (`:1170-1179`):

```rust
fn compute_rlist(handle: i16) {
    let mut list = [Grect::default(); RCAP];
    let n = work_rects(handle, &mut list);
    let Some(w) = win(handle) else {
        return; // no window, no list to own it (callers check the handle first)
    };
    w.rlist[..n].copy_from_slice(&list[..n]);
    w.rcount = n as u16;
    w.rnext = 0;
}
```

Erst rechnen, dann ablegen — `work_rects` liest über `win(handle)` selbst aus
dem Slot, die beiden Zugriffe überlappen also nicht. `unsafe` entfällt hier
ganz.

### 4. `rlist_next` bekommt das Fenster (`:1181-1196`)

**Vorher** (`:1162-1175`): `fn rlist_next() -> Grect`, drei `unsafe`-Zugriffe
auf `RLIST`.

**Nachher** (`:1185-1197`):

```rust
fn rlist_next(handle: i16) -> Grect {
    let Some(w) = win(handle) else {
        return Grect::default();
    };
    let i = w.rnext as usize;
    if i < w.rcount as usize {
        w.rnext = (i + 1) as u16;
        w.rlist[i]
    } else {
        Grect::default()
    }
}
```

Der Cursor ist ein **Index** und wird beim Weitergehen erhöht, unabhängig davon,
was der Aufrufer mit dem Stück macht. Stufe 5 („Cursor muss auch bei
fehlgeschlagenen Stücken weitergehen, genau wie `w_owns`",
`gemwmlib.c:1232-1237`) braucht dafür nur den Work-Schnitt an dieser Stelle
einzusetzen — die Ablage selbst muss sie nicht mehr anfassen.

### 5. Aufrufstelle in `wind_get` (`:2067`)

`let r = rlist_next();` → `let r = rlist_next(handle);`. Sonst nichts; die
Handleprüfung aus FIX-01 (`win(handle).is_none()` an `:2060`) steht unverändert davor, `compute_rlist`
und `rlist_next` sehen also nur gültige Handles.

### 6. Lebenszyklus: `wind_delete` gibt die Rechtecke zurück (`:2021-2029`)

**Vorher** (`:1999-2002`): der Slot wurde nur auf `in_use = false` gesetzt.

**Nachher**, vor demselben Zweizeiler:

```rust
        w.rlist = [Grect::default(); RCAP];
        w.rcount = 0;
        w.rnext = 0;
```

Das ist EmuTOS' `newrect(gl_wtree, w_handle); /* give back recs. */`
(`aes/gemwmlib.c:1476`), auf unsere Ablage übertragen. Ohne es bliebe die Liste
des vorigen Fensterlebens im Slot stehen — genau der Fehler in neuer Form, vor
dem der Auftrag warnt.

### 7. Ein Doc-Kommentar nachgezogen (`:1228`)

`visible_bound` sagte „ohne den gemeinsamen Iteratorzustand (`RLIST`) zu
stören"; der Verweis zeigte nach dem Entfernen ins Leere. Jetzt: „ohne den
Iteratorzustand **irgendeines** Fensters zu stören". Die Funktion selbst ist
unverändert — sie baut weiter lokal über `work_rects` und ist damit auch nach
der Umstellung kein Verbraucher der gespeicherten Liste. Das ist bewusst so
gelassen: `visible_bound`/`w_redraw` ist Stufe 6.

### Die übrigen Lebenszykluspfade — geprüft, nicht angenommen

A23 Abschnitt 4 zählt sie namentlich auf. Alle sind durch die Bindung an den
Slot **automatisch** kohärent; keiner brauchte eine eigene Zeile:

| Pfad | NewTOS | Wirkung auf Liste + Cursor |
|---|---|---|
| `wind_create` | `:1874` `*w = Window::EMPTY` | Beide zurückgesetzt, bevor der Slot neu vergeben wird. Ein wiederverwendetes Handle kann kein Stück aus dem vorigen Fensterleben liefern. |
| `wind_delete` | `:2021-2029` (neu) | Liste geleert, Cursor 0, dann `in_use = false`. |
| `wm_new` | `:2538` `slots()[1..].fill(Window::EMPTY)` | Beide für alle Anwendungsfenster zurückgesetzt (`gemwmlib.c:1886`). |
| `wm_init` | `:2560-2564` `wm_new()` + `init_desk_window()` | Zusätzlich Slot 0 über `*d = Window::EMPTY` (`:388`). |
| `save_windows` | `:834-847` | `*SAVE.slots.get() = *live` kopiert Liste und Cursor mit; `live[1..].fill(Window::EMPTY)` löscht sie für das Kindprogramm. Slot 0 bleibt stehen, wie bisher — der Desktop wird nie abgerissen. |
| `restore_windows` | `:860-865` | Kopiert den ganzen Slot zurück, also auch Liste und Cursor. Der NewTOS-eigene Vertrag „Zustand nach `restore_windows` ist der vor `save_windows`" gilt jetzt auch für den Iterator. |
| Geometrieänderung (`wind_open`, `WF_CURRXYWH`, `close_window`) | unverändert | Die Liste wird bei uns beim **nächsten** `FIRST` neu gebaut. Der Cursor zeigt in die eigene, gültige Ablage — ein `NEXT` über eine Geometrieänderung hinweg liefert das nächste Stück der zuletzt gebauten Liste, nie undefinierten Speicher. EmuTOS lässt `w_rnext` an dieser Stelle auf die freigegebene Liste zeigen (`newrect` `gemwrect.c:174-186` setzt `w_rlist = NULL`, ohne `w_rnext` anzufassen); dieses C-Zeigerverhalten wird ausdrücklich **nicht** nachgebaut (A23 Abschnitt 4). |

## EmuTOS-Beleg

Gepinnter Stand `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(selbst geprüft, `git rev-parse HEAD`).

- `aes/gemlib.h:84-85` — die beiden Felder **in der Fensterstruktur**:

      ORECT *w_rlist;             /* owner rectangle list */
      ORECT *w_rnext;             /* used for search first, search next */

- `aes/gemwmlib.c:1513` — `wm_get` holt mit `pwin = get_pwin(w_handle)` das
  **angefragte** Fenster.
- `aes/gemwmlib.c:1556-1561` — der FIRST/NEXT-Arm nimmt die Quelle aus genau
  diesem Fenster:

      w_getsize(WS_WORK, w_handle, &t);
      po = (w_field == WF_FIRSTXYWH) ? pwin->w_rlist : pwin->w_rnext;
      w_owns(pwin, po, &t, (GRECT *)poutwds);

- `aes/gemwmlib.c:1230-1240` — `w_owns` setzt den nächsten Link **dieses**
  Fensters: `pwin->w_rnext = po = po->o_link;` — und zwar vor dem
  `rc_intersect`, also auch bei einem Stück, das den Schnitt nicht übersteht
  (das ist Stufe 5). Am Ende nur `poutwds->g_w = poutwds->g_h = 0`.
- `aes/gemwrect.c:165-205` — `newrect` baut `pwin->w_rlist` je Fenster auf;
  aufgerufen für **jedes** Fenster aus `draw_change` (`gemwmlib.c:1089`) und
  für ein einzelnes aus `wm_delete` (`:1476`).
- `aes/gemwmlib.c:1476` — `newrect(gl_wtree, w_handle); /* give back recs. */`
  im Löschpfad, der Beleg für Punkt 6 oben.

Ein `FIRST` auf B erreicht in dieser Struktur den Cursor von A nicht. Das hängt
weder an der Auflösung noch am Schatten noch an der Reihenfolge der Stücke —
es ist eine Zuordnungsfrage.

## Der neue Test und was er prüft

`tos-api/tests/aes_desk_background.rs:1282-1416`, Abschnitt „8. the rectangle
list and its cursor belong to one window", genau **ein** `#[test]`:
`the_rectangle_list_and_its_cursor_belong_to_one_window`.

Untergebracht wie FIX-01 und FIX-02 in dieser Datei, weil sie den Aufbau fertig
hat (`SERIAL`-Lock, `rig()`/`fresh()`, `open_window`, `wind_get_rect`,
`wind_get_ret_rect`, `WF_FIRSTXYWH`/`WF_NEXTXYWH`, die Fensterpaargeometrie
`RA`/`RB`); ein eigenes Testbinary hätte rund 700 Zeilen Harness dupliziert.
Die acht vorhandenen Tests der Datei wurden nicht berührt.

Aufbau: `fresh()`, dann Fenster A auf `RA = (40,60,400,300)` und darüber B auf
`RB = (240,180,340,250)` — dasselbe Paar, das der Handletest aus FIX-01 benutzt.
B verdeckt As rechte untere Ecke, A hat dadurch zwei Stücke, B als oberstes
Fenster genau eins. **Beide Handles sind gültig**, die Handleprüfung aus Stufe 1
kann hier also nichts beantworten.

Geprüft wird:

1. Vorbedingung des Aufbaus: A besitzt mindestens zwei Stücke, B mindestens
   eines.
2. Zuordnung ohne Koordinatenvergleich: jedes für A gemeldete Stück liegt in As
   Arbeitsfläche **und** berührt das Fenster darüber nicht; jedes für B
   gemeldete liegt in Bs Arbeitsfläche.
3. Die verschränkte Folge `FIRST(A)`, `FIRST(B)`, `NEXT(A)`:
   - `FIRST(A)` liefert As erstes Stück,
   - `FIRST(B)` liefert **Bs** erstes Stück, nicht As,
   - `NEXT(A)` liefert **As zweites** Stück — nicht das Listenende und nicht ein
     Stück von B.

**Keine eingefrorene Zahl.** Die drei Sollwerte sind die alleinigen Folgen
`solo_a`/`solo_b` desselben Laufs, über den Hilfsaufruf `walk_owner_list`
gewonnen — dieselbe Technik, die FIX-01 („gestörte Folge gegen ungestörte
Folge") und FIX-02 („Stücke nach Seite klassifizieren") benutzen. Damit bleibt
der Test gültig, wenn Stufe 4 den Schatten einbaut (die Stücke werden größer,
die Zuordnung bleibt) und wenn Stufe 5 die x/y am Listenende ändert (der Test
liest nur Stücke **vor** dem Ende). Die Fehlermeldung unterscheidet die drei
Fälle „Endmarke", „ein Stück von B" und „etwas Drittes" im Klartext.

### Beleg, dass der Test den Defekt fängt

Der Test wurde **vor** dem Fix geschrieben und gefahren. Er war rot, mit genau
dem im Auftrag beschriebenen Befund:

    thread 'the_rectangle_list_and_its_cursor_belong_to_one_window' panicked at
    tos-api/tests/aes_desk_background.rs:1399:
    assertion `left == right` failed: NEXT(A) after a FIRST(B) in between
    returned (0, 0, 0, 0) instead of A's second piece (41, 180, 199, 179).
    A rectangle list of [(41, 83, 376, 97), (41, 180, 199, 179)] for A and
    [(241, 203, 316, 226)] for B: that is the end-of-list sentinel, so B's FIRST
    overwrote A's list or cursor …
      left: (0, 0, 0, 0)
     right: (41, 180, 199, 179)

Also dasselbe Muster wie im Hostlauf aus A14: `NEXT(A)` meldet die Endmarke,
weil Bs kürzere Liste den Cursor mitgenommen hat. Nach dem Fix grün.

## Speicherkosten der Umstellung

Gemessen, nicht geschätzt: ein temporärer `#[cfg(test)]`-Aufsatz in `window.rs`
hat `core::mem::size_of` ausgegeben und wurde danach von Hand wieder entfernt
(kein `git`-Eingriff; die Datei endet wieder wie zuvor).

    SIZE Window=416 align=2 NUM_WIN=16 table=6656 both=13312
    SIZE Grect=8 RCAP=16 list_bytes=128

| Größe | vorher | nachher | Delta |
|---|---|---|---|
| `Window` | 284 B | 416 B | **+132 B** (128 B Liste + 2 B `rcount` + 2 B `rnext`) |
| `TABLE.slots` (16 Fenster) | 4 544 B | 6 656 B | +2 112 B |
| `SAVE.slots` (Schnappschuss, 16 Fenster) | 4 544 B | 6 656 B | +2 112 B |
| entfallen: `static RLIST` | 144 B | 0 B | −144 B |
| **statischer Gesamtbedarf** | 9 232 B | 13 312 B | **+4 080 B ≈ 4,0 KiB** |

Die 284 B vorher sind zusätzlich analytisch bestätigt: alle Felder sind
2-Byte-ausgerichtet und die beiden `bool` paaren sich, es gibt in beiden
Fassungen **kein** Füllbyte (Feldsumme = 416 exakt).

**Vertretbar.** Vier Kilobyte statisch in einem Kernel, der den Framebuffer
außerhalb des ST/TT-Pools hält und mit 768 MiB arbeitet, sind kein Faktor; sie
liegen im BSS, nicht auf einem Stack, und werden nie kopiert außer bei
`save_windows`/`restore_windows` (dort wächst ein ohnehin vorhandener
Array-`memcpy` von 4,4 KiB auf 6,5 KiB, einmal je Programmstart bzw. -ende).

**Was das *nicht* ist:** eine Antwort auf die Kapazitätsfrage. EmuTOS hält die
Stücke in einer verketteten Liste aus **einem gemeinsamen Vorrat** von
`NUM_ORECT = NUM_WIN * 10 = 160` Zellen (`aes/gemlib.h:88`, `or_start`/
`get_orect`, `aes/gemwrect.c:36-61`) — dort kostet ein Fenster mit einem Stück
auch nur ein Stück, und ein Fenster mit dreißig Stücken darf sich aus dem
gemeinsamen Vorrat bedienen. Wir zahlen dagegen `RCAP = 16` Stück **pro
Fenster**, ob belegt oder nicht, und schneiden bei 16 ab. Ein gemeinsamer
Vorrat wäre speichersparsamer *und* würde die Abschneidung auflösen — das ist
**A21** und wurde hier ausdrücklich **nicht** umgesetzt. Die Umstellung
vervielfacht den A21-Speicher von einmal 16 Stücken auf 16-mal 16 Stücke; das
ist der Preis dafür, dass der Cursor überhaupt einem Fenster gehören kann.

## Tatsächlich gesehene Zahlen

Alle Läufe am 2026-09-10 im gemeinsamen Arbeitsbaum (`70caee2` plus die fremden
uncommitteten Änderungen sowie FIX-01 und FIX-02), Host macOS/arm64.

`cargo test -p tos-api --tests`, Ausgangsstand nach Stufe 2 — selbst
nachgefahren, bevor eine Zeile geändert wurde:

    13 binaries, 431 passed, 0 failed, 0 ignored

Nach dieser Änderung, derselbe Befehl, tatsächlich gesehen:

    13 binaries, 432 passed, 0 failed, 0 ignored

Aufschlüsselung: `tos_api` (lib) 10 · `aes_appl_init` 1 · `aes_button_dispatch`
1 · `aes_desk_background` **9** (vorher 8) · `aes_dialog_undraw` 2 ·
`aes_object_font` 6 · `aes_reference` 177 · `aes_window_content` 1 ·
`aes_window_drag` 4 · `gemdos_drive_login` 2 · `gemdos_fs_paths` 2 ·
`vdi_raster_reference` 59 · `vdi_reference` 158.

**Die eine zusätzliche Bestehende ist der neue Test. Kein vorhandener Test ist
rot geworden** — genau wie A23 Abschnitt 7.1 es für Stufe 3 vorhergesagt hatte
(„wird rot: nichts"). Die Wächter W1 bis W7 aus A23 7.2 liegen sämtlich in
diesem Lauf und sind grün: die 177 Referenzfälle in `aes_reference` (W1, W2,
W3), `aes_object_font` + `aes_dialog_undraw` (W4), die drei Ziehtests in
`aes_window_drag` (W5) und der Umzugstest `:1146-1198` (W7), die drei
Hintergrundtests in `aes_desk_background` (W6).

Build, beide Boards, je mit erzwungener Neuübersetzung (`touch` auf die
geänderte Quelle, weil ein zweiter Board-Build sonst fälschlich „up to date"
melden kann):

    cargo xtask build --board qemu-virt
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
    cargo xtask build --board rpi4
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
      -> objcopy -> kernel8.img (gitignoriertes Bauartefakt, keine Hardwareaktion)

Warnungszahlen identisch zum Stand nach FIX-02.

Clippy, `cargo clippy -p tos-api --all-targets`, einmal mit und einmal ohne die
Änderung gemessen (beide Dateien vorübergehend aus einer beiseitegelegten Kopie
durch die von Hand zurückgebaute Fassung ersetzt und danach aus der Kopie
zurückgeholt — kein `git`-Eingriff; SHA-256 beider Dateien nach dem Zurückholen
gegen die Kopie geprüft und identisch):

| Ziel | ohne Fix | mit Fix |
|---|---|---|
| `tos-api` (lib) | 81 | 81 |
| `tos-api` (lib, 2. Ziel) | 81 | 81 |
| `tos-api` (lib test) | 82 | 82 |
| `tos-api` (test `vdi_raster_reference`) | 2 | 2 |
| `tos-api` (test `vdi_reference`) | 3 | 3 |

**Delta: 0.** Das Testziel `aes_desk_background` erscheint in beiden Läufen
nicht in den Summenzeilen, erzeugt also weiterhin keine Warnung. Die
verbliebenen Warnungen an `window.rs` sind sämtlich vorbestehend
(`private_interfaces` an jeder öffentlichen `wind_*`-Funktion wegen
`pub(crate) struct Aespb`, plus ein `unused_assignments` an `:1504`); der Fix
fügt keine hinzu und entfernt keine.

## Textvorschlag für docs/STATUS.md

Die Datei ist fremd modifiziert und wurde **nicht** angefasst. Vorschlag zum
Eintragen beim Commit, unter *Recent Decisions*:

> **2026-09-10 — Rechteckliste und Iteratorcursor gehören dem Fenster**
> (Stufe 3 der Fix-Reihenfolge aus `TASK/A23.md` 8.2; schließt D-A14-1).
> `WF_FIRSTXYWH`/`WF_NEXTXYWH` liefen über einen prozessweiten `RLIST` mit
> einem gemeinsamen `rects`/`count`/`cursor`; ein `FIRST` auf ein zweites
> Fenster überschrieb damit Liste und Cursor eines Fensters, das gerade
> iteriert wurde — `FIRST(1)`, `FIRST(2)`, `NEXT(1)` lieferte die Endmarke
> statt des zweiten Stücks von Fenster 1. EmuTOS hält beides als Feld der
> `WINDOW`-Struktur (`ORECT *w_rlist`, `ORECT *w_rnext`, `aes/gemlib.h:84-85`),
> `wm_get` wählt das angefragte Fenster (`aes/gemwmlib.c:1513`, `:1556-1561`)
> und `w_owns` stellt dessen Link weiter (`:1230-1240`). Liste, Zähler und
> Cursor liegen jetzt im Fenster-Slot (`tos-api/src/aes/window.rs`), womit
> `wind_create`, `wind_delete`, `wm_new`, `wm_init` und
> `save_windows`/`restore_windows` sie automatisch gemeinsam zurücksetzen
> beziehungsweise mitkopieren; `wind_delete` gibt die Liste zusätzlich
> ausdrücklich zurück wie EmuTOS' `newrect` im Löschpfad (`:1476`). Kosten:
> `Window` wächst von 284 auf 416 Byte, statisch +4,0 KiB — der gemeinsame
> Stückvorrat (`NUM_ORECT`) bleibt A21. Neuer Regressionstest
> `aes_desk_background::the_rectangle_list_and_its_cursor_belong_to_one_window`,
> der die verschränkte Folge gegen die alleinigen Folgen desselben Laufs
> vergleicht und daher keine Koordinate einfriert. Suite
> `cargo test -p tos-api --tests`: 432 passed, 0 failed (vorher 431).

## Braucht dieser Schritt eine reale Gegenmessung?

**Ja — und sie ist heute nicht möglich, weil der Fall in keiner eingefrorenen
Tabelle steht.** Das ist die Messlücke [A24](A24.md).

- **Quellen- und hostbelegt ist der Fix vollständig.** Das Soll steht Feld für
  Feld in `aes/gemlib.h:84-85` und `aes/gemwmlib.c:1513,1556-1561,1230-1240`;
  der Hosttest fährt die verschränkte Folge und vergleicht sie gegen die
  alleinigen Folgen desselben Laufs. A23 Abschnitt 8.3, Zeile „Stufe 3", nennt
  genau das als das, wofür ein Hosttest reicht.
- **Messbelegt ist er nicht.** Dieselbe Zeile verlangt eine **neue Falltabelle**
  mit `FIRST(1)`, `FIRST(2)`, `NEXT(1)`; die eingefrorene
  `tosdump/cases/aes-r2b-rlist.tsv` enthält keinen solchen Fall (A14, Lücke L1),
  und ihr Kopfkommentar hält sogar ausdrücklich fest, dass NEXT nie ohne
  vorangehendes FIRST **desselben** Fensters gemessen wurde. Spalte
  „Vorhandenes Messorakel" für Stufe 3: **keins**.
- Ohne diese Tabelle bleibt D-A14-1 quellenbelegt und nicht messbelegt. Ein
  grüner Hostlauf ist dafür kein Ersatz.

Eine eigene Hardwareaktion war für diesen Schritt weder nötig noch
durchgeführt; es wurde nichts geflasht und nichts auf ein Volume geschrieben.

## Was offen bleibt

- **A24** — die Falltabelle für die verschränkte Zwei-Fenster-Folge fehlt; siehe
  oben. Solange sie fehlt, ist dieser Fix nicht gegen echte Hardware geeicht.
- **Deckungslücke A23 7.5/3 — `save_windows`/`restore_windows` ist weiterhin
  ungetestet.** A23 nennt in der Stufe-3-Zeile („wird rot") die Auflage, diese
  Lücke **vor** dem Fix mit einem Test zu schließen. Der Auftrag an diese Stufe
  hat auf *genau einen* neuen Test begrenzt, und der ist die verschränkte Folge;
  der Schnappschusspfad ist daher weiter nur durch Codelesen abgedeckt (Tabelle
  oben, Zeilen `save_windows`/`restore_windows`). **Das ist eine bewusst
  ausgewiesene Restlücke, keine Erledigung** — sie gehört als eigener kleiner
  Testauftrag nachgezogen.
- **Stufe 4 (Schatten, D-A13-3)** und **Stufe 5 (F-A15-1)** sind unberührt.
  Stufe 5 fasst dieselbe Ablage an: sie muss die **rohen** Besitzerstücke
  speichern statt der bereits geschnittenen und den Work-Schnitt beim
  Weitergehen ausführen. Die Struktur ist darauf vorbereitet — der Cursor ist
  ein Index in die gespeicherte Liste, das Weiterstellen bei einem
  fehlgeschlagenen Schnitt ist eine Frage der Schleife in `rlist_next`, nicht
  der Ablage. `A14` bleibt deshalb OFFEN; nur D-A14-1 ist erledigt.
- **A21** (gemeinsamer Stückvorrat statt `RCAP = 16` pro Fenster) ist durch
  diese Umstellung **teurer** geworden, nicht billiger: siehe Speicherkosten.
- Kein Commit gesetzt; `docs/STATUS.md` bewusst nicht angefasst.

## Zusätzlicher Befund — nur gemeldet, nicht angefasst, kein Task angelegt

Beim Nachlesen des Lebenszyklus fiel auf, dass unser `compute_rlist` die Liste
**erst beim `FIRST`** baut, während EmuTOS sie in `draw_change` für **jedes**
Fenster im Voraus neu aufbaut (`everyobj(… newrect …)`, `gemwmlib.c:1089`).
Für FIRST/NEXT bei stehender Fensteranordnung — der einzige Zustand, in dem die
Folge überhaupt ein definiertes geometrisches Soll hat — ist das Ergebnis
dasselbe. Beobachtbar würde der Unterschied erst dort, wo EmuTOS die Liste
**zwischen** FIRST und NEXT neu baut: dann läuft der EmuTOS-Cursor in eine
freigegebene Liste, unser Index in die zuletzt gebaute. A23 Abschnitt 4 hält
ausdrücklich fest, dass das kein nachzubauendes Soll ist. Nicht angefasst.
