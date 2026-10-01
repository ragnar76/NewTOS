# A10 — Quittung

**Status: DONE** · 2026-09-10 · wind_create: Kapazitätsgrenze

## Auftrag

Nur das neunte Fenster und die Grenze des konfigurierten Fensterpools aus der
Nachmessung M01 auswerten. Den alten Fall 154 ausdrücklich **nicht** als
aktuellen Defekt voraussetzen, sondern den Istwert feststellen. Bei Gleichheit
ohne Codeänderung schließen.

## Was ich gemacht habe

1. Die beiden Fälle in der eingefrorenen Tabelle bestimmt:
   `tosdump/cases/aes-r2-wind.tsv` Zeile **154** (der neunte `wind_create`, acht
   Preludes davor) und Zeile **170** (der sechzehnte, fünfzehn Preludes davor).
   Zeile 131 als Anker mitgenommen.
2. Beide BINs mit `tosdiff --validate` gegen die Tabelle geprüft, und die
   SHA-256 der Tabelle in `tosdump/cases/` gegen die Kopie in
   `measurements/measurement-2026-09-09/cases/` verglichen — identisch (`5419a709…`).
3. Die Rohwerte **nicht** aus dem vorhandenen Diff übernommen, sondern mit einem
   eigenen, rein lesenden Recordleser (Layout aus
   `tosdump/core/tosdump.h:104-125`, Wörter big-endian nach
   `tosdump/core/record.c:42-56`) direkt aus beiden BINs gelesen.
4. Das Soll aus der gepinnten EmuTOS-Quelle abgeleitet
   (`references/emutos` @ `1a7484ab`): `wm_create` (`aes/gemwmlib.c:1373-1384`),
   `NUM_WIN` (`include/sysconf.h:31-35`), `AES_VERSION` (`include/config.h:1347`),
   Handle 0 dauerhaft `VF_INUSE` (`aes/gemwmlib.c:1290` + `:213`), `wm_new` fegt
   nur `wh ≥ 1` (`:1886`).
5. Die Spec gegengelesen: `docs/api/aes/window-graphics.md:222-238`, `:769`,
   `:1018-1020` stimmen mit der Quelle überein. Kein Spec-Fehler.
6. Unsere Umsetzung gelesen: `tos-api/src/aes/window.rs:210`, `:213`,
   `:1831-1859`, `:2474`.
7. Geprüft, ob es zu diesem Übergang schon einen Test gibt — ja:
   `tos-api/tests/aes_reference.rs:83-105`. Ausgeführt statt nachgebaut.
8. Zusätzlich fünf Grenzfälle gefahren, die die eingefrorene Tabelle nicht
   abdecken, über einen **temporären** Testaufsatz
   (`tos-api/tests/zz_audit_a10_tmp.rs`) — danach gelöscht.
9. Neue Vergleichsdatei mit Nummer ab 08 angelegt, erzeugende Befehle im Kopf.

## Artefakte

- `measurements/measurement-2026-09-09/diff/08-a10-wind-create-kapazitaet.txt` — Rohwerte der
  Fälle 131/154/170 aus beiden BINs, `--validate`-Ausgabe, SHA-256 der
  beteiligten Dateien, tosdiff-Auszug, erzeugende Befehle im Kopf.
- `TASK/A10.md`, Abschnitt „Auswertung — 2026-09-09"; Statuszeile auf ERLEDIGT.
- `TASK/README.md`, Statusspalte der A10-Zeile auf ERLEDIGT.

Eingefrorene Artefakte (`cases/*.tsv`, `*.BIN`, `*.LOG`, `MESSRUN.TXT`,
`RUNINFO.TXT`) wurden nicht angefasst. Bestehende Diff-Dateien 00–07 unverändert.

## Ausgeführte Prüfung

    $ ./tosdump/host/build/tosdiff --validate \
          measurements/measurement-2026-09-09/run2-pi4-r2wind/AES.BIN tosdump/cases/aes-r2-wind.tsv
      → 78 records, build 70ca, case CRC AFB11C9F, accepted
    $ ./tosdump/host/build/tosdiff --validate \
          tosdump/reference/EMUTOS14-AES-r2wind.BIN tosdump/cases/aes-r2-wind.tsv
      → 78 records, build c911, case CRC AFB11C9F, accepted

    Rohwertlesung der Fälle 131/154/170 aus beiden BINs (eigener Leser).

    $ cargo test -p tos-api --test aes_reference \
          wind_create_hands_out_fifteen_application_handles
      → test result: ok. 1 passed

    Temporärer Zusatzaufsatz (danach entfernt), alle grün:
      1. fünfzehn wind_create liefern genau 1..15 in dieser Reihenfolge
      2. der sechzehnte liefert -1 und schreibt nur int_out[0]; [1..6] bleibt Poison
      3. ein weiterer Aufruf danach liefert wieder -1 (kein zerstörter Zustand)
      4. bei vollem Pool wind_delete(7) + wind_create → 7 (tiefster freier Slot)
      5. wind_get(15, WF_FXYWH) → 1, wind_get(16, WF_FXYWH) → 0

## Ergebnis

Abnahme erfüllt, **Gleichheit**, kein Defekt, keine Codeänderung.

| Fall | Aufruf | Pi 4 (`70ca`) | TT EmuTOS 1.4 (`c911`) | Soll |
|---|---|---|---|---|
| 131 | erster `wind_create` nach dem Reset | `1` | `1` | `1` |
| 154 | der neunte `wind_create` | `9` | `9` | `9` |
| 170 | der sechzehnte `wind_create` | `-1` | `-1` | `-1` |

Soll aus `gemwmlib.c:1373-1384` mit `NUM_WIN = 16` (`sysconf.h:31-35` bei
`AES_VERSION 0x0340`, `config.h:1347`) und Handle 0 dauerhaft belegt
(`:1290`, `:213`): fünfzehn Anwendungshandles 1..15, der sechzehnte Aufruf
liefert `-1`. Der neunte Aufruf ist keine Grenze und liefert `9`.

An allen drei Fällen weichen ausschließlich die sechs `global[]`-Wörter
(2, 3, 4, 10, 11, 12) ab — Klasse A06–A09, hier nicht bewertet. `contrl`,
`int_in`, `int_out` und der Trap-Rückgabewert sind bitgleich.

**Der alte Fall 154 ist kein aktueller Defekt.** Er war einer, solange
`MAX_WINDOWS` auf 8 stand; heute steht die Konstante auf 15
(`tos-api/src/aes/window.rs:210`) und der Istwert ist `9`. Die Vorhersage in
`MESSRUN.TXT` und der Fallkommentar in `aes-r2-wind.tsv:105-108` sind veraltet,
nicht der Lauf — das deckt sich mit M01.

## Offen

- **Keine Hardwareabnahme offen.** Der Istwert stammt aus einem Lauf auf realer
  Pi 4 mit genau dem Build, der auch am Kopf des Zweigs steht (`70ca` =
  `70caee2`).
- **Kein Fix offen.**
- Nicht geprüft, weil außerhalb des Auftrags: die `global[]`-Abweichung an
  denselben Fällen (A06–A09) und der Wiederanlauf der Handle-Vergabe nach
  `wind_delete` als eigener Fall 212.
- Ohne Fehlerwirkung, nur als Wartungshinweis notiert: `MAX_WINDOWS`
  (`tos-api/src/aes/window.rs:210`) ist bei uns nicht aus `AES_VERSION`
  (`tos-api/src/aes/appl.rs:84`) abgeleitet, während EmuTOS `NUM_WIN` per
  `#if (AES_VERSION > 0x0320)` daran koppelt. Bei 0x0340 identisch, also durch
  die API nicht beobachtbar.
- Die fünf zusätzlich gefahrenen Grenzfälle liegen nicht dauerhaft im Baum. Sie
  belegen kein Problem; sie festzuhalten wäre eine Empfehlung, keine Auflage.
