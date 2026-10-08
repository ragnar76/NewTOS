# SYSINFO02 — Quittung (FERTIG in QEMU, ein offener Punkt): System Info, Version 2

Datum 2026-10-08 · Basis `desk-rebuild` `7615ed6` + diese Änderung (nicht committet)
· Beleg: [measurement-2026-10-08-sysinfo2](../measurements/measurement-2026-10-08-sysinfo2/RUNINFO.TXT)
· Ticket: [SYSINFO02](SYSINFO02.md) · Vorgänger: [SYSINFO01](SYSINFO01-STATUS-DONE.MD)

## Ergebnis

Das Accessory ` System Info` zeigt unter den Maschinenwerten die Gruppe
**Settings**: acht Zeilen (Memory protection, Address-space IDs, SD card mode,
CPU clock, Block cache, Cache share, Mouse while busy, Serial console), je mit
**Running now** (Ssystem, der wirksame Zustand) und **Next start** (aus
`C:\CMDLINE.TXT`, gelesen wie der Kernel seine Startargumente liest). Ein Klick
auf einen Next-start-Wert klappt eine kleine Auswahlliste auf (eigener
Objektbaum, 3D, Haken am gültigen Wert, Markierung unter dem Zeiger); Klick auf
einen Eintrag wählt, Klick daneben bricht ab. Kein Rohtext der Datei
erscheint irgendwo. Knöpfe **Save** (schreiben, Dialog bleibt offen), **OK**
(schließen; ungespeicherte Änderungen vorher speichern), **Cancel** (schließen
ohne Speichern); vor jedem Schreiben eine `form_alert`-Rückfrage mit den
Änderungen in Worten; Hinweis „Changes take effect after restart." immer
sichtbar.

Speichern: unverwaltete Token bleiben in ihrer Reihenfolge, verwaltete werden
an Ort und Stelle ersetzt oder entfernt, Voreinstellungen entfallen, eine
Zeile; Sicherung `C:\CMDLINE.BAK`, dann `CMDLINE.NEW` → `Fdelete` →
`Frename`. `config.txt` unberührt. Nur Werkzeugkette, kein Kernel-Code.

## Dateien

* `sdk/examples/sysacc.c` — Dialog Version 2, Auswahlliste, Speichern
* `sdk/examples/sysacc-cmdline.h` (neu) — Parse + Zusammenführung, reine Bytelogik
* `sdk/examples/tests/sysacc-cmdline-host.c` (neu) — Host-Test, 80 Prüfungen
* `sdk/run-host-tests.sh` — führt ihn aus

## Prüfung

* QEMU raspi4b aus EmuDESK: Memory protection → Strict, Block cache → 8 MB,
  dritte Liste daneben abgebrochen, Save, Rückfrage listet genau die zwei
  Änderungen; `CMDLINE.TXT` = alte Zeile + `memprot=strict bcache=8192`,
  `CMDLINE.BAK` = alte Bytes. Neustart mit dieser Zeile als `-append` (QEMU
  liest `-append`, nicht die Datei): Running now Strict / 8 MB, Bootzeilen
  stimmen. Aus TeraDesk und QED geöffnet (ohne Datei: „No C:\CMDLINE.TXT
  yet").
* Regression: APITEST 106/0/5; LDGDEMO und Accessory-Titel-Pfad
  pixelgleich zu SYSINFO01 außer dem Dialog selbst.
* Statisch: SDK-Host-Tests 19/0/0 (18 + 1 neu); `sysacc.c` ohne Warnung.
  Rust nicht angefasst.

## Kartenpaket (nicht kopiert)

`measurements/measurement-2026-10-08-sysinfo2/bin/`: `SYSACC.NAC`
(`db2e325c…`), `LIESMICH.TXT`. Kein neuer Kernel nötig (braucht Ssystem
2003–2019, also den SYSINFO01-Kernel oder neuer).

## Offen

* **Esc in der Auswahlliste wirkt nicht** (auch Return/Pfeiltasten dort):
  Während das Accessory in `evnt_multi` schläft, läuft EmuDESK und nimmt die
  Taste aus dem einen globalen Tastenpuffer; EmuTOS gibt sie nur dem
  Maus-Eigentümer (`geminput.c:288-307`). Kernel-AES-Änderung nötig → nicht
  gemacht (Ticketregel), eigenes Ticket. Der Accessory-Code dafür ist da.
* „Mouse while busy" Running now = „not available": keine Ssystem-Nummer für
  `input=aes` (Kernel-Änderung, eigenes Ticket falls gewünscht).
* Echte Pi 4 nicht gelaufen: Firmware liest die gespeicherte Datei beim
  nächsten Start; Dateiname danach `CMDLINE.TXT`; Schreiben auf die
  Boot-Partition der echten Karte.
* `menu_popup` (36) existiert inzwischen im Kernel (A59); laut Ticket nicht
  verwendet.
