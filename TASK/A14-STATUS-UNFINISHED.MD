# A14 — Quittung

**Status: UNFINISHED** · 2026-09-10 · wind_get WF_NEXTXYWH: Iteratorfortschritt

## Auftrag

Nur NEXT nach initialisierendem FIRST auswerten: Reihenfolge, disjunkte
sichtbare Flächen und Ende. Bei Gleichheit die Messlücke ohne Codeänderung
schließen; bei belegtem Unterschied genau einen Fix und einen
Regressionstest benennen. A13, A15 und A16–A23 nicht anfassen.

## Was ich gemacht habe

1. Den Kopfkommentar der eingefrorenen Tabelle `tosdump/cases/aes-r2b-rlist.tsv`
   vollständig gelesen (Zeilen 1–109: die Iteratorregel „NEXT wird nie ohne
   FIRST davor gemessen", die Poison-Absicht, `w_owns` im Wortlaut) und die
   Fall-IDs als **physische Zeilennummern** bestimmt. NEXT-Fälle
   (`int_in[1] = 12`) und damit A14: **127, 133, 166, 174, 183, 204, 213**.
   Im Prelude-Block jedes einzelnen Falles nachgeprüft, dass er einen
   FIRST-Vorlauf trägt — für alle sieben bestätigt.
2. Beide BIN selbst mit `tosdiff --validate` gegen die Tabelle geprüft und
   die SHA-256 von Tabelle, Laufkopie und Referenz nachgerechnet: Tabelle
   `a81a7a6c…`, Pi-Lauf `e0280b7d…`, TT-Referenz `e7ef0a27…` — identisch mit
   dem, was M02/M03 und A13 nennen.
3. Die Rohwörter **nicht** aus `diff/06`/`diff/07` übernommen, sondern mit
   einem eigenen, rein lesenden Recordleser direkt aus beiden BIN gelesen
   (Layout `tosdump/core/tosdump.h:104-125`, Wörter big-endian nach
   `tosdump/core/record.c:43-54`, Poison `0xDEAD` nach `tosdump.h:78`) und
   je Fall zusätzlich **byteweise** verglichen, mit Feldnamen.
4. Den gepinnten EmuTOS-Stand selbst überprüft: `references/emutos` @
   **`1a7484aba1bfba23e2c0899e243b6d8177fe41df`**, `git describe` =
   `VERSION_1_4-71-g1a7484ab`. `git diff --stat VERSION_1_4 1a7484ab` auf
   `aes/gemwmlib.c aes/gemwrect.c aes/gemsuper.c aes/gemgraf.c
   aes/gemobjop.c aes/gemlib.h util/rectfunc.c` zeigt genau einen Zusatz —
   den `WF_OWNER`-Arm `gemwmlib.c:1578-1590` (A05). Alles, was A14 braucht,
   ist gegenüber der gemessenen 1.4 unverändert.
5. Das Soll aus der Quelle geholt, nicht aus der Spec: `wm_get`-FIRST/NEXT-Arm
   (`gemwmlib.c:1556-1562`), `w_owns` (`:1230-1241`), `get_pwin`
   (`:1391-1404`), Handlepflicht (`:1521-1523`), `w_getsize`/`WS_TRUE`
   (`:254-262`, `DROP_SHADOW_SIZE = 2` bei `:49`), `newrect`
   (`gemwrect.c:165-205`, brechendes Rechteck `:190`, eigenes `:203`),
   `mkrect`/`brkrct`/`mkpiece` (`:136-162`, `:101-132`, `:64-98`,
   Reihenfolge TOP/LEFT/RIGHT/BOTTOM `:30-33` + Schleife `:118-122`),
   `everyobj` (`gemobjop.c:76-120`), `rc_intersect` (`rectfunc.c:63-78`),
   `gl_rscreen`/`gl_rfull` (`gemgraf.c:423-424`), `xif`
   (`gemsuper.c:342-344`, `:428-448`) — und **`w_rnext` als Feld der
   `WINDOW`-Struktur** (`gemlib.h:85`), was der Kern des Auftrags ist.
6. Diesen Pfad als Modell nachgebaut und **gegen die TT-Messung geeicht**: es
   trifft alle zwölf Fälle der Tabelle, die es modelliert (122, 127, 133,
   159, 166, 174, 183, 189, 201, 204, 208, 213), exakt. Erst danach mit dem
   Pi-Profil 1920×1080 das Soll für die gemessene Maschine gerechnet.
7. Die Geräteabhängigkeit **nachgefahren statt behauptet**: für 166, 174, 183
   und 213 liefert das Modell bei 1920×1080 Wert für Wert dasselbe wie bei
   640×480. Das Soll für den Pi ist dort der TT-Wert selbst.
8. Sechs Varianten desselben Modells gerechnet (EmuTOS; NewTOS; nur
   Reihenfolge; nur Schatten; Schatten nur am brechenden Rechteck; beides),
   um die Ursachen mechanisch zu trennen statt sie zu vermuten.
9. Die Umsetzung gelesen: `tos-api/src/aes/window.rs:1999-2010`
   (FIRST/NEXT-Arm), `:1054-1064` (`RLIST`), `:1129-1155`
   (`compute_rlist`/`rlist_next`), `:1110-1124` (`work_rects`), `:600-615`
   (`border_pieces`), `:1075-1101` (`subtract`), `:1188-1199`
   (`visible_bound`), `:1205-1228` (`send_redraw_in`/`broadcast_redraw`),
   `:1382-1387` (`draw_frame`), `:1236-1242` (`win`), `:2544-2560`
   (`screen_rect`/`desktop_work`); `tos-api/src/aes/mod.rs:132-140`
   (Rückkopie), `:263` (genulltes `local_intout`).
10. Geprüft, ob es schon einen dauerhaften Test gibt — **nein**: `NEXTXYWH`
    kommt in `tos-api/tests/` nicht vor.
11. **Grenzfälle gefahren**, über einen temporären Testaufsatz
    (`tos-api/tests/zz_audit_a14_tmp.rs`, danach gelöscht), in **beiden**
    Profilen (640×480/4 Planes und 1920×1080/16 Planes), mit dem
    Poison-/Reset-/Prelude-Protokoll aus `tosdump/core/runner_aes.c:123-191`
    nachgebaut: alle sieben Tabellenfälle, der fünfte NEXT hinter dem Ende,
    die verschachtelte Iteration über zwei Fenster in beiden Richtungen,
    NEXT auf Handles 7, −1 und 99, NEXT ohne eigenes FIRST, `OUT_LEN`
    0/1/2/3/5/7 auf einem NEXT am Listenende, und NEXT über ein
    `wind_close` hinweg.
12. Neue Vergleichsdatei mit Nummer 12 angelegt, erzeugende Befehle im Kopf.

## Artefakte

- `measurements/measurement-2026-09-09/diff/12-a14-wind-get-nextxywh.txt` — Validierungen,
  SHA-256, Fallzuordnung, Rohwörter feldweise aus beiden BIN, Bytevergleich,
  Soll je Fall mit Quellenbelegen, die Ursachentrennung über sechs Modelle,
  die vollständigen Hostläufe, die zwei Defekte, die benannten Fixes, die
  Regressionstestvorschläge, die Kompensationsprüfung und die Messlücken.
- `TASK/A14.md`, Abschnitt „Auswertung — 2026-09-10". Statuszeile bleibt
  **OFFEN**, weil A14 ohne Codeänderung nicht geschlossen werden kann.
- `TASK/README.md` unberührt — die A14-Zeile bleibt OFFEN, Zeile 5 und die
  Einstiegszahlen sind nicht angefasst.

Eingefrorene Artefakte (`cases/*.tsv`, alle `*.BIN`, `*.LOG`, `MESSRUN.TXT`,
`RUNINFO.TXT`) wurden nicht angefasst; die Diffs 00–11 sind unverändert.
Keine `.rs`-Datei im Baum geändert; der temporäre Testaufsatz
`tos-api/tests/zz_audit_a14_tmp.rs` ist **gelöscht** (per `rm`, nicht per
`git checkout`/`clean`/`stash`).

## Ausgeführte Prüfung

    $ ./tosdump/host/build/tosdiff --validate \
          measurements/measurement-2026-09-09/run3-pi4-rlist/AES.BIN tosdump/cases/aes-r2b-rlist.tsv
      -> area AES, 15 records of 120 bytes, build 70ca, case CRC B4CC4A5A, accepted
    $ ./tosdump/host/build/tosdiff --validate \
          tosdump/reference/EMUTOS14-AES-r2brlist.BIN tosdump/cases/aes-r2b-rlist.tsv
      -> area AES, 15 records of 120 bytes, build c911, case CRC B4CC4A5A, accepted

    Rohwertlesung der sieben NEXT-Faelle aus beiden BIN (eigener Leser),
    Bytevergleich je Record mit Feldnamen, Poison-Bilanz.

    $ cd references/emutos && git describe --tags
      -> VERSION_1_4-71-g1a7484ab
    $ git diff --stat VERSION_1_4 1a7484ab -- aes/gemwmlib.c aes/gemwrect.c \
          aes/gemsuper.c aes/gemgraf.c aes/gemobjop.c aes/gemlib.h util/rectfunc.c
      -> aes/gemwmlib.c | 13 +++   (nur der WF_OWNER-Arm)

    $ A14_PROFILE=tt cargo test -p tos-api --test zz_audit_a14_tmp -- --nocapture --test-threads=1
    $ A14_PROFILE=pi cargo test -p tos-api --test zz_audit_a14_tmp -- --nocapture --test-threads=1
      -> 2 passed; der vollstaendige Ausdruck steht in diff/12.

    Der Hostlauf im Pi-Profil reproduziert **alle sieben** gemessenen
    NEXT-Faelle Wort fuer Wort (127, 133, 166, 174, 183, 204, 213) — der
    Harness ist damit gegen echte Hardware geeicht.

## Ergebnis

Sieben Fälle ausgewertet. Außerhalb von `global[]` unterscheiden sich in
allen sieben **ausschließlich `int_out`-Wörter**; `int_out[0]` ist auf beiden
Maschinen 1, `int_out[5]`/`[6]` bleiben auf beiden POISON (`OUT_LEN 5`).

| Fall | Pi 4 `70ca` (ret,x,y,w,h) | TT `c911` | Soll (Pi-Profil) | Einordnung |
|---|---|---|---|---|
| 127 | `1, 0, 0, 0, 0` | `1, 31, 63, 0, 0` | `1, 31, 63, 0, 0` | A14-Teil **gleich** (Ende erkannt); `g_x`/`g_y` → **A15** |
| 133 | `1, 0, 0, 0, 0` | `1, 31, 63, 0, 0` | `1, 31, 63, 0, 0` | wie 127; kein Weiterlaufen hinter dem Ende |
| 166 | `1, 31, 240, 296, 17` | `1, 31, 80, 29, 162` | `1, 31, 80, 29, 162` | **abweichend** — D-A13-2 + D-A13-3 |
| 174 | `1, 31, 80, 29, 160` | `1, 262, 80, 65, 162` | `1, 262, 80, 65, 162` | **abweichend** — D-A13-2 + D-A13-3 |
| 183 | `1, 260, 80, 67, 160` | `1, 31, 242, 296, 15` | `1, 31, 242, 296, 15` | **abweichend** — D-A13-2 + D-A13-3 |
| 204 | `1, 0, 0, 0, 0` | `1, 0, 19, 0, 0` | `1, 0, 19, 0, 0` | A14-Teil **gleich**; `g_y` → **A15** |
| 213 | `1, 0, 280, 1920, 800` | `1, 0, 40, 30, 242` | `1, 0, 40, 30, 242` | **abweichend** — D-A13-2 + D-A13-3 |

**166, 174, 183, 213 sind vollständig und mechanisch auf D-A13-2
(Zerlegungsreihenfolge) und D-A13-3 (fehlender `DROP_SHADOW_SIZE` am
brechenden Rechteck) zurückgeführt** — sechs Modellvarianten zeigen: „beides
korrigiert" ist Wert für Wert gleich EmuTOS, keine der beiden Korrekturen
allein reicht, und keiner der beiden Fehler gleicht den anderen aus. Keine
dritte Ursache, kein Geräteparameter: das Soll ist dort geräteunabhängig,
nachgefahren.

**Zwei eigenständige, belegte Defekte, beide CONFIRMED:**

- **D-A14-1 — der Iteratorcursor ist global statt pro Fenster.** EmuTOS hält
  `ORECT *w_rnext` als Feld der `WINDOW`-Struktur (`aes/gemlib.h:85`), gewählt
  über `pwin = get_pwin(w_handle)` (`gemwmlib.c:1513`, `:1559`) und
  geschrieben in `w_owns` (`:1235`); die Spec sagt dasselbe richtig
  (`docs/api/aes/window-graphics.md:335-337`). NewTOS hält einen einzigen
  statischen `RLIST` (`tos-api/src/aes/window.rs:1054-1064`), `rlist_next()`
  nimmt kein Handle (`:1142-1155`). Gefahren: `FIRST(1)` → `1,31,63,296,17`,
  `FIRST(2)` → `1,61,103,176,114`, `NEXT(1)` → `1,0,0,0,0` (Listenende) statt
  `1,31,80,29,162`. Ein Programm, das zwei Fenster in einem Durchgang
  zeichnet, lässt drei Viertel des ersten stehen. Bug, keine hinnehmbare
  Abweichung.
- **D-A14-2 — NEXT prüft sein Handle nicht.** `NEXT(7)` → `1,31,80,29,160`,
  `NEXT(-1)` → `1,260,80,67,160`. Soll: `get_pwin` liefert `NULL`
  (`gemwmlib.c:1395-1401`), `wm_get` liefert `FALSE` (`:1521-1523`,
  `:1591-1592`), `int_out[1..4]` unberührt. Das ist die NEXT-Hälfte von
  D-A13-1; derselbe Ort (`window.rs:1999-2010` steht vor jeder
  Handleprüfung), ein gemeinsamer Fix genügt.

Fixes **F4** (Cursor in die `Window`-Struktur) und **F5** (Arm hinter die
Handleprüfung) sowie Regressionstests **R3**/**R4** sind in `TASK/A14.md` und
`diff/12` benannt und **nicht** umgesetzt; die Entscheidung liegt beim
Anwender.

**Kompensationsprüfung** (für A23): Der Cursor wird an genau fünf Codezeilen
erzeugt/gelesen (`window.rs:1060, 1132, 1145, 1146, 1148`) und nur von
`wind_get` (`:2001`, `:2003`) benutzt — **kein Ausgleich, F4 kann nichts
anderes brechen**. Das brechende Rechteck dagegen wird von **drei** Lesern
benutzt: `walk_rects` (`:646`, Zeichenclip), `compute_rlist` (`:1136`,
`wind_get`) und `visible_bound` (`:1190`) → `send_redraw_in` (`:1205`) →
`broadcast_redraw` (`:1223`), also die `WM_REDRAW`-Rechtecke. `DROP_SHADOW_SIZE`
existiert in `tos-api/` und `kernel/src/` **nirgends**, und `draw_frame`
(`:1382-1387`) malt keinen Schlagschatten — der Zustand ist heute in sich
konsistent. Ein Fix **nur** an `window.rs:611` ließe an jedem Fensterrand einen
2 Pixel breiten Streifen alter Pixel stehen, den dann niemand mehr malt. Das
ist der von A23 gesuchte Fall, konkret benannt. `window.rs:607` (eigenes
Rechteck ohne Schatten) ist durch `WF_FIRST/NEXTXYWH` nachweislich **nicht**
beobachtbar, über `walk_rects` habe ich das nicht gefahren.

## Offen

- **Zwei Fixes stehen aus** (F4, F5). Deshalb UNFINISHED, nicht DONE.
- **L1, die zentrale Messlücke:** die eingefrorene Tabelle enthält **keinen**
  Fall, der zwei Fenster verschachtelt abfragt. D-A14-1 ist damit
  quellenbelegt und hostgefahren, aber **nicht messbelegt**. Eine echte
  Gegenmessung braucht eine **neue** Falltabelle mit der Folge `FIRST(1)`,
  `FIRST(2)`, `NEXT(1)`. Ich habe keine angelegt — das ist ein Messdesign und
  gehört nicht in diesen Auftrag.
- **Reale Gegenmessung offen** für alle gefahrenen Grenzfälle (D-A14-2,
  `OUT_LEN` auf NEXT, fünfter NEXT, NEXT über `wind_close`): sie liefen im
  Hostharness gegen das aus der Quelle abgeleitete Soll. Für diese Eingaben
  existiert **kein** TT-Record; sie sind **quellenbelegt, nicht messbelegt**.
- **L2, ohne Soll:** NEXT ohne vorangehendes FIRST. `w_setup` initialisiert
  `w_rnext` nicht (`gemwmlib.c:205-228`); EmuTOS hat dort kein definiertes
  Verhalten. Kein Vergleich möglich, kein Defektbefund.
- **L3, ohne Soll:** NEXT über eine Geometrieänderung hinweg. Wir liefern den
  beim FIRST genommenen Schnappschuss, EmuTOS zeigt über `w_rnext` in die
  Freiliste (`gemwrect.c:177-181`). Undefiniert, kein Defektbefund — nur
  festgehalten.
- **L4, bereits ticketiert:** unsere Reihenfolge der brechenden Fenster ist die
  Handle-Reihenfolge (`window.rs:609-613`), EmuTOS' die Baumreihenfolge
  unten→oben (`gemwrect.c:198` mit `everyobj`). Ab zwei brechenden Fenstern
  nicht notwendig gleich. Nicht verfolgt — [A22](A22.md) deckt das ab.
- **Nicht bewertet, außerhalb des Auftrags:** die `g_x`/`g_y`-Abweichung am
  Listenende in 127, 133 und 204 (A15), die `global[]`-Abweichung an jedem
  Record (A06–A09), die Menüzeilenhöhe 19 (A20) und die `RCAP = 16`-Grenze
  (A21).
- **Unabhängiger Nebenbefund, gemeldet, kein Task von mir angelegt:** NewTOS
  zeichnet für Fenster überhaupt keinen Schlagschatten
  (`draw_frame`, `window.rs:1382-1387`, malt ausschließlich innerhalb
  `w.curr`), während EmuTOS `W_ACTIVE[ROOT].ob_state = SHADOWED` setzt
  (`gemwmlib.c:1284`) und `DROP_SHADOW_SIZE` an sechs Stellen aufschlägt
  (`gemwmlib.c:257-261, 373-374, 946-947, 1153-1154, 1214-1215`). Das hängt
  mit D-A13-3 zusammen und ist für den A23-Durchgang relevant.
