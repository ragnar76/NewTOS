# A23 — Quittung

**Status: DONE** · 2026-09-10 · Kompensationsanalyse vor dem Fix-Durchgang

## Auftrag

Vor dem Developer-Durchgang für jeden bestätigten Defekt aus A13, A14 und A15
feststellen, ob eine zweite Stelle ihn ausgleicht: alle Erzeuger und Verbraucher
der geänderten Größe auflisten, für jede die EmuTOS-Beziehung belegen, die
vorhandenen Tests benennen, die den heutigen **falschen** Wert kodieren, und je
Defekt entscheiden zwischen lokalem Fix und Übernahme der ganzen Beziehung.
Keine Reparatur. Dieser Lauf hat den Zwischenstand eines abgebrochenen
Vorgängers **vervollständigt**, nicht ersetzt.

## Was ich gemacht habe

1. `TASK/A23.md` vollständig gelesen. Die vorhandenen Abschnitte 1–6 des
   Vorgängers wurden **nicht** umgeschrieben, nicht gekürzt und nicht
   umsortiert. Sie sind in der Abschlussprüfung stichprobenartig gegen die
   Quelle nachgeprüft worden (`w_cpwalk` `aes/gemwmlib.c:363-379`,
   `mod.rs:263`, `object.rs`-Schattenblock, `mkpiece` `aes/gemwrect.c:64-98`,
   `everyobj` `aes/gemobjop.c:76-120`) — kein Fehler gefunden, der eine
   Korrektur an 1–6 nötig gemacht hätte. Die einzige ausgewiesene Korrektur
   steht in **8.1** und betrifft eine fehlende Abhängigkeit, nicht eine falsche
   Aussage.
2. Den gepinnten Referenzstand selbst geprüft: `references/emutos` @
   **`1a7484aba1bfba23e2c0899e243b6d8177fe41df`**. Jede Behauptung mit
   Datei:Zeile belegt; alle im Text genannten EmuTOS-Zeilennummern **einzeln
   nachgeschlagen** und dabei sieben Zitate korrigiert (`w_update` Bildschirmclip
   1010 statt 1013, `ob_sst` th-Byte `gemobjop.c:54-57` statt 59-61,
   Schattenblock `gemoblib.c:889-901` statt 892-899, `gl_waspec`/W_BOX
   `gemwmlib.c:84-86` statt 83, `w_move` ab 938, `wm_mktop` ab 1605,
   brechendes Rechteck `gemwrect.c:190` statt 189).
3. **Ausgangslage gemessen, bevor irgendetwas angefasst wurde:**
   `cargo test -p tos-api --tests` → 13 Zielbinaries, alle grün, 0 failed,
   0 ignored. Ohne diese Messung wäre „wird rot" nicht von „ist schon rot"
   unterscheidbar.
4. Den Suchraum **vollständig** durchgegangen, nicht nur den vom Auftrag
   benannten Kandidaten: alle acht dauerhaften Testbinaries in
   `tos-api/tests/` samt der drei generierten Modulen unter
   `tos-api/tests/aes_reference/`, und alle Unit-Tests in `tos-api/src/aes/`
   (es gibt genau zwei, `init.rs:434` und `pd.rs:317`, beide ohne Bezug zur
   Fenstergeometrie). Erfasst wurden sämtliche Pixelsonden, sämtliche
   `wind_get`-Aufrufe und sämtliche `WM_REDRAW`-Auswertungen.
5. Die Feldabdeckung der generierten Referenzfälle **numerisch** aus den
   `intin`-Werten abgelesen statt aus den Kommentaren: `cases_r1.rs` deckt die
   `wind_get`-Felder 0, 1, 4, 5, 6, 7, 8, 9, 10, 13, 14, 15, 16, 17, 20, 21 ab,
   `cases_r2_wind.rs` die Felder 5, 6, 7, 20 — **11 und 12 fehlen in beiden**.
   Damit ist „kein vorhandener Test kodiert Reihenfolge, Cursor oder P1/P2-x/y"
   belegt und nicht mehr nur behauptet.
6. Die `DELTAS`-Tabelle des Referenzharness (`aes_reference/harness.rs:996-1113`)
   geprüft: ausschließlich `WF_OWNER`-Einträge. Kein vorhandenes Delta erklärt
   eine A13/A14/A15-Abweichung für grün.
7. **Den benannten Kandidaten selbst nachgeprüft und gefahren.** Über einen
   temporären Testaufsatz (`tos-api/tests/zz_audit_a23_tmp.rs`, Kopie von
   `aes_window_content.rs` mit zusätzlichen `eprintln!`, danach gelöscht) den
   heutigen Wert gemessen: das `WM_REDRAW`-Rechteck nach `wind_set(A, WF_TOP)`
   ist `[20, h=1, 61, 102, 316, 207]` = die Arbeitsfläche von A. Aus der Quelle
   abgeleitet, dass EmuTOS dort `TRUE(A) ∩ gl_rfull` = `(60, 60, 340+s, 250+s)`
   liefert, mit TT-`s = 2` also `(60, 60, 342, 252)`.
8. Dabei **eine zweite, vom Vorgänger nicht zugeordnete Rotfärbung im selben
   Test gefunden**: die Pixelassertion `:823-824`. Die Schattengeometrie aus
   `ob_draw` gerechnet und den Schnitt mit dem geprüften Bereich mechanisch
   ausgerechnet statt geschätzt — 180 px liegen im rechten Schattenstreifen.
   Die beiden Assertionen desselben Tests trennen damit die Zeichenhälfte
   (`:823`) von der Nachrichtenhälfte (`:828`) des Fixes.
9. Beim Nachrechnen einen **eigenen Fehler** gefunden und korrigiert: die Breite
   des unteren Schattenstreifens ist `w - th`, nicht `w + th`
   (`aes/gemoblib.c:897-898` auf dem von `gr_inside` eingerückten Rechteck).
   Erst nach der Korrektur stimmt die Zahl: 180 px, nicht 184.
10. Die Schattenempfindlichkeit der übrigen Fenstertests **gerechnet und
    gefahren** (`s = 2`, `4`, `6`): `aes_desk_background.rs:840` hat nur 5 px
    Abstand zur Unterkante des Fensters und wird bei `s ≥ 6` zum zweiten
    Halbfix-Detektor; die `uncovered_strips` in `aes_window_content.rs:810-821`
    sind in allen drei Profilen unberührt.
11. Sieben Wächter benannt (Tests, die beim Fix **nicht** rot werden dürfen),
    darunter die gemessenen TT-Fälle 175–178 für die Handleprüfung und
    `aes_object_font.rs`/`aes_dialog_undraw.rs` als Kanarienvogel gegen ein
    versehentliches Mitändern der Objekt- und Dialogmetrik.
12. Sieben Deckungslücken benannt, in denen **gar kein** Test steht — inklusive
    `save_windows`/`restore_windows`, das der F-A14-1-Fix anfasst und das heute
    von keinem Test gefahren wird.
13. Die Fix-Reihenfolge in sechs Stufen aufgestellt, jede Abhängigkeit begründet,
    und je Stufe getrennt ausgewiesen, was ein Hosttest leistet und was eine
    reale Gegenmessung braucht.
14. Die Ausgangslage nach dem Löschen des Aufsatzes erneut gemessen: wieder
    13 grüne Zielbinaries.

## Artefakte

- **`TASK/A23.md`**, neu: **Abschnitt 7** (Testinventar: Ausgangslage und
  Suchraum, die Tests die rot werden, die Wächter, `s` als Geräteparameter, die
  latent schattenabhängigen Tests, die Deckungslücken) und **Abschnitt 8**
  (Korrektur an A15, die sechsstufige Reihenfolge, Hosttest gegen
  Gegenmessung, was nicht in denselben Commit gehört). Der Vermerk
  „Zwischenstand gesichert … Abschlussprüfung folgt" am Kopf des
  Analyseabschnitts ist durch eine Abschlusszeile ersetzt. Die Abschnitte 1–6
  sind unverändert.
- **`measurements/measurement-2026-09-09/diff/14-a23-compensation-20260910.txt`**, nur
  **angehängt** („ANHANG 2026-09-10, Abschlusslauf A23"): der Ausgangslauf, die
  vollständige Ausgabe des temporären Aufsatzes für `s = 2/4/6`, die Ableitung
  der Schattenstreifen aus der Quelle, die Zuordnung der beiden Assertionen zu
  den beiden Fixhälften und die `mkpiece`-Nachprüfung. Nichts oberhalb der
  Anhangsmarke verändert.
- **`TASK/README.md`**: nur die Statusspalte der A23-Zeile. Zeile 5 bis 20 nicht
  angefasst.

Nicht angefasst: `TASK/A25*`, `TASK/evidence/**`, `tosdump/cases/*.tsv`, alle
`*.BIN`, `*.LOG`, `MESSRUN.TXT`, `RUNINFO.TXT`, die Diffs 00–13, und der fremde
uncommittete Code (`tos-api/src/aes/pd.rs`, `tos-api/tests/aes_appl_init.rs`,
`docs/api/aes/application.md`).

## Ausgeführte Prüfung

    $ cargo test -p tos-api --tests            # vor jeder Aenderung
      -> 13 Zielbinaries, alle "test result: ok", 0 failed, 0 ignored

    $ cargo test -p tos-api --test zz_audit_a23_tmp -- --nocapture
      A23-PROBE curr_a=(60, 60, 340, 250) work_a=(61, 102, 316, 207)
      A23-PROBE WM_REDRAW msg = [20, h=1, 61, 102, 316, 207]
      A23-PROBE s=2: right_band=(400,61,2,251) bottom_band=(61,310,339,2)
      A23-PROBE s=2: line-823 probe=(400,222,157,207)
                     x right_band -> ((400,222,2,90),180), x bottom_band -> None
      A23-PROBE s=2: line-814 strip0/strip1 -> None / None
      test result: ok. 1 passed; 0 failed.

    $ rm tos-api/tests/zz_audit_a23_tmp.rs
    $ cargo test -p tos-api --tests            # nach dem Rueckbau
      -> wieder 13 gruene Zielbinaries

    $ cd references/emutos && git rev-parse HEAD
      -> 1a7484aba1bfba23e2c0899e243b6d8177fe41df

    Feldabdeckung der generierten Referenzfaelle numerisch aus den intin-Werten
    abgelesen (Faelle 155-178 in cases_r1.rs): 0,1,4,5,6,7,8,9,10,13,14,15,16,
    17,20,21 — 11 und 12 fehlen.

## Ergebnis

**Zwei vorhandene Assertionen werden rot, beide im selben Test, und sie gehören
zu zwei verschiedenen Fixen:**

| | Datei:Zeile | Bricht durch | Heute | Neues Soll aus |
|---|---|---|---|---|
| T1 | `tos-api/tests/aes_window_content.rs:828-832` | den unabhängigen `w_redraw`-Befund — **nicht** durch D-A13-3 allein | `[20, h=1, 61, 102, 316, 207]` (die Arbeitsfläche von A) | `aes/gemwmlib.c:887-907` + `util/rectfunc.c:63-78` + `newrect` `aes/gemwrect.c:165-205` → `TRUE(A) ∩ gl_rfull` = `(60, 60, 340+s, 250+s)` |
| T2 | `tos-api/tests/aes_window_content.rs:823-824` | die **Zeichenhälfte** von D-A13-3 — die Geometriehälfte allein macht ihn nicht rot | „kein Pixel in `(400, 222, 157, 207)` verändert" | `W_ACTIVE[ROOT].ob_state = SHADOWED` `aes/gemwmlib.c:1284` + `ob_draw` `aes/gemoblib.c:889-901` + `th` aus `ob_spec` `gemwmlib.c:84-86`/`gemobjop.c:54-57`; 180 px liegen im rechten Schattenstreifen |

Für **D-A13-1/D-A14-2, D-A13-2, D-A14-1 und F-A15-1 wird kein vorhandener Test
rot** — belegt über die numerische Feldabdeckung, nicht behauptet. Das ist eine
Deckungslücke, kein Freibrief.

**Empfohlene Reihenfolge** (Begründung je Abhängigkeit in Abschnitt 8.2):
1. Handleprüfung im FIRST/NEXT-Arm (D-A13-1/D-A14-2) · 2. Zerlegungsreihenfolge
(D-A13-2) · 3. Liste und Zähler pro Fenster (D-A14-1) · 4. Schatten als
**ganze** Beziehung in **einem** Commit (D-A13-3) · 5. P1/P2-x/y (F-A15-1) ·
6. `w_redraw` aus der rohen Besitzerliste (unabhängiger Befund, eigener Auftrag
nötig).

**Korrektur an A15, ausgewiesen in 8.1:** A15s *Begründung* („gegenseitige
Aufhebung, Dispatcherumbau nötig") bleibt widerlegt — Abschnitt 6 hat recht.
A15s *Schlussfolgerung* („nicht vor den A13-Fixes") ist trotzdem richtig, aber
aus einem Grund, den bisher niemand aufgeschrieben hat: das P2-Soll ist das x/y
des letzten fehlgeschlagenen Schnitts, und das hängt an der Zerlegungsreihenfolge
(D-A13-2) **und** am Schatten, weil `mkpiece` die Ursprünge der RIGHT- und
BOTTOM-Stücke aus dem brechenden `WS_TRUE`-Rechteck rechnet
(`aes/gemwrect.c:64-98,190`). F-A15-1 kann mechanisch jederzeit landen; sein
Regressionstest darf seine Zahlen erst danach einfrieren — oder er führt
Reihenfolge und `s` als Parameter.

**Zur 1:1-Frage:** `s` ist ein Geräteparameter, die Beziehung ist es nicht. In
EmuTOS gilt `s = DROP_SHADOW_SIZE = 2*th(W_BOX) = 2` — zwei getrennte Konstanten,
die zueinander passen müssen. Ein Test, dessen Soll an der Schattengröße hängt,
führt `s` als Parameter. Ob das Pi-Profil `s = 2` behält, ist **offen** und
gehört in die Entscheidung des Zeichenfixes; skaliert `s`, verlieren die
eingefrorenen TT-Fälle 166/174/183/213 ihre Schiedsrichterrolle.

Kein Code geändert. Die vier Abnahmepunkte von A23 sind damit erfüllt:
Kompensationsanalyse je Defekt (Abschnitte 2–6), Einstufung je Defekt
(dito), Liste der rot werdenden Tests **vor** dem Fix (Abschnitt 7), kein Code
geändert.

## Offen

Nichts, was A23 selbst noch liefern müsste — die Analyse ist abgeschlossen.
Offen bleibt, was A23 ausdrücklich **an die Fixe weiterreicht**:

- **Der `s`-Wert für das Pi-Profil ist nicht entschieden.** Solange er nicht
  feststeht, kann kein schattenabhängiger Test seine Zahlen einfrieren, und es
  ist unentschieden, ob die vier TT-Fälle 166/174/183/213 nach dem Fix noch als
  Messorakel taugen. Gehört in den Zeichenfix (Stufe 4), nicht in A23.
- **Reale Gegenmessung offen** für die Zeichenhälfte des Schattens (kein Record
  enthält Pixel — Screendump bzw. Pi-Lauf nötig), für D-A14-1 (die eingefrorene
  Tabelle enthält keine verschachtelte Zwei-Fenster-Folge, A14/L1) und für
  FIRST/NEXT auf ungültigem Handle (kein TT-Record).
- **Sieben Deckungslücken** (Abschnitt 7.5), von denen eine — der
  `save_windows`/`restore_windows`-Pfad — **vor** Stufe 3 mit einem Test zu
  schließen ist, weil der Fix genau dort Speicher hinzufügt.
- **Unabhängiger Befund, gemeldet, kein Task von mir angelegt** (bereits vom
  Vorgänger gemeldet, hier nur bestätigt und für Abschnitt 7 verwendet):
  `w_redraw` bildet in EmuTOS `pt ∩ Hülle(rohe Besitzerliste)`; NewTOS bildet
  `damage ∩ Hülle(work-geschnittene Stücke)` (`window.rs:1188-1207`), und der
  dortige Kommentar behauptet Gleichheit. Er ist die Ursache von T1 und braucht
  einen eigenen Auftrag.
- **Nicht bewertet, außerhalb des Auftrags:** RCAP-Grenze (A21), Reihenfolge
  mehrerer brechender Fenster (A22), `global[]`-Abweichungen (A06–A09),
  Menüzeilenhöhe (A20).
