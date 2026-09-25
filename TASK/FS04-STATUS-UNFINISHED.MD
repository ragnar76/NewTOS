# FS04 — STATUS: MESSUNG VOLLSTAENDIG, SCHICHT NICHT LOKALISIERT (2026-09-25)

**Auftrag (Phase 1):** die Messung liefern, die zeigt, welche Schicht die drei
bis vier Sekunden verbraucht, wenn eine kleine Textdatei ueber eine
Partitionsgrenze kopiert wird. Nicht beheben, nicht vermuten.

**Stand:** Die Messung ist gebaut, gefahren und reproduzierbar. Sie beantwortet
die schaerfste Frage des Tickets eindeutig und schliesst mehrere Verdaechtige
mit Zahlen aus. Sie **lokalisiert die Sekunden aber nicht**, weil der Effekt in
QEMU nicht auftritt: die ganze Geste kostet dort **25,0 ms**, nicht 3-4 s. Der
Schritt, der es entscheidet, ist ein Lauf auf der echten Pi — den faehrt der
Anwender, nicht ich. Deshalb `-UNFINISHED`.

Basis: Branch `desk-rebuild`, HEAD `31fef0f`. Am gemeinsamen Baum wurde nichts
geaendert ausser `measurement-2026-09-25-fs04/` und dieser Datei. Nichts
geflasht, nichts ausgeworfen, keine Pi gefahren, nichts committet, kein
`git checkout/restore/clean/stash/reset`; die SD-Karte unter `/Volumes/BOOT`
wurde nicht beruehrt. Alle Instrumentierung liegt auf Scratch-Kopien.

Messung: [`measurement-2026-09-25-fs04/`](../measurement-2026-09-25-fs04/) —
`RUNINFO.TXT` (Verfahren), `NUMBERS.TXT` (die Zahlen auf einer Seite),
`method/` (Patches als Diff + Skripte), `raw/` (serielle Protokolle),
`screendumps/`.

---

## 1. Was nachgestellt wurde — und dass es wirklich eine Kopie war

`tools/make-sd-image.sh` baut **eine** Partition; damit ist eine Kopie ueber
eine Partitionsgrenze nicht nachstellbar. `method/make-sd-image-4part.sh` baut
die Form der physischen Karte nach: 2048 MiB, vier FAT16-Partitionen zu je
~477 MiB (C:–F:), 8-KiB-Cluster, `spf=239`, `numcl=61003` — dieselbe
Clustergroesse und FAT-Laenge, die eine 500-MB-Partition bekommt. Der Inhalt
von C: ist wortgleich der eines Standardabbilds, der Desktop also derselbe wie
in jeder anderen Messung. Der Kernel meldet `[desk] desktop up (4 drives, …)`.

Die Geste treibt der Scratch-IKBD-Injektor aus `measurement-2026-09-17-h03` /
`-2026-09-25-port03` (unveraendert uebernommen): Alt+C oeffnet `C:\`,
`README.TXT` wird aufgenommen und auf das **D:-Laufwerkssymbol** gezogen, der
`COPY FILE(S)`-Dialog mit OK beantwortet. Klickpunkte aus einem eigenen
Sondierungslauf, nicht geschaetzt (`raw/p01-probe-clickpoints.log`).

Belegt, dass es eine echte Kopie war: `D:\README.TXT` mit 64 Byte liegt danach
im Abbild, und `fsck_msdos -n` ist auf C:, D: und E: sauber.

## 2. Die Zaehlung geht bis auf das einzelne SD-Kommando

Zeitstempel an jeder Schichtgrenze, alles im `.bss`, **einmal** nach dem
Vorgang gedruckt — waehrend der Messung geht kein Byte auf die serielle
Schnittstelle (ein Zeichen kostet bei 115200 Baud ~87 us und wuerde genau die
Verteilung dominieren, die gemessen wird). Zeitquelle `CNTPCT_EL0`.

Angezapft: `fun_drag` → `fun_op` → `d_dofcopy` (jeder GEMDOS-Aufruf einzeln) →
GEMDOS-Dispatcher (Zeit und Aufrufe je Opcode) → Sektorcache →
`BlockDev::read_blocks/write_blocks` (**Aufrufe und Sektoren je Aufruf**) →
`emmc::read_one/write_one`, **je Kommando in fuenf Phasen zerlegt**
(DAT_INHIBIT-Warten, CMD17/CMD24, READ/WRITE_RDY, PIO-Schleife, DATA_DONE) →
`wait_clear`/`wait_interrupt` (**Zahl der Abfrageschleifen je Warten**) → dazu
die Cachepflege des Rahmenpuffers (`dc cvac`-Zeilen je Primitiv).

Die EMMC-Ebene ist damit so gemessen, wie der Nachtrag es verlangt: **als Zeit
pro Kommando**, nicht als Summe.

## 3. Die Verteilung, gemessen (QEMU raspi4b, 640x480)

Ab dem OK-Klick bis zum Ende der Fallenbehandlung — das ist genau das, was der
Anwender als Stillstand sieht:

| | Zeit | Anteil |
|---|---|---|
| **Wandzeit gesamt** | **25 021 us** | 100 % |
| `d_dofcopy` (Datei auf, lesen, schreiben, zu) | 4 846 us | 19,4 % |
| davon Geraetezeit (12 Kommandos) | 3 114 us | 12,4 % |
| `rebuild_window` (Verzeichnis neu + 1 Neuzeichnen) | 10 370 us | |
| `redraw_desktop`, 2 Aufrufe | 13 921 us | |
| Verzeichnis-Neueinlesen + zwei Vollbild-Neuzeichnen | **17 330 us** | **69,3 %** |
| Dialog/Buchhaltung des Desktops | ~2 845 us | 11,4 % |

Der ganze Vorgang besteht aus **genau 12 Einzelsektor-Kommandos** (6x CMD17,
6x CMD24). Langsamstes Einzelkommando: ein CMD17 mit **907 us**.
`emmc.spin`: 60 Wartevorgaenge, **0** Abfrageschleifen — in QEMU ist jedes
Warten beim ersten Lesen erfuellt.

**In QEMU verbraucht also keine Schicht Sekunden.** Die Dateisystem- und
Blockebene ist ein Fuenftel des Vorgangs, die VDI-/AES-Ebene zwei Drittel.

## 4. Die schaerfste Frage des Tickets ist beantwortet: der Puffer zerfaellt

Gemessen an `C:\BIG1M.BIN` (1 MiB) nach D:

```
33 Fread  a 32 KiB  ->  blk.read_blocks   calls=2518  Sektoren=2518  max/call=1
32 Fwrite a 32 KiB  ->  blk.write_blocks  calls=2307  Sektoren=2307  max/call=1
```

**Ein 32-KiB-`Fread` endet unten als 64 einzelne CMD17, ein 32-KiB-`Fwrite` als
64 einzelne CMD24.** Auf dem ganzen Weg gibt es keine einzige
Mehrsektor-Uebertragung: `fread`/`fwrite` arbeiten sektorweise ueber einen
512-Byte-Stapelpuffer (`tos-api/src/bdos/fs/file.rs:337-360`, `:401-440`), der
Sektorcache hat 16 Faecher zu 512 Byte (`cache.rs:18-19`), und
`read_blocks`/`write_blocks` zerlegen jeden Auftrag in `read_one`/`write_one`
(`kernel/src/drivers/bcm2711/emmc.rs:339-357`).

**Fuer eine kleine Textdatei ist das allerdings gegenstandslos** — dort ist es
ohnehin ein Sektor. Der Befund erklaert die 3-4 Sekunden des Tickets nicht.

Nebenbefund derselben Messung: 132 338 Cache-Lesezugriffe fuer 1 MiB, davon
130 048 aus `fwrite`, das `resolve_cluster` **fuer jeden Sektor** aufruft und
die Cluster-Kette jedesmal vom Anfang durchlaeuft
(`file.rs:401-406` + `:447-470`) — O(n²) in der Dateilaenge. Fuer 64 Byte null,
fuer 1 MiB 130 000 Nachschlage. Eigener Befund, nicht dieser Defekt.

## 5. Was die Messung ausschliesst — mit Zahlen, nicht mit Argumenten

* **`Dfree` ist nicht im Kopierpfad.** Das GEMDOS-Profil der ganzen Geste
  enthaelt kein `0x36`. (Gemessen kostet ein `Dfree` auf einer
  500-MB-FAT16-Partition **238 Einzelsektor-Lesungen**, 30,3 ms in QEMU — auf
  der Pi bei ~512 us/Sektor rund 122 ms. Nur falls es jemals in den Pfad
  geriete.)
* **Die Partitionsgrenze als solche kostet nichts.** Kalt C:→D: 6/6 Zugriffe,
  gleiche Partition C:→C:\SUB\ 6/6, kalt C:→E: 6/6. Die Zahl der Zugriffe ist
  dieselbe; das Anmelden einer Partition ist **ein** Bootsektor.
* **Die 32-KiB-Puffergroesse ist fuer eine kleine Datei ohne Wirkung** — ein
  `Fread`, ein `Fwrite`, wie das Ticket schon las. (Die Fehllesung im Kommentar
  `kernel/src/desk/dir.rs:97-99` bleibt trotzdem richtigzustellen; sie ist ein
  eigener Punkt, kein Laufzeitbefund.)
* **Der Sektorcache verhaelt sich im kleinen Fall gutartig:** 47 Treffer,
  5 Fehlschlaege, 5 `flush`-Aufrufe, 6 Rueckschreibungen.

## 6. Was sich zwischen Emulator und Pi unterscheidet

Es fehlt ein Faktor von rund 140. Vier Achsen, an denen QEMU und Pi ueberhaupt
auseinanderfallen koennen, jeweils mit der Zahl, die sie tragen muessten:

**A) Das SD-Kommando selbst.** Bei 12 Kommandos muesste **eines im Mittel
250–333 ms** kosten. QEMU modelliert die Karte nicht (`emmc.spin` = 0
Abfrageschleifen, `dev.w.wait_dat` = 47 us insgesamt). Auf echter Hardware sind
genau diese beiden Zaehler die Programmierzeit der Karte.

Strukturell, aus dem Quelltext gelesen und hier nur als Kostenmodell, nicht als
Diagnose: der Treiber faehrt die Karte **einsporig** — `init_card` setzt nie
ACMD6 / `CONTROL0.HCTL_DWIDTH`, also 164 us reine Datenzeit je 512 Byte bei
25 MHz statt 41 us im 4-Bit-Modus —, nutzt **nur Einzelblock**-CMD17/CMD24
(`emmc.rs:84-86`), kein CMD18/CMD25, **kein DMA, keine Interrupts**
(`emmc.rs:1-7`), und wartet mit abgezaehlten Leerschleifen
(`SPIN_LIMIT = 1_000_000`, `emmc.rs:91`). Ein leer auslaufender Wartelauf
kostet auf der Pi die Groessenordnung 0,1 s. Ob das passiert, sagt `emmc.spin`
dort — in QEMU ist er 0.

**B) Die Bildschirmgeometrie.** QEMU meldet 640x480; auf der Pi ist es der
HDMI-Modus, die eingebaute Rueckfallebene ist 1920x1080
(`kernel/src/drivers/mod.rs:31-33`). **Gemessene** Skalierung eines vollen
`redraw_desktop`: 3,13 ms bei 640x480 gegen 19,39 ms bei 1920x1080, also x6,19
in der Zeit und x4,16 in der Cachepflege. Da 69 % des Vorgangs Neuaufbau und
Neuzeichnen sind, verschiebt das ~17 ms auf ~100 ms — deutlich, aber nicht
3,5 s.

**C) Die Cachepflege des Rahmenpuffers.** Eine Fallenbehandlung loest bei
640x480 **1 349 997 `dc cvac`** ueber 86,3 MB Adressspanne aus — das 70fache
des ganzen 1,23-MiB-Bildschirms; der groesste Einzelaufruf pflegt 1 026 752
Byte fuer **ein** Primitiv (84 % des Bildschirms). Die Ursache ist im Quelltext
lesbar: `clean` pflegt „die zusammenhaengende Adressspanne, die das Rechteck
einschliesst" (`kernel/src/vdi_backend.rs:284-305`), fuer ein schmales hohes
Rechteck also das ganze Zeilenband. In QEMU-TCG ist `dc cvac` ein Nichts, auf
einem Cortex-A72 nicht. Groessenordnung bei 1920x1080 (≈5,6 Mio Zeilen):
0,1–0,5 s — **gerechnet, nicht gemessen**.

**D) Es ist nicht derselbe Controller.** QEMU haengt die Karte an den Alt-EMMC
bei `0xFE30_0000` (Startzeile `[emmc] SD card on controller @ …fe300000`), die
echte Pi 4 an EMMC2 bei `0xFE34_0000` — andere Basisfrequenz
(`CAPABILITIES[15:8]`, `emmc.rs:216-231`) und damit ein anderer Teiler. Welche
SD-Taktrate dort tatsaechlich herauskommt, ist in QEMU nicht messbar.

Keine dieser vier Achsen traegt in der Rechnung allein 3,5 s. Das ist der
ehrliche Stand: die Messung kann keine Schicht mit den Sekunden **belasten**,
und sie kann — ausser fuer QEMU selbst — auch keine **entlasten**.

## 7. Eine Erwartung, die die Messung widerlegt hat

Vor dem ersten Lauf war meine Erwartung, die Sekunden lieferten sich aus der
Zerlegung des 32-KiB-Puffers in Einzelsektoren — die Zerlegung gibt es
tatsaechlich, vollstaendig (§4), aber sie kostet bei einer 64-Byte-Datei
**zwoelf** Kommandos, nicht sechzig. Die Zerlegung ist belegt und trotzdem
nicht die Ursache. Das gehoert genauso ins Protokoll wie ein Treffer.

## 8. Der Lauf, der es entscheidet — und der dem Anwender gehoert

Derselbe Patchsatz laeuft unveraendert auf der Pi und druckt dieselbe Tabelle
nach der Geste auf die serielle Konsole; ein Injektor wird dafuer nicht
gebraucht, `fun_drag` setzt die Zaehler selbst zurueck und druckt am Ende von
selbst.

```
cd <frische Kopie des Baums, NICHT der gemeinsame Baum>
python3 measurement-2026-09-25-fs04/method/fs04_probe_patch.py . gesture
cargo run -p xtask --release -- build --board rpi4 --release
```

Dann auf der Pi eine Textdatei von einer Partition auf eine andere ziehen. Drei
Zeilen entscheiden:

* `dev.read_one` / `dev.write_one` (Summe **und** max) — Hunderte von
  Millisekunden heisst: die Sekunden liegen im EMMC-/Kartenpfad, und
  `dev.w.wait_dat` bzw. `dev.r.cmd17` sagen in welcher Phase.
* `emmc.spin` — nahe 1 000 000 heisst: eine Wartschleife laeuft leer aus,
  ~0,1 s je Vorkommen.
* `desk.redraw_desk` / `desk.rebuild_win` / `fb.clean_lines` — Sekunden dort
  heissen VDI-/Rahmenpufferebene, und `vdi_backend.rs:284-305` ist der erste
  Ort zum Nachsehen.

Dazu gehoert die Startzeile `[fb] VideoCore display geometry detected: WxH`;
ohne sie laesst sich die gemessene Skalierung aus §6B nicht anwenden.

## 9. Was offen bleibt

* **Die Lokalisierung selbst.** Ohne Pi-Lauf nicht zu haben.
* **Die reale Kosten je SD-Kommando** — in QEMU prinzipiell nicht messbar.
* **Der `dc cvac`-Preis auf einem A72** — in QEMU-TCG null, auf der Pi nicht.
* **Die Bildschirmgeometrie der Pi** — unbekannt; steht im Startprotokoll.
* Nicht gefahren: ein Ziehen in ein **offenes D:-Fenster** (`fun_win2win`)
  statt auf das Laufwerkssymbol. Der Kopierteil ist derselbe, der
  Neuzeichenteil koennte teurer sein (zwei Fenster statt eines).

## 10. Nebenbefunde, nicht verfolgt

* `fwrite` ruft `resolve_cluster` je Sektor und laeuft die Cluster-Kette
  jedesmal von vorn — O(n²), 130 048 Nachschlage fuer 1 MiB
  (`tos-api/src/bdos/fs/file.rs:401-406`, `:447-470`).
* `vdi_backend.rs:284-305` pflegt die einschliessende Adressspanne statt der
  Zeilen des Rechtecks; ein schmales hohes Primitiv pflegt bis zu 84 % des
  Bildschirms.
* Der EMMC-Treiber laeuft im 1-Bit-Modus (nie ACMD6), ohne Mehrblock-Kommandos,
  ohne DMA, ohne Interrupt.
* `kernel/src/desk/dir.rs:97-99` begruendet die 32-KiB-Wahl mit einer
  Fehllesung von `deskdir.c:909-921` — das Ticket sagt es selbst, es steht
  weiter aus.

## 11. Aufraeumen

Der gemeinsame Baum traegt nur `measurement-2026-09-25-fs04/` und diese Datei.
Kein Produktcode geaendert, `TASK/FS04.md` und `docs/STATUS.md` nicht
angefasst. Die gesamte Instrumentierung liegt auf Scratch-Kopien im
Job-Scratchpad; die Patches sind als Diff in `method/` abgelegt, damit die
Zahlen reproduzierbar sind.
