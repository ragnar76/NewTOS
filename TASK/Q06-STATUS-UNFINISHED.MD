# Q06 — Quittung

**Status: IMPLEMENTIERT · wirtstestbelegt · Abnahme auf echter Pi 4 offen**
· 2026-09-16 · Die abgebrochene Verzweigung in `restore_edit` ist bestimmt; sie
liegt **vor** den vier gesuchten. Ursache ist QEDs eigenes Mengenmodul, das mit
32-Bit-`long` und Big-Endian rechnet. Behoben mit einem neuen Overlay
`sdk/overlays/qed/set.c`; `sdk/apps/qed/src/` ist unangetastet.
**UNFINISHED heisst hier: die Hardwareabnahme fehlt, nicht die Implementierung** —
in QEMU ist der Fehler mangels Eingabe gar nicht ausloesbar.
**Nicht committet** (der Arbeitsbaum traegt unversionierte Fremdarbeit).

Belege: `measurements/measurement-2026-09-16-qed-set/`.

## Die Antwort auf die Frage des Tickets

Q06 fragte, an welcher der vier Verzweigungen in `restore_edit` (`edit.c:2103` /
`:2125` / `:2138` / `:2043`) der Weg endet. Keine davon: der Weg endet **eine
Ebene darueber**.

```c
/* edit.c:2106-2109 */
max = setmax(chg_links);
for (link = setmin(chg_links); link <= max; link++)
{
    if(setin(chg_links,link))            <-- dieser Rumpf wird nie betreten
```

`make_chg` (`edit.c:1919`) vermerkt das geaenderte Fenster mit
`setincl(chg_links, link)`. `restore_edit` sucht es mit `setmin`/`setmax`/`setin`
wieder — und findet es fuer **kein** Fensterhandle. `chg_anz` ist also nicht 0
(die Pruefung bei `:2103` laeuft durch), aber die Schleife arbeitet ins Leere,
und `:2145-2147` raeumt die Aenderungsliste weg:

```c
chg_anz = 0;			/* Keine Änderungen mehr */
setclr(chg_links);
```

Damit faellt **jede** inkrementelle Bildschirmaenderung aus: getipptes Zeichen,
Loeschen, Return, Blockoperationen, Undo, Suchtreffer, das `*` im Fenstertitel,
die Kopfzeile mit Zeile/Spalte. `WM_REDRAW` geht nicht durch `restore_edit`,
sondern ueber `redraw_window` — deshalb erscheint der Text nach einem echten
Neuzeichnen, und deshalb sah der Anwender seine Zeichen erst, nachdem das
Dateifenster offen war.

`setmin`/`setmax` stehen in `sdk/apps/qed/src/set.c`, und das ist keine Datei,
die man aendern darf: sie ist vendierte Fremdquelle.

## Der Befund, selbst nachgemessen

`sdk/apps/qed/src/types.h:13-16` erklaert die Menge:

```c
#define SETSIZE	32				/* Size of a set in longs */
#define SETMAX		(SETSIZE*32 - 1)	/* Max count of elements of a set */
typedef unsigned long	SET[SETSIZE];
```

„32 longs zu je 32 Bit, 1024 Elementnummern." Auf m68k stimmt das. Auf
AArch64/LP64 ist ein `long` **acht** Byte, und die Maschine ist
**little-endian**. `set.c` adressiert dieselben Bits auf zwei Arten:

| Adressierung | Funktionen | neutral? |
|---|---|---|
| byteweise, Byte `elt>>3`, Maske `0x80>>(elt&7)` | `setincl`(:161) `setexcl`(:169) `setchg`(:177) `setin`(:185) | **ja** |
| wortweise, 32 Bit je `*set++`, `0x80000000L`/`0xFFFFFFFFL` als „oberstes Bit" | `setfree`(:8) `setmax`(:30) `setmin`(:47) | nein |
| wortweise, `0xFFFFFFFFL` je Wort geschrieben | `setall`(:77) `setnot`(:105) | nein |

Auf m68k bezeichnen beide Adressierungen fuer dieselbe Elementnummer dasselbe
Bit. Auf LP64/Little-Endian nicht: `setincl(s,1)` setzt Byte 0 auf `0x40`, was
als 64-Bit-Wort `0x0000000000000040` gelesen wird; `setmin` schiebt dieses Wort
nach links, bis es `0x80000000` erreicht, und zaehlt dabei 25 statt 1.

Gemessen mit `measurements/measurement-2026-09-16-qed-set/host/settest` (gleiche
LP64-LE-Vereinbarung wie das Ziel):

```
sizeof(unsigned long)=8   sizeof(SET)=256   SETSIZE=32   SETMAX=1023
byte order: little-endian

setincl(s,0)  ->  setmin=24   setmax=24     (m68k: 0    / 0)
setincl(s,1)  ->  setmin=25   setmax=25     (m68k: 1    / 1)
volle Menge   ->  setmax=511  setfree=64  setcard=512
                                            (m68k: 1023 / 1024 / 1024)

restore_edit loop body: reached for 0 of 41 window handles
```

## Die Entscheidung: welche Bitnummerierung

Der Auftrag stellte zwei Moeglichkeiten zur Wahl — (i) bei 1024 Elementen
bleiben und die oberen Bits **jedes Worts** ungenutzt lassen, oder (ii) die
Nummerierung an die byteadressierte Seite anpassen.

**Gewaehlt: (ii), die flache Byte-Nummerierung.** Element `e` liegt in Byte
`e >> 3` des `SET`-Objekts, in Maske `0x80 >> (e & 7)`.

Die Begruendung, in der Reihenfolge ihres Gewichts:

1. **Sie ist auf m68k bitgleich zur Wort-Nummerierung.** Ein
   Big-Endian-32-Bit-Wort `w` traegt Element `w*32` in seinem obersten Bit und
   `w*32+31` im untersten — das sind genau die Bytes `w*4 .. w*4+3` mit Maske
   `0x80 .. 0x01`. Die ersten 128 Byte eines `SET` tragen damit auf LP64 ein
   **byte-identisches** Bild zu dem, das dasselbe Programm auf m68k baut. Das
   ist die staerkste Form von „dasselbe tun wie das Original".
2. **Sie ist nicht neu.** Es ist die Nummerierung, die vier der siebzehn
   Funktionen ohnehin schon benutzen. Diese vier bleiben **woertlich**.
   Moeglichkeit (i) haette auch sie umschreiben muessen, die obere Haelfte
   jedes Worts verschenkt und ein Bild erzeugt, das m68k nicht mehr gleicht.
3. **`types.h` bleibt unangetastet** (vendiert). `SETMAX` bleibt 1023, es gibt
   weiterhin genau 1024 Elementnummern — unveraendert gegenueber m68k. Auf LP64
   ist das Objekt `SETSIZE * sizeof(unsigned long)` = 256 Byte gross, die oberen
   128 Byte sind damit **Polsterung**. Sie werden von keiner Funktion gelesen und
   von jeder ganzschreibenden Funktion genullt, damit eine Menge auch auf diesem
   Datenmodell vollstaendig durch ihre Elemente bestimmt ist.

**Was `SETMAX` und die Aufrufer voraussetzen — nachgesehen, nicht angenommen:**
`SETMAX` und `SETSIZE` kommen in QED **ausserhalb** von `set.c` und `types.h`
nirgends vor (`grep -a` ueber alle 37 `.c` und alle `.h`). Kein Aufrufer rechnet
also selbst mit der Breite. Die groessten Elemente, die QED je einfuegt, sind
ein Fensterhandle (`edit.c:2215`), eine Projektzeilennummer (`projekt.c:594`,
`:1488`) und ein Zeichencode 0..255 (`str2set`); alle drei waren auf m68k schon
durch `SETMAX` begrenzt und sind es unveraendert. Die `SET`-Felder in `LOCOPT`
(`types.h:79,84`) werden zur Laufzeit aus Zeichenketten aufgebaut (`str2set`),
nicht aus einer Datei gelesen — es gibt kein auf 128 Byte festgelegtes
Dateiformat, das die Polsterung stoeren koennte.

Eine Erweiterung auf die 2048 Bit, die das LP64-Objekt haelt, ist ausgeschlossen:
`setincl`/`setin` weisen `elt > SETMAX` ab, und `SETMAX` steht in der Datei, die
nicht angefasst werden darf.

## Alle siebzehn Funktionen, einzeln

Der Auftrag sprach von neunzehn Funktionen und nannte sechs als wortadressiert.
**Beides stimmt nicht ganz** (siehe „Was am Briefing falsch war"). `set.h`
deklariert **siebzehn**; wortadressiert und damit auf LP64 defekt sind **fuenf**.

| # | Funktion | geaendert? | warum |
|---|---|---|---|
| 1 | `setfree` | **ja** | wortadressiert, 32 fest verdrahtet. Byteweise neu; Rueckgabe fuer die volle Menge bleibt `SETMAX+1` = 1024, genau der Wert, den die m68k-Schleife in `i` stehen laesst |
| 2 | `setmax` | **ja** | wortadressiert. Byteweise neu; leere Menge weiterhin `-1` |
| 3 | `setmin` | **ja** | wortadressiert. Byteweise neu; leere Menge weiterhin `SETMAX+1` = 1024 (davon haengt ab, dass `min > max` die Schleife in `restore_edit` und `setcard` leer laesst) |
| 4 | `setcpy` | nein (Form) | `memcpy` ueber `SETSIZE * sizeof(unsigned long)` war schon breitenneutral; kopiert weiterhin das ganze Objekt |
| 5 | `setall` | **ja** | schrieb `0xFFFFFFFFL` je Wort, also auf LP64 nur die halbe Menge. Jetzt `memset(0xFF)` ueber die 128 adressierbaren Byte + Polsterung genullt |
| 6 | `setclr` | nein (Form) | nullt jetzt das ganze Objekt in einem `memset` statt in einer entrollten Wortschleife — gleiche Wirkung auf jeder Breite |
| 7 | `setnot` | **ja** | XOR mit `0xFFFFFFFFL` je Wort erreichte auf LP64 nur die halbe Menge. **Diese Funktion fehlte in der Aufzaehlung des Auftrags** |
| 8 | `setand` | nein (Form) | byteweise ueber die adressierbare Haelfte statt wortweise ueber das ganze Objekt; gleiche Wirkung, aber immun gegen Polstermuell eines uninitialisierten Stapel-`SET` |
| 9 | `setor` | nein (Form) | dito |
| 10 | `setxor` | nein (Form) | dito |
| 11 | `setincl` | **woertlich** | byteadressiert |
| 12 | `setexcl` | **woertlich** | byteadressiert |
| 13 | `setchg` | **woertlich** | byteadressiert |
| 14 | `setin` | **woertlich** | byteadressiert |
| 15 | `setcmp` | nein (Form) | vergleicht die adressierbaren 128 Byte statt `SETSIZE` longs. Auf m68k ist das dasselbe; auf LP64 waere „alle 32 longs" zusaetzlich die Polsterung, also Bits, die keine Elementnummer benennen kann |
| 16 | `setcard` | **woertlich** | **nicht selbst wortadressiert** — der Rumpf steht unveraendert da und ist richtig, sobald `setmin`/`setmax` es sind. Der Auftrag fuehrte ihn als sechsten Defekt; er ist ein Folgeschaden |
| 17 | `str2set` | **woertlich** | arbeitet nur ueber `setclr`/`setincl` |

`static char bits[]` ist zu `static const unsigned char bits[]` geworden — unter
`-funsigned-char` (QEDs eigenes Flag, `build-qed.sh:92`) ist das dieselbe
Tabelle, aber sie stimmt jetzt auch ohne das Flag.

## Test vor Fix — die rote Ausgabe

`measurements/measurement-2026-09-16-qed-set/host/settest/` vergleicht **drei** Fassungen je
Testfall:

* **ORAKEL** — die reine Definition „Menge von Elementnummern 0..1023" mit der
  Byteabbildung, die `setincl`/`setin` ohnehin benutzen.
* **MODELL** — eine **woertliche Transliteration** von `src/set.c`
  (sha256 `264acfcd…`, unveraendert kopiert), in der nur der Wortzugriff durch
  einen ausdruecklichen Big-Endian-32-Bit-Zugriff ersetzt ist. Das ist, was die
  unveraenderte Quelle auf m68k **tut**, nicht was ich glaube, dass sie tut.
* **GERAET** — der Pruefling, uebersetzt fuer diesen Wirt (macOS arm64, also
  dasselbe LP64-Little-Endian-Datenmodell wie das Ziel), mit QEDs eigenen Flags
  `-funsigned-char -O2`.

ORAKEL gegen MODELL validiert das Modell: **0 Abweichungen** in jedem Lauf.
MODELL gegen GERAET ist das Urteil.

2856 Testmengen, jede prueft alle 17 Funktionen: leere Menge, volle Menge, jedes
einzelne Element 0..1023 als Einermenge, jede volle Menge minus ein Element,
441 Paare auf einem Raster aus Wort- und Byte-Grenzen, 300 Pseudozufallsmengen,
64 Bereiche, `str2set` ueber 13 Zeichenketten, dazu eine Menge mit absichtlich
verschmutzter Polsterung.

**Vorher** (`host/settest-before.log`; beide Prueflinge sind die unveraenderte
`src/set.c`, damit belegt ist, dass der Test rot ist, **bevor** das Overlay
existiert):

```
host data model: sizeof(unsigned long)=8 sizeof(SET)=256 SETSIZE=32 SETMAX=1023
byte order: little-endian

== device: src/set.c (pristine)
   FAIL setall       empty                     first differing byte 4: 00 vs ff
   FAIL setall       empty     setcard after setall   got=512  want(m68k)=1024
   FAIL setfree      full 0..1023                     got=64   want(m68k)=1024
   FAIL setmax       full 0..1023                     got=511  want(m68k)=1023
   FAIL setmin       singleton 1                      got=25   want(m68k)=1
   FAIL setmin       singleton 2                      got=26   want(m68k)=2
   FAIL restore_edit  window handle 0    loop body ran 0 times, want 1
   restore_edit loop body: reached for 0 of 41 window handles
   test sets driven: 2856 (each one checks all 17 functions)
   model-vs-oracle mismatches: 0
   DEVICE vs m68k MODEL: *** RED *** (17563 mismatches)
       setfree      1436
       setmax       2854
       setmin       1853
       setcard      2852
       setall       5712
       setnot       2856
```

Bemerkenswert und nicht selbstverstaendlich: `image`, `setin`, `setincl`,
`setexcl`, `setchg`, `setcmp`, `setcpy`, `setclr`, `setand`, `setor`, `setxor`
und `str2set` melden **0** Abweichungen. Das **Bild im Speicher war nie falsch** —
nur die wortadressierten Leser haben es falsch gelesen. Genau deshalb konnte der
Defekt so lange unauffaellig bleiben.

**Nachher** (`host/settest-after.log`, Pruefling 2 = `sdk/overlays/qed/set.c`):

```
== device: sdk/overlays/qed/set.c
   restore_edit loop body: reached for 41 of 41 window handles
   test sets driven: 2856 (each one checks all 17 functions)
   model-vs-oracle mismatches: 0
   DEVICE vs m68k MODEL: green (0 mismatches)
```

Nachbauen: `cd measurements/measurement-2026-09-16-qed-set/host/settest && sh build.sh &&
./settest-before && ./settest-after`.

## Der QED-Bau

`sdk/apps/build-qed.sh` (`measurements/measurement-2026-09-16-qed-set/build/build-qed.log`):

```
QED COBJS manifest: 37 files
  overlay: memory.c (from sdk/overlays/qed/, src/ untouched)
  overlay: set.c (from sdk/overlays/qed/, src/ untouched)
compiled OK=37  FAILED=0  (of 37)
LINK OK -> .../QED.NAP (568992 bytes)
undefined symbols in the final image (goal 0): 0
```

Das Overlay wurde also nachweislich genommen. Keine einzige
Uebersetzungswarnung (alle 37 `.err`-Dateien leer). `QED.NAP` 571424 → 568992
Byte. Die drei Bibliotheken wurden nicht neu gebaut.

## Maschinennachweis in QEMU — und seine Grenze

QEMU `raspi4b`, headless, 640x480, derselbe Kern in beiden Laeufen (es wurde
kein Rust angefasst). Karte: die Regressionsbasis, darauf **nur** `QED.NAP`
getauscht und `NEWDESK.INF` auf `#Z 01 C:\Apps\QED\QED.NAP@` gesetzt.

```
qed-before  Marke "QED.NAP" nach 15.2 s gefunden
qed-after   Marke "QED.NAP" nach 15.2 s gefunden

serial/qed-before-BOOT-SERIAL.txt gegen qed-after-BOOT-SERIAL.txt : identisch
    [shell] sh_ldapp: starting C:\Apps\QED\QED.NAP, sh_nextapp=2, sh_isgem=1
    [launch] EL0: slot 0 entry 0x1000
    kein Fault, keine Ausnahme

screendumps/qed-before.png gegen qed-after.png            0 von 307200 verschieden
screendumps/qed-after.png gegen
    measurements/measurement-2026-09-14-x01/screendumps/qed-after.png  0 von 307200 verschieden
                                                          (sha256 identisch)
```

Auf dem Bild: QEDs Menueleiste (qed, Datei, Bearbeiten, Fenster, Suchen,
Spezial, Optionen, Shell) ueber gruenem Grund, Mauszeiger. Kein Editorfenster —
QED oeffnet erst auf Benutzerhandlung eines.

**Die Grenze, ausdruecklich:** der eigentliche Fehler ist in QEMU **nicht
ausloesbar**. `raspi4b` meldet `[pcie] RC not present (no response at RC_BASE);
USB disabled` — es gibt dort weder Tastatur noch Maus. Ohne Eingabe wird kein
Editorfenster geoeffnet und keine Taste gedrueckt, also laufen `make_chg` und
`restore_edit` nie. Belegt ist nur: QED startet mit der neuen `set.c`
unveraendert und zeichnet unveraendert. Alles Weitere ist Vorhersage.

## Die drei Vorhersagen fuer die echte Pi 4

Alle drei loesen **kein** Neuzeichnen aus, sind also echte Proben auf
`restore_edit` — anders als „Fenster verschieben", das nichts zeigt, weil die
AES den Inhalt eines verschobenen obersten Fensters mitkopiert statt ihn neu
zu zeichnen (`references/emutos/aes/gemwmlib.c:1025`,
`if (!(moved && (top == gl_wtop)))`).

1. **Ein getipptes Zeichen erscheint sofort** — `char_insert` → `make_chg`
   (LINE_CHANGE) → `restore_edit` → `line_out` → `v_gtext`.
2. **Das `*` im Fenstertitel erscheint nach der ersten Aenderung** —
   WT_CHANGE laeuft durch dieselbe Schleife.
3. **Die Kopfzeile mit Zeile/Spalte folgt dem Cursor** — POS_CHANGE, dito.

Dass der Weg **hinter** der Schleife traegt, ist nicht von mir gemessen, sondern
aus `docs/analysis/qed-typing-redraw/findings.md` uebernommen: dort ist belegt,
dass `free_for_draw` TRUE liefert (640 == 640, 944 == 944, Abschnitt 4) und dass
QED bei `WM_REDRAW` ueber denselben `line_out`/`v_gtext`-Pfad sichtbaren Text
zeichnet. Bleibt eine Restunsicherheit: wenn hinter `restore_indesk` ein
**zweiter**, unabhaengiger Defekt sitzt, erscheint das Zeichen trotz dieses Fixes
nicht. Dann waere aber immerhin die Schleife nachweislich durchlaufen, und die
Suche begaenne bei `edit.c:2026-2043`.

Nebenbei mit repariert, ohne eigene Messung, weil dieselben drei Funktionen
tragend sind: `close_edit` („alle sichern/schliessen" ueber `used_info`,
`edit.c:468-471`) und der gesamte Projektfenster-Weg in `projekt.c`
(`do_for_prj`/`do_for_prj2`/`wi_unclick`/`shift_up`/`shift_down`,
`setcard`/`setmin`/`setmax` an zwoelf Stellen). Beides lief bisher ebenfalls
ins Leere.

## Bauten und Dauertests

* `sdk/apps/build-qed.sh`: 37/37 uebersetzt, 0 Warnungen, 0 undefinierte
  Symbole, Overlay nachweislich genommen.
* `cargo run -p xtask -- test`: **548 bestanden / 0 gefallen / 1 uebersprungen**
  — der im Auftrag genannte Sollstand, unveraendert (der Fix ist C).
* `cargo xtask build --board rpi4 --release` und `--board qemu-virt --release`:
  beide Exit 0, je `tos-api` 64 + `kernel` 94 Warnungen — Delta 0 gegen den in
  `measurements/measurement-2026-09-14-x01` protokollierten Sollstand. Vorher jeweils
  `kernel/src/lib.rs` und `tos-api/src/lib.rs` angefasst, damit das geteilte
  Zielverzeichnis nicht „up to date" melden kann.
* Clippy nicht gefahren und nicht noetig: es wurde **keine Zeile Rust**
  geaendert, das Delta ist strukturell 0.

## Was ich nicht angefasst habe

* `sdk/apps/qed/src/` — vendierte Fremdquelle, unveraendert
  (`set.c` sha256 `264acfcda2e62101a13d3813c86023afafb57f320211bcae0fd0e3002ae92df5`,
  vor und nach der Arbeit derselbe).
* Kein Rust, kein Kernel, keine Bibliothek, kein `tools/`-Skript.
* `kernel/src/dbg_qedtrace.rs` — der temporaere Tracer aus den Voruntersuchungen
  steht weiterhin als F1 („revert before merge") zum Rueckbau an; er war fuer
  diese Arbeit nicht noetig.

## Offen

1. **Abnahme auf echter Pi 4** — die drei Vorhersagen oben, in einem Zug
   abarbeitbar. In QEMU nicht fahrbar (keine Eingabe).
2. **Abnahme durch den Tester** — Boot beider Boards, Screendump-Regression
   (`qed-after.png` muss bitgleich bleiben), `xtask test`.
3. **Nicht committet.** Der Arbeitsbaum traegt in `docs/STATUS.md`,
   `TASK/README.md` und weiteren Pfaden unversionierte Fremdarbeit; die Eintraege
   sind additiv eingefuegt. Das Landen entscheidet der Lead.
