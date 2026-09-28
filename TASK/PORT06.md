# PORT06 — Doom portieren: konform ueber BIOS, AES und VDI

Status: **OFFEN** · Prioritaet: **P3** (nach dem OFAM-Termin) · Typ: Portierung — Anwenderwunsch 2026-09-28

Abhaengigkeiten: — (**nicht** von [SDL01](SDL01.md); siehe §2)

## Die Auflage steht vor allem anderen

**Nur BIOS, AES und VDI. Kein `Physbase`, kein `Logbase`, kein `Setscreen`,
kein direkter Griff in den Framebuffer.**

Der Grund ist nicht Formalismus, sondern Reichweite: eine Grafikkarte ersetzt
den **VDI-Treiber**, nicht den Shifter dahinter. Was ueber die VDI geht, laeuft
auf NewTOS, auf einem TT, auf einem Falcon, auf einer Nova, unter Hatari. Was
`Physbase` anfasst, laeuft nur dort, wo der Shifter sitzt.

### Das staerkere Argument ist nicht Portabilitaet, sondern Geschwindigkeit

Anwender 2026-09-28: *„wenn man VDI-Routinen verwendet, dann wird auch die
Hardwarebeschleunigung verwendet."*

**Das dreht die uebliche Abwaegung um.** Der VDI-Treiber einer Grafikkarte kann
benutzen, was die Karte mitbringt; ein Programm, das selbst in den Framebuffer
schreibt, bekommt davon **nichts** — es hat sich die Beschleunigung durch
seinen eigenen Sonderweg weggenommen. Konformitaet ist hier also der
**schnellere** Weg, nicht der brave.

Fuer uns gilt dasselbe in die Zukunft: die VDI-Zeichenroutinen sollen spaeter
in optimiertes aarch64-Assembler wandern
([[vdi-draw-asm-optimization-later]]). Wer ueber die VDI geht, erbt das;
wer am Framebuffer vorbei arbeitet, nicht.

### ST-RAM ist auch auf dem TT kein Thema

Anwender: *„auch am TT brauchen wir kein ST-RAM, weil die Nova von der CPU
bedient wird."*

Das verallgemeinert, was [FS04](FS04.md) belegt hat: EmuTOS bevorzugt ST-RAM
fuer Puffer wegen **Floppy-/ACSI-DMA**
(`references/emutos/include/config.h:1723-1724`). Eine **CPU-bediente** Karte
hat keine DMA-Einschraenkung — der Grund faellt dort genauso weg wie bei uns.
**Kein Pi-Sonderfall, sondern die allgemeine Regel**, und damit traegt die
TT-RAM-Richtung aus §3 auf beiden Maschinen.

**Die Ball-Demo ist der Beleg in beide Richtungen** (`/Volumes/Daten/Development/ball-demo`,
ausserhalb des Repos). Gezaehlt ueber ihre Quellen: `vro_cpyfm` 41x,
`v_opnvwk` 16x, `v_get_pixel` 18x, `vs_color` 9x, `appl_init`/`graf_handle` je
9x — das ist die konforme Haelfte. Daneben `Vsync` 39x, `Setscreen` 21x,
`Logbase` 18x, `Physbase` 15x — die nicht konforme. Dass sie ein `plat-tt.c`
**und** ein `plat-pistorm.c` braucht, zeigt genau diese Trennlinie.

## 1. Warum es machbar ist — der Weg ist nicht nur vorhanden, sondern gemessen

Doom zeichnet in einen **eigenen 8-Bit-Palettenpuffer** (320x200) und schiebt
ihn je Bild auf den Schirm. Konform heisst das: **`vro_cpyfm` mit einem
Speicher-MFDB**, Palette ueber `vs_color`.

Genau diese Gruppe ist im September gegen echte TT- und Falcon-Dumps geprueft
worden: **61 von 62 Faellen wortgleich** (R02–R05,
`measurement-2026-09-21-r05/`, Falltabelle `tosdump/cases/vdi-raster-screen.tsv`).
Der Bildweg ist also belegt, nicht erhofft.

**Der Doppelpuffer ohne `Physbase`** ist die eine echte Entwurfsfrage: zweites
MFDB im Speicher, und `vro_cpyfm` schreibt es in einem Rutsch. Das ist
dieselbe Operation, die die Ball-Demo mit ihren 41 Aufrufen ohnehin misst.

## 2. Doom braucht SDL nicht

Die Originalfassung trennt die Plattform sauber ab — Video, Ton, Eingabe und
Netz liegen in eigenen Dateien, und jede Portierung ersetzt genau die. Wir
haben Framebuffer ueber VDI, Tastatur, und den 200-Hz-Takt
(`tos-api/src/bios/tick.rs`). Ton und Netz koennen beim ersten Anlauf
stillgelegt werden.

**PORT06 ist damit kein Folgeprojekt von SDL01, sondern ein eigener, moeglicherweise
kuerzerer Weg.**

## 3. Speicher — die Entwurfsentscheidung, die vor der ersten Zeile faellt

**Unsere TPA ist 4 MiB je Prozess** (`tos-api/src/bdos/process/arena.rs:27`).
Doom will fuer seine Zonenverwaltung mehr.

**Richtung des Anwenders (2026-09-28): vorrendern und im TT-RAM ablegen.**
Dort liegen ~942 MB brach (gegen 10 MB ST-RAM), und der Grund, aus dem EmuTOS
ST-RAM bevorzugt, ist Floppy-/ACSI-DMA — den gibt es bei uns nicht
([FS04](FS04.md) hat das belegt). Also: Texturen und Sprites **einmal** in die
Bildschirmform bringen, im TT-RAM halten, und der Bildaufbau wird eine Kopie
statt einer Umrechnung je Bild.

Der Weg dorthin ist `Malloc`/`Mxalloc` mit TT-RAM-Praeferenz; dass ein solcher
Block in den EL0-Raum der Anwendung eingeblendet werden muss, ist bei uns
bekannt und gebaut ([[el0-app-memory-must-be-mapped]]).

## 4. Vor dem Anfangen zu klaeren

* **Lizenz.** Die Quelltextfreigabe von 1997 stand unter einer eigenen,
  **nicht-kommerziellen** Lizenz; die spaeter unter GPL gestellte Fassung ist
  eine andere und die, auf der praktisch alle Portierungen aufbauen. Fuer ein
  Projekt, das sich an EmuTOS' Lizenz anlehnt, ist das keine Formalie —
  **welche Fassung wir nehmen, gehoert entschieden, bevor Quelltext in den Baum
  kommt.** Dazu: Doom braucht eine WAD-Datei; die Shareware-WAD darf
  weitergegeben werden, die Vollversionen nicht.
* **Paletten-Pfad.** Doom ist 8-Bit-indiziert, unsere VDI geht Richtung
  Truecolor ([[vdi-truecolor-workstation]]). Wie ein indizierter Puffer samt
  Palette auf den Schirm kommt, ist **die** Stelle, an der die Arbeit steckt —
  nicht in der Spiellogik.
* **Eingabe.** Tastatur reicht fuer den ersten Anlauf; Maus ueber die AES ist
  vorhanden.

## Abnahme

Doom laeuft, spielbar, mit Tastatur — und ein `/usr/bin/grep` ueber die
Portierung findet **kein** `Physbase`, `Logbase` oder `Setscreen`. Bildrate
gemessen und festgehalten: sie ist der ehrlichste Durchsatztest, den wir
vorfuehren koennen, und beantwortet die Frage des OFAM-Veranstalters
(*wie schnell laeuft das?*) besser als jede Messreihe.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
