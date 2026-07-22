# EmuTOS Build-Config-Profil — Compile-Schalter-Katalog

> Researcher, 2026-07-22. Rein statisch (grep + Quelltext-Lesung), kein
> Build/QEMU. Primärquelle: `references/emutos/include/config.h` (2256
> Zeilen, vollständig gelesen), Sekundärquelle: `references/emutos/Makefile`,
> `references/emutos/tools/localise.c` (generiert `include/i18nconf.h`),
> Cross-Grep über den gesamten `references/emutos/`-Baum für jeden
> `CONF_WITH_*`/`CONF_*`-Verbraucher. EmuTOS-Stand: Commit
> `1a7484aba1bfba23e2c0899e243b6d8177fe41df` (2026-04-14), wie im
> Arbeitsbaum unter `references/emutos/` vorliegend (kein Git-Submodule,
> flacher Checkout).
>
> **Zweck:** Katalog aller EmuTOS-Compile-Zeit-Schalter + Festlegung, welches
> Profil NewTOS matcht, damit AES/VDI/GEMDOS-Audits nicht mehr einzeln
> nachschlagen müssen, ob eine fehlende Funktion ein echter Bug oder eine
> legitime Feature-Absenz ist.
>
> **Wichtige Korrektur gegenüber bestehendem Audit:** `p6-rsrc-fsel-scrp-shel.md`
> (Zeilen 13, 215) behauptet `CONF_WITH_SHUTDOWN=0` für unser Profil und dass
> `SHW_SHUTDOWN` deshalb "wegkompiliert" sei. Das ist **falsch gelesen** — siehe
> Teil 2, Abschnitt "Korrektur CONF_WITH_SHUTDOWN". Die praktische Schlussfolgerung
> (SHW_SHUTDOWN tut im Referenzprofil nichts) bleibt richtig, aber der Mechanismus
> ist ein anderer (Laufzeit-No-Op, nicht Compile-Zeit-Abwesenheit).

---

## Teil 1 — Vollständiger Schalter-Katalog

Alle Werte sind der Default im **Standard-Profil** (siehe Teil 2): keiner der
Blöcke `TARGET_192` / `TARGET_256` / `TARGET_CART` / `MACHINE_ARANYM` /
`MACHINE_FIREBEE` / `MACHINE_AMIGA` / `MACHINE_LISA` / `MACHINE_M548X` ist
aktiv; `CONF_ATARI_HARDWARE` bleibt auf seinem echten Default `1`
(config.h:665-667); kein `__mcoldfire__`. Das ist exakt das Ergebnis von
`make 512`, `make 1024` oder `make prg` (keiner dieser drei Make-Targets
definiert einen der genannten Blöcke — siehe Teil 2).

Spalte "API-relevant?": bezieht sich auf die von uns nachgebildete
Syscall-/Struct-/Verhaltens-Oberfläche (GEMDOS-Trap 1 / BIOS-Trap 13 /
XBIOS-Trap 14 / AES-Opcode / VDI-Opcode). "nein" heißt: reine
Hardware-Anwesenheit, Performance-Interna oder Boot-Kosmetik ohne
beobachtbaren API-Unterschied für ein TOS-Programm.

### 1a. Hardware-Support-Sektion (config.h:824-1220)

| Schalter | config.h:Zeile | Default (Standard-Profil) | gated Feature | API-relevant? |
|---|---|---|---|---|
| `CONF_WITH_ADVANCED_CPU` | 831-837 | 1 | 68010-68060-Unterstützung (vs. reiner 68000) | nein (CPU-Interna) |
| `CONF_WITH_APOLLO_68080` | 842-848 | 1 | Apollo 68080-CPU-Unterstützung | nein |
| `CONF_WITH_ST_MMU` | 853-855 | 1 | ST-MMU-Ansteuerung | nein |
| `CONF_WITH_TT_MMU` | 860-862 | 1 | TT-MMU-Ansteuerung | nein |
| `CONF_WITH_FALCON_MMU` | 867-869 | 1 | Falcon-MMU-Ansteuerung | nein |
| `CONF_STRAM_SIZE` | 875-877 | 0 (Autodetect) | feste ST-RAM-Größe statt Autodetect | nein |
| `CONF_WITH_ALT_RAM` | 882-884 | 1 | Alt-RAM-Erkennung/-Nutzung | **ja** (borderline) — GEMDOS `Mxalloc`-Modi (`bdos/mem.h`, `bdos/umem.c`, `bdos/proc.c`, `bdos/bdosmain.c` referenzieren `CONF_WITH_ALT_RAM`/`CONF_WITH_TTRAM`); ohne Alt-RAM sind TT-RAM-Präferenz-Flags in `Mxalloc` bedeutungslos |
| `CONF_WITH_STATIC_ALT_RAM` | 891-893 | 0 | feste Alt-RAM-Adresse statt Laufzeit-Erkennung | nein |
| `CONF_WITH_TTRAM` | 898-900 | 1 | TT-RAM-Erkennung | **ja** (borderline, s.o.) |
| `CONF_TTRAM_SIZE` | 906-908 | 0 (Autodetect) | feste TT-RAM-Größe | nein |
| `CONF_WITH_MONSTER` | 914-916 | 1 | MonSTer-Erweiterungskarte (RAM) | nein |
| `CONF_WITH_MAGNUM` | 922-924 | 1 | Magnum-RAM-Erweiterungskarte | nein |
| `CONF_WITH_MFP` | 929-931 | 1 | MFP-68901-Basisunterstützung (Timer/IRQ/erste serielle Schnittstelle) | nein (Interrupt-Interna; Bconin/Bconout-Gerät 1 existiert unabhängig davon als API-Oberfläche) |
| `CONF_WITH_MFP_RS232` | 936-938 | 1 | MFP-RS232 | nein |
| `CONF_WITH_TT_MFP` | 943-945 | 1 | zweiter MFP (TT) | nein |
| `CONF_WITH_SCC` | 950-952 | 1 | SCC-Chip (Falcon/TT serielle Schnittstelle B) | nein |
| `CONF_COLDFIRE_TIMER_C` | 958-964 | 0 (nicht `__mcoldfire__`) | ColdFire-Timer-C-Emulation | nein |
| `CONF_WITH_COLDFIRE_RS232` | 969-975 | 0 | ColdFire-interne serielle Schnittstelle | nein |
| `CONF_WITH_YM2149` | 980-982 | 1 | YM2149-Soundchip | **ja** (XBIOS-Soundfunktionsfamilie, nicht AES) |
| `CONF_WITH_PRINTER_PORT` | 987-989 | 1 | Parallele Druckerschnittstelle | nein (Gerätepräsenz, kein API-Formunterschied) |
| `CONF_WITH_MIDI_ACIA` | 994-996 | 1 | MIDI-ACIA | nein |
| `CONF_WITH_IKBD_ACIA` | 1001-1003 | 1 | IKBD-ACIA (Tastatur/Maus-Anschluss) | nein (Transport; Events selbst sind immer da) |
| `CONF_WITH_IKBD_CLOCK` | 1008-1010 | 1 | IKBD-Uhr (Datum/Zeit über Tastaturprozessor) | nein |
| `CONF_WITH_CARTRIDGE` | 1015-1023 | 1 (0 nur bei `DIAGNOSTIC_CARTRIDGE`) | ROM-Port-Cartridge-Unterstützung | nein |
| `CONF_WITH_FDC` | 1028-1030 | 1 | Floppy-Controller | nein (Bootpfad/Gerätepräsenz) |
| `CONF_WITH_ACSI` | 1035-1037 | 1 | ACSI-Bus | nein |
| `CONF_WITH_SCSI` | 1042-1044 | 1 | SCSI-Bus | nein |
| `CONF_WITH_IDE` | 1049-1051 | 1 | Falcon-IDE | nein |
| `CONF_WITH_SDMMC` | 1056-1058 | 0 | SD/MMC-Bus | nein (bei uns irrelevant — eigener SD-Treiber) |
| `CONF_WITH_VAMPIRE_SPI` | 1064-1066 | 0 | Vampire-SPI für SD/MMC | nein |
| `CONF_WITH_ATARI_VIDEO` | 1071-1073 | 1 | Master-Schalter ST-Shifter+ | **ja** — Basis für die gesamte Bildschirm-/VDI-Oberfläche (`bios/screen.c`, `bios/bios.c`, `bios/vectors.S`); bei 0 existiert kein Bildschirm überhaupt |
| `CONF_WITH_STE_SHIFTER` | 1078-1080 | 1 | STE-Shifter (Softscroll, DMA-Sound-Trigger) | **ja** (borderline) — zusätzliche Videomodi/Registeroberfläche für XBIOS `Setscreen`-Familie |
| `CONF_WITH_TT_SHIFTER` | 1085-1087 | 1 | TT-Shifter (TT-High-Mono, TT-Palette) | **ja** (borderline) — zusätzliche VDI-Auflösungen/Palettenfunktionen |
| `CONF_WITH_VIDEL` | 1092-1094 | 1 | Falcon-Videl (Echtfarbmodi, `VsetMode`/`VsetRGB`) | **ja** — Voraussetzung für `CONF_WITH_VDI_16BIT` (Sanity-Check config.h:2198-2202); gated VDI-16-Bit-Truecolor-Oberfläche und XBIOS-Videl-Funktionsfamilie |
| `CONF_VRAM_ADDRESS` | 1101-1103 | 0 (VRAM in ST-/Alt-RAM) | feste VRAM-Adresse für Grafikkarten | nein |
| `CONF_WITH_MEGARTC` | 1108-1110 | 1 | MegaST-Echtzeituhr | nein (XBIOS `Gettime`/`Settime`-Backend, kein Formunterschied) |
| `CONF_WITH_ICDRTC` | 1117-1119 | `= CONF_WITH_ACSI` → 1 | ICD-AdSCSI-Plus-Uhr | nein |
| `CONF_WITH_ULTRASATAN_CLOCK` | 1125-1127 | `= CONF_WITH_ACSI` → 1 | UltraSatan-Uhr | nein |
| `CONF_WITH_DMASOUND` | 1132-1134 | 1 | STe/TT/Falcon-DMA-Sound | **ja** (XBIOS-Soundfunktionsfamilie, nicht AES) |
| `CONF_WITH_DSP` | 1139-1141 | 1 | Falcon-DSP | **ja** — gated `Dsp*`-XBIOS-Funktionsfamilie real (`bios/xbios.c:1004,1275,1431`); nicht AES |
| `CONF_WITH_VME` | 1146-1148 | 1 | Mega-STe-VME-Bus | nein |
| `CONF_WITH_DIP_SWITCHES` | 1154-1156 | 1 | DIP-Schalter (Konfigurationsquelle) | nein |
| `CONF_WITH_NVRAM` | 1161-1163 | 1 | NVRAM | **ja** — gated XBIOS-Opcode 0x2E `Nvmaccess` real vs. `xbios_unimpl` (`bios/xbios.c:1349-1353`) |
| `CONF_WITH_BLITTER` | 1168-1170 | 1 | Blitter-Chip | **ja** (borderline) — reine Performance-Alternative in `vdi/vdi_raster.c`/`vdi_line.c` (Pixelergebnis identisch); zusätzlich gated `aes/geminit.c:427-429` einen echten `Blitmode()`-XBIOS-Aufruf beim EMUDESK.INF-Parsen (`#E`-Zeile) |
| `CONF_WITH_SFP004` | 1175-1177 | 1 | 68881-FPU-Board für Mega ST | nein |
| `CONF_WITH_NOVA` | 1182-1184 | 1 | Nova-Grafikkartenadapter | nein |
| `CONF_WITH_FLEXCAN` | 1190-1192 | 0 | FlexCAN-Controller (Eiffel-Tastatur an ColdFire-Boards) | nein |
| `CONF_WITH_RESET` | 1202-1208 | 1 (ROM-Ziel, nicht `EMUTOS_LIVES_IN_RAM`, nicht ColdFire) | `reset`-Opcode beim Start ausführen | nein (Boot-Interna) |
| `CONF_WITH_BAS_MEMORY_MAP` | 1214-1220 | 0 (nicht `MACHINE_FIREBEE`) | BaS-Speicherlayout | nein |

### 1b. Software-Sektion AES (config.h:1224-1355)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? | AES-Audit-Bezug |
|---|---|---|---|---|---|
| `AES_STACK_SIZE` | 1250-1252 | 590 (LONGs) | interne AES-Stackgröße pro Prozess | nein (Interna) | — |
| `CONF_WITH_3D_OBJECTS` | 1258-1260 | **1** | 3D-Objekt-Rendering wie TOS4 (`OBJC_SYSVAR`, `ob_flags`-3D-Bits) | **ja** | P1 `OBJC_SYSVAR`(48) (`coverage-matrix.md:36,106`); `object.rs:546` |
| `CONF_WITH_COLOUR_ICONS` | 1266-1268 | **1** | Farbicons wie TOS4 | **ja** | P6 `p6-rsrc-fsel-scrp-shel.md:13` |
| `CONF_WITH_EXTENDED_OBJECTS` | 1274-1276 | **1** | MagiC-artige Objekttyp-Erweiterungen (u.a. `G_STRING`-Unterstreichung, `aes/gemoblib.c:847-852`); Voraussetzung für `CONF_WITH_ALT_DESKTOP_GRAPHICS` (Sanity-Check config.h:2192-2196) | **ja** (bisher in keinem P1-P6-Audit einzeln erwähnt) | — (Lücke: sollte in objc_draw-Tiefenprüfung aufgenommen werden) |
| `CONF_WITH_GRAF_MOUSE_EXTENSION` | 1282-1284 | **1** | `graf_mouse()`-Modi M_SAVE/M_RESTORE/M_PREVIOUS | **ja** | P5 (`p5-window-graf.md:108`) |
| `CONF_WITH_LOADABLE_CURSORS` | 1290-1292 | 1 (+ `CURSOR_RSC_NAME` = `A:\EMUCURS.RSC`, Zeile 1294) | Mauszeiger aus Ressourcendatei nachladbar | **ja** (borderline — betrifft Cursor-Init, nicht in P1-P6 einzeln geprüft) | — |
| `CONF_WITH_MENU_EXTENSION` | 1304-1306 | **1** | `menu_popup`/`menu_attach`/`menu_istart`/`menu_settings` | **ja** | P1 (`coverage-matrix.md:35,89-92`); P2 (`p2-event-queue-ctrl.md:163`) |
| `CONF_WITH_NICELINES` | 1312-1314 | 1 | gezeichnete statt gestrichelte Menü-Separatoren | nein (reine Optik, kein API-Unterschied) | — |
| `CONF_WITH_PCGEM` | 1319-1321 | **1** | PC-GEM-kompatible AES-Funktionen (`APPL_YIELD`, `SCRP_CLEAR`, `SHEL_RDEF`/`WDEF`) | **ja** | P1 (`p1-appl-pd-dispatch.md:22,57`); P6 (`p6-rsrc-fsel-scrp-shel.md:12,191,215,242`); `coverage-matrix.md:34,63,141,185-186` |
| `CONF_WITH_WINDOW_COLOURS` | 1328-1330 | **1** | `wind_get`/`wind_set` `WF_COLOR`(18)/`WF_DCOLOR`(19) | **ja** | P5 (`p5-window-graf.md:132`) |
| `AES_VERSION` | 1345-1355 (Formel greift bei allen 4 Voraussetzungen: 1346-1347) | **0x0340** (berechnet aus obigen 4 Schaltern, alle=1) | AES-Versionsstring (`ap_version` in `aes/gemsuper.c:92`); zusätzlich direkter Gate für `WF_OWNER`(20) `aes/gemwmlib.c:1578` (`AES_VERSION>=0x330`) und `NUM_WIN`=16 statt 8 (`include/sysconf.h:24-34`, `AES_VERSION>0x0320`) | **ja** | P5 (`p5-window-graf.md:132,156`) |

### 1c. Software-Sektion BIOS (config.h:1359-1570)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? |
|---|---|---|---|---|
| `TOS_VERSION` | 1367-1372 | 0x206 | kosmetischer Versionswert im OS-Header (`bios/startup.S:81`) | nein — **rein kosmetisch**, keine `#if TOS_VERSION`-Verzweigung irgendwo im Quellbaum außer config.h selbst (geprüft per Grep) |
| `ALWAYS_SHOW_INITINFO` | 1379-1381 | 0 | Willkommensbildschirm auch bei Warmstart | nein |
| `FULL_INITINFO` | 1388-1390 | 1 | vollständiger vs. einzeiliger Willkommensbildschirm | nein |
| `INITINFO_DURATION` | 1403-1405 | 3 (Sekunden) | Anzeigedauer Willkommensbildschirm | nein |
| `CONF_DETECT_FIRST_BOOT_WITHOUT_MEMCONF` | 1414-1420 | 0 (ROM-Ziel, `EMUTOS_LIVES_IN_RAM`=0) | Erstboot-Erkennung ohne `memconf`-Aufruf | nein |
| `CONF_SERIAL_CONSOLE` | 1427-1429 | 0 | Konsole zusätzlich/exklusiv über seriell | nein (Transportwahl, kein API-Formunterschied) |
| `CONF_SERIAL_CONSOLE_ANSI` | 1436-1442 | 0 (folgt `CONF_SERIAL_CONSOLE`) | ANSI- statt VT52-Escapes am seriellen Terminal | nein |
| `CONF_SERIAL_CONSOLE_POLLING_MODE` | 1453-1455 | 0 | Polling statt IRQ für seriellen Tastatur-Ersatz | nein |
| `CONF_SERIAL_IKBD` | 1461-1463 | 0 | IKBD-Geräte über seriell statt ACIA | nein |
| `CONF_WITH_68030_PMMU` | 1470-1472 | 1 | PMMU-Baum auf 68030 | nein |
| `CONF_WITH_68040_PMMU` | 1479-1481 | 0 | PMMU-Baum auf 68040 (für FreeMiNT/ARAnyM-MMU) | nein |
| `CONF_WITH_BIOS_EXTENSIONS` | 1487-1489 | 1 | diverse BIOS-Erweiterungsfunktionen | **ja** — konkret: `Floprate()`-Sonderwerte -2/-3/-4 (TOS3+, `bios/floppy.c:1049-1055`) sind nur mit diesem Schalter real; sonst wird der normale Rate-Pfad genommen |
| `CONF_WITH_EXTENDED_MOUSE` | 1496-1498 | 1 | erweiterte Maus-Scancodes (Tasten 3-5, Rad, Eiffel) | **ja** (borderline) — zusätzliche IKBD-Scancodes, die letztlich als AES-Mausereignisse ankommen |
| `CONF_WITH_FRB` | 1503-1505 | `= CONF_WITH_ALT_RAM` → 1 | `_FRB`-Cookie automatisch setzen | nein |
| `CONF_WITH_MEMORY_TEST` | 1510-1512 | 0 | Speichertest beim Kaltstart | nein |
| `CONF_WITH_XBIOS_SOUND` | 1520-1522 | 1 | Falcon-XBIOS-Soundfunktionen auch auf STe/TT nachgebildet; erfordert `CONF_WITH_DMASOUND` (Sanity-Check 2180-2184) | **ja** (XBIOS, nicht AES) |
| `DEFAULT_BAUDRATE` | 1527-1529 | B9600 | Default-Baudrate serielle Ports | nein |
| `HD_DETECT_RETRIES` | 1544-1546 | 0 | Retry-Anzahl Festplattenerkennung | nein |
| `CONF_WITH_1FAT_SUPPORT` | 1558-1560 | 0 | Ein-FAT- statt Zwei-FAT-Dateisysteme | nein (Dateisystem-intern, für Programme transparent) |
| `CONF_WITH_GPT_SUPPORT` | 1566-1568 | 1 | GUID-Partitionstabellen | nein (Partitionierungs-intern) |

### 1d. Software-Sektion GEMDOS (config.h:1571-1608)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? |
|---|---|---|---|---|
| `GEMDOS_VERSION` | 1589-1591 | **0x2000** | `Sversion()`-Rückgabewert | **ja, aber nur kosmetisch** — einzige Verwendung im gesamten Baum ist `return (long)GEMDOS_VERSION;` in `bdos/bdosmain.c:300` (per Grep verifiziert); keine `#if GEMDOS_VERSION`-Verzweigung irgendwo — die GEMDOS-Implementierung selbst verhält sich unabhängig von diesem Wert immer gleich |
| `CONF_LOGSEC_SIZE` | 1605-1607 | 512 | maximale logische Sektorgröße für Atari-FAT | nein (nur für Atari-native FAT relevant, bei uns über den Autodetect-Pfad "falls größer gefunden" ohnehin situativ) |

### 1e. Software-Sektion VDI (config.h:1611-1663)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? |
|---|---|---|---|---|
| `CONF_WITH_GDOS` | 1620-1622 | 1 | `vst_load_fonts()`/`vst_unload_fonts()` als GDOS-Interception-Stubs | **ja** (VDI) |
| `CONF_WITH_VDI_16BIT` | 1628-1630 | 1 | Falcon-16-Bit-Truecolor-VDI-Modi; erfordert `CONF_WITH_VIDEL` (Sanity-Check 2198-2202) | **ja** (VDI) |
| `CONF_WITH_VDI_TEXT_SPEEDUP` | 1636-1638 | 1 | Textausgabe-Performance | nein (Ergebnis identisch, nur schneller) |
| `CONF_WITH_VDI_VERTLINE` | 1644-1646 | 1 | Vertikallinien-Performance | nein |
| `MAX_VERTICES` | 1653-1655 | **1024** | Vertex-Obergrenze für `v_fillarea`/`v_pline`/`v_pmarker` (TOS2: 512, TOS3/4: 1024) | **ja** (VDI, echtes Limit für den Aufrufer) |
| `NUM_VDI_HANDLES` | 1660-1662 | **128** | max. gleichzeitig offene VDI-Workstations | **ja** (VDI, `v_opnvwk`-Fehlerschwelle) |

### 1f. Hard-Disk-APIs (config.h:1666-1694)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? |
|---|---|---|---|---|
| `CONF_WITH_EXTERNAL_DISK_DRIVER` | 1675-1677 | 0 | eingebauter Festplattentreiber vs. externer Treiber aus dem Root-Sektor | nein (BIOS-Backend-Wahl); schließt `CONF_WITH_XHDI`/`CONF_WITH_SCSI_DRIVER` aus (Sanity-Check 2204-2208) |
| `CONF_WITH_SCSI_DRIVER` | 1684-1686 | `= !CONF_WITH_EXTERNAL_DISK_DRIVER` → 1 | SCSI-Treiber-API für Anwenderprogramme | **ja** (eigene API-Familie, nicht AES/GEMDOS-Kern) |
| `CONF_WITH_XHDI` | 1691-1693 | `= !CONF_WITH_EXTERNAL_DISK_DRIVER` → 1 | XHDI-Cookie/-API | **ja** (eigene API-Familie, nicht AES/GEMDOS-Kern) |

### 1g. Allgemein (config.h:1697-1711)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? |
|---|---|---|---|---|
| `USE_STATIC_INLINES` | 1708-1710 | 1 | reine Compiler-Optimierung | nein |

### 1h. Desktop-Sektion — EmuDesk, nicht AES-Kern (config.h:1714-1869)

Diese Schalter betreffen `desk/` (EmuDesk), das NewTOS laut ADR-022
(`docs/MEMORY.md`) 1:1 in `kernel/src/desk/` nachbaut. Sie sind kein
AES-Opcode-Gating, aber Desktop-Verhalten, das unsere `desk/`-Portierung
matchen muss.

| Schalter | config.h:Zeile | Default | gated Feature | Bereits in NewTOS referenziert? |
|---|---|---|---|---|
| `CONF_PREFER_STRAM_DISK_BUFFERS` | 1726-1732 | 1 (`CONF_ATARI_HARDWARE`=1) | DMA-Puffer in ST-RAM statt Alt-RAM | nein (Interna) |
| `CONF_WITH_ALT_DESKTOP_GRAPHICS` | 1739-1741 | 1 | linksbündige Dialogtitel + Unterstrich statt Atari-zentriert; erfordert `CONF_WITH_EXTENDED_OBJECTS` (Sanity-Check 2192-2196) | nein — sollte geprüft werden |
| `CONF_WITH_BACKGROUNDS` | 1747-1749 | 1 | Hintergrundmuster/-farbe konfigurierbar | nein |
| `CONF_WITH_BOTTOMTOTOP` | 1755-1757 | 1 | Menüpunkt "Cycle windows" | nein |
| `CONF_WITH_CACHE_CONTROL` | 1762-1768 | 1 (nicht ColdFire) | Menüpunkt "Cache"; berührt auch `aes/geminit.c` Cache-Zustand beim EMUDESK.INF-Parsen | nein |
| `CONF_WITH_DESKTOP_CONFIG` | 1774-1776 | 1 | Dialog "Desktop configuration" | ja — `desk/inf.rs:138` |
| `CONF_WITH_DESKTOP_SHORTCUTS` | 1782-1784 | 1 | Datei-/Ordner-Verknüpfungen auf dem Desktop | nein |
| `CONF_WITH_EASTER_EGG` | 1790-1792 | `= CONF_WITH_YM2149` → 1 | EmuDesk-Easter-Egg | nein (irrelevant für uns — kein YM2149) |
| `CONF_WITH_FILEMASK` | 1798-1800 | 1 | konfigurierbare Fenster-Dateimaske | ja — `desk/main.rs:1003` |
| `CONF_WITH_FORMAT` | 1805-1807 | 1 | Diskettenformatierung im EmuDesk; erfordert `CONF_WITH_FDC` (Sanity-Check 2186-2190) | nein |
| `CONF_WITH_PRINTER_ICON` | 1812-1814 | 1 | Drucker-Icon im EmuDesk | nein |
| `CONF_WITH_READ_INF` | 1819-1821 | 1 | Menüpunkt "Read .INF file" | ja — `desk/menu.rs:716` |
| `CONF_WITH_SEARCH` | 1826-1828 | 1 | Menüpunkt "Search" | ja — `desk/main.rs:1100`, `desk/menu.rs:711` |
| `CONF_WITH_SELECTALL` | 1834-1836 | 1 | Menüpunkt "Select all items" | nein |
| `CONF_WITH_SHOW_FILE` | 1842-1844 | 1 | Datei per Doppelklick anzeigen | nein |
| `CONF_WITH_SIZE_TO_FIT` | 1850-1852 | 1 | Menüpunkt "Size to fit" | ja — `desk/win.rs:245,274`, `desk/global.rs:211` |
| `CONF_WITH_VIEWER_SUPPORT` | 1859-1861 | 1 | konfigurierbarer Default-Viewer | nein |
| `CONF_WITH_WINDOW_ICONS` | 1866-1868 | 1 | Datei-/Ordner-Icons in Fenstern | nein |

### 1i. Emulator-Sektion (config.h:1872-1925)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? |
|---|---|---|---|---|
| `CONF_WITH_UAE` | 1880-1882 | 0 | UAE-Emulator-Erweiterungen (nur Amiga) | nein — irrelevant für uns |
| `USE_STOP_INSN_TO_FREE_HOST_CPU` | 1889-1891 | 1 | `STOP`-Opcode in Idle-Loops | nein |
| `STONX_NATIVE_PRINT` | 1900-1902 | 0 | StonX-artiger `native_print()` | nein |
| `DETECT_NATIVE_FEATURES` | 1908-1914 | **1** (nicht ColdFire) | NatFeat-Schnittstelle (Emulator-Hypervisor-Erkennung) zur Laufzeit sondieren | nein direkt, aber **steuert den Default zweier anderer Schalter** (`CONF_WITH_ARANYM`, `CONF_WITH_SHUTDOWN` — siehe Korrektur in Teil 2) |
| `CONF_WITH_ARANYM` | 1919-1925 | 1 (da `DETECT_NATIVE_FEATURES`=1) | ARAnyM-spezifische Workarounds | nein (reine Quirk-Behandlung, kein API-Formunterschied) |

### 1j. Debug-Sektion (config.h:1929-2051)

| Schalter | config.h:Zeile | Default | gated Feature | API-relevant? |
|---|---|---|---|---|
| `CONF_DEBUG_AES_STACK` | 1937-1939 | 0 | AES-Stack-Tracing | nein |
| `CONF_DEBUG_DESK_STACK` | 1944-1946 | 0 | Desktop-Stack-Monitoring | nein |
| `CONSOLE_DEBUG_PRINT` | 1954-1956 | 0 | Debug-Ausgabe auf BIOS-Konsole | nein |
| `RS232_DEBUG_PRINT` | 1963-1969 | 0 | Debug-Ausgabe auf MFP-RS232 | nein |
| `SCC_DEBUG_PRINT` | 1976-1978 | 0 | Debug-Ausgabe auf SCC-Port B | nein |
| `COLDFIRE_DEBUG_PRINT` | 1983-1989 | 0 | Debug-Ausgabe ColdFire-seriell | nein |
| `MIDI_DEBUG_PRINT` | 1996-1998 | 0 | Debug-Ausgabe auf MIDI | nein |
| `CARTRIDGE_DEBUG_PRINT` | 2012-2014 | 0 | Debug-Ausgabe über Cartridge-Port | nein |
| `HAS_KPRINTF` | 2017-2022 (Formel) | **1** (weil `DETECT_NATIVE_FEATURES`=1) | ob `kprintf()` überhaupt verfügbar ist | nein |
| `CONF_WITH_SHUTDOWN` | 2028-2034 | **1** (weil `DETECT_NATIVE_FEATURES`=1 — siehe Korrektur Teil 2!) | `shutdown()`-Funktion kompiliert; Laufzeitverhalten separat über `can_shutdown()` | **ja** — gated `SHW_SHUTDOWN`-Fall in `aes/gemshlib.c:177-184` real (kompiliert, kein Dead-Code) |
| `CONF_WITH_EJECT` | 2040-2042 | 0 | automatischer Floppy-Eject | nein |
| `CONF_WITH_BUS_ERROR` | 2049-2051 | 1 | Bus-Error-Exception bei ungültigem Speicherzugriff (echte Atari-Hardware-Eigenschaft) | nein (Hardware-Trap-Verhalten, nicht TOS-API) |

### 1k. Nicht in config.h: Makefile- und generierte Schalter

| Schalter | Quelle | Default (Standard-Profil) | gated Feature | API-relevant? |
|---|---|---|---|---|
| `WITH_AES` | `Makefile:147` (Default), `:627` (`cart`-Ziel setzt 0) | **1** | ob `aes/` + `desk/` überhaupt in den Build aufgenommen werden (`Makefile:334,451`) | **ja** — Master-Schalter, ohne den die gesamte AES-Oberfläche fehlt |
| `WITH_CLI` | `Makefile:150` (Default), `:533` (`192`-Ziel setzt 0) | 1 | ob `cli/` (EmuCON) aufgenommen wird | nein (kein AES/GEMDOS-API-Unterschied) |
| `CONF_WITH_NLS` / `CONF_MULTILANG` | generiert in `include/i18nconf.h` durch `tools/localise.c:648-666`, gesteuert über Makefile-Variable `UNIQUE`/`COUNTRY` (`Makefile:126-137`) | **1 / 1** (kein `UNIQUE`-Länder-Build gewählt) | Mehrsprachigkeits-Tabellen (Tastaturlayouts, Zeichensätze, Länder-Array) zur Laufzeit vs. fest einkompiliertes Einzelland | **ja** (borderline) — betrifft `bios/country.c`, nicht direkt AES-Opcodes, aber Länder-/Tastatur-abhängige Semantik |

**Nicht existent in EmuTOS** (waren als Beispiele in der Aufgabenstellung genannt,
per Grep über den gesamten Baum verifiziert **nicht vorhanden**): `CONF_UNIQUE_COUNTRY`,
`CONF_MULTITASK`, `CONF_WITH_PREEMPTIVE`. EmuTOS hat kein Preemptive-Multitasking-Compile-Flag
— die AES ist grundsätzlich kooperativ (Yield-basiert über `dsptch()`/`APPL_YIELD`),
das ist keine Compile-Zeit-Option, sondern die einzige existierende Architektur.

### 1l. Target-/Machine-Profile (Build-Selektion, nicht einzelne Feature-Schalter)

EmuTOS hat **keine** `MACHINE_FALCON`/`MACHINE_TT`/`MACHINE_ST`-Compile-Symbole
(per Grep verifiziert: `include/config.h` + alle `.c`/`.h` — nur `MACHINE_AMIGA`,
`MACHINE_ARANYM`, `MACHINE_FIREBEE`, `MACHINE_LISA`, `MACHINE_M548X` existieren).
ST/STE/TT/Falcon sind **ein einziger Build** mit Laufzeit-Hardware-Erkennung
(Cookie-Jar, `_MCH`); es gibt kein Compile-Zeit-"Falcon-Profil".

| Symbol | Gesetzt durch | config.h-Block | Kernaussage ggü. Standard-Profil |
|---|---|---|---|
| `TARGET_192` | `Makefile:533` (`make 192`) | 210-394 | Minimal-ROM für ST: praktisch alle `CONF_WITH_*` der AES-/Desktop-Sektion auf 0 (3D_OBJECTS, WINDOW_COLOURS, MENU_EXTENSION, GRAF_MOUSE_EXTENSION, PCGEM, COLOUR_ICONS, EXTENDED_OBJECTS …) → `AES_VERSION` fällt auf `0x0140` (TOS 1.04-Niveau). **Das ist der Minimal-Build, den wir laut Aufgabenstellung NICHT matchen.** |
| `TARGET_256` | `Makefile:548` (`make 256`) | 402-448 | (Mega)STe-Profil: `3D_OBJECTS`=0, `MENU_EXTENSION`=0, `VDI_16BIT`=0, `MAX_VERTICES`=512; `GRAF_MOUSE_EXTENSION`/`WINDOW_COLOURS` bleiben Default=1 → `AES_VERSION`=`0x0320` (TOS 2.06/3.06-Niveau, passend zu `REF_OS=TOS206`, `Makefile:551`) |
| `TARGET_CART` | `Makefile:626` (`make cart`) | 469-554 | 128K-Diagnose-Cartridge; `WITH_AES=0` (`Makefile:627`) — **AES komplett ausgebaut**, kein Desktop; explizit `CONF_WITH_SHUTDOWN=0` erzwungen (536-538, einziger Ort mit expliziter Override statt Formel) |
| `TARGET_FLOPPY` | `Makefile:835` (`make flop`) | 559-563 | nur `MAX_VERTICES=512` abweichend, sonst volles Feature-Set (RAM-TOS) |
| `TARGET_PRG`/`prg` | `Makefile:796` | — (kein Block) | RAM-TOS, volles Feature-Set; `EMUTOS_LIVES_IN_RAM=1` (config.h:36-40) ändert nur Boot-Interna |
| `TARGET_512`/`512` | `Makefile:561` (`make 512`) | — (**kein Block, fällt auf generische Defaults durch**) | Volles Feature-Set, `REF_OS=TOS404` |
| `TARGET_1024`/`1024` | `Makefile:592` (`make 1024`) | — (**kein Block**) | Volles Feature-Set, `REF_OS=TOS404` |
| `MACHINE_ARANYM` | `Makefile:607` (`make aranym`) | 61-122 | Falcon-artige virtuelle Maschine; `68040_PMMU=1`, mehrere reine HW-Chips aus (ACSI/SCSI/TT_MFP/STE+TT_SHIFTER/MEGARTC/VME/SFP004/68030_PMMU/MONSTER/MAGNUM/NOVA/FORMAT/EASTER_EGG=0); AES-Sektion **nicht** berührt → `AES_VERSION` bleibt `0x0340` wie unser Ziel-Profil. `REF_OS=TOS404`. Am nächsten an unserem Ziel-Profil, aber kein 1:1-Match (andere HW-Chip-Auswahl) |
| `MACHINE_FIREBEE` | `Makefile:719` (`make firebee`) | 127-203 | ColdFire; diverse Chips aus (s.o.), `CONF_WITH_SHUTDOWN` zusätzlich über `defined(MACHINE_FIREBEE)`-Klausel erzwungen=1 **und** `can_shutdown()` gibt für FireBee `TRUE` zurück (`bios/bios.c:869` `#elif defined(MACHINE_FIREBEE) return TRUE`) — hier funktioniert `SHW_SHUTDOWN` also wirklich, anders als im Standard-Profil |
| `MACHINE_AMIGA` | `Makefile:649` (`make amiga`) | 568-592 | `CONF_ATARI_HARDWARE=0` → kompletter `!CONF_ATARI_HARDWARE`-Block (673-806) schaltet alle Atari-Chips aus; irrelevant für uns |
| `MACHINE_LISA` | `Makefile:901` (`make lisaflop`) | 597-624 | wie Amiga, `CONF_ATARI_HARDWARE=0`; irrelevant für uns |
| `MACHINE_M548X` | `Makefile:740/750/761` | 629-660 | ColdFire-Eval-Board, `CONF_ATARI_HARDWARE=0`; irrelevant für uns |

---

## Teil 2 — Ziel-Profil NewTOS

> **ENTSCHEIDUNG BESTÄTIGT (User, 2026-07-22):** Ziel-Profil ist der
> **`make 512`-Voll-Build als Atari Falcon030 / TOS4** (AES_VERSION 0x0340,
> 3D-Look, voller `CONF_WITH_*`-Umfang). „Wir nehmen alles, was der Falcon
> braucht." **Einzige Ausnahme: die Hardware-Ebene (BIOS/XBIOS)** — dort wird
> die *API-Oberfläche* (Trap-Nr./ABI/Fehlercodes) 1:1 nachgebildet, aber das
> *Backend* ist der RPi4/AArch64-HAL statt Falcon-Chips (Shifter/Videl/DSP/
> YM2149/MFP/…), konsistent mit den Projekt-Non-Goals + Memory
> `altitude-syscall-not-hardware-emulation`. → Für die AES/VDI/GEMDOS-Audits
> gilt damit: **jede AES-Divergenz/Absenz ist ein echter 1:1-Gap**; legitim
> ist nur ein laufzeit-HW-gegateter No-Op (Shutdown, physischer Rez-Wechsel).
> Siehe Memory `emutos-target-profile-falcon-tos4`.

### Festlegung

NewTOS matcht das **Standard-Voll-Build-Profil**, das entsteht, wenn EmuTOS
mit `make 512`, `make 1024` oder `make prg` gebaut wird — d.h. **kein**
`TARGET_192`/`TARGET_256`/`TARGET_CART`, **kein** `MACHINE_*`-Symbol, `WITH_AES=1`
(Makefile-Default), `CONF_ATARI_HARDWARE=1` (config.h:665-667, unkonditionaler
Default). Begründung (aus Projekt-Memory + Verifikation in diesem Katalog):

- Projekt zielt auf **TOS4-Semantik + 3D-Look** (`cflib-apps-expect-tos4-semantics.md`,
  `aes-dialog-3d-look.md`) und **`AES_VERSION 0x0340`**
  (`native-64bit-abi-future.md`-Umfeld, `aes-object-abi-is-tree-abi.md`).
- `AES_VERSION=0x0340` verlangt laut Formel (config.h:1346-1347) alle vier:
  `CONF_WITH_3D_OBJECTS=1`, `CONF_WITH_MENU_EXTENSION=1`,
  `CONF_WITH_WINDOW_COLOURS=1`, `CONF_WITH_GRAF_MOUSE_EXTENSION=1` — das sind
  exakt die vier **unkonditionalen Defaults** im Standard-Profil (Teil 1b).
  `TARGET_192`/`TARGET_256` schalten mindestens eines davon ab und fallen
  auf `0x0140`/`0x0320` — das sind **nicht** unsere Ziele.
- Weder `TARGET_512` noch `TARGET_1024` noch `TARGET_PRG` definieren einen
  Sonderblock in config.h — sie sind bit-identisch zum reinen Fallthrough-Default,
  was die einfachste, eindeutigste Referenz ist (kein Chip-Subset wie bei
  `MACHINE_ARANYM`/`MACHINE_FIREBEE`).

**Kanonische on/off-Liste** (nur API-relevante bzw. AES-Audit-relevante Schalter,
Rest siehe Teil 1 — dort bereits vollständig mit Default belegt):

| Schalter | Zielwert | Konsequenz für Audits |
|---|---|---|
| `CONF_WITH_3D_OBJECTS` | 1 | `OBJC_SYSVAR` MUSS real sein — Absenz = echter Bug |
| `CONF_WITH_MENU_EXTENSION` | 1 | `MENU_POPUP/ATTACH/ISTART/SETTINGS` MÜSSEN real sein — Absenz = echter Bug |
| `CONF_WITH_WINDOW_COLOURS` | 1 | `WF_COLOR`/`WF_DCOLOR` in `wind_get`/`wind_set` MÜSSEN real sein — Absenz = echter Bug |
| `CONF_WITH_GRAF_MOUSE_EXTENSION` | 1 | `graf_mouse` M_SAVE/M_RESTORE/M_PREVIOUS MÜSSEN real sein — Absenz = echter Bug |
| `CONF_WITH_PCGEM` | 1 | `APPL_YIELD`, `SCRP_CLEAR`, `SHEL_RDEF`/`WDEF` MÜSSEN real sein — Absenz = echter Bug |
| `CONF_WITH_COLOUR_ICONS` | 1 | Farbicon-Handling MUSS real sein |
| `CONF_WITH_EXTENDED_OBJECTS` | 1 | MagiC-Objekterweiterungen (u.a. `G_STRING`-Unterstreichung) MÜSSEN real sein |
| `CONF_WITH_ALT_DESKTOP_GRAPHICS` | 1 | Desktop-Dialogtitel linksbündig+Linie statt zentriert — **Desktop-Verhalten**, kein AES-Opcode |
| `AES_VERSION` | 0x0340 | `WF_OWNER`(20) real, `NUM_WIN`=16 |
| `CONF_WITH_VIDEL` + `CONF_WITH_VDI_16BIT` | 1 / 1 | VDI-16-Bit-Truecolor-Modi MÜSSEN real sein |
| `CONF_WITH_GDOS` | 1 | `vst_load_fonts`/`vst_unload_fonts`-Stubs MÜSSEN vorhanden sein |
| `MAX_VERTICES` | 1024 | `v_fillarea`/`v_pline`/`v_pmarker`-Limit ist 1024, **nicht** 512 (TOS2-Wert) |
| `NUM_VDI_HANDLES` | 128 | max. gleichzeitig offene VDI-Workstations |
| `CONF_WITH_NVRAM` | 1 | XBIOS `Nvmaccess`(0x2E) MUSS real sein |
| `CONF_WITH_SHUTDOWN` | **1** (kompiliert), aber Laufzeit-No-Op (s.u.) | `SHW_SHUTDOWN` ist im Quellcode kein Dead-Branch; NewTOS' honest-No-Op ist trotzdem korrekt, aber aus anderem Grund als bisher dokumentiert |
| `WITH_AES` | 1 (Makefile-Ebene) | Grundvoraussetzung, dass AES überhaupt existiert |

### Korrektur `CONF_WITH_SHUTDOWN`

Bestehendes Audit `p6-rsrc-fsel-scrp-shel.md:13,215` behauptet
`CONF_WITH_SHUTDOWN=0` und SHW_SHUTDOWN sei "wegkompiliert". **Das ist ein
Lesefehler der config.h-Bedingung** (analog zum `scasb`-Präzedenzfall aus
`docs/MEMORY.md`):

- config.h:2028-2034: `CONF_WITH_SHUTDOWN` = 1, wenn
  `DETECT_NATIVE_FEATURES || MACHINE_FIREBEE || MACHINE_AMIGA || MACHINE_LISA`.
- `DETECT_NATIVE_FEATURES` ist **nicht** an `CONF_ATARI_HARDWARE` gekoppelt —
  es ist unabhängig davon default `1` für jede Nicht-ColdFire-Zielplattform
  (config.h:1908-1914), **auch für reine Atari-ROM-Builds** (512/1024/prg).
- Also: `CONF_WITH_SHUTDOWN=1` im Standard-Profil → `aes/gemshlib.c:177-184`
  (`case SHW_SHUTDOWN: if (can_shutdown()) {...}`) **ist real kompiliert**,
  kein `#if 0`-Dead-Code.
- Der tatsächliche No-Op-Effekt kommt zur **Laufzeit**, nicht zur Compile-Zeit:
  `can_shutdown()` (`bios/bios.c:867-872`) prüft `has_nf_shutdown()` (NatFeat-Sonde
  — auf echter, nicht emulierter Hardware liefert das per Definition `FALSE`,
  da real existierende Atari-Hardware kein NatFeat-Interface hat) und fällt für
  einen reinen `CONF_ATARI_HARDWARE`-Build (kein `MACHINE_FIREBEE`/`AMIGA`/`LISA`)
  auf `return FALSE` (`bios.c:872`, `#else`-Zweig). `sh_write()` setzt bei
  `can_shutdown()==FALSE` weder `sh_doexec` noch `sh_nextapp` — der Shutdown-Wunsch
  verpufft, `sh_write()` selbst gibt trotzdem `TRUE` zurück (Zeile 191, unbedingt).
- **Praktische Konsequenz für NewTOS unverändert:** `SHW_SHUTDOWN` als
  ehrliches No-Op ist weiterhin die korrekte 1:1-Nachbildung — NewTOS hat kein
  m68k-NatFeat-Äquivalent auf AArch64-Bare-Metal, der Laufzeitpfad landet
  analog im "nichts tun"-Zweig. Nur die **Begründung** in `p6-rsrc-fsel-scrp-shel.md`
  muss korrigiert werden: nicht "wegkompiliert", sondern "kompiliert, aber
  `can_shutdown()`-Äquivalent liefert auf Bare-Metal-Hardware ohne
  Emulator-Hypervisor-Schnittstelle `FALSE`".
- Gegenbeispiel zur Bestätigung: Für `MACHINE_FIREBEE` liefert `can_shutdown()`
  unbedingt `TRUE` (`bios.c:869`) — dort führt `SHW_SHUTDOWN` tatsächlich zu
  einem echten Shutdown-Versuch (`firebee_shutdown()`, `bios.c:873`/`bios.c:847`).
  Das zeigt, dass der Mechanismus laufzeit-, nicht compile-zeit-bedingt ist.

### Entscheidung (User, 2026-07-22): `GEMDOS_VERSION` → echter Falcon030/TOS4-Wert

`GEMDOS_VERSION` ist im EmuTOS-Standard-Profil `0x2000` (config.h:1589-1591);
einzige Verwendung ist `Sversion()`'s Rückgabewert (`bdos/bdosmain.c:300`),
keine Verhaltensverzweigung hängt daran. EmuTOS' eigener "TOS4-Look"-Build
meldet über `Sversion()` also trotzdem GEMDOS v2.00.

**Beschluss:** NewTOS soll konsistent **Falcon030/TOS4** kommunizieren →
`Sversion()` gibt den **echten Falcon-TOS4-GEMDOS-Wert** zurück, nicht EmuTOS'
`0x2000`. Das ist eine **bewusste, dokumentierte Abweichung vom EmuTOS-Default**
(sonst matchen wir 1:1) und gehört als eigener `D-xxx`-Eintrag ins
`docs/api/_discrepancies.md`.

**Zu verifizieren, NICHT raten (Projekt-Regel):** der exakte Zahlenwert. Ein
echter Atari Falcon030 mit TOS 4.0x meldet GEMDOS **0.30** → `Sversion()`
byte-getauscht **`0x3000`** (plausibel, aber vor dem Setzen gegen
Compendium/Profibuch + ggf. reales TOS4 gegenprüfen). Betrifft ausschließlich
den `Sversion()`-Rückgabewert, keine Struktur/kein Verhalten. Analog ist zu
prüfen, ob auch der **`TOS_VERSION`**-OS-Header-Wert (Standard `0x206`,
config.h:1367) auf den Falcon-TOS4-Wert (`0x0404`?) gehoben werden soll — gleiche
„TOS4-konsistent"-Logik, ebenfalls verifizieren.

### Was `TARGET_192`/`TARGET_256`-Absenzen legitimiert vs. was ein echter Bug ist

Da unser Ziel-Profil **keinem** dieser reduzierten Targets entspricht, legitimiert
**keiner** der dortigen Feature-Abschaltungen eine Absenz bei uns. Die
`coverage-matrix.md`-Schlussfolgerung ("Keine der gegateten Gruppen ist bei uns
legitim MISSING") ist durch diesen Katalog bestätigt: alle vier
AES-Audit-relevanten Gates (`PCGEM`, `MENU_EXTENSION`, `3D_OBJECTS`,
`WINDOW_COLOURS`, `GRAF_MOUSE_EXTENSION`) sind im Standard-Profil `1`.
Zusätzlich neu identifiziert und noch nicht in P1-P6 geprüft:
`CONF_WITH_EXTENDED_OBJECTS=1` (MagiC-Objekterweiterungen) und
`CONF_WITH_COLOUR_ICONS=1` — beide Default `1`, beide sollten also ebenfalls
real vorhanden sein; keiner der beiden ist Gegenstand einer eigenen
Tiefenprüfung in P1-P6 (Lücke, siehe Report an den Lead).

---

## Teil 3 — NewTOS-Gating-Ist-Zustand

**Ergebnis: NewTOS hat kein EmuTOS-config.h-Äquivalent für die AES-/VDI-/GEMDOS-API-Oberfläche.**
Verifiziert per Grep über `kernel/`, `tos-api/`, `tos-types/`:

- Es existiert **keine** Datei `config.h`/`config.rs` o.ä. in `kernel/src/`,
  `tos-api/src/`, `tos-types/src/` (Suche nach `config*`/`*conf*.rs` liefert
  nichts außer den drei `Cargo.toml`-Dateien).
- `[features]`-Sektionen existieren nur in zwei Cargo.toml:
  - `tos-types/Cargo.toml`: `default = []`, `std = []` — reine Host-Test-Ermöglichung,
    kein TOS-Feature-Gate.
  - `kernel/Cargo.toml`: `board-rpi4` / `board-qemu-virt` — **Hardware-Board-Auswahl**
    (Track B, gegenseitig exklusiv erzwungen via `compile_error!` in
    `kernel/src/lib.rs:19-26`), betrifft SD-Karte/Block-Device-Pfade
    (`kernel/src/lib.rs:271-272`), nicht AES/VDI/GEMDOS-Funktionsumfang.
  - `tos-api/Cargo.toml` hat **gar keine** `[features]`-Sektion.
- Alle in `kernel/src/desk/*.rs` und `tos-api/src/aes/*.rs` gefundenen
  Vorkommen von `CONF_WITH_*`/`3D_OBJECTS`/`PCGEM`/`MENU_EXTENSION`/
  `WINDOW_COLOURS`/`GRAF_MOUSE`/`COLOUR_ICONS`/`SHUTDOWN` sind **ausschließlich
  Doc-Kommentare, die auf die EmuTOS-Quelle verweisen** (Zitate wie
  `` `CONF_WITH_SIZE_TO_FIT` `` in `desk/win.rs:245`), keine `#[cfg(...)]`
  oder `const`-Schalter mit tatsächlicher Verzweigungswirkung im NewTOS-Code.
  Verifiziert per gezielter Grep-Kontrollprüfung an allen neun Fundstellen.

**Folgerung:** Der AES-/VDI-/GEMDOS-Featureumfang ist bei NewTOS **implizit
hartverdrahtet** auf exakt das Standard-Voll-Build-Profil aus Teil 2 — es gibt
keinen Mechanismus, ihn zur Compile- oder Laufzeit zu reduzieren (kein
Äquivalent zu `TARGET_192`). Das ist konsistent mit dem Projektziel
("Falcon/TT-klassiges Voll-Profil"), bedeutet aber auch: jede der in Teil 1b
gelisteten AES-Audit-relevanten Funktionsgruppen, die NewTOS NICHT dispatcht,
ist per Konstruktion ein **echter Gap**, niemals eine legitime, durch einen
Compile-Schalter erklärbare Absenz — es gibt bei uns keinen "Schalter", der
umgelegt sein könnte. Die "config.h:NNNN"-Zitate in den P1-P6-Audits beziehen
sich, wie in der Aufgabenstellung vermutet, ausschließlich auf die
EmuTOS-`config.h`, nicht auf eine NewTOS-Datei — bestätigt.

---

## Zitierte Quellen (Zusammenfassung)

- `references/emutos/include/config.h` (vollständig gelesen, 2256 Zeilen)
- `references/emutos/Makefile` (Zeilen 40-147, 267, 334, 439-935 für
  Target-/`WITH_AES`/`WITH_CLI`/`UNIQUE`-Logik)
- `references/emutos/tools/localise.c:600-666` (generiert `include/i18nconf.h`)
- `references/emutos/include/sysconf.h:24-34` (`NUM_WIN` via `AES_VERSION`)
- `references/emutos/bdos/bdosmain.c:300` (`GEMDOS_VERSION`-Verwendung)
- `references/emutos/bios/startup.S:81` (`TOS_VERSION`-Verwendung)
- `references/emutos/bios/floppy.c:1030-1060` (`CONF_WITH_BIOS_EXTENSIONS`)
- `references/emutos/bios/xbios.c:1004,1275,1349-1353,1431` (`CONF_WITH_DSP`,
  `CONF_WITH_NVRAM`)
- `references/emutos/aes/geminit.c:420-434` (`CONF_WITH_BLITTER`,
  `CONF_WITH_CACHE_CONTROL`)
- `references/emutos/aes/gemshlib.c:150-192` (`CONF_WITH_SHUTDOWN`,
  `SHW_SHUTDOWN`)
- `references/emutos/bios/bios.c:735-772,855-876` (`shutdown()`,
  `can_shutdown()`)
- `references/emutos/include/biosext.h:60-75` (`can_shutdown()`-Deklaration)
- `references/emutos/aes/gemoblib.c:847-852` (`CONF_WITH_EXTENDED_OBJECTS`)
- `references/emutos/aes/gemwmlib.c:1566-1590` (`WF_COLOR`/`WF_DCOLOR`/`WF_OWNER`)
- `docs/analysis/aes-audit/{coverage-matrix,p1-appl-pd-dispatch,p5-window-graf,p6-rsrc-fsel-scrp-shel}.md`
  (Cross-Referenz + eine identifizierte Korrektur)
- `kernel/Cargo.toml`, `tos-api/Cargo.toml`, `tos-types/Cargo.toml`,
  `kernel/src/lib.rs:19-26` (NewTOS-Gating-Ist-Zustand)
