# FS11-AUX — Quittung: AUX als „Port da, kein Kabel“

**Status: ERLEDIGT (Developer) · Hosttests + QEMU raspi4b on/off/strict belegt ·
Tester-Abnahme offen · nichts committet · Pi offen** · 2026-10-13 · Zweig `main`,
Basis HEAD `73e9f898` · Beleg
[measurements/measurement-2026-10-13-fs11-aux/](../measurements/measurement-2026-10-13-fs11-aux/RUNINFO.TXT)
· Ticket [FS11](FS11.md) („Nachtrag 2026-10-13“) · Spec
`docs/api/gemdos/file/3f-fread.md` Punkt 1a, `40-fwrite.md` W15–W17, W25, W28,
`docs/api/bios/{01,02,03,08}`, `docs/api/gemdos/console/{03,04,12,13}`.

## Umsetzung

* `tos-api/src/bios/chardev.rs`: neuer Tabelleneintrag `AuxNoCable` fuer BIOS 1 —
  `Bconout` nimmt an, verwirft, liefert 1; `Bcostat` -1; `Bconstat` 0; `Bconin` 0
  ohne Blockieren (dokumentierte Abweichung von `bconin1`). PRN, MIDI, IKBD, 6/7
  bleiben `Absent`. `install_device` unveraendert (IO01 ersetzt den Eintrag).
* Nur Kommentare angepasst: `tos-api/src/gemdos/console.rs` (`Cauxin`/`Cauxout`/
  `Cauxis`/`Cauxos`), `kernel/src/console.rs`, `kernel/src/desk/mod.rs`.
* `kernel/src/selftest_console.rs` (nicht verdrahtet): `Cauxout` erwartet 1.
* `sdk/apps/apitest/apitest.c`: BC14 `Bconout(1)` → 1, BC22 `Bcostat(1)` → -1.

## Tests

`tos-api/tests/fs11_device_rw.rs`: W15–W17 (Zaehler statt 0), W25 (2), neu W28
(`Fwrite(2,…)` ohne `Fforce`: 5, 1, 0x1000; nichts auf CON), BIOS-/GEMDOS-Profil
mit AUX-Werten. `fs10_fseek_device_handles.rs`: `Fwrite(-2)` = 4/2. Unit-Test WT4
in `console.rs`: PRN 0, AUX 3. Rot 7 Fehlschlaege, gruen alle.

## Basislinien

Hosttests **1323/0/1** (1322 + W28) · Release beider Boards tos-api 64 / kernel 94,
0 Fehler · Clippy 76/125 beide Boards · Driftprobe 31 · SDK-Hosttests 19/0/0.

## QEMU raspi4b (on / off / strict)

FS11A und FS11DEV (Sonde mit neuen AUX-Erwartungen + W28) **PASS**; STDOUTDB
unveraendert; APITEST 106/0/5 (BC14/BC22 OK). Haenger-Reproduktion
(alte crt0, Handle 2 = AUX) **laeuft durch**, ebenso das neue ARGV-Kind
(`STDERRA`, neue crt0, argv[0] gesetzt, kein `Fforce`): alle stderr-Schreiber
liefern ihre Laenge. Rot-Kontrolle mit dem Kernel vor der Aenderung: beide
haengen. EmuCON uebergibt kein ARGV (`cli/cmdexec.c:52`), daher eigene Sonde.

## Offen

qemu-virt nur gebaut, nicht gebootet; asid=on nicht gefahren; echter Pi 4 offen.
Folge von Option A: stderr alter Programme ist unsichtbar (AUX verwirft) statt
haengend.
