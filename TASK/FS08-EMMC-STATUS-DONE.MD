# FS08-EMMC — Quittung (FERTIG, QEMU): SD-Treiber mit Mehrblock-Befehlen und 4-Bit-Bus

Datum 2026-10-07 · Basis `desk-rebuild` `8a53e48` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-emmc-mb](../measurements/measurement-2026-10-07-emmc-mb/RUNINFO.TXT)
· Ticket: [FS08](FS08.md), Teil 1 (Treiber). Teil 2 „FAT wie EmuTOS“ folgt getrennt.

## Auftrag

Entscheidung des Anwenders 2026-10-07: Mehrblock-Befehle + 4-Bit-Bus im
EMMC2-Treiber, Rückfall per Boot-Argument `emmc=compat`.

## Was geändert ist

* `kernel/src/drivers/bcm2711/emmc.rs`: `read_blocks`/`write_blocks` geben
  einen Auftrag in Stücken bis 65 535 Sektoren als **je ein** Befehl aus —
  `CMD18`/`CMD25` mit Block Count und Auto CMD12 (SD-HC 3.00 §1.11, §2.2.3,
  §2.2.5, §3.7.2.1), ein Sektor weiter `CMD17`/`CMD24`. PIO je Block mit
  Buffer Read/Write Ready; Transfer Complete deckt das Auto CMD12 ab
  (§3.7.1.2 Anm. 2); dessen R1b wird in `RESP3` geprüft (§2.2.7 Tab. 2-12).
  Spuriöses `OUT_OF_RANGE` nach einem `CMD18` bis zum letzten Block wird nach
  SD Phys 9.10 §4.3.3 / SD-HC 3.00 §3.7.1.2 Anm. 3 ignoriert (Auto-CMD12-
  Antwort und einmal im nächsten Befehl, nur wenn dieser innerhalb der Karte
  bleibt). Auto CMD Error (Bit 24) wird benannt gezählt, das Auto CMD Error
  Status Register (§2.2.23) gesichert, Wiederherstellung nach §3.10.2.
* 4-Bit-Bus beim Init: `ACMD51 SEND_SCR`, `SD_BUS_WIDTHS` (SD Phys 9.10 §5.6),
  dann Ablauf SD-HC 3.00 §3.4 (Card-Interrupt-Status aus, `ACMD6` Arg `'10'`
  mit R1-Prüfung, Host Control 1 Bit 1, Card-Interrupt-Status an). Jeder
  Fehlschlag lässt Karte und Host auf 1 Bit; die Bootzeile
  `[emmc] FS08 mode: …` nennt Modus, Busbreite und Grund.
* `kernel/src/drivers/bcm2711/emmc_xfer.rs` (neu, rein): Bits, Stückelung,
  SCR-Auswertung, OUT_OF_RANGE-Regel — 8 Hosttests über `tools/xtask`.
* `kernel/src/bootargs.rs` `EMMC_COMPAT` + Test; `kernel/src/lib.rs` setzt
  den Modus vor `emmc::init`. `emmc=compat` = 1 Bit, ein `CMD17`/`CMD24` je
  Sektor (Treiber wie vorher).
* `kernel/src/selftest_emmc.rs` (nur `selftest-emmc`): Mehrblock-Test über die
  letzten 64 Sektoren, zerstörungsfrei.
* `docs/hardware/emmc-sd.md` §1, §4a, §4b, §5, §7.6.
* BIOS `Rwabs` reichte die Sektorzahl schon in **einem** Aufruf durch
  (`tos-api/src/bios/mod.rs:139-221`) — dort nichts zu ändern. FAT-/GEMDOS-
  Puffer unberührt (Folgeaufgabe).

## Messwerte (QEMU raspi4b, memprot=on)

| Messung | Soll | Ist |
|---|---|---|
| Bootzeile, Vorgabe | 4 Bit, Mehrblock | `bus 4-bit (ACMD6 ok, SCR SD_BUS_WIDTHS=0x5)`, `CONTROL0=0x00000f02` |
| Bootzeile, `emmc=compat` | 1 Bit, CMD17/24 | `emmc=compat … bus 1-bit`, `CONTROL0=0x00000f00` |
| Selbsttest 64 Sektoren bis Kartenende, beide Modi | gleich, OK | multi = 64× single, Schreiben/Rücklesen/Wiederherstellen OK, +0 Fehler |
| GEMDOS-Schreibtests im Selbsttest, beide Modi | gleich | gleiche Ausgabe, **Karten danach bytegleich**, fsck sauber |
| FSBENCH Vorgabe / `emmc=compat` | 100/100 crc ok | **100/100 / 100/100**, D:\FSBENCH unverändert, fsck C: D: sauber |
| QEMU-Trace FSBENCH: `CMD18` | = L1-Aufrufe mit 128 Sektoren (358) | **358**, Auto CMD12 358 |
| APITEST beide Modi | 106/0/5 | **106/0/5 / 106/0/5** |
| Hosttests | 1176/0/1 + neu | **1185/0/1** (+9) |
| Release-Bau rpi4 / qemu-virt | 64/94 | **64/94** |
| clippy rpi4 / qemu-virt | 76/125 | **76/125**, keine Meldung in geänderten Dateien |
| `desk-drift-probe.sh` | 31 | **31** |
| SDK-Hosttests | 16/0/0 | **16/0/0** |

QEMU-Zeiten sind nicht aussagekräftig (keine Kartenlatenz modelliert).

## Pi-Paket (nicht kopiert)

`measurements/measurement-2026-10-07-emmc-mb/bin/kernel8.img`, sha256
`dbe7835aed9f7a5b93e952e803bc9b7228581d51d671b49ee3f1d8285dd9817c`, mit
`bin/LIESMICH.TXT` (Erwartung, Rückfall `emmc=compat` in `cmdline.txt`,
`C:\AUTO\FSBENCH.NC` → `.NCP` für den Vergleichslauf, danach wieder parken).

## Offen

* Echter Pi 4 nicht gefahren (Abnahmegang des Anwenders).
* GUI-Regression (EmuDESK, TeraDesk, QED öffnen/sichern, zView, S2ALL) — Tester.
  GEMDOS schreibt weiter einzelne Sektoren (`CMD24`); `CMD25` erreicht nur
  BIOS-`Rwabs` mit mehr als einem Sektor.
* circle meldet, dass `SEND_SCR` auf der Pi 4 mit DATA_TIMEOUT scheitern kann
  → dann bleibt 1 Bit (Bootzeile), Mehrblock bleibt aktiv.
* Die Auto-CMD12-Antwort wird nach SD-HC Tab. 2-12 aus `RESP3` gelesen; circle
  liest sie nicht. Legte die EMMC2 sie nicht dort ab, schlügen Mehrsektor-
  Zugriffe mit EREADF/EWRITF fehl → `emmc=compat`.
* High Speed (50 MHz, Option D) nicht gemacht; echter SD-Takt (M1) offen.
* `docs/STATUS.md` nicht angefasst (Lead).
