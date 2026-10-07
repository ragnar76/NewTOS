# FS08-CPUCLK — Quittung (FERTIG, QEMU; Pi-Nachweis offen): ARM-Takt beim Booten auf das Firmware-Maximum

Datum 2026-10-07 · Basis `desk-rebuild` `b473178` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-cpuclk](../measurements/measurement-2026-10-07-cpuclk/RUNINFO.TXT)
· Ticket: [FS08](FS08.md), Teil 3. Teil 1: [FS08-EMMC](FS08-EMMC-STATUS-DONE.MD), Teil 2: [FS08-FAT](FS08-FAT-STATUS-DONE.MD).

## Auftrag

Entscheidung des Anwenders 2026-10-07: die echte Pi 4 meldet 600 MHz ARM-Takt
(`measurements/measurement-2026-10-07-fs08-pi/FSBENCH.LOG` Zeile 3), nominal
1500. Beim Booten den ARM-Takt auf das Maximum der Firmware heben, eine
Bootzeile, Rückfall per Bootargument.

## Quellen (Track B)

* Firmware-Wiki *Mailbox property interface* (Quelle #4, jetzt lokal in
  `references/rpi4-boot/`): „Get max clock rate“ `0x00030004`, „Set clock
  rate“ `0x00038002` (inkl. `skip setting turbo`), „Get clock rate measured“
  `0x00030047`, Clock-ID ARM = 3.
* `references/circle` (Quelle #5): dieselben IDs, `TPropertyTagSetClockRate`,
  Maximum wird mit `bSkipTurbo = FALSE` gesetzt.
* Raspberry-Pi-Doku `config.txt` *Overclocking options* (neu Quelle #6, lokal
  in `references/rpi4-boot/`): warum 600 (force_turbo=0 + kein OS-Regler,
  `arm_freq_min` 600), Spannungsregelung und Überhitzungsschutz bleiben bei
  der Firmware.
* Dokumentiert in `docs/hardware/mailbox-framebuffer.md` §3f.

## Was geändert ist

* `kernel/src/drivers/bcm2711/clock_tags.rs` (neu, rein): Nachrichtenaufbau
  „Set clock rate“, Antwortregel, MHz/°C-Formatierung; 5 Hosttests.
* `kernel/src/drivers/bcm2711/mailbox.rs`: `get_max_clock_rate`,
  `get_clock_rate_measured`, `set_clock_rate`.
* `kernel/src/bootargs.rs`: `cpuclk=firmware` + Hosttest.
* `kernel/src/lib.rs`: `raise_arm_clock` (nur board-rpi4), früh im Boot;
  Bootzeile `[clock] ARM <vorher> -> <nachher> MHz (max <max>, measured <ist>,
  SoC <t> C)` bzw. `[clock] ARM <vorher> MHz kept: <Grund>`.
* `tools/xtask/src/main.rs`: `clock_tags.rs` im Host-Testlauf.

## Prüfungen

| Prüfung | Vorher | Nachher |
|---|---|---|
| Hosttests | 1195/0/1 | **1201/0/1** (+6 neu) |
| Release-Bau rpi4 / qemu-virt (tos-api/kernel) | 64/94 | **64/94** |
| clippy rpi4 / qemu-virt | 76/125 | **76/125** |
| `desk-drift-probe.sh` / SDK-Hosttests | 31 / 16/0/0 | **31 / 16/0/0** |
| QEMU raspi4b Desktop | — | Bootzeile `[clock] ARM 700 -> 700 MHz (max 700, measured ?, SoC 25.0 C)`, Dump pixelgleich zu HEAD |
| QEMU raspi4b `cpuclk=firmware` | — | `[clock] ARM 700 MHz kept: …`, Desktop |
| QEMU raspi4b APITEST (memprot an) | 105/0/6 (HEAD) | **105/0/6**, Dump pixelgleich |
| QEMU virt Desktop | — | serielles Log identisch, Dump pixelgleich |
| fsck aller Läufe | — | sauber |

QEMUs Firmware-Modell meldet fest 700 MHz (aktuell = Maximum), quittiert
„Set clock rate“ mit 700 und kennt „measured“ nicht — der echte Anstieg ist
nur auf der Pi nachweisbar.

## Pi-Paket (nicht kopiert)

`measurements/measurement-2026-10-07-cpuclk/bin/`: `kernel8.img`
(sha256 `ff3540e5…`), `LIESMICH.TXT` (FSBENCH-Kopf „CPU clock“ + spins/tick
als Gegencheck, serielle Bootzeile, Rückfall `cpuclk=firmware`), `SHA256SUMS`.

## Offen

* Echte Pi 4: Nachweis 600 → 1500 MHz (Paket oben), dabei Temperatur unter
  Dauerlast beobachten.
* Kein Laufzeit-Regler: der Takt bleibt nach dem Boot auf Maximum; Drosselung
  nur durch die Firmware (80–85 °C, Unterspannung).
* `Ssystem(S_NEWTOS_CPUCLOCK)` liefert „Get clock rate“ = angeforderter Takt;
  eine Firmware-Drosselung zeigt erst „measured“ — eigener Auftrag, falls
  gewünscht.
* `docs/STATUS.md`-Eintrag beim Commit (Lead).
