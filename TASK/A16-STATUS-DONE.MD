# A16 — Quittung

**Status: DONE** · geschlossen 2026-09-12 · `wind_close`: WS_CURR / WS_PREV /
WS_WORK bleiben unaktualisiert

> **Abschluss 2026-09-12 — die reale Gegenmessung liegt vor und bestaetigt die
> Implementierung.** Falltabelle `aes-r3-wind-state.tsv` (39 Faelle, CRC
> `979FAC60`), EmuTOS-Referenz ueber Hatari (`etos512de.img`, TT-Medium; das
> Verfahren ist bitgleich gegen zwei echte TT-Dumps validiert), NewTOS in QEMU
> `raspi4b` auf demselben 640x480-Profil und zusaetzlich auf echter Pi 4.
>
> **Alle sieben Felder stimmen.** 301/306/311 (vor dem Close) und
> 317/323/329/335 (danach). Fall 317 = `1,23,-24,-46` deckt sich mit dem
> Maschinen-Orakel 337 (`wind_calc(WC_WORK, 0x0FEF, 0,0,0,0)`) auf **derselben**
> Maschine, ist also nicht zufaellig getroffen. 335 zeigt `WS_FULL` unveraendert.
> 344 == 340 auf **beiden** Maschinen: die fruehe Absage (`gemwmlib.c:1446-1447`)
> fuer ein nie geoeffnetes Fenster wird eingehalten.
>
> **Aber:** dieselben Faelle 340/344 decken einen *benachbarten* Defekt auf, den
> A16 auftragsgemaess nicht beruehrt hat — `wind_create` schreibt `WS_WORK`,
> EmuTOS nie (`tos-api/src/aes/window.rs:2267`). Der bekommt ein eigenes Ticket
> und ist **nicht** Teil dieses Abschlusses.
>
> Beleg: `docs/analysis/aes-r3-wind-state-einordnung-2026-09-12.md`, Abschnitt 4.
> Messartefakte: `measurements/measurement-2026-09-12-hatari-r3/`,
> `measurements/measurement-2026-09-11-qemu-r3/`, `measurements/measurement-2026-09-11-pi4-r3/`.

**Der urspruengliche Text von 2026-09-11 folgt unveraendert.** Damals galt:
Implementierung fertig und hostverifiziert, die reale Gegenmessung fehlt —
genau wie bei [A25](A25-STATUS-UNFINISHED.MD). Kein
Commit; die Änderung liegt uncommittet im gemeinsamen Arbeitsbaum, aufgesetzt
auf FIX-01 bis FIX-06.

Gepinnter EmuTOS-Stand für alle Belege: `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (`git rev-parse HEAD` selbst
nachgeprüft).

## Auftrag

Nur der Zustand der drei Fensterrechtecke **nach `wind_close`** auf einem
Fenster, das erzeugt und geöffnet war. Kein Umbau des Fenstermanagers, keine
Änderung an `wind_open`, `wind_create` oder der Rechteckliste.

## Der Istwert — alle drei Felder einzeln festgestellt

Nicht aus `WF_CXYWH` hochgerechnet: jedes Feld ist mit einem **eigenen**
`wind_get`-Aufruf gemessen, jeweils einmal vor und einmal nach dem
`wind_close`, über einen temporären Messaufsatz (`tos-api/tests/zz_a16_tmp.rs`,
nach dem Lauf wieder entfernt — er liegt nicht im Baum). Profil: der
Referenz-Harness, TT-Medium 640×480 bei 4 Planes.

Ablauf je Messung — `wind_new`-Reset, dann
`wind_create(0x0FEF, 10, 20, 300, 200)`, `wind_open(1, 30, 40, 320, 240)`,
`wind_set(1, WF_CURRXYWH, 70, 90, 260, 180)`, dann optional `wind_close(1)`,
dann das eine `wind_get` mit `OUT_LEN 5`:

| Feld | vor dem Close | **Ist** nach dem Close (vorher) | **Soll** = jetzt |
|---|---|---|---|
| `WF_WXYWH` (4) | `1, 71, 113, 236, 134` | `1, 71, 113, 236, 134` | `1, 1, 23, −24, −46` |
| `WF_CXYWH` (5) | `1, 70, 90, 260, 180` | `1, 70, 90, 260, 180` | `1, 0, 0, 0, 0` |
| `WF_PXYWH` (6) | `1, 30, 40, 320, 240` | `1, 30, 40, 320, 240` | `1, 70, 90, 260, 180` |

Alle drei Felder waren betroffen, nicht nur `WF_CXYWH`: das Fenster antwortete
nach dem Schliessen exakt so wie vorher. Der A11-Nebenbefund (`1, 30, 40, 320,
240` für `WF_CXYWH`) ist derselbe Defekt ohne den Zwischenzug `wind_set`.

`1, 1, 23, −24, −46` ist keine geratene Zahl, sondern die Antwort der AES auf
`wind_calc(WC_WORK, 0x0FEF, 0, 0, 0, 0)` in demselben Lauf — `wm_calc` auf
`gl_rzero`, mit den Randstärken dieses Geräteprofils. Für ein Fenster mit Rand
ist das Ergebnis negativ; das ist EmuTOS' Rechnung, kein Rundungsfehler
(`*pw = w - lb - rb` bei `w = 0`).

## Was geändert wurde

Zwei Dateien. Nichts sonst angefasst.

### 1. `tos-api/src/aes/window.rs`, `close_window` (jetzt `:2347-2427`)

**Vorher** (der Rumpfanfang, `:2353-2364` in der Fassung vor dieser Stufe):

```rust
fn close_window(handle: i16) -> bool {
    let old_top = get_top();
    let Some((r, was_open)) = win(handle).map(|w| {
        let was = w.open;
        w.open = false;
        (w.curr, was)
    }) else {
        return false; // invalid handle
    };
    if !was_open {
        return false; // valid but not open — no action (gemwmlib.c:1446-1447)
    }
```

**Nachher** (`:2375-2399`, Kommentare hier gekürzt):

```rust
fn close_window(handle: i16) -> bool {
    let old_top = get_top();
    let Some(r) = win(handle).filter(|w| w.open).map(|w| {
        let c = w.curr;
        w.prev = c;                    // WS_PREV = old WS_CURR   (`:1069-1070`)
        set_curr(w, Grect::default()); // WS_CURR = gl_rzero, WS_WORK recalc (`:1073-1077`)
        w.open = false;
        c
    }) else {
        return false; // no such window, or it was not open
    };
```

Der Rest der Funktion — Schadensrechteck, `plus_shadow`, `redraw_clip`,
`broadcast_redraw`, Rückgabewert — ist **unverändert**. Der Doc-Kommentar der
Funktion trägt jetzt den `draw_change`-Auszug und die Ableitung der drei
Sollwerte.

Vier Punkte, die die Reihenfolge ausmachen:

1. **`r` wird vor dem Überschreiben gelesen.** Das Schadensrechteck ist
   weiterhin das alte `WS_CURR`, genau EmuTOS' `c` aus `:1069`.
2. **Die Reihenfolge ist die von EmuTOS**: `WS_PREV = WS_CURR`, dann
   `WS_CURR = gl_rzero`, dann `WS_WORK` neu, dann `open = false`.
   `set_curr` ist genau das Paar aus `w_setsize(WS_CURR, …)` und
   `wm_calc(WC_WORK, …)` (`window.rs:1605-1608`), dieselbe Hilfsfunktion, die
   `wind_open` und `wind_set(WF_CURRXYWH)` benutzen.
3. **`open = false` bleibt vor der Schadensrechnung.** Unser einzelnes
   `open`-Flag trägt in EmuTOS zwei Rollen: `ob_delete(gl_wtree, w_handle)`
   **vor** `draw_change` (`:1452`) und `w_flags &= ~VF_ISOPEN` **nach**
   `draw_change` (`:1454`). Jeder Baumlauf bei uns (`get_top`,
   `border_pieces`, `broadcast_redraw`, `find_handle`, `compose`) prüft
   `open`, entspricht also dem Baum, nicht dem Flag. Läge es hinter dem
   Redraw, wäre das geschlossene Fenster beim `get_top` nach dem Schliessen
   noch oben (der neue Rahmen käme nicht in den Schaden) und `border_pieces`
   würde `ws_true(0,0,0,0) = (0,0,2,2)` als deckendes Rechteck von den anderen
   Fenstern abziehen. Deshalb: die drei Rechtecke, dann das Flag, dann der
   Schaden — beide EmuTOS-Reihenfolgen zugleich erfüllt.
4. **Die frühe Absage schreibt nichts.** `filter(|w| w.open)` ersetzt das
   nachgelagerte `if !was_open`. `wm_close` kehrt bei unbekanntem Handle
   (`:1443-1445`) und bei einem nicht offenen Fenster (`:1446-1447`) um, **bevor**
   `draw_change` überhaupt erreicht wird. Das ist kein Feinschliff: das
   Desktopfenster ist `VF_INUSE`, aber nie `VF_ISOPEN` (gemessener Fall 194
   der eingefrorenen Tabelle). Würde `close_window` unbedingt schreiben, wäre
   nach einem `wind_close(0)` das `WS_CURR` des Desktops `0,0,0,0` — und
   `find_handle` (`window.rs:3624-3628`) sowie jede andere Trefferprüfung hätte
   keinen Bildschirm mehr. Der Fall 194 läuft grün, weil die Absage vor dem
   Schreiben steht.

### 2. `tos-api/tests/aes_reference.rs`, ein neuer Test (`:164-278`)

Siehe „Der neue Test".

### Nicht angefasst

`kernel/src/desk/`, `kernel/src/shell_loop.rs`, `tos-api/src/aes/pd.rs`,
`docs/STATUS.md`, `TASK/A25*`, `TASK/evidence/**`, alle eingefrorenen Artefakte
(`tosdump/cases/*.tsv`, `*.BIN`, `*.LOG`, `measurement-*`). Innerhalb von
`window.rs` nur `close_window`: `wind_close`, `wind_delete`, `wind_open`,
`wind_set`, `wind_get`, `set_curr`, `border_pieces`, `w_union`,
`send_redraw_in`, `broadcast_redraw`, `compute_rlist`, `rlist_owns`,
`save_windows`/`restore_windows` sind nur **gelesen** worden.

## EmuTOS-Beleg

| Aussage | Quelle |
|---|---|
| `wm_close` schliesst über `draw_change(w_handle, &gl_rzero)` | `aes/gemwmlib.c:1449,1453` |
| `ob_delete` nimmt das Fenster **vor** `draw_change` aus dem Baum | `aes/gemwmlib.c:1452` |
| `VF_ISOPEN` fällt erst **nach** `draw_change` | `aes/gemwmlib.c:1454` |
| unbekanntes Handle → FALSE, ohne etwas zu ändern | `aes/gemwmlib.c:1443-1445` |
| nicht offenes Fenster → FALSE, ohne etwas zu ändern | `aes/gemwmlib.c:1446-1447` |
| `WS_PREV = altes WS_CURR` | `aes/gemwmlib.c:1069-1070` |
| `WS_CURR = pt` | `aes/gemwmlib.c:1073` |
| `WS_WORK = wm_calc(WC_WORK, kind, pt)` | `aes/gemwmlib.c:1074-1077` |
| alle drei **vor** der `VF_ISOPEN`-Abkürzung | `aes/gemwmlib.c:1085-1086` |
| `gl_rzero` ist 0,0,0,0 | `aes/gemgraf.c:425` |
| `wm_calc` liefert `w − lb − rb`, also negativ bei `w = 0` | `aes/gemwmlib.c:1864-1867` |
| `WC_WORK == 1` | `include/aesdefs.h:556` |
| `WS_FULL` bleibt liegen; der Slot wird erst von `wm_delete` zurückgesetzt | `aes/gemwmlib.c:1476-1481` |
| `wm_set`s `WF_CXYWH`-Arm ist `draw_change(w_handle, pinwds)` | `aes/gemwmlib.c:1676-1678` |
| `wm_open` überschreibt `WS_PREV` nachträglich mit dem Öffnungsrechteck | `aes/gemwmlib.c:1426` |
| `rc_intersect` scheitert an einem Rechteck mit `w ≤ 0` | `util/rectfunc.c:63-78` |

## Kompensationsprüfung (A23-Pflicht)

**Vollständige Liste der Schreiber und Leser von `curr`, `prev` und `work`.**
`Window` ist privat in `tos-api/src/aes/window.rs`; ausserhalb dieser Datei
greift **kein** Code auf die Felder zu (geprüft: `grep -rn --include="*.rs" -e
"\.curr" -e "\.prev" -e "\.work" -e "set_curr" tos-api/src kernel/src` — die
Treffer sind `bdos::process::set_current` und ein USB-Registername, kein
Fensterfeld). Der Kernel-Desktop kommt nur über `wind_get`/`wind_set` heran.

### Schreiber

| Stelle | schreibt | Wirkung durch A16 |
|---|---|---|
| `init_desk_window` `:509-512` | `curr`, `prev`, `work` des Desktops | keine — Handle 0 ist nie `open`, die frühe Absage schützt es |
| `wind_create` `:2265-2267` | alle drei auf `gl_rzero` / `calc(full)` | keine |
| `wind_open` `:2300-2303` | `prev = curr`, `set_curr(r)`, danach `prev = r` | keine — `prev` wird ohnehin überschrieben; ein geschlossenes Fenster bringt jetzt `curr = 0` mit, das `set_curr` sofort ersetzt |
| `wind_set(WF_CXYWH)` `:2712-2714` | `prev = curr`, `set_curr(r)` | auf einem **geschlossenen** Fenster liest es jetzt `curr = 0` statt des Altrechtecks — **das ist EmuTOS**, `draw_change` sieht nach einem `wm_close` dasselbe `gl_rzero` |
| **`close_window` (neu)** | `prev = curr`, `set_curr(gl_rzero)` | der Fix |
| `save_windows` `:1024` | `live[1..] = Window::EMPTY` | keine — Blockkopie der ganzen Tabelle, kein Feldzugriff |
| `restore_windows` `:1042-1044` | Tabelle aus `SAVE` zurück | keine — Blockkopie; der geschlossene Zustand reist unverändert mit |
| `wind_delete` `:2437-2461` | `rlist`/`rcount`/`rnext`/`in_use`/`open` | keine — schreibt die drei Rechtecke **weder vorher noch nachher** (siehe „Offen") |
| `wm_new` `:3008` | `slots()[1..] = Window::EMPTY` | keine |

### Leser

| Stelle | liest | gleicht den Fehler aus? |
|---|---|---|
| `wind_get` `:2575-2577` | `work`/`curr`/`prev` | **nein — hier war der Fehler sichtbar.** Reine Durchreiche, ohne Umrechnung |
| `border_pieces` `:781-790` | `ws_true(curr)` des Fensters **und** jedes höheren | **nein**, doppelt abgeriegelt: `Some(w) if w.open` für das eigene Fenster, `ow.in_use && ow.open` für die Decker. Ein geschlossenes Fenster liefert `n = 0` und deckt niemanden |
| `is_broken` `:734-739` | `ws_true(curr)`, `z` | **nein** — `filter(\|w\| w.open)` bzw. `ow.open` |
| `compose` / `paint_bg_clip` / `draw_frame_pieces` / `walk_rects` | über `border_pieces`, `w.open` | **nein** — `compose` iteriert `w.in_use && w.open` |
| `w_union` `:1502` (FIX-06) | rohe Liste aus `border_pieces` | **nein** — liefert `None` für ein geschlossenes Fenster, unabhängig von `curr` |
| `send_redraw_in` `:1552` (FIX-06) | `work` | **nein** — erreichbar nur über `broadcast_redraw` (Filter `w.open`) und `top_window` |
| `top_window` `:703` (FIX-04/06) | `curr` des zu toppenden Fensters | **nein** — wird auf dem Schliesspfad nie aufgerufen |
| `redraw_open` `:2863` | über `draw_frame_pieces` | **nein** — explizites `if w.open` |
| `draw_frame` `:1792`, `vtrack`/`htrack`/`draw_vslider`/`draw_hslider` `:2023-2131` | `curr` | **nein** — nur aus `draw_frame_pieces` erreichbar, das für ein geschlossenes Fenster null Stücke bekommt |
| `gadget_at` `:3146`, `gad_rect` `:3222`, `drag_size` `:3358`, `drag_move` `:3482` | `curr`, `work` | **nein** — der Steuerungspfad findet das Fenster über `find_handle`, das `w.open` prüft |
| `chk_ctrl` `:3602` | `work` von `get_top()` | **nein** — `get_top` liefert nur offene Fenster oder `DESKWH` |
| `find_handle` `:3624-3628` | `curr` des Desktops und jedes **offenen** Fensters | **nein** — und genau deshalb darf `wind_close(0)` das Desktopfenster nicht anfassen (siehe Punkt 4 oben) |
| `compute_rlist` `:1371` | über `border_pieces` | **nein** — `rcount = 0` nach dem Schliessen |
| `rlist_owns` `:1428` | `work` | **eine belegte Verhaltensänderung, siehe unten** |
| `kernel/src/desk/*` | nur über `wind_get`/`wind_set` | **nein** — `desk::close_window` (`main.rs:1625`) liest nach dem Close nur `wind_get_top()` und die Arbeitsfläche des **neuen** Topfensters; `inf.rs:841` läuft über die desktopeigene Slotliste, aus der `win_free` das Fenster im selben Zug entfernt; `do_wfull` (`main.rs:1439`) liest `WF_CXYWH`/`WF_PXYWH` nur auf ein `WM_FULLED` hin, also an einem offenen Fenster |

**Ergebnis: keine Stelle gleicht den Fehler heute aus.** Der falsche Wert war
an genau einer Stelle sichtbar — `wind_get` — und an genau einer Stelle
wirksam: bei einem `wind_set(WF_CURRXYWH)` auf ein geschlossenes Fenster wäre
das falsche `curr` ins `prev` gewandert. Der Zeichen- und Redraw-Pfad ist
gegen `curr`/`work` eines geschlossenen Fensters durchgehend über das
`open`-Flag abgeriegelt, an jeder einzelnen Stelle.

### Die eine belegte Verhaltensänderung: `WF_NEXTXYWH` über einen Close hinweg

Ruft eine Anwendung `WF_FIRSTXYWH`, schliesst das Fenster und ruft dann
`WF_NEXTXYWH`, läuft `rlist_owns` die **stehengebliebene** Stückliste ab und
schneidet jedes Stück gegen `w.work`. Bisher war das die alte, lebende
Arbeitsfläche (ein Stück konnte `Owns::Visible` liefern); jetzt ist es die aus
`gl_rzero` gerechnete, deren Breite/Höhe negativ ist, also scheitert
`rc_intersect` an jedem Stück und der Lauf endet in `Owns::Exhausted`.

**Das ist EmuTOS, nicht ein Nebenschaden.** `ob_delete` nimmt das Fenster vor
`draw_change` aus `gl_wtree` (`:1452`), also erreicht
`everyobj(gl_wtree, ROOT, NIL, newrect, …)` (`:1089`) es nicht mehr: `w_rlist`
und `w_rnext` bleiben in EmuTOS ebenfalls stehen, während `WS_WORK` schon das
`gl_rzero`-Ergebnis trägt. `w_owns` (`:1230-1240`) schneidet dagegen und
scheitert an jedem Stück — dieselbe Endkontraktion, die A15 beschrieben hat.
Unser `rc_intersect` (`window.rs:1276-1285`) ist wortgleich mit
`util/rectfunc.c:63-78`, einschliesslich des Verhaltens bei negativer Breite.
`WF_FIRSTXYWH` selbst ist unverändert: `compute_rlist` → `border_pieces` gibt
für ein geschlossenes Fenster 0 Stücke, das Ergebnis ist `Owns::Empty` wie
vorher.

Kein Test im Baum deckt diese Folge ab (A31 ist der offene Task dazu); die
volle Suite bleibt grün.

## Der neue Test

**Genau einer**, in `tos-api/tests/aes_reference.rs:164-278`:
`window_rectangles_after_a_close_are_the_ones_draw_change_leaves`.

Vorher geprüft, ob ein vorhandener Test den Übergang schon abdeckt:

- `tos-api/tests/aes_reference.rs:108-162`
  (`window_rectangles_after_create_and_open_match_the_tt`) deckt **Erzeugen und
  Öffnen** ab (Fälle 228/230/232/250/253/259) — **nicht** das Schliessen. Nicht
  nachgebaut, nicht verändert.
- Die generierten Fälle zu `wind_close` —
  `tos-api/tests/aes_reference/cases_r2_wind.rs:265-330`, Fälle 188/190/193/194
  — haben `OUT_LEN 1` und prüfen ausschliesslich den Rückgabewert. Kein Fall
  liest danach ein Rechteck.
- `tos-api/tests/aes_desk_background.rs:754-786` prüft die **Pixel** nach einem
  `wind_close`, keine Felder.

Der Test steht neben dem Erzeugen/Öffnen-Test, weil er dessen Fortsetzung ist
und den Wiedergabeapparat (`R2Call`, `r2w_replay`, `harness::Pb`) fertig
vorfindet. Orakel ist die **gepinnte Quelle**, nicht die Tabelle — dasselbe
Muster wie beim schon vorhandenen `wf_owner_…`-Test derselben Datei, der es
ebenso begründet.

Aufbau: `wind_new`-Reset, `wind_create(0x0FEF, 10, 20, 300, 200)`,
`wind_open(1, 30, 40, 320, 240)`, **`wind_set(1, WF_CURRXYWH, 70, 90, 260, 180)`**,
dann `wind_close(1)`, dann drei getrennte `wind_get`-Läufe.

Der Zwischenzug `wind_set` ist der Kern der Trennschärfe: `wm_open`
überschreibt `WS_PREV` nachträglich mit dem Öffnungsrechteck (`:1426`, Fall
253), ohne die Verschiebung läsen `WF_PXYWH` vor und nach dem Close denselben
Wert und der Test könnte eine richtige Umsetzung nicht von einer unterscheiden,
die gar nichts schreibt.

**Keine eingefrorene Zahl, die an einer Grösse hängt.** Die Erwartungen:

- `WF_CXYWH` → `[1, 0, 0, 0, 0]`. Das ist `gl_rzero`, eine EmuTOS-Konstante
  (`gemgraf.c:425`), kein Geräteparameter.
- `WF_PXYWH` → das **zur Laufzeit** mit `wind_get(WF_CXYWH)` vor dem Close
  zurückgelesene Rechteck.
- `WF_WXYWH` → die **zur Laufzeit** erfragte Antwort der AES auf
  `wind_calc(WC_WORK, 0x0FEF, 0, 0, 0, 0)` (Opcode 108). Damit folgt die
  Erwartung den Randstärken des Geräteprofils (`adj_hbox`, `adj_wbox`,
  `GL_HBOX`), statt sie festzuschreiben.
- Keine Schattenzahl im Spiel; `DROP_SHADOW_SIZE`/`DROP_SHADOW_COLOR` kommen in
  diesem Test nicht vor.

Drei Vorprüfungen machen ihn selbstprüfend: das Fenster muss vor dem Close da
sein, sein Rechteck muss von `0,0,0,0` abweichen, und die geschlossene
Arbeitsfläche muss von der offenen abweichen.

**Beleg, dass jede der drei Behauptungen einzeln greift.** Kontrolliert
nachgefahren, jeweils mit einer von Hand zurückgeschriebenen Teilfassung von
`close_window` (Datei vorher beiseitegelegt und daraus zurückgeholt — **kein
`git`**, `diff` gegen die Kopie danach identisch):

| Fassung | rot wird | gemeldet |
|---|---|---|
| unverändert (vor dem Fix) | `WF_CXYWH` | `left: [1, 70, 90, 260, 180]` / `right: [1, 0, 0, 0, 0]` |
| nur `curr = gl_rzero` | `WF_PXYWH` | `left: [1, 30, 40, 320, 240]` / `right: [1, 70, 90, 260, 180]` |
| `curr` + `prev`, `work` stehengelassen | `WF_WXYWH` | `left: [1, 71, 113, 236, 134]` / `right: [1, 1, 23, −24, −46]` |

Jede der drei Sollaussagen hat also ihren eigenen Gegenbeweis; der Test ist
nicht durch eine einzige Behauptung getragen.

## Tatsächliche Testzahlen

Ausgangsstand laut Auftrag: 13 Binaries, 435 passed, 0 failed, 0 ignored.

    $ cargo test -p tos-api --tests
    binaries: 13   passed: 436   failed: 0   ignored: 0

Je Binary: lib 10 · aes_appl_init 1 · aes_button_dispatch 1 ·
aes_desk_background 12 · aes_dialog_undraw 2 · aes_object_font 6 ·
**aes_reference 178** (vorher 177) · aes_window_content 1 · aes_window_drag 4 ·
gemdos_drive_login 2 · gemdos_fs_paths 2 · vdi_raster_reference 59 ·
vdi_reference 158.

**Kein vorhandener Test ist rot geworden.** +1 ist genau der neue Test. Die
vier eingefrorenen `wind_close`-Fälle (188/190/193/194) und der Pixeltest
`aes_desk_background::…wind_close…` sind grün geblieben — Fall 194
(`wind_close(0)` auf das Desktopfenster) ist dabei der Wächter über der frühen
Absage.

Bauen und Clippy, beide Boards:

    $ cargo xtask build --board qemu-virt   → exit 0
    $ cargo xtask build --board rpi4        → exit 0 (kernel8.img geschrieben)

    tos-api  build : 64 Warnungen vorher → 64 nachher   (Delta 0)
    tos-api  clippy: 81 Warnungen vorher → 81 nachher   (Delta 0)
    kernel   build : 94 Warnungen (unberührt, keine Kernel-Datei geändert)

Die Vorher-Zahlen sind nicht geschätzt, sondern gemessen: die Datei wurde dafür
kurz durch die von Hand rekonstruierte Fassung ohne den Fix ersetzt und danach
aus der beiseitegelegten Kopie zurückgeholt (`diff` identisch, kein `git`).
`cargo clippy -p tos-api --tests` meldet **keine** Warnung in
`tests/aes_reference.rs`.

## Textvorschlag für `docs/STATUS.md`

`docs/STATUS.md` ist auftragsgemäss **nicht** angefasst. Vorschlag für eine
Zeile unter *Recent Decisions* (Commit-Hash nachtragen):

> | 2026-09-11 | **`wind_close` liess alle drei Fensterrechtecke stehen — behoben (A16).** `wm_close` schliesst nicht durch ein Flag, sondern durch `draw_change(w_handle, &gl_rzero)` (`aes/gemwmlib.c:1449,1453`), und `draw_change` schreibt `WS_PREV = altes WS_CURR`, `WS_CURR = gl_rzero` und `WS_WORK = wm_calc(WC_WORK, kind, gl_rzero)` **vor** der `VF_ISOPEN`-Abkürzung (`:1069-1077` gegen `:1085-1086`); das Flag fällt erst danach (`:1454`). Bei uns schrieb `close_window` keines der drei: ein geschlossenes Fenster antwortete auf `WF_CXYWH`, `WF_PXYWH` und `WF_WXYWH` genau wie vorher (einzeln gemessen: `1, 70, 90, 260, 180` / `1, 30, 40, 320, 240` / `1, 71, 113, 236, 134`, Soll `1, 0,0,0,0` / `1, 70, 90, 260, 180` / `1, 1, 23, −24, −46`). Fix lokal in `tos-api/src/aes/window.rs::close_window`, in EmuTOS' Reihenfolge; die frühe Absage für ein nicht offenes Fenster (`:1446-1447`) bleibt vor dem Schreiben, sonst verlöre das Desktopfenster — `VF_INUSE`, nie `VF_ISOPEN`, gemessener Fall 194 — sein Bildschirmrechteck und damit jede Trefferprüfung. **Kompensationsprüfung nach A23:** kein zweiter Leser glich den Fehler aus; der Zeichen- und Redraw-Pfad ist über das `open`-Flag durchgehend abgeriegelt (`border_pieces`, `is_broken`, `compose`, `broadcast_redraw`, `find_handle`). Eine belegte Folgeänderung: `WF_NEXTXYWH` über einen Close hinweg endet jetzt in der Endkontraktion statt in einem sichtbaren Stück — ebenfalls EmuTOS (`:1452` gegen `:1089`, `w_owns` `:1230-1240`). Ein Regressionstest, `tos-api/tests/aes_reference.rs::window_rectangles_after_a_close_are_the_ones_draw_change_leaves`; Erwartungen zur Laufzeit zurückgelesen bzw. über `wind_calc(WC_WORK, …, gl_rzero)` erfragt, keine eingefrorene Geometrie. `cargo test -p tos-api --tests`: 13 Binaries, 436 passed, 0 failed (vorher 435). Beide Boards gebaut, Clippy-Delta 0 (tos-api 81, unverändert). **Offen: reale Gegenmessung** — die eingefrorene `aes-r2-wind.tsv` hat für diesen Übergang keinen Fall und darf keinen bekommen; nötig wäre eine neue Tabelle mit eigener CRC und ein TT-Lauf. |

## Was offen bleibt

1. **Die reale Gegenmessung — der Grund für UNFINISHED.** Das Soll ist aus der
   gepinnten Quelle abgeleitet und auf dem Host verifiziert; ein Record vom
   Atari TT gibt es für diesen Übergang nicht. Nach der Arbeitsregel ersetzt
   ein grüner Hosttest keine offene Hardwareabnahme.
2. **Zur Gegenmessung (Abnahmepunkt 5), ausdrücklich beantwortet.** Ein
   Messfall **wäre möglich** und ist sogar besonders billig: vier Aufrufe
   (`wind_create`, `wind_open`, optional `wind_set(WF_CURRXYWH)`,
   `wind_close`) als Prelude, danach je ein `wind_get(1, 4/5/6)` mit
   `OUT_LEN 5` als gemessener Aufruf — nichts davon blockiert, nichts braucht
   Eingaben, und der `wind_new`-Reset der Runde 2 setzt den Zustand sauber
   zurück. Er gehört **nicht** in `tosdump/cases/aes-r2-wind.tsv`: die ist
   eingefroren (CRC `AFB11C9F`, 78 Fälle) und trägt die Gültigkeit der
   vorhandenen Vergleiche. Er gehört in eine **neue** Tabelle mit eigener CRC,
   in der Bauart von `aes-r2b-rlist.tsv` — Vorschlag
   `tosdump/cases/aes-r3-wind-close.tsv`, Reset `wind_new`, mit dem
   zugehörigen TT-Lauf in ein eigenes Verzeichnis. Zusätzlich drei Fälle, die
   sich anbieten: dieselben drei Felder nach einem `wind_close` auf ein
   **erzeugtes, nie geöffnetes** Fenster (muss unverändert `gl_rzero` bleiben,
   weil `wm_close` vorher umkehrt), und `wind_get(0, WF_CXYWH)` nach einem
   `wind_close(0)` (muss den Bildschirm liefern). **Angelegt habe ich nichts**
   — weder Tabelle noch Lauf.
3. **Statuszeilen nicht gesetzt.** `TASK/A16.md` steht weiter auf `OFFEN` und
   die A16-Zeile in `TASK/README.md` ebenso; ich schliesse den Task nicht, also
   setze ich ihn auch nicht. Vorschlag für die Leitung, in der Schreibweise von
   A25: **`IMPLEMENTIERT — HW OFFEN`**.
4. **Zusätzlicher Befund unterwegs — gemeldet, nicht repariert, kein Task
   angelegt** (Arbeitsregel): unser `wind_delete` (`window.rs:2437-2461`) setzt
   die Rechtecke **nicht** zurück. EmuTOS' `wm_delete` schreibt
   `WS_CURR = WS_PREV = gl_rscreen` und `WS_FULL = WS_WORK = gl_rfull`
   (`aes/gemwmlib.c:1477-1480`); bei uns bleibt stehen, was der Close
   hinterlassen hat. Sichtbar nur über ein `wind_get` auf ein gelöschtes
   Handle, und da schlägt die `in_use`-Prüfung ohnehin vorher zu — beobachtbar
   also erst, wenn dasselbe Handle neu vergeben wird, und dann überschreibt
   `wind_create` alle vier. Kein Fall der eingefrorenen Tabelle berührt es;
   A16 erlaubt keinen Eingriff ausserhalb von `close_window`. Derselbe Absatz
   gilt für `wm_new`, das die Slots mit `Window::EMPTY` füllt, statt sie über
   `wm_close` + `wm_delete` zu schicken.
5. **Nicht geprüft, weil ausserhalb des Auftrags:** A28/A29 (Startgrenze
   Desktop → Kindprogramm, Deskriptor beim Programmende — anderer Strang),
   A21 (`RCAP`), A31 (Rechteckliste über `save_windows`/`restore_windows`).
