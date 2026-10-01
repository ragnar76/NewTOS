# A11 — Quittung

**Status: DONE** · 2026-09-10 · wind_get WF_CXYWH: erzeugtes ungeöffnetes Fenster

## Auftrag

Nur das Anfangsrechteck nach `wind_create` und **vor** `wind_open` aus der
Nachmessung M01 auswerten — der alte Fall 230. Bei Gleichheit ohne Codeänderung
schließen; bei Abweichung genau diesen Übergang mit Referenzbeleg und
Regressionstest korrigieren.

## Was ich gemacht habe

1. Den Fall in der eingefrorenen Tabelle bestimmt: `tosdump/cases/aes-r2-wind.tsv`
   Zeile **230** (`wind_get(1, WF_CXYWH)`, `IN_LEN 2`, `OUT_LEN 5`), davor genau
   ein Prelude, Zeile 229 (`wind_create(0x0FEF, 10, 20, 300, 200)`). Zeile 228
   (`WF_FXYWH`) als Anker mitgelesen; 232/250/253 nur zur Kontrolle, nicht
   bewertet — die gehören zu A12.
2. Beide BINs mit `tosdiff --validate` gegen die Tabelle geprüft und die SHA-256
   der Tabelle in `tosdump/cases/` gegen die Laufkopie in
   `measurements/measurement-2026-09-09/cases/` verglichen — identisch (`5419a709…`).
3. Die Rohwerte **nicht** aus den vorhandenen Diffs übernommen, sondern mit einem
   eigenen, rein lesenden Recordleser (Layout `tosdump/core/tosdump.h:104-125`,
   Wörter big-endian nach `tosdump/core/record.c:42-56`, Poison `0xDEAD` nach
   `tosdump.h:78`) direkt aus beiden BINs gelesen.
4. Das Soll zuerst aus der Spec genommen (`docs/api/aes/window-graphics.md:224-226`
   und `:297`) und dann gegen die gepinnte EmuTOS-Quelle geprüft
   (`references/emutos` @ `1a7484ab`): `wm_create` (`aes/gemwmlib.c:1377-1380`),
   `gl_rzero = 0,0,0,0` (`aes/gemgraf.c:425`), `wm_get`-Arm
   (`aes/gemwmlib.c:1532-1533`, `:1595-1596`), TRUE-Rückgabe für ein
   `VF_INUSE`-Handle (`:1513-1524`, `:1598`, `get_pwin` `:1391-1404`),
   Rückkopierregel `xif` (`aes/gemsuper.c:342-343`, `:443-444`). Spec korrekt.
5. Unsere Umsetzung gelesen: `tos-api/src/aes/window.rs:1846-1848` (Erzeugung),
   `:2064` und `:2135-2139` (Ausgabe), `tos-api/src/aes/mod.rs:132-140`
   (`OUT_LEN`-Rückkopierung).
6. Geprüft, ob es zu diesem Übergang schon einen Test gibt — ja:
   `tos-api/tests/aes_reference.rs:108-162`. Ausgeführt statt nachgebaut, und
   zusätzlich belegt, dass er diskriminiert (der TT-Record für 230 ist ungleich
   dem für 228, also fällt eine Umsetzung durch, die hier das Erzeugungsrechteck
   lieferte).
7. Zehn Grenzfälle gefahren, die die eingefrorene Tabelle nicht abdeckt, über
   einen **temporären** Testaufsatz (`tos-api/tests/zz_audit_a11_tmp.rs`) —
   danach gelöscht.
8. Neue Vergleichsdatei mit Nummer 09 angelegt, erzeugende Befehle im Kopf.

## Artefakte

- `measurements/measurement-2026-09-09/diff/09-a11-wind-get-cxywh-created.txt` — Rohwerte des
  Falls 230 (plus Anker 228/232) aus beiden BINs, `--validate`-Ausgaben,
  SHA-256 der beteiligten Dateien, die zehn Grenzfälle und die
  Quellenbelege; erzeugende Befehle im Kopf.
- `TASK/A11.md`, Abschnitt „Auswertung — 2026-09-10"; Statuszeile auf ERLEDIGT.
- `TASK/README.md`, Statusspalte der A11-Zeile auf ERLEDIGT.

Eingefrorene Artefakte (`cases/*.tsv`, `*.BIN`, `*.LOG`, `MESSRUN.TXT`,
`RUNINFO.TXT`) wurden nicht angefasst. Bestehende Diff-Dateien 00–08
unverändert. Keine `.rs`-Datei im Baum geändert.

## Ausgeführte Prüfung

    $ ./tosdump/host/build/tosdiff --validate \
          measurements/measurement-2026-09-09/run2-pi4-r2wind/AES.BIN tosdump/cases/aes-r2-wind.tsv
      → 78 records, build 70ca, case CRC AFB11C9F, accepted
    $ ./tosdump/host/build/tosdiff --validate \
          tosdump/reference/EMUTOS14-AES-r2wind.BIN tosdump/cases/aes-r2-wind.tsv
      → 78 records, build c911, case CRC AFB11C9F, accepted

    Rohwertlesung des Falls 230 aus beiden BINs (eigener Leser).

    $ cargo test -p tos-api --test aes_reference \
          window_rectangles_after_create_and_open_match_the_tt
      → test result: ok. 1 passed

    Temporärer Zusatzaufsatz (danach entfernt), alle grün:
      1. Fall 230 wortgleich nachgespielt → gleich TT-Record und gleich Pi-Record
      2. der TT-Record für 230 ist ungleich dem für 228 (Test diskriminiert)
      3. Handle 2 (zweites erzeugtes Fenster) → 1, 0,0,0,0, Rest Poison
      4. Handle 15 (letzter Slot) → 1, 0,0,0,0, Rest Poison
      5. Slot 1 nach open+delete neu erzeugt → 1, 0,0,0,0 (kein Altrechteck)
      6. wind_create(0x0FEF, −5, −7, 0, 0) → 1, 0,0,0,0
      7. OUT_LEN 1 → nur int_out[0] = 1, Rest bleibt Poison
      8. OUT_LEN 0 → gar nichts geliefert, auch kein Rückgabewort
      9. OUT_LEN 7 → 1, 0,0,0,0 und zwei Überschusswörter (siehe Offen)
     10. Handle nach wind_delete / nie erzeugt → int_out[0] = 0 (FALSE)

## Ergebnis

Abnahme erfüllt, **Gleichheit**, kein Defekt, keine Codeänderung.

| Fall | Aufruf | Pi 4 (`70ca`) | TT EmuTOS 1.4 (`c911`) | Soll |
|---|---|---|---|---|
| 230 | `wind_get(1, WF_CXYWH)` nach `wind_create`, vor `wind_open` | `1, 0, 0, 0, 0` | `1, 0, 0, 0, 0` | `1, 0, 0, 0, 0` |

`int_out[5..6]` bleibt auf beiden Maschinen Poison. `contrl`, `int_in`,
`addr_in`, `addr_out` und der Trap-Rückgabewert sind bitgleich; abweichend sind
nur die sechs `global[]`-Wörter (2, 3, 4, 10, 11, 12) — Klasse A06–A09, hier
nicht bewertet.

Soll aus `aes/gemwmlib.c:1377-1380`: `wm_create` legt das Rechteck des Aufrufers
**nur** in `WS_FULL` ab und setzt `WS_CURR`/`WS_PREV` auf `gl_rzero`, und
`gl_rzero` ist die Konstante 0,0,0,0 (`aes/gemgraf.c:425`). **Das ist kein
Geräteparameter**: für diesen Fall ist Gleichheit zwischen Pi (1920×1080,
16 Planes) und TT (640×480, 4 Planes) gefordert, nicht mit dem Geräteprofil zu
entschuldigen — und sie liegt vor.

Die ungeprüfte Fremdbeobachtung („Fall 230 soll auf beiden Maschinen 1, 0,0,0,0
liefern"; „ein Test existiert bereits") hat sich in der eigenen Messung
bestätigt. Der Test steht bei `:108-162`, nicht bei `:119-158`, und deckt den
Übergang wirklich ab — nachgeprüft, nicht übernommen.

## Offen

- **Kein Fix offen, keine Hardwareabnahme offen.** Der Istwert stammt aus einem
  Lauf auf realer Pi 4 mit genau dem Build am Kopf des Zweigs (`70ca` =
  `70caee2`).
- **Belegte Randbeobachtung außerhalb dieses Auftrags, braucht einen eigenen
  Task:** derselbe Aufruf **nach `wind_close`** liefert bei uns
  `WF_CXYWH = 1, 30, 40, 320, 240`, EmuTOS-Soll ist `1, 0, 0, 0, 0`.
  `wm_close` ruft `draw_change(w_handle, &gl_rzero)` (`aes/gemwmlib.c:1449`,
  `:1453`), und `draw_change` setzt `WS_PREV = altes WS_CURR` und
  `WS_CURR = pt` noch **vor** der `VF_ISOPEN`-Abkürzung (`:1069-1073` gegen
  `:1085-1086`). Unser `close_window` (`tos-api/src/aes/window.rs:1928-1958`)
  schreibt weder `curr` noch `prev`. Betrifft `WF_CXYWH` *und* `WF_PXYWH`. Die
  eingefrorene Tabelle hat für diesen Übergang keinen Fall; der Befund stammt
  aus dem Hostlauf gegen die Quelle, nicht aus einer Messung auf dem TT. Nicht
  in A11 repariert, weil A11 ausdrücklich auf das Anfangsrechteck vor
  `wind_open` begrenzt ist.
- Nicht geprüft, weil außerhalb des Auftrags: die `global[]`-Abweichung an
  diesem Fall (A06–A09), `WF_PXYWH` desselben Zustands (Fall 232, A12) und die
  Rechtecke des Desktopfensters, Handle 0 (A02).
- Ohne Fehlerwirkung, nur als Hinweis: bei `OUT_LEN` größer als die vom Arm
  geschriebene Wortzahl und bei einem FALSE-Rückgang kopiert EmuTOS Inhalt
  seines **uninitialisierten** lokalen `int_out` (`aes/gemsuper.c:429`,
  `:443-444`; auf dem TT an 244/247 als `0, −1, 0, 0, 0` sichtbar), wir liefern
  dort 0 (`tos-api/src/aes/mod.rs:259-263`). Ein Unterschied gegen einen auf der
  Referenzseite unbestimmten Wert; Fall 230 ist davon nicht betroffen.
- Die zehn zusätzlich gefahrenen Grenzfälle liegen nicht dauerhaft im Baum. Sie
  belegen kein Problem; sie festzuhalten wäre eine Empfehlung, keine Auflage.
