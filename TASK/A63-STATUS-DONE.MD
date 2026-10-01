# A63 — STATUS: DONE

Die Felder des Dateiauswaehlers bleiben `G_FTEXT`/`G_FBOXTEXT` und behalten ihre
Schablone; die eigenen Puffer tragen dafuer die Rohform. Bearbeitet 2026-09-17,
Branch `desk-rebuild`, Basis HEAD `c4b25e9`, Baum vorher sauber.
Messlauf: [`measurements/measurement-2026-09-17-fsel/`](../measurements/measurement-2026-09-17-fsel/RUNINFO.TXT).
Gemeinsam mit [A64](A64.md) bearbeitet; A64 zuerst, Begruendung in
[A64-STATUS-DONE.MD](A64-STATUS-DONE.MD).

## Der Fehler, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll), Felder setzen | `references/emutos/aes/gemfslib.c:685-693` (`inf_sset`, `fmt_str(pisel, selname)`) |
| EmuTOS (Soll), Listenschlitze | `references/emutos/aes/gemfslib.c:359-381` (`fs_format`) |
| EmuTOS (Soll), Rueckgabe | `references/emutos/aes/gemfslib.c:827` und `:914-916` (`unfmt_str`) |
| EmuTOS (Soll), Rohform | `references/emutos/util/optimize.c:43-69` (`fmt_str`), `:77-96` (`unfmt_str`), `:103-112` (`inf_sset`) |
| EmuTOS (Soll), Zeichnung | `references/emutos/aes/gemoblib.c:718-720` (`ob_format(te_just, te_ptext, te_ptmplt, …)`) |
| NewTOS (vorher) | `tos-api/src/aes/fsel.rs::set_ted` — Typ auf `G_BOXTEXT`/`G_TEXT`, `te_ptmplt`/`te_pvalid` geleert, `te_txtlen = cap`, `te_color`/`te_just` ueberschrieben |
| NewTOS (nachher) | `set_ted` biegt **nur** `te_ptext` um; `fs_format`/`run`/`modal` fuellen die Rohform |

Gepinnte Quelle: `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.

## Warum die Herabstufung drei Dinge auf einmal gekostet hat

Die Schablone traegt die **Literale**. Gemessen am ausgelieferten
`tos-api/src/aes/gem_fsel.nrsc`:

| Objekt | `te_ptmplt` | `te_pvalid` | `te_txtlen` |
|---|---|---|---|
| `FSDIRECT` (ted 0) | 38 Unterstriche | 38 x `P` | 39 |
| `FSSELECT` (ted 1) | `Selection: ________.___` | 11 x `F` | 12 |
| `F1NAME`..`F9NAME` (ted 3..11) | `_ ________.___ ` | `xFFFFFFFFFFF` | 13 |

Also: die Unterstriche **sind** die sichtbare Feldbegrenzung (was der Anwender
als "fehlenden Rahmen" gelesen hat), die Beschriftung `Selection:` steht in der
Schablone und in keinem eigenen Objekt, und der `.` zwischen Name und Endung ist
ebenfalls Schablone — deshalb die festen 8.3-Spalten. Eine geleerte Schablone
nimmt alle drei gleichzeitig weg. Ein Schlitz hinter dem Listenende bekommt in
EmuTOS genau ein Leerzeichen (`:376-377`); `ob_format` fuellt die uebrigen
Positionen mit `_` auf (`gemoblib.c:402-403`) — daher die Unterstrichzeilen.

## Die Frage aus dem Auftrag: hatte die Herabstufung einen zweiten Grund?

**Nein.** Die beiden Begruendungen, die im Quelltext standen, sind beide erledigt:

1. **"unsere Puffer halten einfache Zeichenketten"** — das war der wahre Grund
   und ist behoben, indem die Puffer jetzt die Rohform tragen (siehe unten).
2. **`te_txtlen = cap` als Umgehung von A37** — gegenstandslos. A37 hat zwei
   Haelften; die zweite (der konvertierte Blob reserviert die zugesagten
   `te_txtlen` Byte hinter `te_ptext`) ist erledigt und durch
   `tos-api/tests/aes_rsrc_tedinfo_reserve.rs` bewacht. Am Blob nachgemessen:
   `ted[0].te_ptext` bei Offset 2944, das naechste bei 3061 — 117 Byte fuer
   `te_txtlen` 39; die Schlitze haben je 42 Byte fuer 13. Es bleibt **eine**
   echte Schranke, und die betrifft nicht den Blob, sondern **unsere** Puffer:
   jeder Schreiber begrenzt sich an `te_txtlen` (`ins_char`,
   `gemobed.c:117`), also darf ein Feld nur umgebogen werden, wenn unser Puffer
   die zugesagte Laenge fasst. Das steht jetzt als Bedingung in `set_ted`
   (96 >= 39, 16 >= 12, 16 >= 13 — sie greift an keiner der drei Stellen).

`te_color = BLACK_TEXT` und `te_just = 0` waren ebenfalls ueberfluessig: die
Ressource traegt `te_color = 0x1100` (schwarze Schrift) und `te_just = TE_LEFT`.
Beide Zuweisungen sind mit entfallen.

## Was die Puffer jetzt tragen

| Puffer | Inhalt | Wandlung |
|---|---|---|
| `SYS.dir` (96) | der Pfad, unveraendert | die Schablone hat nur Datenplaetze, die Rohform ist der Pfad |
| `SYS.sel` (16) | `fmt_str` der Auswahl, z. B. `CONFIG  TXT` | herein `fmt_str` (`gemfslib.c:688`), heraus `unfmt_str` (`:915`) |
| `SYS.names[i]` (16) | Kennbyte + `fmt_str(name)`, z. B. `" CONFIG  TXT"`; `\007` fuer einen Ordner (`:254`) | `fs_format`; ein Schlitz hinter dem Listenende ein einzelnes Leerzeichen (`:376-377`) |

Beim Anklicken eines Dateischlitzes schreibt der Modalzweig `fmt_str(name)` in
`SYS.sel` — EmuTOS' `strcpy(ad_fname, selname + 1)` (`:827`), also der Rohtext
des Schlitzes ohne sein Kennbyte.

`fs_format` setzt jetzt zusaetzlich `ob_type = G_FBOXTEXT` auf jedem Schlitz
(`:380`) — dieselbe Zeile, die im Quelltext von EmuTOS beweist, dass die Schlitze
Schablonenfelder sind und bleiben.

## Die Beschriftung "Auswahl:" — geprueft, gehoert zu A63

Der Auftrag liess offen, ob die fehlende Beschriftung aus der Herabstufung folgt.
**Sie folgt daraus.** Es gibt kein `G_STRING`-Objekt fuer sie; sie ist die
Literalvorlaufstrecke der `FSSELECT`-Schablone (`"Selection: ________.___"`,
ted 1 im Blob). Mit wiederhergestellter Schablone erscheint sie — im Abzug
belegt. Dass dort `Selection:` statt `Auswahl:` steht, ist die `nls`-Frage aus
dem Nebenbefund und **nicht mitgeaendert**.

## Test vor Fix

`tos-api/src/aes/fsel.rs`, Modul `tests`. Rote Ausgabe vollstaendig in
[`measurements/measurement-2026-09-17-fsel/build/newtest-RED.txt`](../measurements/measurement-2026-09-17-fsel/build/newtest-RED.txt):

```
---- set_ted_keeps_the_formatted_type_template_and_length ----
assertion `left == right` failed: FSDIRECT type
  left: 22            (G_BOXTEXT — herabgestuft)
 right: 30            (G_FBOXTEXT)

---- fs_format_fills_the_slots_with_the_indicator_and_the_raw_name ----
assertion `left == right` failed: file slot
  left:  [67, 79, 78, 70, 73, 71, 46, 84, 88, 84, 0, 0]     "CONFIG.TXT"
 right:  [32, 67, 79, 78, 70, 73, 71, 32, 32, 84, 88, 84]   " CONFIG  TXT"
```

Der dritte Fall (`fmt_str_and_unfmt_str_convert_between_plain_and_raw_8_3`) prueft
die neuen Wandler gegen EmuTOS' eigene Beispiele im Kommentar von
`util/optimize.c:30-41` (`SAMPLE.PRG`, `TESTPROG.C`, `TEST`, `TESTWINDOW.C`,
`TEST.A.B.C`, `TESTTESTTEST`) und war von Anfang an gruen — die Wandler sind das
Werkzeug, nicht der Fix. Danach alle sechs gruen, siehe
[`newtest-GREEN.txt`](../measurements/measurement-2026-09-17-fsel/build/newtest-GREEN.txt).

## Bildabnahme

Referenz: `docs/analysis/fsel-reference-2026-09-17/fselectr.png`.
Unser Abzug: `measurements/measurement-2026-09-17-fsel/screendumps/fsel-after.png`
(QEMU `raspi4b`, headless, 640x480).

**Gemessen** (Programm
[`method/compare-geometry.py`](../measurements/measurement-2026-09-17-fsel/method/compare-geometry.py),
Schwarzlaeufe je wurzelrelativer Zeile): Wurzelkasten in beiden Bildern
**320x356**, **338 von 356** Zeilen bitgleich; die 18 uebrigen sind in
[A64-STATUS-DONE.MD](A64-STATUS-DONE.MD) einzeln zugeordnet, keine davon eine
Schablonen- oder `fs_start`-Frage.

**Angesehen** (nicht gemessen, weil die Referenz keine Dateien listet — `rscview`
rendert die leere Ressource):

| | Referenz | vorher | jetzt |
|---|---|---|---|
| Verzeichnisfeld | Unterstrichzeile | graue Leiste | `C:\*.*` + Unterstriche bis Spalte 38 |
| Auswahlfeld | `________.___` mit Beschriftung | graue Leiste, Beschriftung fehlt | `Selection: ________.___` |
| Dateiliste | feste 8.3-Spalten | `CONFIG.TXT` | `BIG     .NCP`, `CLOCKTST.NCP`, `CONFIG  .TXT` |
| Ordnerzeilen | — (Referenz listet nichts) | `ACC` | Kennglyphe + `ACC_____.___` |

Der Abzug zeigt neun belegte Schlitze, also keine leere Zeile; der Fall
"Schlitz hinter dem Listenende = eine Unterstrichzeile" ist durch den Hosttest
abgedeckt, nicht durch das Bild. Im Abzug fehlt ausserdem die Ueberschrift
`ITEM SELECTOR`: `fsel::preview()` ruft `apply_title` nicht auf — eine Grenze des
Vorschaupfads, kein Befund von A63/A64.

## Beleglage

| | |
|---|---|
| Hosttests | 551 → **557 gruen / 0 rot / 1 ignoriert** |
| Bau rpi4 / qemu-virt (release, frisch) | beide `rc=0`, tos-api 64 rustc, kernel 94 rustc — Grundlinie |
| Clippy | tos-api 81, kernel 125 (beide Boards) — Grundlinie |
| Rueckbau der Messverdrahtung | `md5(kernel/src/lib.rs)` vorher = nachher = `5434f7b25c5641c7fab1fce0b1c98b85`, von Hand, kein `git checkout` |

## Offen

- Abnahme durch den Tester, insbesondere das **Tippen** in beiden Feldern auf
  echter Pi 4: `te_pvalid` ist zurueck, also pruefen `FSDIRECT` gegen `P`
  (`.?*a-zA-Z0-9_\:`, Grossschreibung erzwungen) und `FSSELECT` gegen `F`
  (`:?*a-zA-Z0-9_`, kein `.`) — ein getippter Punkt springt in `FSSELECT` jetzt
  auf die Endung, statt eingefuegt zu werden. Das ist EmuTOS-Verhalten
  (`gemobed.c:424-438`), aber es ist eine spuerbare Aenderung gegenueber der
  bisherigen "alles erlaubt"-Fassung.
- Nebenbefund gemeldet, nicht geaendert: gemischte Beschriftungen
  (deutscher Titel, englische Felder) — `nls`, eigenes Ticket.
- Nicht committet; die Arbeit liegt im Baum.
