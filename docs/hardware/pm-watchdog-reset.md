# Raspberry Pi 4 (BCM2711): Software-Neustart ueber den PM-Watchdog

> Track B nach `docs/SOURCE-ANALYSIS-PROTOCOL.md`. Zu TASK/RST01.
> **Quellenlage vorab, ohne Beschoenigung:** Das BCM2711-Datenblatt im Repo
> **dokumentiert diesen Mechanismus nicht.** Alle Registerangaben unten
> stammen aus tertiaeren Quellen (Circle, QEMU). Sie sind **keine
> Datenblatt-Fakten**.

## 1. Primaerquelle: BCM2711-Datenblatt

`references/datasheets/RP-008248-DS-1-bcm2711-peripherals.pdf`, per
`pdftotext -layout` komplett durchsucht (2026-10-17). Suchbegriffe:
`watchdog`, `wdog`, `PM_`, `RSTC`, `RSTS`, `rstc`, `reboot`, `power-cycle`,
`system reset`, Adresse `0x7e1000xx`/`0x7e100000`. **Kein Treffer**, der den
Power-Manager/Watchdog beschreibt. Die einzigen Treffer fuer `PASSWD`/`5a`
betreffen den Clock-Manager (`bcm.txt` Zeilen 5688, 5757: „Clock Manager
password "5a"“), nicht den PM. Das Inhaltsverzeichnis des Datenblatts fuehrt
keinen Power-Manager-Abschnitt (Kapitel: Aux, BSC, DMA, GPIO, Interrupts, PCM,
PWM, SPI, System Timer, UART, Timer (ARM side), ARM Mailboxes, …). [gelesen]

Ebenfalls ohne Treffer: `references/arm-arm` (CPU-Architektur, kein SoC-PM) und
`references/rpi4-boot` (nur `config-txt-overclocking.adoc` und das
Mailbox-Property-Interface). [Grep ueber beide Verzeichnisse: nur `circle` und
`freemint/sys/syscalls.master` trafen auf `watchdog`/`PM_RSTC`.]

## 2. Tertiaere Quelle: Circle (`references/circle`, `6177984e`)

Circle ist im Repo als tertiaere Pi-Referenz gefuehrt (`references/README.md`;
Protokoll Track B Stufe 6). Es ist bare-metal-Code, der auf echter Hardware
laeuft, aber **kein Datenblatt**.

* Peripherie-Basis fuer `RASPPI == 4`: `ARM_IO_BASE = 0xFE000000`
  (`include/circle/bcm2835.h:29-30`). PM-Block: `ARM_PM_BASE = ARM_IO_BASE +
  0x100000` fuer `RASPPI <= 4` (`bcm2835.h:261-265`), also **0xFE100000**
  (Summe von mir gebildet) [gelesen].
* Register (`bcm2835.h:267-269`): `ARM_PM_RSTC = BASE+0x1C`,
  `ARM_PM_RSTS = BASE+0x20`, `ARM_PM_WDOG = BASE+0x24`.
* Konstanten (`bcm2835.h:274-279`): `ARM_PM_PASSWD = 0x5A << 24`,
  `ARM_PM_RSTC_CLEAR = 0xFFFFFFCF`, `ARM_PM_RSTC_REBOOT = 0x20`,
  `ARM_PM_RSTC_RESET = 0x102`, `ARM_PM_RSTS_PART_CLEAR = 0xFFFFFAAA`,
  `ARM_PM_WDOG_TIME = 0x000FFFFF`.
* Kuerzeste Neustartfolge (`lib/sysinit.cpp:231-241`, `reboot()`): 
  `write32(ARM_PM_WDOG, ARM_PM_PASSWD | 1);`
  `write32(ARM_PM_RSTC, ARM_PM_PASSWD | 0x20);` (`PM_RSTC_WRCFG_FULL_RESET`),
  dann Endlosschleife bis der Reset zuschlaegt.
* Variante mit Partitionswahl (`lib/bcmwatchdog.cpp`, `Restart`): schreibt
  vorher `ARM_PM_RSTS` mit `PASSWD | Partitionsbits` (Partition 63 =
  „halt“-Kennung der Firmware laut Kommentar im Circle-Quelltext, dort in
  `Restart` als „firmware uses the RSTS register to know which partition to
  boot from“), `WDOG = PASSWD | 10` („about 150us timeout“, Kommentar),
  dann `RSTC` wie oben.
* `Start(timeout)` (`bcmwatchdog.cpp`): `WDOG = PASSWD | (s << 16 & 0xFFFFF)`
  — die Zeitbasis ist dort nicht erklaert; die Zeiteinheit ist **nicht
  belegt** (QEMU nennt 65536 Ticks/s, s. u.; keine Hardware-Bestaetigung).

Offen/ungeklaert [kein Beleg]: ob auf der Pi 4 nach dem Watchdog-Reset der
Bootloader/Firmware neu laeuft (Pi4 laedt `kernel8.img` neu) — Circle
bestaetigt das indirekt (Reboot-Funktion), eine Datenblatt-/Firmwaredoku
dazu liegt im Repo nicht vor. Auch **RAM-Inhalt ueber den Reset** (fuer eine
„warm = Kern ohne Firmware neu“-Variante irrelevant, fuer „cold = echter
Neustart“ relevant): nicht belegt; nicht annehmen, dass `memvalid`-artige
Werte ueberleben.

Linux (`drivers/watchdog/bcm2835_wdt.c`) und U-Boot wurden **nicht**
konsultiert (kein Quelltext im Repo); es steht hier nichts aus Gedaechtnis.

## 3. QEMU `raspi4b`

Kein QEMU-Quelltext lokal (`references/` hat keinen QEMU-Baum; installiert ist
`qemu-system-aarch64 11.0.2`, Binary nicht untersucht). Quelle stattdessen
**online, master-Zweig** (nicht die installierte Version 11.0.2 — Abweichung
moeglich), Abfrage 2026-10-17:

* `hw/misc/bcm2835_powermgt.c`: Offsets `R_RSTC 0x1c`, `R_RSTS 0x20`,
  `R_WDOG 0x24`; `PASSWORD 0x5a000000`, Maske `0xff000000`; Schreibzugriffe
  ohne Passwort werden mit `LOG_GUEST_ERROR` protokolliert und verworfen.
  `RSTC` mit `(rstc & 0x30) == 0x20` **und** `wdog != 0` armt einen Timer
  (`wdog` / 65536 Ticks je Sekunde); beim Ablauf: ist `rsts & 0xfff == 0x555`
  → `qemu_system_shutdown_request(GUEST_SHUTDOWN)`, sonst
  `qemu_system_reset_request(GUEST_RESET)`. `WDOG`-Wert 0 loescht den Timer
  (kein sofortiger Reset).
* `hw/arm/bcm2835_peripherals.c`: der Power-Manager wird in
  `raspi_peripherals_base_init` angelegt (`TYPE_BCM2835_POWERMGT`) und in
  `bcm_soc_peripherals_common_realize` bei `PM_OFFSET` eingeblendet.
  `hw/arm/bcm2838_peripherals.c` (Typ der `raspi4b`) leitet von
  `TYPE_BCM_SOC_PERIPHERALS_BASE` ab und legt den PM nicht selbst an.
  [Schluss: raspi4b erbt den PM ueber die Basisklasse; die Abfrage zeigt kein
  Verwerfen. Nicht durch einen Lauf bestaetigt.]
* Folge fuer RST01 [Schluss]: Mit `WDOG != 0` und `RSTC = PASSWD|0x20` loest
  QEMU einen Gast-Reset aus. Ob QEMUs Gast-Reset das Kernel-Image neu laedt
  und ob `-no-reboot` gesetzt ist (dann Beenden statt Neustart), haengt vom
  Aufruf ab; `tools/`-Aufrufe darauf **nicht** geprueft. Empfehlung: kurzer
  Lauf auf QEMU, bevor man sich darauf verlaesst (Test, kein Auftrag hier).

## 4. Zusammenfassung der Quellenstufen

| Behauptung | Stufe |
|---|---|
| PM-Block bei `0xFE100000`, RSTC `+0x1C`, RSTS `+0x20`, WDOG `+0x24` | Circle (tertiaer, Hardware-getestet), QEMU-Offsets stimmen ueberein; **nicht im Datenblatt** |
| Passwort `0x5A` im oberen Byte | Circle, QEMU; nicht im Datenblatt |
| Neustart = `WDOG` ≠ 0 schreiben, dann `RSTC = PASSWD \| 0x20` | Circle `sysinit.cpp:231-241`; QEMU-Modell konsistent |
| Zeiteinheit von `WDOG` | nur QEMU (65536/s); Circle: Kommentar „150 µs“ fuer Wert 10 — beide nicht Datenblatt, untereinander ungeprueft |
| Pi 4 bootet danach die Firmware neu | nur indirekt (Circle `reboot()`); nicht dokumentiert im Repo |
| Datenblatt dokumentiert den PM | **nein** (komplett durchsucht) |
| raspi4b modelliert den PM | online-Quelle master; installierte 11.0.2 ungeprueft; kein Lauf |

## 5. QEMU-Lauf (TASK/RST01, 2026-10-17)

Installiertes `qemu-system-aarch64 11.0.2`, `-M raspi4b`: die Folge aus
`kernel/src/drivers/bcm2711/pm.rs` (`WDOG = PASSWD|1`, `RSTC = PASSWD|0x20`)
loest einen Gast-Reset aus. Mit `-no-reboot` (so rufen `cargo xtask run` und die
Tester-Skripte QEMU auf) **beendet sich QEMU** (rc 0) unmittelbar nach der
Kernmeldung; ohne `-no-reboot` startet der Gast neu und bootet das Kernabbild
erneut bis zum Desktop. Damit ist der raspi4b-PM nachgebildet (Abschnitt 3,
Zeile „raspi4b modelliert den PM“: durch Lauf bestaetigt). Fuer die echte Pi 4
bleibt die Quellenlage unveraendert (Circle, kein Datenblatt).
Beleg: `measurements/measurement-2026-10-17-rst01/RUNINFO.TXT`.
