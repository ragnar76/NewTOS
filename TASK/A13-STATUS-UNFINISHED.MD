# A13 — Quittung

**Status: UNFINISHED** · 2026-09-10 · wind_get WF_FIRSTXYWH: erstes sichtbares
Rechteck

## Auftrag

Nur das FIRST-Ergebnis aus M02 + M03 auswerten, einschließlich leerer,
verdeckter und teilweise sichtbarer Fenster. Bei Gleichheit die Messlücke
ohne Codeänderung schließen; bei belegtem Unterschied genau einen Fix und
einen Regressionstest benennen.

## Was ich gemacht habe

1. Den Kopfkommentar der eingefrorenen Tabelle `tosdump/cases/aes-r2b-rlist.tsv`
   vollständig gelesen (Zeilen 1–109: Iteratorregel, Poison-Absicht, `w_owns`
   im Wortlaut, die drei Dinge, die die Messung klären soll) und die
   Fall-IDs als **physische Zeilennummern** bestimmt. FIRST-Fälle
   (`int_in[1] = 11`) und damit A13: **122, 144, 159, 189, 201, 208, 226,
   230**. NEXT-Fälle und damit A14/A15: 127, 133, 166, 174, 183, 204, 213.
   Von den zehn abweichenden Fällen aus `diff/07` gehören mir **144, 201,
   208**.
2. Beide BINs selbst mit `tosdiff --validate` gegen die Tabelle geprüft und
   die SHA-256 von Tabelle, Laufkopie, Referenz und Referenzarbeitskopie
   verglichen — Tabelle identisch (`a81a7a6c…`), Referenz identisch
   (`e7ef0a27…`).
3. Die Rohwörter **nicht** aus `diff/06`/`diff/07` übernommen, sondern mit
   einem eigenen, rein lesenden Recordleser direkt aus beiden BINs gelesen
   (Layout `tosdump/core/tosdump.h:104-125`, Wörter big-endian nach
   `tosdump/core/record.c:43-54`, Poison `0xDEAD` nach `tosdump.h:78`) und
   je Fall zusätzlich **byteweise** verglichen.
4. Den gepinnten EmuTOS-Stand selbst überprüft: `references/emutos` @
   **`1a7484ab`**, `git describe` = `VERSION_1_4-71-g1a7484ab`.
   `git diff VERSION_1_4..1a7484ab` auf `aes/gemwmlib.c`, `aes/gemwrect.c`,
   `aes/gemsuper.c`, `aes/gemgraf.c`, `util/rectfunc.c` zeigt genau einen
   Zusatz — den `WF_OWNER`-Arm `gemwmlib.c:1578-1590` (A05). Alles, was A13
   braucht, ist gegenüber der gemessenen 1.4 unverändert. Der Stand ist
   also für diesen Auftrag der richtige.
5. Das Soll zuerst aus der Spezifikation genommen
   (`docs/api/aes/window-graphics.md:294-358`, `:807-811`, `:848-851`) und
   dann gegen die Quelle geprüft: `wm_get`-Kopf und Handlepflicht
   (`gemwmlib.c:1505-1524`), der FIRST/NEXT-Arm (`:1556-1562`), `w_owns`
   (`:1230-1241`, Listenende `:1240`), `rc_intersect`
   (`util/rectfunc.c:63-78`), `w_getsize`/`WS_TRUE`-Schatten (`:254-262`,
   `DROP_SHADOW_SIZE = 2` bei `:49`), `w_setup` (`:205-228`), `wm_create`
   (`:1370-1385`), `wm_delete` (`:1466-1485`), `wm_new` (`:1875-1893`),
   `draw_change` (`:1069-1077`, `:1089`), `newrect`/`mkrect`/`brkrct`/
   `mkpiece` (`gemwrect.c:30-33,64-98,101-132,136-162,165-205`), `gl_rfull`
   und `gl_hbox` (`gemgraf.c:412-424`), `wm_calc` mit 3D
   (`gemwmlib.c:1807-1868`, `ADJ3DSTD` `include/obdefs.h:136`),
   `wm_init`-Desktop (`:1287-1294`), `everyobj` ab `ROOT`
   (`gemobjop.c:76-100`), der Dispatcher-Kopierpfad `xif`
   (`gemsuper.c:342-344`, `:428-448`, lokales **nicht initialisiertes**
   `int_out` bei `:432`, `RET_CODE == int_out[0]` `include/aesdefs.h:140`).
6. Die Poison-Bilanz je Fall bestimmt und die Deutung der TT-Werte belegt,
   statt sie zu vermuten: die `31, 63` in Fall 144 sind der Stapelrest des
   vorangehenden AES-Aufrufs, und das ist über **alle fünfzehn** Records
   widerspruchsfrei (127/133 tragen die `31, 63` ihres FIRST-Preludes, 204
   die `0, 19` des Desktop-FIRST-Preludes).
7. Für jeden Fall einzeln entschieden, ob der Sollwert konstant, abgeleitet
   oder geräteabhängig ist, mit Quelle — und die Geräteabhängigkeit
   **nachgefahren**, nicht behauptet: unsere Umsetzung liefert im
   Hostharness bei 640×480/4 Planes für 201 exakt `1, 0, 19, 640, 461` und
   für 208 exakt `1, 0, 19, 640, 21`, also die TT-Werte.
8. Die Umsetzung gelesen: `tos-api/src/aes/window.rs:1999-2010`
   (FIRST/NEXT-Arm), `:1129-1155` (`compute_rlist`, `rlist_next`),
   `:1110-1124` (`work_rects`), `:600-615` (`border_pieces`), `:1075-1101`
   (`subtract`), `:1236-1242` (`win`), `:2556-2560` (`desktop_work`);
   `tos-api/src/aes/mod.rs:132-140` (Rückkopie), `:263` (genulltes
   `local_intout`).
9. Geprüft, ob es schon einen dauerhaften Test gibt — **nein**: `FIRSTXYWH`
   kommt in `tos-api/tests/` nicht vor, und für diese Tabelle existiert kein
   generiertes Fallmodul.
10. **Grenzfälle gefahren**, über einen temporären Testaufsatz
    (`tos-api/tests/zz_audit_a13_tmp.rs`, danach gelöscht): ungültige
    Handles, Zustand nach `wind_close`, voll verdecktes Fenster, exakt auf
    der Arbeitsfläche verdecktes Fenster, voll verdeckter Desktop,
    `kind = NONE`, `OUT_LEN 0…7`, `IN_LEN 0/1`, Iteratorrücksetzung durch
    zwei FIRST hintereinander, FIRST abwechselnd auf zwei Fenster, und drei
    Deckungsgeometrien, in denen das erste Stück **nicht** das TOP-Stück ist.
11. Neue Vergleichsdatei mit Nummer 11 angelegt, erzeugende Befehle im Kopf.

## Artefakte

- `measurement-2026-09-09/diff/11-a13-wind-get-firstxywh.txt` —
  Validierungen, SHA-256, Fallzuordnung, Rohwörter feldweise aus beiden BINs,
  Soll je Fall mit Quellenbelegen, der vollständige Hostlauf, die drei
  Defekte, die benannten Fixes, der Regressionstestvorschlag und die offenen
  Punkte.
- `TASK/A13.md`, Abschnitt „Auswertung — 2026-09-10". Statuszeile bleibt
  **OFFEN**, weil A13 ohne Codeänderung nicht geschlossen werden kann.
- `TASK/README.md` unberührt — die A13-Zeile bleibt OFFEN, die
  Einstiegszahlen sind nicht angefasst.

Eingefrorene Artefakte (`cases/*.tsv`, alle `*.BIN`, `*.LOG`, `MESSRUN.TXT`,
`RUNINFO.TXT`) wurden nicht angefasst; die Diffs 00–10 sind unverändert.
Keine `.rs`-Datei im Baum geändert; der temporäre Testaufsatz
`tos-api/tests/zz_audit_a13_tmp.rs` ist **gelöscht**.

## Ausgeführte Prüfung

    $ ./tosdump/host/build/tosdiff --validate \
          measurement-2026-09-09/run3-pi4-rlist/AES.BIN tosdump/cases/aes-r2b-rlist.tsv
      → area AES, 15 records of 120 bytes, build 70ca, case CRC B4CC4A5A, accepted
    $ ./tosdump/host/build/tosdiff --validate \
          tosdump/reference/EMUTOS14-AES-r2brlist.BIN tosdump/cases/aes-r2b-rlist.tsv
      → area AES, 15 records of 120 bytes, build c911, case CRC B4CC4A5A, accepted

    Rohwertlesung der acht FIRST-Fälle aus beiden BINs (eigener Leser),
    Bytevergleich je Record, Poison-Bilanz.

    $ cd references/emutos && git describe --tags
      → VERSION_1_4-71-g1a7484ab
    $ git diff --stat VERSION_1_4 1a7484ab -- aes/gemwmlib.c aes/gemwrect.c \
          aes/gemsuper.c aes/gemgraf.c util/rectfunc.c
      → aes/gemwmlib.c | 13 +++++++++++++   (nur der WF_OWNER-Arm)

    $ cargo test -p tos-api --test zz_audit_a13_tmp -- --nocapture --test-threads=1
      → 2 passed; der vollständige Ausdruck steht in diff/11.

## Ergebnis

Messlücke geschlossen, **aber drei belegte Defekte**. Fünf der acht
FIRST-Fälle sind außerhalb des `global[]`-Blocks bytegleich; zwei sind
reine Geräteparameter, nachgefahren; einer ist eine hinnehmbare Abweichung
an einer Stelle, die EmuTOS offenlässt.

| Fall | Zustand | TT (`c911`) | Pi 4 (`70ca`) | Einordnung |
|---|---|---|---|---|
| 122 | ein Rechteck | `1, 31, 63, 296, 194` | gleich | Gleichheit |
| 144 | **leer** | `1, 31, 63, 0, 0` | `1, 0, 0, 0, 0` | hinnehmbar, kein Soll für `g_x`/`g_y` |
| 159 | vier Stücke | `1, 31, 63, 296, 17` | gleich | Gleichheit (schatten- und reihenfolgeunabhängig) |
| 189 | ein Rechteck | `1, 61, 103, 176, 114` | gleich | Gleichheit |
| 201 | ein Rechteck | `1, 0, 19, 640, 461` | `1, 0, 19, 1920, 1061` | Geräteparameter `gl_width`/`gl_height` |
| 208 | mehrere Stücke | `1, 0, 19, 640, 21` | `1, 0, 19, 1920, 21` | Geräteparameter `gl_width` |
| 226 | `OUT_LEN 1` | `1` + Poison | gleich | Gleichheit |
| 230 | `OUT_LEN 3` | `1, 31, 63` + Poison | gleich | Gleichheit |

Die Defekte liegen nicht in den acht gemessenen Fällen, sondern in
Deckungsgeometrien, die die eingefrorene Tabelle nicht enthält — belegt
über die Quelle und über die TT-Messung der NEXT-Fälle derselben Tabelle:

- **D-A13-1** `WF_FIRSTXYWH` liefert `TRUE` für ein ungültiges Handle
  (`window.rs:1999-2010` steht vor der Prüfung; EmuTOS
  `gemwmlib.c:1521-1523` liefert `FALSE`). CONFIRMED.
- **D-A13-2** Das erste Stück ist falsch, sobald es nicht das TOP-Stück ist:
  unsere Zerlegung erzeugt BOTTOM vor LEFT/RIGHT (`window.rs:1084-1097`),
  EmuTOS TOP/LEFT/RIGHT/BOTTOM (`gemwrect.c:30-33,118-122`). K1: EmuTOS
  `232, 63, 95, 79` gegen NewTOS gemessen `31, 140, 296, 117`. CONFIRMED.
- **D-A13-3** Das verdeckende Rechteck wird ohne `DROP_SHADOW_SIZE`
  abgezogen (`window.rs:611` nimmt `curr`, EmuTOS `WS_TRUE`,
  `gemwrect.c:190` + `gemwmlib.c:254-262`, `= 2` bei `:49`). K2: EmuTOS
  `202, 63, 125, 194` gegen NewTOS gemessen `200, 63, 127, 194`. CONFIRMED.

Fix F1/F2/F3 und Regressionstest R1/R2 sind in `TASK/A13.md` und in
`diff/11` benannt und **nicht** umgesetzt; die Entscheidung liegt beim
Anwender.

## Offen

- **Drei Fixes stehen aus** (F1, F2, F3). Deshalb UNFINISHED, nicht DONE.
- **Reale Gegenmessung offen** für alle gefahrenen Grenzfälle: sie liefen im
  Hostharness bei 640×480/4 Planes gegen das aus der Quelle abgeleitete
  Soll. Für diese Eingaben existiert **kein** TT-Record; sie sind
  quellenbelegt, nicht messbelegt. Eine echte Gegenmessung braucht eine
  **neue** Falltabelle.
- **Nicht geprüft, ausdrücklich offen:** bei drei oder mehr sich
  überlappenden Fenstern ist der Weg selbst verschieden (iteratives
  `subtract` in Handlereihenfolge gegen `newrect` + `everyobj` in
  Objektbaumreihenfolge). Ob dieselbe Stückmenge herauskommt, ist hier
  weder gemessen noch bewiesen; die eingefrorene Tabelle öffnet nur zwei
  Fenster. Nicht verfolgt, braucht eine eigene Messung.
- **Restabweichung ohne Anwendungswirkung, benannt statt versteckt:** ist
  die Liste nicht leer, schneidet aber kein Stück die Arbeitsfläche, hat
  EmuTOS für `g_x`/`g_y` einen bestimmten Wert (letzter fehlgeschlagener
  `rc_intersect`, `util/rectfunc.c:72-75`) — `1, 31, 259, 0, 0` gegen unsere
  `1, 0, 0, 0, 0`. Ein Redraw nach Vorschrift liest die beiden Wörter nie.
- **Spezifikationslücken, gemeldet, nicht geändert** (Researcher):
  `docs/api/aes/window-graphics.md:341` und `:848-851` verschweigen die
  Zwei-Wort-Regel von `w_owns`; `:354-358` erklärt die Zerlegung zur bloßen
  Referenz („any way it likes as long as … the same set of pieces") und
  lizenziert damit D-A13-2 und D-A13-3; `:807-811` nennt weder die
  Reihenfolge noch `WS_TRUE` als brechendes Rechteck. Kein Task von mir
  angelegt.
- **Nicht bewertet, außerhalb des Auftrags:** die NEXT-Fälle 127, 133, 166,
  174, 183, 204, 213 (A14/A15) und die `global[]`-Abweichung an jedem
  Record (A06–A09). D-A13-2 und D-A13-3 sind an den NEXT-Fällen sichtbar,
  aber hier ausschließlich über ihre Wirkung auf das FIRST-Ergebnis geführt.
