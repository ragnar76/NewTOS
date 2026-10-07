# SYSINFO01 — Quittung (FERTIG, QEMU): „System Info"-Dialog statt `form_alert`

Datum 2026-10-07 · Basis `desk-rebuild` `c50a9c9` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-07-sysinfo](../measurements/measurement-2026-10-07-sysinfo/RUNINFO.TXT)
· Ticket: [SYSINFO01](SYSINFO01.md) · Spezifikation: [`docs/api/gemdos/154-ssystem.md`](../docs/api/gemdos/154-ssystem.md) §„System-information modes (2003–2019)"

## Ergebnis

Das Accessory heißt im Desk-Menü ` System Info` und öffnet einen modalen
GEM-Dialog *NewTOS System Information* (612 × 324 px, 3D wie `form_alert`:
`FL3DBAK`+`OUTLINED`-Wurzel, `FL3DACT`+`DEFAULT`-OK; gerahmte Gruppen; je Wert
ein eigenes Objekt in fester Wertspalte — Platz für spätere Bedienelemente).
OK/Return schließt. Alle Werte kommen beim Öffnen frisch vom Kernel und sind
der **wirksame** Zustand; Unbekanntes steht als `not available`, nie als
erfundene Zahl.

## Neue `Ssystem`-Unterfunktionen (Spezifikation zuerst)

| Nr. | Name | Wert |
|---:|---|---|
| 2003 | `S_NEWTOS_CPUMEAS` | gemessener ARM-Takt, Hz (Mailbox `0x0003_0047`, live) |
| 2004 | `S_NEWTOS_CPUPOLICY` | 0 Firmware / 1 fest Maximum / 2 Maximum verweigert |
| 2005 | `S_NEWTOS_MEMPROT` | 0 aus / 1 ein / 2 streng |
| 2006 | `S_NEWTOS_ASID` | 0 aus / 1 ein / 2 Test (`asid=tiny`) |
| 2007 | `S_NEWTOS_SDMODE` | 0 schnell / 1 Kompatibilität |
| 2008 | `S_NEWTOS_SDBUS` | ausgehandelte Busbreite, Bit |
| 2009 | `S_NEWTOS_BCACHE` | Cache-Speicher KiB, 0 = aus |
| 2010 | `S_NEWTOS_BCACHEPCT` | Anteil je Lesung %, `ERROR` ohne Cache |
| 2011/2012 | `S_NEWTOS_STRAM`/`TTRAM` | Pool-Größe KiB (TT 0 = keins) |
| 2013–2015 | `S_NEWTOS_SCRWIDTH/HEIGHT/DEPTH` | Framebuffer |
| 2016 | `S_NEWTOS_GPUMEM` | VideoCore-Speicher KiB (Mailbox `0x0001_0006`, beim Boot gelesen) |
| 2017 | `S_NEWTOS_KEYBOARD` | 0 US / 1 Deutsch / 2 `KEYBOARD.KBD` / 3 durch Programm geändert |
| 2018 | `S_NEWTOS_SERIAL` | 0 aus / 1 ein |
| 2019 | `S_NEWTOS_KVERSION` | Zeichenkette in Puffer (FreeMiNT `S_KNAME`/`strncpy_f`-Regel) |

Skalare wie 2000/2001 (`arg1` Zeiger oder 0, `arg2` muss 0 sein → sonst
`EINVAL`); unbekannt → `ERROR`, nichts geschrieben; 2020 → `EINVFN`. Fakten
aus Treibern/Boot über neue `Host`-Methoden (Vorgabe `None`), Kernzustand aus
`tos-api` direkt (memprot, mmu, bcache, Pools).

## Prüfung

* QEMU raspi4b: Dialog aus EmuDESK, TeraDesk und QED geöffnet und geschlossen;
  Werte ändern sich mit `memprot=strict bcache=0 emmc=compat cpuclk=max`,
  `memprot=off asid=tiny bcache=8192 bcachepct=20` und `#K de` — jeweils gegen
  die eigenen Bootzeilen des Laufs geprüft.
* Regression: APITEST 106/0/5; LDGDEMO 10/10 Bilder bytegleich zu ACCTITLE;
  ACCTITLE-Pfad: END-Bilder bytegleich, Abweichungen nur Zeigerweg bzw. neuer
  Eintragstext/Dialog (gewollt).
* Statisch: Hosttests 1235/0/1 (+8), Release beide Bretter 64/94, Clippy 76/125,
  Drift 31, SDK 18/0/0 — alles Ausgangswert.

## Kartenpaket (nicht kopiert)

`measurements/measurement-2026-10-07-sysinfo/bin/`: `SYSACC.NAC`
(`06c7ceb6…`), `kernel8.img` (`916c91db…`, release rpi4), `LIESMICH.TXT`.
Beide gehören zusammen.

## Offen

* Echte Pi 4 nicht gelaufen: *Measured* (QEMU beantwortet `0x0003_0047`
  nicht), lebende Takt-/Temperaturwerte, echte Auflösung, GPU-Aufteilung,
  *Serial port Off* ohne `serial=on`.
* qemu-virt nur gebaut (keine Accessory-Sitzung dort).
* Bearbeiten (Version 2) nicht Teil dieses Tickets.
