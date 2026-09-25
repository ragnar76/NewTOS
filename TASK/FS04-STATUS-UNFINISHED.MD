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

**Nachtrag 2026-09-25: die Pi hat keine serielle Mitschrift.** Der Anwender
sieht nur den Bildschirm, `kprintln` ist dort also unsichtbar und die ganze
Messung waere verloren gewesen. Die Sonde schreibt ihren Bericht deshalb jetzt
**in eine Datei auf der Karte** — Einzelheiten in §12.

Das fertige, gepruefte Abbild liegt in
[`measurement-2026-09-25-fs04/pi-probe/kernel8-fs04probe.img`](../measurement-2026-09-25-fs04/pi-probe/).
Selbst gebaut wird es so:

```
cd <frische Kopie des Baums, NICHT der gemeinsame Baum>
python3 measurement-2026-09-25-fs04/method/fs04_probe_patch.py . gesture
cargo run -p xtask --release -- build --board rpi4 --release
```

Dann auf der Pi eine Textdatei von einer Partition auf eine andere ziehen und
`C:\FS04.TXT` auslesen. Drei Zeilen entscheiden:

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


---

## 12. Nachtrag 2026-09-25: der Bericht landet auf der Karte

Der Anwender hat auf Nachfrage bestaetigt, dass auf der Pi **keine serielle
Ausgabe** mitgeschnitten wird — nur der Bildschirm. Die Sonde ist deshalb um
eine Berichtssenke erweitert worden. Umfang: nur die Sonde, weiterhin
Messgeruest auf Scratch-Kopien, kein Produktcode im gemeinsamen Baum, keine
Behebung.

### Was sich geaendert hat

| | |
|---|---|
| **Ziel** | `C:\FS04.TXT`, Wurzel der Bootpartition, reiner Text mit CR/LF |
| **Startblock** | beim Hochfahren ein kurzer Block mit Bildschirmgeometrie, Stride, Desktop-Rechteck, `CNTFRQ` und Laufwerksliste |
| **Messblock** | bei **jedem** Ablegen ein Block mit der vollen Tabelle |
| **Anhaengen** | `Fseek(0, h, 2)` vor dem Schreiben — mehrere Versuche stapeln sich, nichts wird ueberschrieben |
| **Seriell** | bleibt, wortgleich, damit die QEMU-Zahlen vergleichbar bleiben |
| **Neu im Kernel-Scratch** | `dos_seek` in `desk/dosbind.rs`, `vdi_backend::fs04_fb_geometry()`, `desk::fs04drv::boot_note()` |

Der Startblock ist nicht nur Bequemlichkeit: er traegt die
**Bildschirmgeometrie**, ohne die die in §6B gemessene Skalierung nicht
anwendbar ist — und in QEMU ist sie nicht zu erfahren. Er ist ausserdem der
Grund, warum sich das **uebergebene** Abbild selbst pruefen laesst: es legt die
Datei schon beim Hochfahren an, ohne dass jemand klicken muss.

### Wie die Reihenfolge garantiert ist

Das Schreiben der Datei ist selbst GEMDOS-, FAT-, Sektorcache- und
EMMC-Verkehr — also genau das, was gemessen wird. `report()` laeuft deshalb in
vier streng geordneten Schritten, und die Datei-Ein-/Ausgabe ist der letzte:

1. `take_snapshot` kopiert **jeden** Zaehler aus den Atomics in `SNAP`
   (40 Slots x cnt/tck/max/min, plus die belegten GEMDOS-Opcodes). Ab hier
   kann nichts mehr aendern, was berichtet wird.
2. `format_block` rendert den ganzen Block in den statischen Textpuffer —
   immer noch kein I/O.
3. `emit_serial` gibt den Puffer aus — kostenlos, und wortgleich zur Datei.
4. `write_card` oeffnet, springt ans Ende, schreibt, schliesst.

Aufgerufen wird `report()` erst, **nachdem das gemessene Zeitfenster
geschlossen ist**: der `fun_drag`-Mantel verbucht `DRAG_TOTAL` und ruft den
Bericht erst danach; der Treiberbetrieb liest seinen Endzeitstempel vor dem
Aufruf. Die Kosten von Schritt 4 werden gemessen und im **naechsten** Block als
`previous report write` ausgewiesen — sie verschwinden also nicht, sie stehen
nur nicht in den Zahlen, die sie verfaelschen wuerden.

Zwei Nebenwirkungen, die ehrlich dazugehoeren und im `pi-probe/README.TXT`
stehen:

* Der **wahrgenommene** Stillstand enthaelt das Schreiben der Datei (in QEMU
  1,4–3,0 ms). Die Zahl steht im naechsten Block, ist also nicht verborgen.
* `C:\FS04.TXT` ist ein Symbol mehr im C:-Fenster; beim Neuzeichnen kostet das
  rund 1 % (`fb.clean` 2282 ohne, 2310 mit).

### Lesbarkeit, weil ein anderer das liest

Reine Zaehlslots drucken jetzt `n=<zahl>` statt eines sinnlosen
`min=295147905179us`, und jeder Block traegt einen Hinweis, dass
`desk.fun_op` / `desk.copy_pass` / `desk.fun_drag` **vor** dem
Bestaetigungsdialog beginnen und daher die Wartezeit auf den Klick enthalten —
die Zahl, auf die es ankommt, ist `wall`. Ohne diesen Hinweis liest sich
`desk.fun_op = 8 505 831 us` wie ein Achtsekundenbefund, und er ist keiner.

### Geprueft, bevor es uebergeben wurde

| Pruefung | Ergebnis |
|---|---|
| genau dieses Abbild in `qemu -M raspi4b` | bis `[desk] desktop up`, **0** `[EXC]`/`[PANIC]` (`raw/pi03-boot-of-handover-image.log`) |
| `C:\FS04.TXT` entsteht und ist lesbar | Startblock, 268 Byte, aus dem Abbild zurueckgelesen |
| zweiter Start auf derselben Karte | **angehaengt** (536 Byte), nicht ueberschrieben |
| Ablegeblock mit voller Tabelle | `raw/g07-FS04.TXT` im Wortlaut (injektorgetriebene Variante desselben Patches, `method/fs04-gesture-r2.diff`) |
| `fsck_msdos -n` C: und D: nach allen Laeufen | sauber |
| Zahlen gegen Fassung 1 | g05 `wall` 25 021 us gegen g07 `wall` 24 987 us, 6+6 Sektorzugriffe in beiden |

`sha256 53bbbeab356e30a4bfe994f86fcdcda964c1fe29b85f8a9d0ddfbb0dc5fc762e`,
1 036 376 Byte, release, Board rpi4.

### Zwei Fassungen, beide vollstaendig abgelegt

`method/README.TXT` haelt auseinander, welche Rohdaten zu welcher Fassung
gehoeren: die Zahlen in §3–§6 stammen aus Fassung 1 (nur seriell,
`method/fs04-probe-640.diff`, `-1920.diff`, `fs04-gesture.diff`), das
Uebergabeabbild aus Fassung 2 (`method/fs04_probe_patch.py`,
`fs04-pi-handover.diff`, `fs04-gesture-r2.diff`). Ein `.diff` ist ein
vollstaendiger Patch; Fassung 1 bleibt damit reproduzierbar, obwohl das Skript
inzwischen Fassung 2 ist.
