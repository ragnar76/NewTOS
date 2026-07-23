# EmuTOS BIOS (Trap #13) + Boot-Init — Funktions-/Init-Inventar

> Researcher, 2026-07-23. Für BB3 (BIOS-Dispatcher-Audit) und BB1
> (Boot-Init-Audit), s. `docs/analysis/bios-audit/00-PLAN.md`. Liefert dem
> nachfolgenden Auditor eine Checkliste: jede BIOS-Trap-13-Funktion +
> jedes Boot-Init-Element, mit Kurzverhalten, EmuTOS-Referenz und der
> HW-abstrakt/RPi4-adaptiert-Klassifikation (die API-Fläche ist 1:1 zu
> matchen; das HW-Backend ist RPi4/BCM2711-HAL-spezifisch, siehe
> `docs/COMPATIBILITY.md` + Ziel-Profil Falcon030/TOS4 `make 512`).
>
> **Kein Vergleich mit dem NewTOS-Ist-Zustand** — das ist Aufgabe des
> nachfolgenden Auditors. Diese Tabelle beschreibt ausschließlich das
> EmuTOS-Soll (Spec-Extraktion), mit Verweis auf den jeweiligen
> vollständigen Spec-Eintrag.
>
> XBIOS (Trap #14) ist **nicht** in diesem Inventar (separater Scope,
> BB4).

## Legende

- **HW-abstrakt** = die Funktion/das Verhalten ist reine API-Semantik
  (ABI, Fehlercodes, Struktur-Layout) — 1:1 auf RPi4 nachzubilden, kein
  Hardware-Unterschied relevant.
- **RPi4-adaptiert** = die Funktion redet mit Atari-Hardware
  (Shifter/MFP/ACIA-IKBD/WD1772/ACSI/DMA); die *API-Fläche* bleibt 1:1,
  das *Backend* wird auf BCM2711-HAL umgestellt (Non-Goal:
  Atari-Hardware-Emulation).
- **gemischt** = ein Teil der Funktion ist reine ABI-Logik, ein Teil
  hardwarenah.

---

## Teil 1 — BIOS-Trap-13-Dispatcher-Funktionen (12)

| ID | Name | Kurzverhalten | Klassifikation | EmuTOS-Referenz | Spec |
|---|---|---|---|---|---|
| 0x00 | `Getmpb` | Füllt eine Memory Partition Block (MPB) mit dem statischen ST-RAM-Free-List-Descriptor (`themd` = `[membot,memtop)`); `mp_mfl`/`mp_mal` geschrieben, `mp_rover` unangetastet. Kein Fehlerpfad, keine Eingabevalidierung. Historisch genau einmal von GEMDOS-Init aufgerufen. | HW-abstrakt (reine Speicherbuchhaltung; `membot`/`memtop` selbst kommen aus RPi4-Speicherkarte statt ST-RAM-Detektion — die *Werte* sind RPi4-adaptiert, die *Funktion* ist abstrakt) | `bios/biosmem.c:134` (`getmpb`); Boot-Herkunft `bios/biosmem.c:51` (`bmem_init`) | `docs/api/bios/00-getmpb.md` |
| 0x01 | `Bconstat` | Meldet, ob am Eingabegerät `handle` (0-7) ein Byte wartet: 3-Stufen-Dispatch (Boot-Guard → optionaler Bconmap-Remap → `bconstat_vec[handle]`). `-1`=bereit, `0`=nicht bereit/ungültig/abwesend. | gemischt (Dispatch-Logik + Handle-Tabelle = abstrakt; die einzelnen Status-Routinen lesen MFP/SCC/ACIA-Register = RPi4-adaptiert) | `bios/bios.c:914` (`bconstat`); Standard-Tabelle `bios/chardev.c:30` | `docs/api/bios/01-bconstat.md` |
| 0x02 | `Bconin` | Liest (und konsumiert) ein Zeichen von `handle`, blockierend. CON (Handle 2) liefert ein gepacktes LONG: ASCII in Bits 0-7, IKBD-Scancode in Bits 16-23, optional `shifty`-Snapshot in Bits 24-31 (nur wenn `conterm` Bit 3 gesetzt — Default aus). | gemischt (Packing-Kontrakt = abstrakt; Queue-Dequeue/UART-Lesen = RPi4-adaptiert) | `bios/bios.c:958` (`bconin`); Encoding-Kommentar `bios/bios.c:948-956` | `docs/api/bios/02-bconin.md` |
| 0x03 | `Bconout` | Schreibt ein Zeichen an `handle`. Rückgabe geräteabhängig: `1L` = angenommen (Standard), `0L` = Drucker busy/Timeout (Handle 0 einzige Ausnahme mit echtem "retry"-Signal). | gemischt (Dispatch/Rückgabe-Kontrakt abstrakt; VT52/UART/Drucker-Backend RPi4-adaptiert) | `bios/bios.c:991` (`bconout`) | `docs/api/bios/03-bconout.md` |
| 0x04 | `Rwabs` | Liest/schreibt Sektoren, logisch (Laufwerksbuchstabe→Partition→physisch) oder physisch (`RW_NOTRANSLATE`). Retry-Loop + `etv_critic`-Critical-Error-Kette bei anhaltendem Fehler (nur logisch). Undokumentierte `adr==NULL`-Mediachange-Funktion (nur Floppy); logisches Schreiben auf Sektor 0 armt `forcechange`. | gemischt (Flags/Fehlercode-Kontrakt + `etv_critic`-Mechanik abstrakt; Sektor-I/O selbst = WD1772/ACSI/IDE/SCSI → **EMMC2-SD-Karte**, RPi4-adaptiert) | `bios/bios.c:1045` (`lrwabs`) → `bios/blkdev.c:406` (`blkdev_rwabs`) | `docs/api/bios/04-rwabs.md` (inkl. neu dokumentiertem `etv_critic`→AES-`eralert`-Pfad) |
| 0x05 | `Setexc` | Liest/schreibt ein 32-Bit-Vektor-Longword an Adresse `4*num`; `vector==-1` = nur Abfrage. Kein Range-Check. GEM-Hooks (`etv_timer/critic/term/xtra`) liegen bei `num=0x100..0x103` → `$400..$40C`. | HW-abstrakt (reine Vektortabellen-Semantik; der physische `4*num`-Adressraum selbst ist m68k-spezifisch und hat auf AArch64 keine Entsprechung — NewTOS braucht einen Software-Shadow, aber der **Kontrakt** get/set/-1-Query ist abstrakt) | `bios/bios.c:1070` (`setexc`) | `docs/api/bios/05-setexc.md` |
| 0x06 | `Tickcal` | Gibt die Konstante `20` (ms/Tick, 50 Hz GEMDOS/AES-Tick) zurück. Kein Bezug zu `hz_200`. | HW-abstrakt (reiner ABI-Konstante; unabhängig vom tatsächlichen Timer) | `bios/bios.c:1096` (`tickcal`) | `docs/api/bios/06-tickcal.md` |
| 0x07 | `Getbpb` | Liest den Bootsektor neu ein (keine Cache), parst FAT12/FAT16-BPB-Felder, klassifiziert FAT-Typ nach Clustercount, lehnt FAT32 ab (`recsiz=0`, NULL-Return). Setzt `mediachange`/`forcechange` zurück. Kein separater Fehlercode — nur `0L`/Pointer. | gemischt (BPB-Feldberechnung/FAT-Klassifikation abstrakt; Bootsektor-Lesen = RPi4-adaptiert, EMMC2-SD) | `bios/bios.c:1120` (`getbpb`) → `bios/blkdev.c:569` (`blkdev_getbpb`) | `docs/api/bios/07-getbpb.md` |
| 0x08 | `Bcostat` | Meldet Sendebereitschaft für `handle` (0-7). Historische Vertauschung: Handle 3 → `bcostat4` (IKBD), Handle 4 → `bcostat3` (MIDI) — **Kontrakt**, Programme verlassen sich darauf. | gemischt (Dispatch+Swap-Kontrakt abstrakt; Backend RPi4-adaptiert) | `bios/bios.c:1143` (`bcostat`); Swap-Kommentar `bios/chardev.c:40` | `docs/api/bios/08-bcostat.md` |
| 0x09 | `Mediach` | 3-Zustands-Rückgabe (`MEDIANOCHANGE`/`MEDIAMAYCHANGE`/`MEDIACHANGE`) oder negativer Fehlercode (`EUNDEV` bei ungültigem Laufwerk, Treiberfehler durchgereicht). Latched bis `Getbpb` zurücksetzt; `forcechange` (Sektor-0-Schreiben) erzwingt sofort `MEDIACHANGE`. | gemischt (3-Zustands-Kontrakt + Latch/Reset-Handshake abstrakt; Hardware-Sondierung = WD1772-Change-Line/SCSI-TUR → **SD, nicht wechselbar**, RPi4-adaptiert) | `bios/bios.c:1184` (`mediach`) → `bios/blkdev.c:783` (`blkdev_mediach`) | `docs/api/bios/09-mediach.md` |
| 0x0A | `Drvmap` | Gibt das globale `drvbits`-Bitmap unverändert zurück (kein Neuberechnen bei jedem Aufruf). Bit `n` = Laufwerk `n` (0=A:) vorhanden. | gemischt (Bitmap-Kontrakt abstrakt; Befüllung aus Atari-Floppy/ACSI-Enumeration → **SD-MBR-Partitionstabelle**, RPi4-adaptiert) | `bios/bios.c:1206` (`drvmap`) → `bios/blkdev.c:833` (`blkdev_drvmap`) | `docs/api/bios/0a-drvmap.md` |
| 0x0B | `Kbshift` | Liest/schreibt das `shifty`-Modifikator-Byte (`flag>=0`=setzen, sonst Query), immer den *alten* Wert zurückgebend (auf Byte trunkiert). | gemischt (Get/Set/Truncate-Kontrakt abstrakt; `shifty`-Quelle = ACIA/IKBD-6301-Scancode-ISR → **USB-HID über xHCI**, RPi4-adaptiert) | `bios/ikbd.c:145` (`kbshift`) | `docs/api/bios/0b-kbshift.md` |

**Zusatz-Notiz zur Dispatcher-Mechanik:** die Trap-13-Trampolin-Logik
selbst (`_biostrap`/`biosxbios`, Save-Area, Out-of-Range-Verhalten =
Rückgabe der Funktionsnummer) ist vollständig HW-abstrakt (reine
ABI-Konvention) und bereits in `docs/api/bios/_dispatcher.md`
dokumentiert — kein separater Inventar-Eintrag nötig.

---

## Teil 2 — Boot-Init-Elemente (Low-Memory-Sysvars, OSHEADER, Cookie-Jar)

Vollständige Details: `docs/api/bios/_boot-init.md`. Kurzübersicht für
die Auditor-Checkliste:

| Element | Kurzverhalten | Klassifikation | EmuTOS-Referenz |
|---|---|---|---|
| Cold-/Warmboot-Unterscheidung (`memvalid`/`memval2`/`memval3`) | Prüft drei Magic-Werte; bei Erfolg (Warmboot) bleibt der gesamte Sysvar-Bereich `$400`-`$800` unangetastet, bei Misserfolg (Coldboot) wird er komplett auf 0 gelöscht, dann neu befüllt | RPi4-adaptiert (die RAM-Bank-Sondierung selbst ist reine ST/TT-MMU-Hardware-Logik, für RPi4 ohne Äquivalent; der Cold/Warm-**Kontrakt** — reset-resident-Code überlebt bei validem `resvalid`/`resvector` — ist HW-abstrakt und muss NewTOS-seitig nachgebildet werden, falls geportete Apps ihn nutzen) | `bios/memory.S:117-142,560-610` |
| `resvalid`/`resvector` (Warm-Reset-Sprung) | Vor jeder Boot-Init geprüft; bei Match springt die CPU direkt auf `resvector`, überspringt den normalen Boot. EmuTOS selbst setzt sie nie, nur `panicasm.S` löscht `resvalid` | HW-abstrakt (App-Konvention, kein Atari-Chip beteiligt) | `bios/startup.S:502-519`, `bios/panicasm.S:115` |
| OSHEADER-Instanz | Statische ROM/Text-Daten (kein Laufzeit-Bau); `sysbase = &os_header` wird einmal zur Laufzeit gesetzt (`bios_init`) | HW-abstrakt (reines Struktur-Layout + ein Zeiger-Write; **Blocker für 1:1-portierte Apps**, s. Gap-Doc) | `bios/startup.S:73-114`, `bios/bios.c:358` |
| `os_run`→`run` (aktuelle PD) | GEMDOS-seitig gepflegt (`bdosmain.c`/`proc.c`), nicht BIOS | HW-abstrakt | `bdos/bdosmain.c:339`, `bdos/proc.c:575,609` |
| Cookie-Jar (`cookie_init`+`fill_cookie_jar`) | Legt 20-Slot-Array an, veröffentlicht via `$5a0`, befüllt mit Hardware-Erkennungs-Cookies | gemischt (Array/Scan-Datenmodell abstrakt; die *Inhalte* — `_CPU`/`_VDO`/`_FPU`/`_SND`/`_FDC` — sind Atari-Hardware-Ergebnisse, für RPi4 größtenteils **nicht vorhanden**, s. `_cookie-jar.md`'s Pi-Cookie-Set) | `util/cookie.c:26-35`, `bios/machine.c:665` |
| `chardev_init` (bcon*_vec-Tabellen) | Initialisiert die 4×8-Einträge-Vektortabellen + `CHARDEV_AVAILABLE`-Flag | gemischt (Tabellen-/Handle-Kontrakt abstrakt; die Handler selbst RPi4-adaptiert) | `bios/chardev.c` |
| `blkdev_init` (`hdv_*`-Vektoren, `dskbufp`, `pun_ptr`) | Installiert `hdv_bpb/rw/mediach` auf die Block-Device-Handler, allokiert den Disk-Puffer | gemischt (Vektor-Kontrakt abstrakt; Backend = **EMMC2-SD**, RPi4-adaptiert) | `bios/blkdev.c:60-110` |
| `bios_init`-"misc. variables"-Block | Setzt `sysbase`, `savptr`, `etv_timer/critic/term`, `swv_vec`, `nvbls`/`vblqueue`, `dumpflg` in einem Codeblock | HW-abstrakt | `bios/bios.c:356-373` |
| `biosmain`-Boot-Sequenz (`Drvmap`, `bootdev`, `blkdev_boot`, `autoexec`, `Pexec` Shell) | Orchestriert den restlichen Boot bis zum ersten Programmstart | gemischt (Ablauf-Kontrakt abstrakt; Laufwerks-/Boot-Device-Erkennung RPi4-adaptiert) | `bios/bios.c:781-878` |

---

## Zusammenfassung (Zahlen)

- **12** BIOS-Trap-13-Dispatcher-Funktionen — alle mit vollständigem
  Spec-Eintrag in `docs/api/bios/00-getmpb.md` … `0b-kbshift.md`
  (bereits vorhanden, in dieser Pass validiert, keine inhaltlichen
  Fehler gefunden; eine Lücke — der `etv_critic`-Kritischer-Fehler-Pfad
  in `04-rwabs.md` — wurde in dieser Pass geschlossen).
- **0** reine HW-abstrakte Funktionen ohne jeden Hardware-Bezug:
  `Getmpb`, `Setexc`, `Tickcal` sind am nächsten dran (Setexc/Tickcal
  = 0 Hardware-Bezug; Getmpb bezieht sich auf RPi4-Speicherkarte statt
  ST-RAM, aber die Funktion selbst ist reine Buchhaltung).
- **9 von 12** Funktionen sind **gemischt** (ABI-Kontrakt abstrakt,
  Backend RPi4-adaptiert) — das ist die Norm für BIOS, nicht die
  Ausnahme, weil BIOS per Definition die Hardware-Grenzschicht ist.
- **8** Boot-Init-Elemente dokumentiert (Cold/Warmboot-Unterscheidung,
  resvalid/resvector, OSHEADER, os_run, Cookie-Jar, chardev_init,
  blkdev_init, die zwei bios.c-Sequenzblöcke).

## Cross-references

- `docs/api/bios/_boot-init.md` — vollständige Sysvar-Tabelle +
  Boot-Zeit-Ablauf (Kern-Deliverable dieser Phase)
- `docs/api/bios/00-getmpb.md` … `0b-kbshift.md` — je Funktion voller
  Spec-Eintrag
- `docs/api/bios/_dispatcher.md` — Trap-13-Trampolin-Mechanik
- `docs/api/bios/_state.md` — kuratierte Sysvar-Teilmenge je Trap-Gruppe
- `docs/api/bios/_cookie-jar.md` — Cookie-Jar-Datenmodell (inkl.
  ADR-012-Konflikt-Notiz)
- `docs/api/structures/osheader.md` — OSHEADER-Feldlayout (validiert,
  Korrekturen dieser Pass)
- `docs/analysis/sysvar-osheader-lowmem-gap.md`,
  `docs/analysis/bios-audit/00-PLAN.md` — Auslöser/Auftrag
