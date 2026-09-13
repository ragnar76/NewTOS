# FIX-06 — das Rechteck in der `WM_REDRAW`-Nachricht

Status: **DONE** (2026-09-11) · Stufe 6 der Fix-Reihenfolge aus
[A23](A23.md), Abschnitt 8.2 · deckt den **unabhängigen Befund** aus A23
Abschnitt 6, Schlussabsatz ab (kein A23-Fix, kein A13/A14/A15-Defekt)

Kein Commit. Die Änderung liegt uncommittet im gemeinsamen Arbeitsbaum, auf
FIX-01 bis FIX-05 aufgesetzt.

## Auftrag

`w_redraw` bildet das gesendete Rechteck **nicht** aus den work-geschnittenen
Stücken. Selbst nachgelesen am gepinnten Stand
`1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(`references/emutos`, `git rev-parse HEAD` bestätigt):

```c
static void w_redraw(WORD w_handle, GRECT *pt)          /* :886 */
{
    GRECT   t, d;
    AESPD   *ppd;

    ppd = D.w_win[w_handle].w_owner;

    /* make sure work rect and word rect intersect */
    rc_copy(pt, &t);
    w_getsize(WS_WORK, w_handle, &d);
    if (rc_intersect(&t, &d))                            /* :896 */
    {
        /* make sure window owns a rectangle */
        if (w_union(D.w_win[w_handle].w_rlist, &d))      /* :899 */
        {
            /* intersect redraw rect with union of owner rects */
            if (rc_intersect(&d, &t))                    /* :902 */
                ap_sendmsg(wind_msg, WM_REDRAW, ppd, w_handle,
                           t.g_x, t.g_y, t.g_w, t.g_h);  /* :903 */
        }
    }
}
```

Drei getrennte Anweisungen, und die Reihenfolge der beiden `rc_intersect`-Aufrufe
ist der ganze Inhalt:

- `rc_intersect(&t, &d)` schreibt sein Ergebnis in **`d`**, nicht in `t` — der
  **zweite** Parameter ist der überschriebene (`util/rectfunc.c:61-78`,
  nachgelesen). `t` trägt danach weiterhin das **volle** Schadensrechteck; der
  Rückgabewert wird nur als **Nichtleer-Prüfung** benutzt: „berührt das
  Schadensrechteck die Arbeitsfläche überhaupt".
- `w_union(w_rlist, &d)` (`gemwmlib.c:869-884`) **überschreibt `d`** mit der
  Hülle der Besitzerliste. Diese Liste hält die **rohen** `WS_TRUE`-Reststücke,
  die `newrect` einhängt (`aes/gemwrect.c:190,203`); auf diesem Weg schneidet
  sie **niemand** auf die Arbeitsfläche. Der Work-Schnitt gehört zu `w_owns` und
  damit allein zu `WF_FIRSTXYWH`/`WF_NEXTXYWH` (`:1230-1240,1556-1561`) — so
  hält der Besitzer sein eigenes Zeichnen in seiner Arbeitsfläche
  (`desk/desksupp.c:223-229`).
- Gesendet wird `d ∩ t`, also `Schaden ∩ Hülle(rohe Besitzerliste)`.

Bei uns stand stattdessen (Zeilen des Arbeitsbaums **vor** dieser Stufe, selbst
nachgeprüft):

- `tos-api/src/aes/window.rs:1490-1502` (`visible_bound`) — Hülle über
  `work_rects`, also über die schon **work-geschnittenen** Stücke.
- `:1507-1511` (`send_redraw_in`) — `intersect_grect(damage, visible_bound(...))`,
  ein einziger Schnitt.
- `:1347-1367` (`work_rects`) — der Vorfilter, der jeden erfolglosen Schnitt
  verwarf.

Das ist aus **zwei** unabhängigen Gründen ein anderes Rechteck: der
Arbeitsflächenschnitt selbst, und die Tatsache, dass „erst Stücke schneiden,
dann Hülle" im Allgemeinen kleiner ist als „erst Hülle, dann schneiden".

### Der Kommentar war mitzuziehen

A23 hält ausdrücklich fest, dass der Kommentar an `visible_bound`/`send_redraw_in`
EmuTOS **falsch** beschrieb („`pt` ∩ work ∩ `w_union(w_rlist)`") und erst in
dieser Stufe zu berichtigen ist. Das ist geschehen; die Funktion heißt jetzt
`w_union`, weil „visible bound" nach der Korrektur aktiv irreführend wäre.

### `top_window` gehört dazu — nachgewiesen, nicht angenommen

Der FIX-04-Entwickler hat gemeldet, dass `top_window` (`window.rs:680` in der
Fassung vor Stufe 5) `w.curr` an `send_redraw_in` reicht, EmuTOS dagegen
`TRUE(curr) ∩ gl_rfull`. **Es ist dieselbe Beziehung, und es ist in denselben
Commit gegangen.** Beleg, in dieser Reihenfolge:

1. **Quelle.** `draw_change`s Top-Zweig: `c` ist das `WS_CURR` des Fensters
   (`gemwmlib.c:1069-1070`); die Vereinigung bei `:1198-1201` mit dem
   `WS_CURR` von `gl_wtop` ist ein Nulleffekt, weil `gl_wtop` **dieses** Fenster
   ist; der gemeinsame Schluss `:1213-1215` addiert den Schatten unbedingt;
   `w_update` schneidet dann auf `gl_rfull` (`:1010`) und ruft `w_redraw`
   (`:1024-1030`). Das Schadensrechteck ist also `curr + Schatten`, auf den
   Bildschirm geschnitten.
2. **Messung im Baum.** Mit korrigiertem `w_redraw`, aber unverändertem
   `top_window` liefert T1 `(60, 60, 340, 250)` statt der aus der Quelle
   abgeleiteten `(60, 60, 342, 252)` — kontrolliert nachgefahren (Datei von Hand
   zurückgeschrieben, kein `git`). Ohne diesen Teil ist das von A23 genannte
   Soll **nicht** erreichbar.

Der `gl_rfull`-Schnitt aus `w_update:1010` ist bei dieser Geometrie ein
Nulleffekt (das Fenster liegt ganz auf dem Schirm), gehört aber zur genannten
Beziehung und ist mit umgesetzt.

Ausdrücklich **nicht** Gegenstand und **nicht** angefasst: die Kapazitätsgrenze
`RCAP` (A21), P1/P2 der Rechteckliste (Stufe 5, erledigt), Schatten und
Stückreihenfolge (Stufen 2 und 4, erledigt), `subtract`, `border_pieces`,
`walk_rects`, `draw_frame_pieces`, `is_broken`, `compute_rlist`, `rlist_owns`,
`wind_get`.

## Was geändert wurde

Eine Quelldatei und zwei Testdateien.

`tos-api/src/aes/window.rs`: **+114 / −57 Zeilen**, gemessen gegen den
rekonstruierten Ausgangsstand derselben Datei (im Arbeitsbaum liegen FIX-01…05
und fremde Änderungen uncommittet, `git diff` taugt dafür nicht). Die
Rekonstruktion trifft die vor der Arbeit gemessene Zeilenzahl **3774** exakt.
Davon sind 95 hinzugefügte und 28 entfernte Zeilen Kommentar mit Quellenbeleg;
der reine Code samt Leerzeilen umfasst **19 hinzugefügte und 29 entfernte**
Zeilen — die Stufe macht den Code kürzer.

### 1. `w_union` ersetzt `visible_bound` (`:1471-1507`, `fn` ab `:1502`)

**Vorher:** Hülle über `work_rects`, Rückgabe `Grect` mit `(0,0,0,0)` als
„leer".

**Nachher:**

```rust
fn w_union(handle: i16) -> Option<Grect> {
    let mut list = [Grect::default(); RCAP];
    let n = border_pieces(handle, &mut list);
    list.iter().take(n).copied().reduce(union_grect)
}
```

- **rohe** Liste (`border_pieces`), kein Work-Vorfilter;
- `Option` ist das `FALSE` der C-Funktion bei leerer Liste (`:871-872`) — genau
  das, was `w_redraw` daran hindert, überhaupt zu senden. Ein `Grect` mit
  `w = h = 0` kann „leere Liste" und „leerer Schnitt" nicht auseinanderhalten.

Der Doc-Kommentar hält jetzt fest, **warum** die Liste roh ist, und warum die
gespeicherte Kopie aus `compute_rlist` hier **nicht** gelesen werden darf: sie
entsteht erst, wenn eine Anwendung nach einem Stück gefragt hat, und sie zu
benutzen hieße, den Cursor eines fremden Iterators anzufassen. `border_pieces`
ist derselbe Erzeuger, den `compute_rlist` speichert.

### 2. `send_redraw_in` ist `w_redraw` (`:1508-1567`, `fn` ab `:1551`)

```rust
fn send_redraw_in(handle: i16, damage: Grect) {
    let Some(work) = win(handle).map(|w| w.work) else { return };
    let mut t = damage;
    rc_intersect(desktop_work(), &mut t);   // w_update `:1010`
    let mut d = work;
    if !rc_intersect(t, &mut d) { return; } // `:896` — Tor, kein Schnitt
    let Some(d) = w_union(handle) else { return }; // `:899` überschreibt d
    if rc_intersect(d, &mut t) {            // `:902`
        send_redraw(handle, t);
    }
}
```

Die Primitive `rc_intersect` gibt es seit Stufe 5 (`:1266-1288`); sie wird hier
zum zweiten Leser und macht die „schreibt in `p2`"-Semantik im Rust-Code sichtbar,
die den ganzen Unterschied ausmacht. Das Überschreiben von `d` durch `w_union`
ist als Shadowing geschrieben, damit die Zeile eins zu eins gegen `:899` lesbar
bleibt.

Der `gl_rfull`-Schnitt aus `w_update:1010` sitzt hier statt in
`broadcast_redraw`, weil **beide** Eingänge in den Fan-out (`broadcast_redraw`
und `top_window`s Einzelrunde) durch diese Funktion laufen und „einmal vor der
Schleife" dasselbe Rechteck ergibt wie „einmal pro Runde". Der Doc-Kommentar
sagt das ausdrücklich.

### 3. `top_window` reicht das Schadensrechteck von `draw_change` (`:664`, Rumpfende `:702-713`)

**Vorher:** `send_redraw_in(handle, r)` mit `r = w.curr`.
**Nachher:** `send_redraw_in(handle, plus_shadow(r))`, mit
Herleitung im Kommentar (`gemwmlib.c:1069-1070,1198-1201,1213-1215`).

`plus_shadow` (nicht `ws_true`) ist die richtige Primitive: `draw_change`s
Schluss addiert **unbedingt**, `w_getsize(WS_TRUE, …)` nur bei nichtleerem
Rechteck (`:253-261`). Für ein offenes Topfenster ist das Ergebnis gleich, die
Herkunft nicht.

### 4. `work_rects` entfernt (stand vor der Änderung bei `:1339-1367`)

`visible_bound` war nach Stufe 5 sein **einziger** Leser — FIX-05 hat das
ausdrücklich vermerkt. Mit dessen Umstellung auf die rohe Liste hat die Funktion
keinen Aufrufer mehr, und eine private Funktion ohne Aufrufer ist eine **neue**
`dead_code`-Warnung. Sie ist deshalb in derselben Änderung entfernt worden.

**Das ist ausdrücklich nicht das „Aufgehen von `work_rects` in `visible_bound`",
das der Auftrag ausgeschlossen hat** — es ist das Entfernen einer Funktion, die
diese Änderung verwaist hat. Verhaltensfrei: null Aufrufer vorher wie nachher.
Gemeldet, nicht stillschweigend erledigt.

### 5. Vier Doc-Korrekturen ohne Codewirkung

- Lesertabelle an `DROP_SHADOW_SIZE` (`:267`): der `WM_REDRAW`-Eintrag nennt
  jetzt `w_union` über `border_pieces` statt `visible_bound` über `work_rects`.
- `top_window`s Kopfkommentar: „`pt ∩ WS_WORK ∩ union(rlist)` … its whole work
  area" → `pt ∩ union(w_rlist)`, also die ganze `WS_TRUE`-Fläche auf den Schirm
  geschnitten.
- `broadcast_redraw`: „the part of its now-visible area" → „the part of the
  exposed damage region the window owns".
- `subtract` (`:1326`) und `compute_rlist` (`:1392-1394`): die Verweise auf
  `visible_bound`/`work_rects` zeigen auf `w_union` und halten fest, dass **kein**
  Leser der Liste außer `w_owns` schneidet.

### Was bewusst unverändert blieb

`border_pieces`, `subtract`, `put`, `RCAP`, `walk_rects`, `draw_frame_pieces`,
`draw_frame`, `is_broken`, `ws_true`, `plus_shadow`, `intersect_grect`,
`union_grect`, `rc_intersect`, `compute_rlist`, `Owns`, `rlist_owns`,
`wind_get`, `close_window`, `redraw_dialog_area`, `wind_set(WF_CURRXYWH)`,
`send_redraw` — alle nur gelesen. `redraw_clip` (der Zeichenpfad) ist nicht
angefasst; diese Stufe hat **keine** Pixelwirkung, nur eine Nachrichtenwirkung.

## EmuTOS-Beleg

| Aussage | Quelle |
|---|---|
| `w_redraw` in voller Länge, drei geschachtelte Prüfungen | `aes/gemwmlib.c:886-907` |
| `rc_intersect` überschreibt den **zweiten** Parameter | `util/rectfunc.c:61-78` |
| `w_union` läuft die Liste ab und liefert FALSE bei leerer Liste | `aes/gemwmlib.c:869-884` |
| `w_rlist` enthält die **rohen** `WS_TRUE`-Reste | `aes/gemwrect.c:190,203` |
| der Work-Schnitt gehört zu `w_owns`, nicht zu `w_redraw` | `aes/gemwmlib.c:1230-1240,1556-1561` |
| `w_update` schneidet das Schadensrechteck auf `gl_rfull` | `aes/gemwmlib.c:1010` |
| `w_update` ruft `w_redraw` je Fenster in seiner Schleife | `aes/gemwmlib.c:1024-1030` |
| Top-Zweig: `c` ist `WS_CURR`, dann `+ DROP_SHADOW_SIZE` | `aes/gemwmlib.c:1069-1070,1213-1215` |
| die Vereinigung mit `gl_wtop`s `WS_CURR` beim Top-Fall | `aes/gemwmlib.c:1194-1211` |
| `fm_dial(FMD_FINISH)` = `w_drawdesk(pt)` + `w_update(DESKWH, pt, …)` | `aes/gemfmlib.c:375-379` |
| der Besitzer schneidet sein Zeichnen selbst über FIRST/NEXT | `desk/desksupp.c:223-229` |

## T1 — rot geworden, wie vorhergesagt

`tos-api/tests/aes_window_content.rs`, `clicking_a_window_never_erases_its_content`.
Der Test ist beim ersten Lauf **nach** der Codeänderung genau an der von A23
benannten Stelle gebrochen, und zwar mit **genau** dem dort hergeleiteten Wert:

    thread 'clicking_a_window_never_erases_its_content' panicked at
    tos-api/tests/aes_window_content.rs:935:5:
    assertion `left == right` failed: w_redraw sends `pt ∩ WS_WORK ∩ union(rlist)`
    = A's whole work area, now that A is on top (aes/gemwmlib.c:887-907,1004-1048)
      left: [20, 1, 60, 60, 342, 252]
     right: [20, 1, 61, 102, 316, 207]

Das ist **(60, 60, 342, 252)** — Zeichen für Zeichen der Wert, den A23 Abschnitt
6 und Zeile T1 in 7.1 aus `gemwmlib.c:887-907` für `RA = (60,60,340,250)` bei
`s = 2` vorhergesagt haben. Die Umkehrprobe von A23 hat damit über alle sechs
Stufen gehalten: T1 blieb in den Stufen 1 bis 5 grün und wird von genau dieser
Stufe rot.

### Wie das neue Soll abgeleitet wurde

**Aus der Quelle, nicht aus dem beobachteten Wert.** Die Kette, im Test als
Kommentar mitgeschrieben:

1. `wm_set(WF_TOP)` → `wm_mktop` → `draw_change`, Top-Zweig
   (`:1676-1680,1605-1615,1112-1136`). `c` = `WS_CURR(A)` (`:1069-1070`); die
   Vereinigung bei `:1198-1201` ist ein Nulleffekt (`gl_wtop` **ist** A); `:1213-1215`
   addiert den Schatten. Schaden = `curr(A) + s`.
2. `w_update` schneidet auf `gl_rfull` (`:1010`) und ruft `w_redraw` einmal für
   das getoppte Fenster (`:1024-1030`).
3. `w_redraw`: `:896` ist das Tor, `:899` überschreibt `d` mit der Hülle der
   **rohen** Liste, gesendet wird `d ∩ t`.
4. A ist oben und unverdeckt, seine Liste ist das eine Rechteck `WS_TRUE(A)`,
   die Hülle ist dieses Rechteck.

⇒ `WS_TRUE(A) ∩ gl_rfull`.

Im Test steht das **als Beziehung**, nicht als Zahl:

```rust
let curr_a = wind_get_rect(&mut rig.pb, ha, WF_CXYWH);
let true_a = (curr_a.0, curr_a.1, curr_a.2 + S, curr_a.3 + S);
let full   = wind_get_rect(&mut rig.pb, 0, WF_WXYWH);   // gl_rfull
let want   = overlap(true_a, full).expect("A must be on screen");
```

`S` ist `tos_api::aes::window::DROP_SHADOW_SIZE`, beide Geometrien werden zur
Laufzeit zurückgelesen, `gl_rfull` kommt aus der Arbeitsfläche des
Desktopfensters. Dazu eine Vorprüfung `assert_ne!(want, work_a)`, damit die
Behauptung nicht bei einem Schattenwert von 0 leer würde.

Nachgefahren: mit `DROP_SHADOW_SIZE = 4` statt `2` bleibt T1 grün (die Konstante
wurde dafür kurz geändert und aus einer vorher abgelegten Kopie zurückgeschrieben;
`diff` gegen die Kopie: identisch, kein `git`).

Die Pixelhälfte des Tests (T2, `:823-824` in A23s Zählung) ist **nicht**
angefasst worden; sie war seit Stufe 4 grün und ist es geblieben.

## Der neue Test

`tos-api/tests/aes_desk_background.rs:1791-1986`, neuer Abschnitt „11. the
rectangle `w_redraw` puts into the WM_REDRAW message" (196 Zeilen, reiner
Anhang; die elf vorhandenen Tests der Datei wurden nicht berührt). Unterbringung
wie bei FIX-01, FIX-02, FIX-04 und FIX-05 in dieser Datei, weil sie den Aufbau
fertig hat (`SERIAL`, `rig()`/`fresh()`, `open_window`, `wind_get_rect`,
`form_dial`, die Konstante `S`) — und weil `aes_window_content.rs` nur **einen**
Test enthält und sein `setup()` den Host einmalig installiert, ein zweiter Test
dort also den Aufbau umbauen würde.

`the_redraw_message_spans_the_raw_owner_rectangle_not_the_work_clipped_one`
(`:1857`) — **ein** Test, der beide Hälften des Unterschieds in **einem** Lauf
trennt.

Aufbau: ein Fenster `MSG_UNDER = (40,100,400,300)` und darüber ein zweites, das
`WS_TRUE(unten)` oben, links und rechts **überragt** und dessen eigene
`WS_TRUE`-Unterkante auf der **Mitte der Arbeitsfläche** des unteren Fensters
liegt. Damit sind `have_piece[TOP]`, `[LEFT]` und `[RIGHT]` falsch und `brkrct`
lässt genau **ein** Stück übrig, das BOTTOM-Stück (`aes/gemwrect.c:107-125`).
Das ist der Punkt: dann ist

- die Hülle **nicht** das ganze `WS_TRUE`-Rechteck des unteren Fensters — eine
  Umsetzung, die `WS_TRUE ∩ Schaden` sendete, fällt hier durch;
- das Stück ragt links, rechts und unten über die Arbeitsfläche hinaus — eine
  Umsetzung, die die work-geschnittene Hülle sendete, fällt ebenfalls durch.

Beide Geometrien werden über `wind_get(WF_CXYWH)`/`(WF_WXYWH)` **zurückgelesen**,
das überdeckende Fenster wird **relativ** dazu platziert, und die Vorbedingungen
werden in `brkrct`s eigenen Begriffen geprüft: die vier `have_piece[]`-Vergleiche
auf den beiden `WS_TRUE`-Rechtecken (`:190` der Brecher, `:203` unseres).

Das Schadensrechteck kommt aus `form_dial(FMD_FINISH)` mit dem ganzen Schirm als
„großem" Rechteck — EmuTOS `fm_dial` (`aes/gemfmlib.c:375-379`) ist
`w_drawdesk(pt)` + `w_update(DESKWH, pt, DESKWH, FALSE)`, also genau der
`w_update`→`w_redraw`-Fan-out, über ein vom Aufrufer bestimmtes Rechteck. Damit
ist das Ergebnis die Hülle selbst und kein Artefakt des Schadens.

Die Erwartung ist die `mkpiece`-BOTTOM-Formel (`aes/gemwrect.c:71-74,89-92`)
über den zurückgelesenen Werten, mit `gl_rfull` geschnitten:

```rust
let piece = (true_u.0, true_o.1 + true_o.3, true_u.2,
             (true_u.1 + true_u.3) - (true_o.1 + true_o.3));
let want  = overlap(piece, full).expect(...);
```

Vor der eigentlichen Behauptung stehen drei Vorprüfungen, die den Test
selbstprüfend machen: die Hülle muss vom ganzen `WS_TRUE` abweichen, sie muss die
Arbeitsfläche treffen (sonst schließt das Tor bei `:896` und es kommt gar keine
Nachricht) und sie muss über die Arbeitsfläche hinausragen.

**Keine eingefrorene Zahl.** Schattengröße als `S`, beide Geometrien aus
demselben Lauf, `gl_rfull` zur Laufzeit gelesen, das überdeckende Fenster relativ
platziert. Nachgefahren mit `DROP_SHADOW_SIZE = 4`: grün.

Zwei kleine Hilfen kamen mit dem Test in die Datei: `next_mesag` (ein
`evnt_multi` mit zusätzlich armiertem `MU_TIMER`, damit ein leerer Puffer nicht
blockiert) und `overlap` (Rechteckschnitt, den diese Datei noch nicht hatte).
Die Nachrichtenschlange wird vor der Messung geleert, weil gleichfenstrige
Redraws in der FIFO per Rechteckvereinigung verschmelzen (`doq`,
`aes/gemqueue.c:45-56`) und ein Rest sonst mit hineingerechnet würde.

### Beleg, dass der neue Test den Defekt fängt

Kontrolliert nachgefahren: `w_union` von Hand auf die **alte** Regel
zurückgeschrieben (Hülle über die work-geschnittenen Stücke), Datei danach aus
einer vorher abgelegten Kopie zurückgeholt (kein `git`). Der Test war rot, mit
der Diagnose im Klartext:

    assertion `left == right` failed: the WM_REDRAW rectangle is
    `damage ∩ w_union(w_rlist)` = (40, 261, 402, 141) — the hull of the RAW
    owner list, taken before any work clip (aes/gemwmlib.c:899-905, 869-884;
    aes/gemwrect.c:190,203). The work-clipped hull would be
    Some((41, 261, 376, 138)) and the window's whole WS_TRUE rect
    (40, 100, 402, 302); neither is what EmuTOS sends. …
      left: (41, 261, 376, 138)
     right: (40, 261, 402, 141)

Die drei Rechtecke sind paarweise verschieden — der Test trennt tatsächlich alle
drei Kandidatenregeln, nicht nur zwei.

## Wirkung auf die anderen Leser — geprüft, nicht angenommen

- **Gesendete Rechtecke werden größer, nie kleiner** (sie sind nicht mehr auf
  `WS_WORK` beschnitten). Das ist EmuTOS-Verhalten; der Besitzer schneidet sein
  Zeichnen selbst über `WF_FIRSTXYWH`/`WF_NEXTXYWH`.
- **Kernel-Desktop:** `kernel/src/desk/redraw.rs:79-92`
  (`do_wredraw_nocursor`) läuft die AES-Besitzerliste ab und zeichnet je Stück
  nur in `rc_intersect(clip, t)` — die Stücke sind work-geschnitten
  (`rlist_owns`), also kann ein größeres Schadensrechteck **nicht** über den
  Fensterrahmen malen. Genau die Absicherung, die EmuTOS' `do_wredraw`
  (`desk/desksupp.c:223-229`) hat. Nur gelesen, nicht geändert.
- **Zusätzliche Nachrichten sind möglich**, wo vorher keine kam: ein Fenster,
  dessen Arbeitsfläche vollständig verdeckt ist, dessen Randstücke aber noch
  leben, bekommt jetzt eine Nachricht, sofern der Schaden auch seine
  Arbeitsfläche berührt (Tor bei `:896`). Der Besitzer zeichnet daraufhin nichts,
  weil FIRST/NEXT ihm kein sichtbares Stück liefert. Das ist EmuTOS.
- **Zeichenpfad:** unberührt. `redraw_clip`/`compose`/`draw_frame_pieces`/
  `walk_rects` sind nicht angefasst; diese Stufe verändert **kein** Pixel
  unmittelbar.
- **`broadcast_redraw`-Aufrufer** (`close_window`, `wind_set(WF_CURRXYWH)`,
  `redraw_dialog_area`): unverändert aufgerufen; sie bekommen den `gl_rfull`-
  Schnitt aus `w_update:1010`, den sie vorher nicht hatten. Das kann ein
  gesendetes Rechteck nur verkleinern, und Fenster liegen ohnehin unter der
  Menüzeile.
- **`send_redraw` (ohne `_in`)** im Move-Pfad ist nicht angefasst; er hat seine
  eigene, dokumentierte Abweichung.

## Tatsächlich gesehene Zahlen

Alle Läufe am 2026-09-11 im gemeinsamen Arbeitsbaum (`70caee2` plus die fremden
uncommitteten Änderungen sowie FIX-01…FIX-05), Host macOS/arm64.

`cargo test -p tos-api --tests`, Ausgangsstand nach Stufe 5 — selbst
nachgefahren, bevor eine Zeile geändert wurde:

    13 binaries, 434 passed, 0 failed, 0 ignored

Nach Fix und Test, derselbe Befehl, tatsächlich gesehen:

    13 binaries, 435 passed, 0 failed, 0 ignored

Aufschlüsselung: `tos_api` (lib) 10 · `aes_appl_init` 1 · `aes_button_dispatch`
1 · `aes_desk_background` **12** (vorher 11) · `aes_dialog_undraw` 2 ·
`aes_object_font` 6 · `aes_reference` 177 · `aes_window_content` 1 ·
`aes_window_drag` 4 · `gemdos_drive_login` 2 · `gemdos_fs_paths` 2 ·
`vdi_raster_reference` 59 · `vdi_reference` 158.

Die eine zusätzliche Bestehende ist der neue Test. **Genau ein vorhandener Test
wurde rot: T1** — die Vorhersage aus A23 8.2, Zeile „6". Er ist mit dem neu aus
der Quelle abgeleiteten Soll wieder grün.

Die Wächter W1 bis W7 aus A23 7.2 liegen sämtlich in diesem Lauf und sind grün:
die 177 Referenzfälle in `aes_reference` (W1, W2, W3), `aes_object_font` +
`aes_dialog_undraw` (W4), die drei Ziehtests in `aes_window_drag` (W5) und —
für diese Stufe der wichtigste — **W7**, `moving_a_window_off_another_redraws_
the_one_underneath` (`aes_window_drag.rs:1146-1198`), der genau prüft, dass die
`WM_REDRAW`-Nachricht an das untere Fenster nicht verloren geht. Er ist grün
geblieben, wie A23 es für diese Stufe vorhergesagt hat („das korrigierte
Rechteck ist **größer**, nicht kleiner"). Dazu W6, die drei Hintergrundtests in
`aes_desk_background`.

Build, beide Boards, je mit erzwungener Neuübersetzung (`touch` auf die geänderte
Quelle, weil ein zweiter Board-Build sonst fälschlich „up to date" melden kann):

    cargo xtask build --board qemu-virt
      -> Compiling tos-api, Compiling kernel
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
    cargo xtask build --board rpi4
      -> Compiling tos-api, Compiling kernel
      -> tos-api (lib) 64 warnings, kernel (lib) 94 warnings, 0 errors
      -> objcopy -> kernel8.img (gitignoriertes Bauartefakt, keine Hardwareaktion)

Warnungszahlen identisch zum Stand nach FIX-05.

Clippy, `cargo clippy -p tos-api --all-targets`:

| Ziel | vorher | nachher |
|---|---|---|
| `tos-api` (lib) | 81 | 81 |
| `tos-api` (lib test) | 82 | 82 |
| `tos-api` (test `vdi_raster_reference`) | 2 | 2 |
| `tos-api` (test `vdi_reference`) | 3 | 3 |
| `tos-api` (test `aes_desk_background`) | — | — |

**Delta: 0.** Unterwegs hatte der neue Testcode **eine** eigene Warnung erzeugt
(`clippy::unnecessary_lazy_evaluations`, `.then(|| *buf)`); sie ist behoben
(`then_some`), nicht stehengelassen. Belegt statt behauptet:
`cargo clippy -p tos-api --lib --message-format=short` nennt für `window.rs`
weiterhin **genau elf** Fundstellen — zehn `private_interfaces` an je einer
öffentlichen `wind_*`-Funktion und ein `unused_assignments` an `top` in
`draw_frame`; alle elf sind vorbestehend und liegen außerhalb der geänderten
Bereiche (die Zeilennummern verschieben sich nur um den Umfang der Änderung).
Kein clippy-eigener Lint an den geänderten Funktionen.

Rauchprobe QEMU (`cargo xtask run --board qemu-virt`, nach 45 s beendet):
sauberer Hochlauf über 1542 Zeilen bis `[shell] sh_ldapp: starting NEWDESK` und
`[desk] desktop up (0 drives)`; **null** Treffer für `panic|exception|unhandled`.
Das ist eine Rauchprobe, **kein** Abnahmelauf — der gehört dem Tester.

## Textvorschlag für docs/STATUS.md

Die Datei ist fremd modifiziert und wurde **nicht** angefasst. Vorschlag zum
Eintragen beim Commit, unter *Recent Decisions*:

> **2026-09-11 — `WM_REDRAW`: das Rechteck ist die Hülle der rohen
> Besitzerliste** (Stufe 6 der Fix-Reihenfolge aus `TASK/A23.md` 8.2; schließt
> den unabhängigen Befund aus A23 Abschnitt 6). `w_redraw` schickte
> `Schaden ∩ Hülle(work-geschnittene Stücke)`. EmuTOS macht drei getrennte
> Dinge (`aes/gemwmlib.c:886-907`): `rc_intersect(&t,&d)` schreibt in **`d`**
> (`util/rectfunc.c:61-78`) und dient nur als Nichtleer-Prüfung, ob der Schaden
> die Arbeitsfläche überhaupt berührt; `w_union(w_rlist,&d)` (`:869-884`)
> **überschreibt** `d` mit der Hülle der **rohen** `WS_TRUE`-Stücke
> (`aes/gemwrect.c:190,203`); gesendet wird `d ∩ t`. Das Rechteck reicht damit
> über Rahmen und Schlagschatten hinaus — der Besitzer schneidet sein Zeichnen
> selbst über `WF_FIRSTXYWH`/`WF_NEXTXYWH`, und nur dort schneidet EmuTOS auf
> die Arbeitsfläche (`w_owns`). Mitkorrigiert, weil dieselbe Beziehung: das
> Schadensrechteck beim Topping ist `curr + DROP_SHADOW_SIZE`, auf `gl_rfull`
> geschnitten (`draw_change` `:1213-1215`, `w_update` `:1010`), nicht das nackte
> `curr`. `tos-api/src/aes/window.rs`: `visible_bound` → `w_union` über
> `border_pieces` mit `Option`-Rückgabe für die leere Liste, `send_redraw_in`
> als wortgetreues `w_redraw`, `top_window` mit `plus_shadow`; der verwaiste
> Helfer `work_rects` ist entfallen. Der Kommentar an
> `visible_bound`/`send_redraw_in` hatte EmuTOS falsch beschrieben und ist
> berichtigt. Neuer Regressionstest
> `aes_desk_background::the_redraw_message_spans_the_raw_owner_rectangle_not_the_work_clipped_one`;
> `aes_window_content::clicking_a_window_never_erases_its_content` wurde wie in
> A23 vorhergesagt rot und hat sein Soll neu aus der Quelle bekommen
> (`WS_TRUE(A) ∩ gl_rfull` statt `WORK(A)`). Suite `cargo test -p tos-api
> --tests`: 435 passed, 0 failed (vorher 434).

## Braucht dieser Schritt eine reale Gegenmessung?

**Es gibt dafür kein Messorakel — und es kann heute keins geben.**

A23 8.3 sagt für Stufe 6: „den ganzen Fix aus der Quelle" / Gegenmessung „—
(keine Pixelwirkung, nur die Nachricht)" / vorhandenes Orakel „keins; kein Fall
der Tabelle liest `WM_REDRAW`". Selbst nachgeprüft:

- `tosdump/cases/aes-r2b-rlist.tsv` (eingefroren, CRC-gebunden) und die übrigen
  Falltabellen rufen AES-Funktionen auf und vergleichen `int_out`. Eine
  `WM_REDRAW`-Nachricht steht **nicht** in `int_out`; sie landet in der
  Nachrichtenschlange des Besitzers und ist nur über `evnt_mesag`/`evnt_multi`
  lesbar. Kein vorhandener Fall tut das.
- Auch mittelbar gibt es kein Orakel: die Stufe verändert kein Pixel. Ein
  Screendump vor und nach der Änderung ist identisch, solange die Anwendung ihre
  eigene Clip-Disziplin einhält — was der Kernel-Desktop tut.

**Wäre ein neuer Messfall möglich?** Ja, mit einer Einschränkung, und er ist
**nicht** angelegt:

- **Was er tun müsste:** zwei Fenster öffnen (das obere überdeckt TOP, LEFT und
  RIGHT des unteren, seine Unterkante mitten in dessen Arbeitsfläche),
  anschließend eine Aktion auslösen, die den Fan-out anstößt — `form_dial(3, …)`
  über den ganzen Schirm ist die einfachste —, und **danach** `evnt_multi` mit
  `MU_MESAG` **und** `MU_TIMER` aufrufen und die acht Wörter des Puffers als
  Ergebnis ablegen. Erwartet: `msg[0] = 20`, `msg[3]` = Handle des unteren
  Fensters, `msg[4..8]` = das BOTTOM-Stück nach `mkpiece`, auf `gl_rfull`
  geschnitten.
- **Wohin:** eine **neue** Falltabelle unter `tosdump/cases/` — nicht in
  `aes-r2b-rlist.tsv` (eingefroren, CRC-gebunden) —, mit eigenem Laufverzeichnis
  und einer neuen TT-Referenz `EMUTOS14-AES-*.BIN` neben den vorhandenen.
  Sinnvoll gebündelt mit den offenen Messlücken derselben Familie: A22 (drei und
  mehr überlappende Fenster), A24 (verschachtelte FIRST/NEXT) und dem P2-Fall aus
  FIX-05.
- **Was der Dumper dafür braucht und heute nicht hat:** einen Fall, der eine
  **Nachricht** statt eines `int_out` aufzeichnet, und einen **blockierungsfreien**
  Weg, sie abzuholen. `evnt_mesag` blockiert auf leerer Schlange — auf echter
  Hardware wäre das ein Hänger, wenn die Nachricht ausbleibt. Deshalb der
  armierte Timer. Das ist ein Messdesign-Task nach der Regel aus `TASK/README.md`
  („Zustandsreset, Rückbau, Referenzverfügbarkeit und blockierende Aufrufe klären,
  bevor ein Hardwarefall gebaut wird"), kein Anhängsel dieser Stufe.
- **Was er nicht kann:** die Zahlen sind geräteabhängig (Fontmetrik →
  Arbeitsfläche, `DROP_SHADOW_SIZE`, `gl_rfull`). Verglichen werden muss die
  **Beziehung** — `mkpiece`-Formel über den zurückgelesenen Geometrien —, so wie
  der Hosttest es tut, nicht der nackte Zahlenwert vom TT.

Zusätzlich bleibt die Abnahme aus A23 8.3 für die **Zeichenhälfte** von Stufe 4
offen (Schatten auf echter Hardware); die berührt diese Stufe nicht.

## Was offen bleibt

- Die reale Gegenmessung dieser Nachrichtenbeziehung, siehe oben. Blockiert
  diesen Fix nicht — A23 stuft Stufe 6 selbst als quellenbelegt ein.
- Die Spezifikation `docs/api/aes/window-graphics.md` beschreibt das
  `WM_REDRAW`-Rechteck bisher nicht in dieser Schärfe. **Nicht geändert** —
  Spec-Korrekturen liegen bei A18/A26, und der Auftrag war auf den Code
  begrenzt.
- Die Abnahme (Boot, `svc`-Selbsttests, Screendumps, `fsck_msdos`, beide Boards)
  gehört dem Tester und ist hier bewusst **nicht** vorweggenommen.

## Zusätzliche Befunde — nur gemeldet, nicht angefasst, kein Task angelegt

1. **`work_rects` ist mit dieser Stufe verwaist und wurde deshalb entfernt.**
   FIX-05 hatte den einen verbliebenen Leser (`visible_bound`) ausdrücklich
   vermerkt. Das Entfernen ist verhaltensfrei (null Aufrufer) und war nötig, um
   keine neue `dead_code`-Warnung zu hinterlassen. Falls der Lead die Funktion
   erhalten will, ist die Wiederherstellung ein Einfüge-Diff; sie steht wörtlich
   oben unter „Was geändert wurde / 4.".
2. **`send_redraw` im Move-Pfad** (`wind_set(WF_CURRXYWH)`,
   `window.rs`, „This window's content") sendet weiterhin
   `work` statt des Rechtecks, das `w_redraw` daraus machen würde — mit
   dokumentierter Begründung (EmuTOS blittet das Topfenster in `w_move` und
   überspringt es dann mit `moved = TRUE`, `:944-987`). Nach dieser Stufe ist
   das die einzige verbliebene Stelle, an der ein `WM_REDRAW`-Rechteck **nicht**
   durch `send_redraw_in` läuft. Nicht angefasst; ob die Begründung nach den
   Stufen 4 und 6 noch trägt, ist eine eigene Frage.
3. **`aes_window_content.rs` kann keinen zweiten Test aufnehmen**, ohne seinen
   Aufbau umzubauen: `setup()` installiert den Host bei jedem Aufruf und es gibt
   keinen `SERIAL`-Riegel wie in `aes_desk_background.rs`. Solange die Datei
   genau einen Test hat, ist das harmlos. Nicht angefasst.
