# BIOS + XBIOS — Machbarkeits-Triage (Implement / HW-Absenz / Weglassen)

> Stand: 2026-07-23, Branch `desk-rebuild`. Scope **nur BIOS (Trap #13) +
> XBIOS (Trap #14)** — die höheren Ebenen (GEMDOS/AES/VDI/Util) sitzen auf
> BIOS/XBIOS auf, sind HW-unabhängig und werden 1:1 umgesetzt, daher nicht
> triagiert (User-Entscheidung 2026-07-23).
> Quellen (nicht aus Gedächtnis): `emutos-full-function-inventory.md` §3
> (XBIOS-Opcode-Tabelle, quellenverifiziert), `bios-findings.md` +
> `00-DASHBOARD.md` (BIOS-Audit), `CLAUDE.md` (Projekt-Non-Goals).

## Zweck

Der User will bewusst entscheiden, welche Routinen wir (1) **echt umsetzen**,
(2) als **ehrlichen HW-Absenz-Kontraktwert** beantworten (kein Gerät vorhanden,
dokumentierter „no device"-Rückgabewert), oder (3) **ganz weglassen** (fällt auf
den generischen Catch-all, wie die Floppy-Routinen). Ziel: keine versteckten
Stubs; jede Nicht-Implementierung ist eine dokumentierte, bewusste Kategorie.

## Kategorien (und warum HW-Absenz KEIN Stub ist)

| Kat. | Bedeutung | Rückgabe-Kontrakt | Rule-#2-Konformität |
|---|---|---|---|
| **I — Implement** | echte Umsetzung, HW an RPi4/BCM2711-HAL adaptiert | reale Semantik | echte Implementierung |
| **A — HW-Absenz** | Gerät existiert auf RPi4 nicht; **dokumentierter** „kein Gerät"-Wert | `EUNDEV`/`0`/no-op/Dummy — **das, was EmuTOS auf Nicht-Atari-HW auch liefert** | **kein Stub**: kein hardcodierter Fake, sondern der Kontrakt für „Gerät fehlt" |
| **X — Weglassen** | Opcode nie sinnvoll aufgerufen / in EmuTOS selbst `xbios_unimpl` / per `CONF_WITH_*` fürs Zielprofil aus | Catch-all: Funktionsnummer zurück (EmuTOS `xbios_do_unimpl`) | **kein Stub**: EmuTOS selbst tut hier nichts anderes |

**Wichtige Abgrenzung A vs. X:** Floppy/ACSI (`Floprd` etc.) ist **A, nicht X**.
EmuTOS *implementiert* diese Funktionen und liefert ohne Laufwerk `EUNDEV`/
`EDRVNR`. „Weglassen" (X → Funktionsnummer-Echo) würde von EmuTOS-mit-fehlendem-
Laufwerk *abweichen*. Eine App, die Floppy prüft, muss ein sauberes „kein
Laufwerk" sehen, kein Müll-Echo. Deshalb: Floppy = **A (EUNDEV)**, bereits
korrekt umgesetzt.

---

## BIOS (Trap #13, 12 Funktionen) — alle Kategorie I

BIOS ist die HW-Abstraktion selbst → **jede Funktion wird implementiert**, das
Backend ist auf RPi4 adaptiert (EMMC-SD statt WD1772/ACSI, USB-HID/UART statt
IKBD-ACIA, Framebuffer statt Shifter). Keine Weglassungen. Der Audit-Katalog
`bios-findings.md` deckt die *Bugs* (B1/H1/M1–M4), nicht die Kategorisierung.

| Op | Name | Kat. | Backend-Adaption | Status |
|---|---|---|---|---|
| 0 | `Getmpb` | I | rein logisch (MD/MPB, ST-RAM-Pool) | HAS |
| 1 | `Bconstat` | I | UART0/USB-HID-Konsole | HAS |
| 2 | `Bconin` | I | UART0/USB-HID | HAS (L2 conterm-Edge offen) |
| 3 | `Bconout` | I | UART0 + Framebuffer-VT52 | HAS |
| 4 | `Rwabs` | I | EMMC-SD (statt WD1772/ACSI) | HAS (M1 etv_critic + L1 phys-mode offen) |
| 5 | `Setexc` | I | Vektor-Tabelle | HAS (B1a: NVEC=0x120 sperrt `$5A0`-Vektor; H1 Dispatch) |
| 6 | `Tickcal` | I | rein logisch (200 Hz) | HAS |
| 7 | `Getbpb` | I | FAT16-BPB aus SD | HAS (M2–M4 FS-Härtung offen) |
| 8 | `Bcostat` | I | UART0-TX-Ready | HAS |
| 9 | `Mediach` | I | SD-Media-Change | HAS |
| 10 | `Drvmap` | I | SD-Partitions-Map (C–F) | HAS |
| 11 | `Kbshift` | I | Keyboard-Shift-State | HAS |
| — | Boot-Init/OSHEADER/Sysvars | I | **B1 BLOCKER — Kernel-Boot-Befüllung fehlt** | OFFEN (Machbarkeits-Entscheidung) |

**BIOS-Ergebnis:** 12/12 Implement. Keine Triage-Entscheidungen offen; die
offenen Punkte sind *Bugs* (Audit), keine Kategorie-Fragen.

---

## XBIOS (Trap #14) — die eigentliche Triage

103 real implementierte EmuTOS-Zielfunktionen. Gruppiert nach HW-Domäne.
Legende Status: **HAS** = NewTOS hat ein Gegenstück; **—** = NO-COUNTERPART
(fällt aktuell auf den Catch-all).

### A) Screen/Video — ST/TT-Basis → Kategorie I (RPi4-Framebuffer)

| Op | Name | Kat. | Begründung | Status |
|---|---|---|---|---|
| 0x02 | `Physbase` | I | Framebuffer-Basis | HAS |
| 0x03 | `Logbase` | I | Framebuffer-Basis | HAS |
| 0x04 | `Getrez` | I | ST-Rez-Code des festen Modus | HAS |
| 0x05 | `Setscreen` | I | Framebuffer-Adressen | HAS |
| 0x06 | `Setpalette` | I | Palette (16-Farb-Kompat-Schicht) | HAS |
| 0x07 | `Setcolor` | I | einzelne Palettenfarbe | HAS |
| 0x15 | `Cursconf` | I | Textcursor-Blinkrate | HAS |
| 0x25 | `Vsync` | I | Vblank-Warten (RPi4-Vsync) | HAS |
| 0x40 | `Blitmode` | A | kein BLiTTER → Kontrakt `0x0000` | HAS |

### B) Falcon-VIDEL (0x58–0x5F, 7 Fn) → **ENTSCHEIDUNG (Empfehlung I, API-Ebene)**

Zielprofil ist Falcon030/TOS4 → VIDEL ist das *echte* Video-Subsystem, **kein**
Non-Goal (der Shifter ist Non-Goal, VIDEL nicht). Memory
`vdi-truecolor-workstation` plant Truecolor-VDI ohnehin. Unser Screen ist ein
fester RPi4-Truecolor-Framebuffer → wir implementieren die **beobachtbare
API-Ebene** (Modus-Queries beantworten), nicht die VIDEL-Register.

| Op | Name | Empf. | Umsetzung auf festem Framebuffer |
|---|---|---|---|
| 0x58 | `Vsetmode` | I | Modus annehmen/validieren; ein fester Modus |
| 0x59 | `Vmontype` | I | Kontrakt-Monitortyp (VGA/RGB) melden |
| 0x5A | `Vsetsync` | A | keine Sync-Umschaltung → no-op |
| 0x5B | `Vgetsize` | I | Bildspeichergröße des festen Modus rechnen |
| 0x5D | `Vsetrgb` | I | **Truecolor-Palette — koppelt an VDI-Truecolor-Arbeit** |
| 0x5E | `Vgetrgb` | I | Truecolor-Palette lesen |
| 0x5F | `Vfixmode` | A | Modus-Freeze bedeutungslos → no-op/echo |

### C) TT-Shifter (0x50–0x57, 8 Fn) → **Kategorie X (Weglassen)**

`CONF_WITH_TT_SHIFTER` — TT-spezifisch, für Falcon **aus**. EmuTOS selbst
enthält diese Slots im Falcon-Profil nicht → nichts zu portieren. **Offener
Punkt #2** (Inventar): der tatsächliche `make 512`-Zustand von
`CONF_WITH_TT_SHIFTER` ist noch gegen den Build zu bestätigen; Falcon030 = aus.

`Esetshift` `Egetshift` `Esetbank` `Esetcolor` `Esetpalette` `Egetpalette`
`Esetgray` `Esetsmear` → alle **X**.

### D) Floppy / ACSI-DMA → **Kategorie A (EUNDEV) — bereits korrekt**

Der vom User genannte „ganz weglassen"-Fall — Empfehlung aber **A, nicht X**
(s. Abgrenzung oben: EmuTOS liefert ohne Laufwerk EUNDEV, kein Echo).

| Op | Name | Kat. | Status |
|---|---|---|---|
| 0x08 | `Floprd` | A → `EUNDEV` | HAS |
| 0x09 | `Flopwr` | A → `EUNDEV` | HAS |
| 0x0A | `Flopfmt` | A → `EUNDEV` | HAS |
| 0x13 | `Flopver` | A → `EUNDEV` | HAS |
| 0x29 | `Floprate` | A → `EUNDEV` | HAS |
| 0x2A | `DMAread` | A → `EUNDEV` | HAS |
| 0x2B | `DMAwrite` | A → `EUNDEV` | HAS |
| 0x12 | `Protobt` | I | reine Software (BPB→Bootsektor), HAS |

### E) MFP-Timer/Interrupt (68901) → **Kategorie X — CLAUDE.md-Non-Goal**

`CONF_WITH_MFP`; RPi4 nutzt GIC-400, kein MFP; m68k-Interrupt-Vektor-Konzept ohne
AArch64-Äquivalent. CLAUDE.md: „no … MFP … in the kernel core".

| Op | Name | Kat. |
|---|---|---|
| 0x0D | `Mfpint` | X |
| 0x1A | `Jdisint` | X |
| 0x1B | `Jenabint` | X |
| 0x1F | `Xbtimer` | X |

### F) Sound — PSG / DMA-Sound / DSP → **ENTSCHEIDUNG**

**F1 — YM2149 PSG (0x1C–0x20):** CLAUDE.md-Non-Goal (kein YM2149 im Kernel).
Aktuell **A** (Kontrakt 0 / no-op). RPi4 *hat* Audio-HW → echte Umsetzung ist
**Zukunft** (eigene Audio-Phase), jetzt **A** belassen.

| Op | Name | Kat. jetzt | Status |
|---|---|---|---|
| 0x1C | `Giaccess` | A → 0 | HAS |
| 0x1D | `Offgibit` | A → no-op | HAS |
| 0x1E | `Ongibit` | A → no-op | HAS |
| 0x20 | `Dosound` | A → no-op | HAS |

**F2 — Falcon DMA-Sound (0x80–0x8D, 14 Fn):** `CONF_WITH_DMASOUND`, alle
NO-COUNTERPART. **Empfehlung A** für die Lock/Status/Modus-Queries (App-Probes
kriegen definierte Antwort: `Locksnd`→ok, `Sndstatus`→kein Fehler,
`Devconnect`/`Setmode`→no-op), **echter Audio-Backend = Zukunft**. Alternativ
komplett **X** bis zur Audio-Phase. 14 Fn: `Locksnd` `Unlocksnd` `Soundcmd`
`Setbuffer` `Setsndmode` `Settracks` `Setmontracks` `Setinterrupt` `Buffoper`
`Dsptristate` `Gpio` `Devconnect` `Sndstatus` `Buffptr`.

**F3 — Falcon DSP56001 (0x60–0x7F, 32 Fn):** `CONF_WITH_DSP`, alle
NO-COUNTERPART. RPi4 hat **keinen** DSP56001 und **kein** Äquivalent; sehr wenige
geportete GEM-Apps nutzen ihn. **Empfehlung:** minimal **A** für die Präsenz-
Probes (`Dsp_available`→0, `Dsp_lock`→Fehler = „kein DSP"), die 30
Verarbeitungs-Ops **X**. So crasht keine App, die den DSP nur *abfragt*, ohne 32
tote Funktionen zu bauen.

### G) Keyboard / Maus / IKBD → **ENTSCHEIDUNG (Empfehlung I, wenn Keyboard-Infra steht)**

NewTOS-Tastatur = USB-HID bzw. Atari-IKBD-über-UART2 (Memory
`keyboard-via-atari-ikbd`). Diese XBIOS-Fn sind aktuell „needs infra"-deferrt.

| Op | Name | Empf. | Begründung |
|---|---|---|---|
| 0x00 | `Initmous` | I (deferred) | Maus-Paket-Handler an Maus-HAL |
| 0x0E | `Iorec` | A | Roh-IOREC-Puffer-Zeiger; ohne ACIA → definierter Dummy |
| 0x10 | `Keytbl` | I (deferred) | Tastatur-Layout-Tabellen (norm/shift/caps) |
| 0x18 | `Bioskeys` | I (deferred) | Tastatur-Vektoren zurücksetzen |
| 0x0C | `Midiws` | A | keine MIDI-ACIA → no-op (RPi4-MIDI = Zukunft) | HAS |
| 0x19 | `Ikbdws` | A | keine ACIA → no-op | HAS |
| 0x22 | `Kbdvbase` | I | KEYTAB-Zeiger | HAS |
| 0x23 | `Kbrate` | I | Wiederholrate | HAS |

### H) Serial / RS-232 → **Kategorie I (PL011) — vorhanden**

| Op | Name | Kat. | Status |
|---|---|---|---|
| 0x0F | `Rsconf` | I | HAS (dünner PL011-Adapter) |
| 0x2C | `Bconmap` | I | HAS (Kontrakt: ein serieller Port) |

### I) Uhr / Zufall / Misc-Software → **Kategorie I — vorhanden**

| Op | Name | Kat. | Status |
|---|---|---|---|
| 0x11 | `Random` | I | HAS (reine SW) |
| 0x16 | `Settime` | I | HAS (teilt GEMDOS-Walluhr) |
| 0x17 | `Gettime` | I | HAS |
| 0x21 | `Setprt` | I | HAS (Desktop-Konfig-Wort, reine SW) |
| 0x14 | `Scrdmp` | A | kein Drucker → no-op | HAS |

### J) Misc-HW mit Entscheidung

| Op | Name | Empf. | Begründung |
|---|---|---|---|
| 0x26 | `Supexec` | **ENTSCHEIDUNG** | m68k-Supervisor-Konzept; AArch64-EL-Modell anders. Manche Apps führen priv. Code via Supexec aus → bräuchte kontrollierten EL1-Trampolin. Empf.: **I (später)**, sicherheitssensibel; jetzt deferred/X |
| 0x2E | `Nvmaccess` | **ENTSCHEIDUNG** | `CONF_WITH_NVRAM`; RPi4 hat keine Batterie-NVRAM. Empf.: **A** (Fehler/all-null) jetzt; optional file-/EEPROM-Backing später |

### K) EmuTOS-eigene `xbios_unimpl`-Slots (39) → **Kategorie X — kein Gap**

0x01 `ssbrk`, 0x0B, 0x24 `prtblk`, 0x27 `puntaes`, 0x28, 0x2D, 0x2F–0x3F (17),
0x41–0x4F (15), 0x5C. EmuTOS selbst liefert hier `xbios_do_unimpl` (Funktions-
nummer zurück) → NewTOS-Catch-all ist bereits identisch. **Kein NewTOS-Gap.**

---

## Zusammenfassung — Zahlen

| Kategorie | BIOS | XBIOS | Bemerkung |
|---|---|---|---|
| **I — Implement** | 12 | ~24 (Screen 8, VIDEL-API ~4, Serial 2, Uhr/Random/Setprt 4, Kbd-HAS 2, Protobt 1, + Kbd-deferred 3) | Screen/Serial/Uhr vorhanden; VIDEL + Kbd = Ausbau |
| **A — HW-Absenz** | — | ~20 (Floppy/ACSI 7, PSG 4, Midiws/Ikbdws/Scrdmp/Iorec 4, Blitmode 1, DMA-Sound-Probes ~) | ehrliche „kein Gerät"-Kontraktwerte, kein Stub |
| **X — Weglassen** | — | ~62 (TT-Shifter 8, MFP 4, DSP-Verarbeitung ~30, unimpl-Slots 39-überlappend) | Catch-all; EmuTOS tut hier selbst nichts |

## Offene Lead-/User-Entscheidungen (die echten Triage-Calls)

1. **VIDEL (0x58–0x5F)** — beobachtbare API auf festem RPi4-Framebuffer
   implementieren (Empf. **ja**, koppelt an geplante Truecolor-VDI-Arbeit)?
2. **TT-Shifter (0x50–0x57)** — Weglassen (Empf. **ja**, Falcon-Profil); vorab
   `CONF_WITH_TT_SHIFTER`-Zustand im echten `make 512` bestätigen.
3. **DSP56001 (0x60–0x7F, 32 Fn)** — minimal A (`Dsp_available`→0) + Rest X
   (Empf. **ja**); volle DSP-Emulation = nein (kein HW-Äquivalent)?
4. **DMA-Sound (0x80–0x8D, 14 Fn)** — A (Probes beantworten) jetzt, echtes
   RPi4-Audio-Backend als Zukunft (Empf. **ja**) — oder komplett X bis dahin?
5. **PSG-Audio (0x1C–0x20)** — jetzt A/no-op belassen, echtes Audio Zukunft
   (Empf. **ja**)?
6. **Keyboard-XBIOS (Initmous/Keytbl/Bioskeys)** — I, sobald Keyboard-Infra
   steht (Empf. **ja**, gehört zur Keyboard-Subsystem-Phase)?
7. **Supexec (0x26)** — I via kontrolliertem EL1-Trampolin (später) oder X?
8. **Nvmaccess (0x2E)** — A jetzt (Empf.), file-/EEPROM-Backing als Option?

## Reihenfolge-Empfehlung

Die Triage ändert die aktuelle Umsetzung kaum — das meiste ist **bereits korrekt
kategorisiert** (Screen/Serial/Uhr = I done, Floppy/PSG = A done, unimpl/MFP/
TT-Shifter = X korrekt). Echte *neue* Arbeit fällt nur bei den Entscheidungen 1
(VIDEL, koppelt an Truecolor-VDI) und 6 (Keyboard-XBIOS, koppelt an Keyboard-
Subsystem) an — beide sind eigene Subsystem-Phasen, nicht Teil des jetzigen
BIOS-Audits. Empfehlung: BIOS-Audit-Fixes (B1/H1/M1–M4) zuerst, VIDEL/Keyboard-
XBIOS mit ihren jeweiligen Subsystem-Phasen.
