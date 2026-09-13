# A12 — Quittung

**Status: DONE** · 2026-09-10 · wind_get WF_PXYWH: Zustand nach wind_open

## Auftrag

Nur die Aktualisierung des **vorherigen** Rechtecks beim Öffnen aus der
Nachmessung M01 auswerten — der alte Fall 253. Bei Gleichheit ohne
Codeänderung schließen; bei Abweichung genau diesen Übergang mit Referenzbeleg
und Regressionstest korrigieren.

## Was ich gemacht habe

1. Den Fall in der eingefrorenen Tabelle bestimmt: `tosdump/cases/aes-r2-wind.tsv`
   Zeile **253** (`wind_get(1, WF_PXYWH)`, `IN_LEN 2`, `OUT_LEN 5`), davor zwei
   Preludes — Zeile 251 `wind_create(0x0FEF, 10, 20, 300, 200)` und Zeile 252
   `wind_open(1, 30, 40, 320, 240)`. Zeile **232** (`WF_PXYWH` vor dem Öffnen)
   als Ausgangszustand desselben Übergangs mitgelesen; 250 und 259 nur als
   Anker, nicht bewertet.
2. Beide BINs selbst mit `tosdiff --validate` gegen die Tabelle geprüft und die
   SHA-256 der Tabelle in `tosdump/cases/` gegen die Laufkopie in
   `measurement-2026-09-09/cases/` verglichen — identisch (`5419a709…`).
3. Die Rohwerte **nicht** aus den vorhandenen Diffs übernommen, sondern mit
   einem eigenen, rein lesenden Recordleser direkt aus beiden BINs gelesen
   (Layout `tosdump/core/tosdump.h:19-36` und `:104-126`, Wörter big-endian nach
   `tosdump/core/record.c:42-56`, Poison `0xDEAD` nach `tosdump.h:78`), und
   Record 253 zusätzlich **byteweise** verglichen.
4. Das Soll zuerst aus der Spezifikation genommen
   (`docs/api/aes/window-graphics.md:298`) und dann gegen die gepinnte
   EmuTOS-Quelle geprüft (`references/emutos` @ **`1a7484ab`**): `wm_open`
   (`aes/gemwmlib.c:1410-1432`), die eigentliche Aktualisierung
   `w_setsize(WS_PREV, w_handle, pt)` (`:1427`) **nach** `draw_change`
   (`:1426`, `:1069-1073`), `w_setsize` = reines `rc_copy` (`:265-268`),
   `wm_get`-Arm (`:1535-1537`, `:1596`), `w_getsize` ohne Shadow-Zuschlag für
   `WS_PREV` (`:254-262`), `FALSE`-Pfade (`:1415-1419`), `TRUE` (`:1431`),
   Dispatcher (`aes/gemsuper.c:333-335`, `include/aesdefs.h:475-478`),
   Rückkopierregel `xif` (`aes/gemsuper.c:343`, `:444-445`).
5. Die Frage „konstant oder geräteabhängig?" selbst entschieden **und
   gefahren**, nicht pauschal beantwortet: auf dem ganzen Pfad steht keine
   Bildschirm- oder Fontgröße, und `wind_open(1, 0,0,2000,1500)` auf einem
   640×480-Arbeitsplatz liefert `WF_PXYWH = 1, 0, 0, 2000, 1500`,
   unbeschnitten. Gleichheit zwischen Pi (1920×1080, 16 Planes) und TT
   (640×480, 4 Planes) ist hier also **gefordert**.
6. Unsere Umsetzung gelesen: `tos-api/src/aes/window.rs:1866-1897` (Öffnen),
   `:1247-1250` (`set_curr`), `:2065` und `:2136-2140` (Ausgabe).
7. Geprüft, ob es zu diesem Übergang schon einen Test gibt — ja:
   `tos-api/tests/aes_reference.rs:108-162` (die Fundstelle stimmt, die
   Abdeckung 228/230/232/250/253/259 stimmt). Ausgeführt statt nachgebaut und
   belegt, dass er diskriminiert.
8. Elf Grenzfälle gefahren, die die eingefrorene Tabelle nicht abdeckt, über
   einen **temporären** Testaufsatz (`tos-api/tests/zz_audit_a12_tmp.rs`) —
   danach gelöscht.
9. Neue Vergleichsdatei mit Nummer 10 angelegt, erzeugende Befehle im Kopf.

## Artefakte

- `measurement-2026-09-09/diff/10-a12-wind-get-pxywh-after-open.txt` —
  Validierungen, SHA-256, Rohwerte der Records 232/253 (plus Anker 250/259) aus
  beiden BINs, Bytevergleich des Records 253, die Quellenbelege und die elf
  Grenzfälle; erzeugende Befehle im Kopf.
- `TASK/A12.md`, Abschnitt „Auswertung — 2026-09-10"; Statuszeile auf ERLEDIGT.
- `TASK/README.md`, Statusspalte der A12-Zeile auf ERLEDIGT.

Eingefrorene Artefakte (`cases/*.tsv`, `*.BIN`, `*.LOG`, `MESSRUN.TXT`,
`RUNINFO.TXT`) wurden nicht angefasst. Bestehende Diff-Dateien 00–09
unverändert. Keine `.rs`-Datei im Baum geändert; der temporäre Testaufsatz
`tos-api/tests/zz_audit_a12_tmp.rs` ist gelöscht.

## Ausgeführte Prüfung

    $ ./tosdump/host/build/tosdiff --validate \
          measurement-2026-09-09/run2-pi4-r2wind/AES.BIN tosdump/cases/aes-r2-wind.tsv
      → 78 records, build 70ca, case CRC AFB11C9F, accepted
    $ ./tosdump/host/build/tosdiff --validate \
          tosdump/reference/EMUTOS14-AES-r2wind.BIN tosdump/cases/aes-r2-wind.tsv
      → 78 records, build c911, case CRC AFB11C9F, accepted

    Rohwertlesung der Fälle 232/253 aus beiden BINs (eigener Leser) und
    Bytevergleich des Records 253.

    $ cargo test -p tos-api --test aes_reference \
          window_rectangles_after_create_and_open_match_the_tt
      → test result: ok. 1 passed

    Temporärer Zusatzaufsatz (danach entfernt), alle grün:
      1. Fall 253 wortgleich nachgespielt → 1, 30,40,320,240
      2. Wert weder Erzeugungsrechteck noch gl_rzero (Test diskriminiert)
      3. Re-open eines offenen Fensters mit anderem Rechteck → 0, PREV bleibt
      4. wind_open auf nie erzeugtes Handle → 0
      5. wind_open(1, 0,0,0,0) → PREV = 0,0,0,0
      6. wind_open(1, −5,−7,100,50) → wortgleich negativ
      7. wind_open(1, 0,0,2000,1500) → unbeschnitten (Geräteunabhängigkeit)
      8. close + erneutes open mit anderem Rechteck → PREV folgt dem neuen open
      9. zweites Fenster: PREV je Handle getrennt
     10. kind = NONE (0) → PREV wortgleich, nur WORK ist kindabhängig
     11. OUT_LEN 1 / 3 / 5 / 7 → genau OUT_LEN Wörter zurückkopiert

## Ergebnis

Abnahme erfüllt, **Gleichheit**, kein Defekt, keine Codeänderung.

| Fall | Aufruf | Pi 4 (`70ca`) | TT EmuTOS 1.4 (`c911`) | Soll |
|---|---|---|---|---|
| 232 | `WF_PXYWH` nach `wind_create`, vor `wind_open` | `1, 0, 0, 0, 0` | `1, 0, 0, 0, 0` | `1, 0, 0, 0, 0` |
| **253** | `WF_PXYWH` nach `wind_open(1, 30,40,320,240)` | **`1, 30, 40, 320, 240`** | **`1, 30, 40, 320, 240`** | **`1, 30, 40, 320, 240`** |

Record 253 ist außerhalb des `global[]`-Blocks **bytegleich**; abweichend sind
exakt die zehn Bytes der sechs `appl_init`-Wörter 2, 3, 4, 10, 11, 12 (Klasse
A06–A09, hier nicht bewertet). `int_out[5..6]` bleibt beidseitig Poison, die
Trap-Rückgabe ist beidseitig 0.

Soll aus `aes/gemwmlib.c:1427`: `wm_open` überschreibt `WS_PREV` **nach**
`draw_change` noch einmal mit dem übergebenen Rechteck, `w_setsize` ist ein
reines `rc_copy` (`:265-268`). **Das ist kein Geräteparameter** — der Wert ist
das Argument des Aufrufers, und der Pfad kennt keine Bildschirm- oder
Fontgröße; nachgefahren mit einem Rechteck größer als der Bildschirm, das
unbeschnitten zurückkommt. Gleichheit war gefordert und liegt vor.

Die ungeprüften Fremdbeobachtungen haben sich in der eigenen Messung bestätigt:
Fall 253 liefert auf beiden Maschinen `1, 30,40,320,240`, Fall 250 dieselben
vier Werte, und der dauerhafte Test liegt tatsächlich bei
`tos-api/tests/aes_reference.rs:108-162` und deckt 228/230/232/250/253/259 ab.

## Offen

- **Kein Fix offen, keine Hardwareabnahme offen.** Der Istwert stammt aus dem
  Lauf auf realer Pi 4 mit dem Build am Kopf des Zweigs (`70ca` = `70caee2`).
- **Dokumentationslücke, gemeldet, nicht behoben:** der `wind_open`-Abschnitt
  der Spezifikation (`docs/api/aes/window-graphics.md:239-256`) nennt unter
  „Globals written" das PREV-Rechteck, sagt aber nicht, welchen **Wert** es
  bekommt, und zitiert `aes/gemwmlib.c:1427` nicht. Der beobachtbare Teil des
  Übergangs (`WF_PXYWH == WF_CXYWH` direkt nach dem Öffnen) fehlt damit im
  Soll. Kein Task von mir angelegt.
- **Kommentarnachweis um eine Zeile daneben:** `tos-api/src/aes/window.rs:1881`
  und `tos-api/tests/aes_reference.rs:116` zitieren `:1426` für die
  PREV-Überschreibung; die Zeile ist `1427`. Nur Kommentar, keine API-Wirkung,
  nicht geändert.
- Nicht bewertet, weil außerhalb des Auftrags: die `global[]`-Abweichung an
  diesem Fall (A06–A09), die vier Überschusswörter hinter einem `FALSE` bzw.
  hinter `OUT_LEN 7` (auf der Referenzseite unbestimmt, `aes/gemsuper.c:429`,
  `:444-445`; in A11 vermerkt), `WF_CXYWH`/`WF_FXYWH` desselben Zustands
  (Fälle 250/259) und der `wind_close`-Übergang — der aus A11 bekannte Befund,
  dass unser `close_window` weder `WS_CURR` noch `WS_PREV` schreibt, liegt als
  **A16** und wurde hier nicht angefasst.
- Beobachtung ohne Abweichung: `wind_open(0, pt)` auf das Desktopfenster wird
  auch von EmuTOS angenommen — Handle 0 trägt `VF_INUSE` (`aes/gemwmlib.c:1290`,
  `:213`) und nie `VF_ISOPEN` (nur `:1424` setzt es) — und überschreibt dessen
  `WS_PREV`; `wm_new` läuft nur über die Handles 1..`NUM_WIN`−1 (`:1886`) und
  stellt es nicht wieder her. Unser Verhalten ist identisch.
- Die elf Grenzfälle liefen im Hostharness (640×480, 4 Planes) mit Sollwerten
  aus der Quelle; für diese Eingaben existiert **kein** TT-Record, sie sind
  also belegt gegen die Quelle, nicht gegen eine Messung. Sie liegen nicht
  dauerhaft im Baum; sie festzuhalten wäre eine Empfehlung, keine Auflage.
