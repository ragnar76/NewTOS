# Q07 — „Klemmbrett-Ordner konnte nicht angelegt werden", obwohl er existiert

**Datum:** 2026-09-12 · **Typ:** Anwendungsdiagnose (Ursache, keine Reparatur)
**Ticket:** [TASK/Q07.md](../../TASK/Q07.md) · **Branch:** `desk-rebuild`
**Status:** Ursache **belegt und gemessen**. Kein Produktcode geändert.

---

## Kurzfassung

QEDs Klemmbrett-Prüfung fragt über cflib mit
`Fattrib("C:\CLIPBRD\", 0, 0)` — **mit abschließendem Backslash**. NewTOS
beantwortet genau diese Form mit `EFILNF (-33)`; ohne Backslash liefert es
seit dem TOS4-Lift vom 2026-07-21 korrekt `0x10`. Weil die Prüfung damit
scheitert, geht QED in den Anlege-Zweig, `Dcreate` auf den **bestehenden**
Ordner liefert `EACCDN (-36)`, und QED zeigt den Alarm. Die Spezifikation
behauptet an zwei Stellen, die Backslash-Form sei bereits abgedeckt — das ist
falsch.

---

## 1. Reproduktion (selbst nachgeprüft, nicht übernommen)

QEMU `raspi4b`, Kernel `--board rpi4 --release --features gemdos-trace`,
SD-Abbild aus `tools/make-sd-image.sh`, QED über die `#Z`-Autorun-Zeile in
`C:\NEWDESK.INF` gestartet (`#Z 01 C:\Apps\QED\QED.NAP@`), Bildschirmabzug
über QMP `screendump`.

| Lauf | Vorbedingung | Ergebnis |
|---|---|---|
| 1 | frisches Abbild, **kein** `C:\CLIPBRD` | QED startet sauber, Menüleiste steht, **kein** Alarm. Danach liegt `C:\CLIPBRD` im Abbild. |
| 2 | **dasselbe** Abbild, `C:\CLIPBRD` existiert | Alarmbox „Klemmbrett-Ordner konnte nicht angelegt werden!" |

Die Reproduktion aus dem A38-Nebenbefund ist damit **bestätigt**. QED legt den
Ordner im ersten Lauf selbst an und scheitert im zweiten an seinem eigenen Werk.

Belege (Messartefakte im Job-Scratch, nicht im Repo):
`run1/shot-55s.png` (sauberer Start), `run2/shot-45s.png` (Alarmbox),
`run3/QEDTRACE.LOG` (Syscall-Mitschnitt).

---

## 2. Was QED tatsächlich aufruft — gemessen, nicht gelesen

Für den Mitschnitt wurde der **bereits im Baum liegende** temporäre
EL0-Syscall-Tracer `kernel/src/dbg_qedtrace.rs` benutzt (Schalter
`QED_SVC_TRACE`), weil er beim `evnt_multi`-Eintritt schreibt und deshalb auch
dann noch eine Datei hinterlässt, wenn QED in der modalen Alarmbox hängt. Der
Schalter wurde nach dem Lauf wieder auf `false` gesetzt; die Datei ist
byteidentisch zum Ausgangszustand (SHA-256 vorher/nachher gleich).

Auszug `C:\QEDTRACE.LOG`, Lauf 2 (Ordner existiert):

```
AES    op=80  scrp_read    -> 1          (liefert leeren Pfad)
GEMDOS #1  fn=0x19 Dgetdrv   -> 2
GEMDOS #1  fn=0x0e Dsetdrv   -> 4
AES    op=81  scrp_write   -> 1
GEMDOS #1  fn=0x43 Fattrib   -> -33
    Fattrib  path="C:\CLIPBRD\"
GEMDOS #1  fn=0x39 Dcreate   -> -36
    Dcreate  path="C:\CLIPBRD"
AES    op=78  graf_mouse   -> 1
AES    op=54  form_center  -> 1          (die Alarmbox)
```

### Die Aufrufkette mit `Datei:Zeile`

1. `sdk/apps/qed/src/clipbrd.c:262` — `scrp_read(clip_dir)` liefert einen
   leeren Pfad (AES-Scrap-Verzeichnis ist beim Booten leer; EmuTOS `sc_read`
   verhält sich identisch, `references/emutos/aes/gemsclib.c:40-45`).
2. `sdk/apps/qed/src/clipbrd.c:265-278` — kein `SCRAPDIR`/`CLIPBRD` im
   Environment, also `get_first_drive()` (`clipbrd.c:233-255`, die gemessenen
   `Dgetdrv`/`Dsetdrv`) → `clip_dir = "C:\CLIPBRD\"`, **mit** Backslash am Ende.
3. `sdk/apps/qed/src/clipbrd.c:279` — `scrp_write(clip_dir)`.
4. `sdk/apps/qed/src/clipbrd.c:281` — `if (!make_normalpath(clip_dir))`.
5. `sdk/cflib/cflib-21/fsnorm.c:85-92` — `make_normalpath` **hängt in jedem
   Fall einen Backslash an**, bevor es prüft:
   ```c
   i = (int)strlen(p);
   if (p[i - 1] != '\\') { p[i] = '\\'; p[i + 1] = '\0'; }
   ret = path_exists(p);
   ```
   Der Prüfpfad lautet also immer `"C:\CLIPBRD\"`.
6. `sdk/cflib/cflib-21/fsexists.c:41` — `path_exists`:
   ```c
   if ((attr = Fattrib(pathname, 0, 0)) >= 0 && (attr & FA_DIR) != 0) r = TRUE;
   ```
   → `Fattrib("C:\CLIPBRD\", 0, 0)` = **-33** → `FALSE`.
7. `sdk/apps/qed/src/clipbrd.c:288-294` — QED schneidet den Backslash ab und
   legt an:
   ```c
   strcpy (s, clip_dir); s[strlen(s)-1] = EOS;
   if (Dcreate(s) != 0) { note(1, 0, NOSCRAP); clip_dir[0] = EOS; }
   ```
   → `Dcreate("C:\CLIPBRD")` = **-36** → Alarm `NOSCRAP`
   (`sdk/apps/qed/src/qed.h:512`), Klemmbrett abgeschaltet.

**QED unterscheidet an dieser Stelle nicht zwischen „gibt es schon" und
„geht nicht"** — das ist so in der Anwendung angelegt und bleibt unangetastet.
Der Fehler liegt eine Ebene tiefer: die Prüfung in Schritt 6 hätte `TRUE`
liefern müssen, dann wird Schritt 7 nie erreicht.

---

## 3. Was wir liefern, was EmuTOS liefert, was TOS 3.06 liefert

Ein eigenes Prüfprogramm stellt in **einer** Quelle dieselben 18 Aufrufe an
alle drei Systeme; auf der m68k-Seite freistehend (kein mintlib, nur
`trap #1`), auf der AArch64-Seite über `sdk/build.sh` (`svc #1`).
EmuTOS/TOS liefen unter Hatari auf einer **echten FAT12-Diskette** (A:),
nicht über Hataris GEMDOS-Laufwerksemulation — gemessen wird also das BDOS des
jeweiligen Systems, nicht Hatari.

| # | Aufruf | **NewTOS** | EmuTOS 1.4 | TOS 3.06 |
|---|---|---|---|---|
| 01 | `Fattrib("X:\CLIPBRD\",0,0)` Ordner **+ Backslash** | **-33** | -33 | -33 |
| 02 | `Fattrib("X:\CLIPBRD",0,0)` Ordner | **16** | -33 | -33 |
| 03 | `Fattrib("X:\TEST.TXT",0,0)` Datei | 32 | 32 | 32 |
| 04 | `Fattrib("X:\TEST.TXT\",0,0)` Datei + Backslash | -34 | -34 | -33 |
| 05 | `Fattrib("X:\NOSUCHD\",0,0)` | -34 | -34 | -33 |
| 06 | `Fattrib("X:\",0,0)` Wurzel | -33 | -33 | -33 |
| 07 | `Fattrib("X:\CLIPBRD\.",0,0)` | -33 | -33 | -33 |
| 08 | `Dcreate("X:\CLIPBRD")` existiert | **-36** | -36 | -36 |
| 09 | `Dcreate("X:\CLIPBRD\")` | -34 | -34 | -34 |
| 10 | `Dsetpath("X:\CLIPBRD")` | 0 | 0 | 0 |
| 11 | `Dsetpath("X:\CLIPBRD\")` | 0 | 0 | 0 |
| 12 | `Dsetpath("X:\")` | 0 | 0 | 0 |
| 13 | `Fsfirst("X:\CLIPBRD",0x10)` | 0 | 0 | 0 |
| 14 | `Fsfirst("X:\CLIPBRD\",0x10)` | -33 | -33 | -33 |
| 15 | `Fsfirst("X:\TEST.TXT",0)` | 0 | 0 | 0 |
| 16 | `Dcreate("X:\Q07NEW")` frisch | 0 | 0 | 0 |
| 17 | `Fattrib("X:\Q07NEW",0,0)` frischer Ordner | **16** | -33 | -33 |
| 18 | `Ddelete("X:\Q07NEW")` | 0 | 0 | 0 |

Lesart:

* **Fall 02/17** ist der bereits bewusst übernommene TOS4-Lift — hier weichen
  wir von EmuTOS/TOS 1-3 ab, mit Absicht und vertraglich abgesegnet
  (`docs/COMPATIBILITY.md:215-228`).
* **Fall 01 ist die Lücke.** Genau die Form, die cflib erzeugt, ist vom Lift
  **nicht** erfasst.
* Fall 08 (`Dcreate` auf bestehenden Ordner → `EACCDN`) ist **überall gleich**
  und damit **kein** Fehler bei uns. Der zweite Teil von QEDs Kette ist korrekt.
* Fall 04/05 zeigen eine kleine Abweichung EmuTOS↔TOS 3.06 (`EPTHNF` gegen
  `EFILNF`); wir folgen dort EmuTOS. Nebenbefund, außerhalb dieses Auftrags.

### Wo es bei uns genau auseinandergeht

`tos-api/src/bdos/fs/file.rs:574` — `fattrib` löst mit `path::findit` auf.
`tos-api/src/bdos/fs/path.rs:160-161` — `resolve_parent` gibt für einen Pfad
mit Trennzeichen am Ende **die leere Restkomponente** zurück (bewusst, und
EmuTOS-treu: `references/emutos/bdos/fsdir.c:1761-1791`, `getpath` liefert 0,
und `findit` bricht mit leerem `*sp` ab).
`tos-api/src/bdos/fs/path.rs:168` — `findit` reicht die leere Komponente an
`dir::fcb_name` weiter; `tos-api/src/bdos/fs/dir.rs:148-173` liefert dafür
`Some([b' '; 11])`, also elf Leerzeichen.
`tos-api/src/bdos/fs/file.rs:591-593` — `find_in_dir` findet zu elf
Leerzeichen nichts → **`return EFILNF`**.

Das ist **exakt** EmuTOS' Weg (`xchmod` → `scan` auf `builds("")`,
`references/emutos/bdos/fsdir.c:402-416`) — nur dass EmuTOS auch Fall 02
verweigert und wir dort schon weiter sind. Wir sind auf halbem Weg stehen
geblieben.

---

## 4. Was ist das Soll?

### EmuTOS / TOS 1-3 — belegt, gemessen

`Fattrib` auf Ordner: `EFILNF`, in beiden Schreibweisen.
Quellenkommentar `references/emutos/bdos/fsdir.c:391-400`:
„In TOS versions prior to TOS4, Fattrib() only works with files, not folders …
At this time, EmuTOS behaves like Atari TOS 1-3." Durch die TOS-3.06-Messung
(Spalte 3) unabhängig bestätigt.

**Konsequenz, unbequem, aber wahr:** auf EmuTOS und auf einem echten TT mit
TOS 3.06 zeigt QED denselben Alarm. Unsere Fälle 01/08 sind gegenüber
EmuTOS/TOS 1-3 **nicht** falsch. Wer EmuTOS-Treue als alleinigen Maßstab
nimmt, müsste diesen Befund schließen und QED für kaputt erklären.

### TOS 4.04 — teilweise belegt, teilweise **unentschieden**

Belegt ist nur die Aufhebung der Ordner-Sperre (derselbe EmuTOS-Kommentar,
`fsdir.c:393-399`, samt der beiden TOS4-Einschränkungen beim Setzen).
**Nicht belegt** ist, ob TOS 4.04 auch die Form mit Trennzeichen am Ende
auflöst. Ein TOS-4.04-ROM liegt auf diesem Rechner **nicht** vor
(`~/Documents/Ingo/Technik & Projekte/tos4.zip` enthält `GEM.PRG` +
`DESKICON.RSC`, kein ROM). Da TOS4s BDOS dieselbe `findit`/`getpath`-Zerlegung
benutzt wie TOS 1-3, ist zu **vermuten**, dass auch TOS4 hier `EFILNF`
liefert — das ist eine Vermutung und wird hier ausdrücklich **nicht** als
Beleg geführt. Wer den Punkt braucht, muss ein TOS-4.04-ROM messen.

### FreeMiNT — belegt, und für cflib die einzig relevante Referenz

`references/freemint/sys/dosdir.c:959` — `sys_f_attrib` löst mit
`path2cookie(p, name, follow_links, &fc)` auf und gibt in `:1001`
`return xattr.attr;` für **Dateien und Ordner** zurück.
`references/freemint/sys/filesys.c:964-1010` — `relpath2cookie` schält bei
`"C:\CLIPBRD\"` in der zweiten Runde eine **leere** Komponente ab und ruft
`xfs_lookup(dir, "", res)`.
`references/freemint/sys/fatfs.c:5757` — `fatfs_lookup` behandelt den leeren
Namen **ausdrücklich wie `"."`**:
```c
/* 1 - itself */
if (!*name || (name[0] == '.' && name[1] == '\0')) { ... *fc = *dir; return E_OK; }
```
Unter FreeMiNT liefert `Fattrib("C:\CLIPBRD\", 0, 0)` also die Attribute des
Ordners. Genau darauf ist cflib gebaut — die Bibliothek trägt im Kopf jeder
Datei „Modified for FreeMiNT CVS by Frank Naumann" (`fsnorm.c:5`).

### Das Soll für NewTOS

**Die FreeMiNT-Semantik, und zwar nicht als neue Grundsatzentscheidung,
sondern als Vollendung der bereits getroffenen.** Begründung:

1. `docs/COMPATIBILITY.md:215-228` sanktioniert die Abweichung bereits — mit
   wörtlich dieser Begründung: „cflib `path_exists` (used by QED's clipboard
   init and every FreeMiNT/cflib app) probes a directory with
   `Fattrib(path, 0, 0) >= 0 && (attr & FA_DIR)`". Der Vertrag hat nur
   übersehen, dass der Pfad, den cflib übergibt, auf einen Backslash endet.
   Der erklärte Zweck der Abweichung ist heute **nicht erfüllt**.
2. Die Projektnotiz `cflib-apps-expect-tos4-semantics` benennt genau diesen
   Fall (Fattrib auf Verzeichnisse ↔ QED-Klemmbrett).
3. FreeMiNT ist sanktionierte Quelle und hier **eindeutig**; TOS4 ist für
   diese Teilfrage nicht belegbar.
4. Es kann kein regelkonformes TOS-1-3-Programm brechen: die Abweichung ist
   rein additiv (ein bisher negativer Rückgabewert wird positiv). Ein Programm,
   das sich darauf verlässt, dass `Fattrib("X:\DIR\")` **scheitert**, ist nicht
   konstruierbar-plausibel — es hätte auf TOS keinen Nutzen.

---

## 5. Vorschlag für den minimalen Fix (Vorschlag, kein Patch)

**Ort:** `tos-api/src/bdos/fs/file.rs`, Funktion `fattrib`, **nur** der
GET-Zweig (`wrt == 0`, ab `:581`).

**Inhalt:** vor der Auflösung die Trennzeichen am Pfadende (`\` und `/`)
abschneiden, sofern danach noch eine Komponente übrig bleibt; dann wie bisher
`path::findit` + `find_in_dir`. Damit löst `"C:\CLIPBRD\"` auf denselben
Verzeichniseintrag auf wie `"C:\CLIPBRD"` und liefert dessen **echtes**
Attributbyte (nicht ein synthetisches `0x10`).

**Warum dort und nicht in `path::findit`:** die leere Restkomponente ist für
`Fsfirst`, `Fopen`, `Fcreate`, `Dcreate`, `Frename` **gemessen EmuTOS-treu**
(Fälle 09/14: `-34`/`-33`, identisch zu EmuTOS **und** TOS 3.06). Ein Eingriff
in `resolve_parent` würde alle diese Funktionen mitverändern — das wäre eine
breite, unbelegte Abweichung. Die Änderung gehört in den einen Aufrufer, dessen
Vertrag bereits eine sanktionierte Abweichung trägt.

**Wirkung auf QED** (aus der Quelle abgeleitet, mit gemessenen Eingangswerten;
nicht nachgestellt, weil die Reparatur dem Entwickler gehört):
`Fattrib("C:\CLIPBRD\") >= 0 && (attr & 0x10)` → `path_exists` = `TRUE` →
`make_normalpath` = `TRUE` (`fsnorm.c:97`) → der Zweig `clipbrd.c:283-295`
wird gar nicht betreten → kein `Dcreate`, kein Alarm. `clip_dir` behält den
Backslash, `clip_name` wird `"C:\CLIPBRD\scrap.txt"` (`clipbrd.c:304-305`) —
also derselbe Name wie im ersten Lauf. Der erste Lauf (Ordner fehlt) bleibt
unverändert: `Fattrib` = `-33`, `Dcreate` = `0`, kein Alarm.

**Eine offene Teilentscheidung, die der Fix nicht heimlich treffen darf:**
das Laufwerkswurzel-Verzeichnis (`"C:\"`, Fall 06). Nach dem Abschneiden bliebe
nur `"C:"` übrig, also wieder eine leere Komponente, und es gibt keinen
Verzeichniseintrag, dessen Attributbyte man lesen könnte. FreeMiNT liefert hier
die Attribute der Wurzel (`FA_DIR`), EmuTOS und TOS 3.06 liefern `EFILNF`.
Für QED ist der Fall irrelevant; für cflibs Dateiauswahl
(`sdk/cflib/cflib-21/filesel.c:205`, `make_normalpath` auf Benutzereingaben)
ist er es nicht. Entweder bewusst bei `EFILNF` bleiben (EmuTOS-treu, eine
zweite Fehlerquelle derselben Klasse bleibt offen) oder `0x10` synthetisieren
(FreeMiNT-treu). **Nicht** stillschweigend entscheiden.

---

## 6. Zwei Folgebefunde, die mit derselben Ursache zusammenhängen

### 6a. Die Spezifikation behauptet, es funktioniere bereits — falsch

`docs/api/gemdos/file/43-fattrib.md:167`:

> „The trailing-backslash form `Fattrib("C:\SUBDIR\", 0, 0)` resolves to the
> same directory."

und `:258-260`:

> „Both `"C:\CLIPBRD"` and the trailing-backslash form `"C:\CLIPBRD\"` resolve
> to the same directory (`path::findit` drops the empty trailing component)."

`path::findit` **verwirft die leere Endkomponente nicht** — es reicht sie an
`fcb_name` weiter, das daraus elf Leerzeichen macht
(`tos-api/src/bdos/fs/path.rs:160-169`, `dir.rs:148-173`). Die eigene
Doc-Komment-Erklärung von `resolve_parent` (`path.rs:120-131`) sagt das sogar
ausdrücklich — die beiden Aussagen widersprechen sich im selben Baum. Gemessen:
`-33`. **Die Spezifikation ist an dieser Stelle falsch und gehört korrigiert**
(Researcher), unabhängig davon, wie die Fix-Entscheidung ausfällt.

### 6b. Der eigene Selbsttest prüft den Fall — und ist nirgends verdrahtet

`kernel/src/selftest_emmc.rs:490` misst genau die Backslash-Form und
Zusicherung (a) in `:501-503` verlangt
`dir_bs >= 0 && (dir_bs & 0x10) != 0`. Nach der Messung muss diese Zusicherung
heute `false` drucken. Der Test ist allerdings nur ein Modul
(`kernel/src/lib.rs:76`) ohne Aufrufer im Bootpfad — er läuft in keinem
regulären Lauf mit. Die Notiz in der Spezifikation („Selftest: … verified on
QEMU raspi4b") ist damit nicht nachvollziehbar. Zusicherung (a) druckt
außerdem nur ein `true`/`false` in eine Zeile voller anderer Ausgaben, statt
hart zu scheitern.

---

## 7. Dieselbe Fehlerklasse an anderer Stelle — cflibs `app_init`

Derselbe Mitschnitt, früher im Lauf:

```
GEMDOS #1  fn=0x43 Fattrib   -> -33
    Fattrib  path="C:\Apps\QED\"
GEMDOS #1  fn=0x19 Dgetdrv   -> 2
GEMDOS #1  fn=0x47 Dgetpath  -> 0
    Dgetpath buf="\APPS\QED"
```

`sdk/cflib/cflib-21/appinit.c:104-107`:

```c
shel_read(path, s);
split_filename(path, gl_appdir, NULL);
if (!path_exists(gl_appdir))
    get_path(gl_appdir, 0);          /* Rückfall auf das AKTUELLE Verzeichnis */
```

`gl_appdir` endet auf einen Backslash, `path_exists` scheitert aus demselben
Grund, und cflib **überschreibt das Programmverzeichnis mit dem aktuellen
Verzeichnis**. In diesem Lauf fällt es nicht auf, weil unsere Shell das
aktuelle Verzeichnis zufällig schon auf `\APPS\QED` gesetzt hat — die
`Dgetpath`-Antwort ist zufällig richtig. Sobald das aktuelle Verzeichnis vom
Programmverzeichnis abweicht, verliert **jede** cflib-Anwendung ihr
Programmverzeichnis und sucht Konfiguration, Hilfe und Zusatzdateien am
falschen Ort. Betroffen ist damit nicht nur QEDs Klemmbrett, sondern jeder
Start einer cflib-Anwendung. Derselbe Fix deckt beides ab.

---

## 8. Was gemessen wurde und was nicht

**Gemessen:**

* Reproduktion in QEMU `raspi4b`, zwei Läufe auf demselben Abbild, mit
  Bildschirmabzügen (Alarmbox im zweiten Lauf sichtbar).
* QEDs tatsächliche Aufruffolge samt Pfadzeichenketten und Rückgabewerten
  (EL0-Syscall-Mitschnitt).
* 18 GEMDOS-Rückgabewerte auf NewTOS (QEMU raspi4b, FAT16),
  EmuTOS 1.4 (Hatari TT, FAT12-Diskette, echtes BDOS) und
  Atari TOS 3.06 (Hatari TT, FAT12-Diskette, echtes BDOS).

**Nicht gemessen / offen:**

* **TOS 4.04** — kein ROM auf diesem Rechner. Die Frage, ob TOS4 die
  Backslash-Form auflöst, bleibt **unentschieden**.
* **Wirkung des vorgeschlagenen Fixes** — aus Quelle und gemessenen Eingangs­
  werten abgeleitet, nicht nachgestellt (die Reparatur gehört dem Entwickler).
* **Ausschneiden/Einfügen** (die interaktive Hälfte der Abnahme) — braucht die
  echte Pi 4 mit Tastatur und Maus.
* Die MagiC/MagiCPC-Seite (`cf_magxPC`-Sonderweg in `fsexists.c:43-56`) wurde
  nicht betrachtet; für NewTOS ist das Cookie nicht gesetzt.

---

## 9. Zustand des Arbeitsbaums

* **Kein Produktcode geändert.** Der einzige Eingriff war der Schalter
  `QED_SVC_TRACE` in `kernel/src/dbg_qedtrace.rs` (ausdrücklich temporäres
  Debug-Werkzeug, kein Produktcode); er ist zurückgesetzt, die Datei ist
  byteidentisch zum Ausgangszustand (SHA-256 geprüft).
* `sdk/apps/qed/` unangetastet.
* `kernel8.img` (unversioniert, `.gitignore:8`) wurde neu gebaut.
* Keine `git checkout`/`clean`/`stash`/`restore`/`reset`-Befehle benutzt,
  nichts committet.
* Alle Prüfabbilder, Prüfprogramme und Mitschnitte liegen im Job-Scratch,
  nicht im Repo. Keine QEMU- oder Hatari-Prozesse übrig; kein Abbild mehr
  eingehängt; die Partitionen der echten SD-Karte (`/dev/disk7`, `NEWTOS1-3`)
  wurden nicht angefasst.
