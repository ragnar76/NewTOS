# Q07 — Quittung

**Status: DONE** · 2026-09-12 · `Fattrib` GET loest jetzt auch die Schreibweise
mit Trennzeichen am Pfadende auf. QED startet **zweimal hintereinander auf
demselben Abbild ohne Alarm** (Bildschirmabzuege), und cflibs `app_init` findet
sein Programmverzeichnis wieder. **Nicht mitentschieden und ausdruecklich
offen:** die Laufwerkswurzel `Fattrib("C:\", 0, 0)` bleibt `EFILNF (-33)`.

## Was geaendert wurde

| Datei | Stelle | Was |
|---|---|---|
| `tos-api/src/bdos/fs/file.rs` | neu `without_trailing_seps` (vor `fattrib`) | schneidet `\` und `/` am Pfadende ab, **nur solange danach eine Komponente uebrig bleibt** — `"C:\"`, `"\"`, `""` bleiben unangetastet |
| `tos-api/src/bdos/fs/file.rs` | neu `subdir_attrib` | loest einen Pfad auf und gibt das Attributbyte **nur** zurueck, wenn der Eintrag ein Verzeichnis ist |
| `tos-api/src/bdos/fs/file.rs` | `fattrib`, GET-Zweig (`wrt == 0`), erste Anweisung | probiert die gekuerzte Schreibweise; trifft sie kein Verzeichnis, faellt alles in den **unveraenderten** Code darunter |
| `tos-api/tests/gemdos_fattrib_dir.rs` | neu | 3 Tests: die gehobene Form, die sieben unveraenderten Schreibweisen (inkl. Wurzel und SET), die Nachbarn `Fsfirst`/`Fopen`/`Dcreate` |
| `docs/api/gemdos/file/43-fattrib.md` | Divergenz-Block, Fehlertabelle, Abnahmekriterien, Edge-Case 1, Versionsnotiz, Implementierungsnotizen | die beiden falschen Behauptungen (`path::findit` verwerfe die leere Endkomponente) korrigiert; beschreibt jetzt das **tatsaechliche** Verhalten samt Wurzel-Ausnahme |
| `docs/COMPATIBILITY.md` | Abschnitt „Sanctioned FreeMiNT/TOS4 extensions" | die Abweichung deckt jetzt ausdruecklich **beide** Schreibweisen, mit der Wurzel als benannter offener Punkt |
| `docs/STATUS.md` | Phase-09-Zeile + datierter Recent-Decisions-Eintrag | im selben Arbeitsgang, **ohne** `git stash` |

**Nicht angefasst:** `path::resolve_parent`, `path::findit`, `dir::fcb_name`,
`sdk/apps/qed/`, `sdk/apps/teradesk/`, `sdk/cflib/`,
`kernel/src/dbg_qedtrace.rs`, `kernel/src/selftest_emmc.rs`.

## Warum die enge Fassung reicht — selbst nachgeprueft

Der vorgeschlagene Fix („Trennzeichen abschneiden") haette **drei** Faelle
mitverschoben, nicht einen:

* `Fattrib("C:\TEST.TXT\", 0, 0)` — heute `-34`; nach blindem Abschneiden `32`
  (das Attributbyte der **Datei**).
* `Fattrib("C:\NOSUCHD\", 0, 0)` — heute `-34`; nach blindem Abschneiden `-33`.
* `Fattrib("C:\", 0, 0)` — die Wurzel, die ausdruecklich nicht mitentschieden
  werden darf.

FreeMiNT teilt diese Antworten **nicht** pauschal: `relpath2cookie` prueft am
Kopf jeder Schleifenrunde `if (!S_ISDIR(xattr.mode)) r = ENOTDIR;`
(`references/freemint/sys/filesys.c:970-976`), und `ENOTDIR` ist dasselbe `-34`
(`sys/mint/errno.h:277`). Fuer die Datei mit Trennzeichen liegen EmuTOS,
TOS 3.06 **und** FreeMiNT also auf `-34` — es gibt dort nichts zu heben.

Deshalb ist der Eingriff als **Sonde** gebaut statt als Umschreibung: die
gekuerzte Schreibweise wird nur angenommen, wenn sie ein Verzeichnis benennt.
Alles andere laeuft durch den alten Code. Die Abweichung ist damit streng
additiv — ein bisher negativer Rueckgabewert wird positiv, und zwar in genau
einem Fall.

## `Fattrib("C:\", 0, 0)` — die Teilentscheidung, die nicht getroffen wurde

**Heute: `-33` (EFILNF). Nach dem Fix: `-33`. Unveraendert, in beide
Richtungen.** Gemessen, nicht zugesichert (Fall 06 der Pruefreihe unten).

Zwei Sicherungen, damit das nicht durch eine spaetere Bearbeitung kippt:

1. `without_trailing_seps` gibt fuer `"C:\"` bewusst `None` zurueck — der
   Doc-Kommentar sagt, warum: die Wurzel hat keinen Verzeichniseintrag, dessen
   Attributbyte man lesen koennte, und FreeMiNTs synthetisches `FA_DIR` ist eine
   eigene Entscheidung.
2. `tos-api/tests/gemdos_fattrib_dir.rs` nagelt `Fattrib("C:\", 0, 0) == EFILNF`
   und `Fattrib("C:/", 0, 0) == EFILNF` fest.

**Offen bleibt damit:** cflibs Dateiauswahl (`filesel.c:205`, `make_normalpath`
auf Benutzereingaben) laeuft fuer eine reine Laufwerkswurzel weiterhin in
dieselbe Fehlerklasse. Das ist eine Vorlage, keine Erledigung.

## Beleg 1 — QED, zweimal auf demselben Abbild

QEMU `raspi4b`, Kernel `--board rpi4 --release`, Abbild aus
`tools/make-sd-image.sh`, QED ueber `#Z 01 C:\Apps\QED\QED.NAP@` in
`C:\NEWDESK.INF`, Bildschirmabzug ueber QMP `screendump`.

| Lauf | Vorbedingung | Ergebnis |
|---|---|---|
| 1 | frisches Abbild, **kein** `C:\CLIPBRD` | QED startet sauber; danach liegt `C:\CLIPBRD` im Abbild (nachgesehen) |
| 2 | **dasselbe** Abbild, `C:\CLIPBRD` existiert | QED startet sauber, Menueleiste steht, **kein Alarm** — bei 45 s und bei 60 s |

Der Gegenbeweis liegt daneben: derselbe Aufbau mit dem Kernel **vor** dem Fix
zeigt im zweiten Lauf die Alarmbox „Klemmbrett-Ordner konnte nicht angelegt
werden!" (Bildschirmabzug der Q07-Diagnose, im Job-Scratch nachgesehen).

## Beleg 2 — die Rueckgabewerte, auf dem echten Kernel gemessen

Ein EL0-Konsolenprogramm in `C:\AUTO\` stellt dieselben 18 Fragen wie die
Q07-Diagnose plus die beiden `app_init`-Pfade; Ausgabe ueber die serielle
Leitung. Verglichen wurde Zeile fuer Zeile mit der Messung **vor** dem Fix:

```
$ diff vorher nachher
3c3
< 01 Fattrib  dir+sep  "C:\CLIPBRD\" -> -33
---
> 01 Fattrib  dir+sep  "C:\CLIPBRD\" -> 16
```

**Genau eine Zeile bewegt sich.** Unveraendert, jeweils mit dem alten Wert:

```
03 Fattrib  file     "C:\TEST.TXT"  ->  32      12 Dsetpath root     "C:\"         ->   0
04 Fattrib  file+sep "C:\TEST.TXT\" -> -34      13 Fsfirst  dir 0x10 "C:\CLIPBRD"  ->   0
05 Fattrib  missing  "C:\NOSUCHD\"  -> -34      14 Fsfirst  dir+sep  "C:\CLIPBRD\" -> -33
06 Fattrib  root     "C:\"          -> -33      15 Fsfirst  file     "C:\TEST.TXT" ->   0
07 Fattrib  dir+dot  "C:\CLIPBRD\." -> -33      16 Dcreate  fresh    "C:\Q07NEW"   ->   0
08 Dcreate  existing "C:\CLIPBRD"   -> -36      17 Fattrib  fresh    "C:\Q07NEW"   ->  16
09 Dcreate  exist+se "C:\CLIPBRD\"  -> -34      18 Ddelete  fresh    "C:\Q07NEW"   ->   0
```

Und die beiden neuen Faelle, der zweite Nutzniesser:

```
19 Fattrib  appdir+s "C:\Apps\QED\" ->  16      (vorher -33)
20 Fattrib  appdir   "C:\Apps\QED"  ->  16
```

Damit liefert `path_exists(gl_appdir)` in `sdk/cflib/cflib-21/appinit.c:104-107`
`TRUE`, und der Rueckfall `get_path(gl_appdir, 0)`, der das Programmverzeichnis
mit dem **aktuellen** Verzeichnis ueberschreibt, wird nicht mehr betreten.

## Beleg 3 — Nicht-Regression mit Tests, nicht mit Zusicherung

`tos-api/tests/gemdos_fattrib_dir.rs` fuehrt `Fsfirst`, `Fopen` und `Dcreate`
auf `"C:\SUB\"` mit (alle **vor** dem Fix schon gruen, also echte Waechter):

* `Fsfirst("C:\SUB\", 0x37)` = `EFILNF` — `scan(builds(""))`, `bdos/fsdir.c:462-470`
* `Fopen("C:\SUB\", 0)` = `EFILNF` — `bdos/fsopnclo.c:246-257`
* `Dcreate("C:\SUB\")` = `EPTHNF` — `ixcreat`s eigene Sperre, `bdos/fsopnclo.c:117-118`
* `Dcreate("C:\SUB")` = `EACCDN` — ueberall gleich; der zweite Teil von QEDs
  Kette war nie der Fehler

Von den drei neuen Tests scheiterte **vor** dem Fix genau einer, und zwar mit
`Fattrib("C:\SUB\", 0, 0) -> -33`. Die beiden Nicht-Regressionstests waren
vorher wie nachher gruen.

Dazu weiterhin gruen: `tos-api/tests/gemdos_fs_paths.rs`, das denselben
Grenzfall fuer `Fsfirst`/`Fopen`/`Fcreate`/`Dcreate` aus einer TeraDesk-Spur
festhaelt. **Kein bestehender Test musste angefasst werden.**

## Weitere Nicht-Regression

* Eingebauter Desktop **ohne** `#Z`: kommt normal hoch, `C:`-Fenster mit
  26 Eintraegen (Bildschirmabzug, `[desk] desktop up (1 drives)`).
* TeraDesk ueber `#Z 01 C:\Apps\TERADESK\DESKTOP.NAP@`: startet, Menueleiste +
  Laufwerks-/Drucker-/Papierkorbsymbole stehen (Bildschirmabzug).

## Abnahme

| Punkt | Ergebnis |
|---|---|
| `cargo run -p xtask -- test` | vorher **478 passed / 0 failed / 1 ignored**, nachher **481 / 0 / 1** |
| `build --board rpi4` | sauber, frisch gebaut (`tos-api` 64 Warnungen, `kernel` 94 — Ausgangsstand) |
| `build --board qemu-virt` | sauber, frisch gebaut (dieselben Zahlen) |
| `cargo clippy -p tos-api` | **81 vorher, 81 nachher**; keine einzige Warnung zeigt auf `bdos/fs/file.rs`, die neue Testdatei ist ebenfalls warnungsfrei |
| QED zweimal auf demselben Abbild | **ohne Alarm**, beide Laeufe |

Die verbindliche Verhaltensabnahme (Selftests, Screendumps, `fsck_msdos`, beide
Boards) gehoert dem Tester.

## Offen

1. **`Fattrib("C:\", 0, 0)`** — `EFILNF` wie EmuTOS/TOS 3.06; FreeMiNT liefert
   `FA_DIR`. Bewusst nicht entschieden, siehe oben.
2. `kernel/src/selftest_emmc.rs:490` prueft diesen Fall, ist aber in **keinem**
   Bootpfad verdrahtet und druckt `true`/`false`, statt zu scheitern. Nach dem
   Fix wuerde Zusicherung (a) `true` drucken — belegen kann sie nichts, solange
   sie nicht laeuft. Unangetastet gelassen; gefuehrt in [A53](A53.md).
3. `Fattrib("C:\SUBDIR\.", 0, 0)` = `-33` (Fall 07), wie EmuTOS und TOS 3.06.
   FreeMiNTs `fatfs_lookup` behandelt `"."` wie den leeren Namen und wuerde das
   Attribut liefern. Ausserhalb dieses Auftrags, unveraendert gelassen.
4. Fall 04/05 (`-34` gegen TOS 3.06s `-33`): bekannter Nebenbefund der
   Diagnose, wir folgen EmuTOS. Unveraendert.
5. **TOS 4.04 bleibt ungemessen** — kein ROM auf diesem Rechner. Ob TOS4 die
   Form mit Trennzeichen aufloest, ist weiterhin unentschieden; die Begruendung
   des Fixes stuetzt sich auf FreeMiNT, nicht auf eine TOS4-Vermutung.

## Ausserhalb des Auftrags beobachtet (nicht angefasst)

* `tos-api/src/bdos/fs/dir.rs` `fcb_name("")` liefert `Some([b' '; 11])` — elf
  Leerzeichen als gueltiger Name. Fuer jeden Aufrufer heute folgenlos (kein
  Eintrag traegt ihn), aber ein `None` waere ehrlicher.
