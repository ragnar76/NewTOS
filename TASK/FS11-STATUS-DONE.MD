# FS11 — Quittung: `Fread`/`Fwrite` auf Zeichengeraeten ueber eine Geraeteweiche

**Status: ERLEDIGT (Developer) · Hosttests + QEMU raspi4b on/off/strict belegt ·
Tester-Abnahme offen · nichts committet · Pi offen** · 2026-10-13 · Zweig `main`,
Basis HEAD `7bb9579a` (Spec-Commit) · Beleg
[measurements/measurement-2026-10-13-fs11/](../measurements/measurement-2026-10-13-fs11/RUNINFO.TXT)
· Ticket [FS11](FS11.md) · Spec `docs/api/gemdos/file/3f-fread.md`,
`40-fwrite.md` („Character-device path“, „NewTOS implementation notes“,
„NewTOS host tests“) und die FS11-Notizen in `docs/api/gemdos/console/`,
`docs/api/bios/{01,02,03,08}`, `docs/api/xbios/0c-midiws.md`.

## Umsetzung (Entwurf des Anwenders vom 2026-10-13)

* **Eine Geraeteweiche** (`tos-api/src/bios/chardev.rs`): `trait CharDevice`
  (`bconstat`/`bconin`/`bconin_restartable`/`bconout`/`bcostat`), Tabelle
  `DEVICES[8]` nach BIOS-Geraetenummer (PRN 0, AUX 1, CON 2, MIDI 3, IKBD 4,
  Raw-CON 5, 6/7), `install_device(dev, entry)` als einziger Andockpunkt fuer
  [IO01](IO01.md). PRN/AUX/MIDI/IKBD/6/7 = `Absent` (alles 0, nie blockierend);
  CON und Raw-CON wie bisher. `Bcostat` behaelt den IKBD/MIDI-Tausch
  (`bios/chardev.c:38-42`). BIOS 1/2/3/8 laufen ueber die Tabelle.
* **GEMDOS** (`tos-api/src/gemdos/console.rs`): `device_read` — HUGE-Waechter,
  Laenge 1 = `conin` (Echo, CR gespeichert), 2..32767 = der vorhandene
  `edit_line` ueber `Bconin`/`Bconout` des Geraets (= `cgets`), 0 und
  32768..65535 → 0; `device_write` — `count <= 0` → `count`, CON = `Cconws`-Regel
  (`output_break`, Rohbyte), sonst `Bconout` mit Kurzzaehlung bei 0. Kein
  AUX/PRN-Sonderfall. `Cconin`/`Cnecin`/`Crawcin`/`Cconout`/`Cconws`/`Cconis`/
  `Cconos`, `Cauxin`/`Cauxout`/`Cauxis`/`Cauxos`, `Cprnout`/`Cprnos` gehen ueber
  die Tabelle (`Cprnout`/`Cprnos`/`Cauxis`/`Cauxos` waren feste Rueckgaben).
* `tos-api/src/bdos/fs/file.rs`: `Target::Device(numl)`, `Fread`/`Fwrite` rufen
  die Geraetewege; `console_read`/`console_write` entfernt. `Fseek` unveraendert 0.
* `tos-api/src/xbios/system.rs`: `Midiws(cnt, ptr)` sendet `cnt+1` Bytes aus
  `ptr` an `Bconout(3)` des MIDI-Eintrags (vorher: Argumente nicht gelesen).

## Tests

`tos-api/tests/fs11_device_rw.rs`, 43 Tests: R1–R32, RT1–RT3, W1–W27, WT1–WT3,
dazu BIOS-Absent-Profil, GEMDOS AUX/PRN, `Cconout`/`Cconws` ueber CON-Eintrag,
`Bcostat`-Tausch, `Midiws`, `Fseek` unveraendert. Rot (nach der Tabelle, vor den
Fread/Fwrite-Aenderungen) 15/28, gruen 43/0. WT4 und der Speicherfehler beim
Lesen als Unit-Tests in `console.rs` (Host kann keinen Benutzerzeiger
fehlschlagen lassen). `fatvol` hat eine Konsolen-Attrappe bekommen; im FS10-Test
ist `Fwrite(-2/-3)` jetzt 0 statt 4 (Spec W15/W18).

## Basislinien

Hosttests **1322/0/1** (1277 + 43 + 2) · Release beider Boards tos-api 64 /
kernel 94, 0 Fehler · Clippy 76/125 beide Feature-Saetze (keine Warnung in
beruehrten Dateien) · Driftprobe 31 · SDK-Hosttests 19/0/0.

## QEMU raspi4b (on / off / strict)

FS11A (Produktkernel) und FS11DEV (Tasten-Injektor-Kernel, Teile A/B/C)
**PASS** in allen drei Modi; FSEEKCON PASS, STDOUTDB gruen, ATEXIT PASS,
APITEST 106/0/5 (seriell bis auf Kartenauflistung gleich zu crt0-main). Rot-Lauf
mit HEAD-Kernel: AUX/PRN-Schreiben landet auf dem Schirm, `Fread(-3,…)` blockiert.

## Befund (nicht Teil, SDK, unveraendert)

mintlib-`stdin` erreicht `Fread` auf NewTOS nicht: `sdk/libc/newtos-arch/isatty.c`
setzt `__open_stat[].flags` nie, `__read` nimmt daher den RAW-Weg (`Crawcin`, kein
Echo, kein CRMOD, liest **zwei** Tasten pro Byte — `unix/read.c:176,183`),
`fgets` endet nie. Mit den Flags, die mintlibs eigenes `__isatty` setzt
(CRMOD|ECHO), laufen `getchar`/`fgets` ueber den neuen Zeilenweg korrekt
(Probe Teil C). Die Tester-Notiz in `3f-fread.md` setzt den Fread-Weg voraus.

## Sichtbare Verhaltensaenderungen (laut Spec)

`Fwrite` auf AUX/PRN (auch Std-Handle 2/3 ohne Umlenkung) → 0, nichts auf dem
Schirm; `Fread` dort → `len` NULs ohne Warten; `Fread` auf CON zeilenweise mit
Echo/Editierung bzw. `conin` bei Laenge 1; `Fwrite(-1, -5)` → -5; Ctrl-C vorn in
der Warteschlange beendet ein `Fwrite` auf CON; `Midiws` liest seinen Puffer.

## Offen

* Tester-Abnahme (Regressionsstichprobe Desktop/TeraDesk/QED/Doom/S2ALL/EmuCON),
  qemu-virt-Laeufe, echte Pi.
* `docs/STATUS.md` (Basislinie Hosttests 1322, *Recent Decisions*) beim Commit.
* Spec-Notiz zu mintlib/`isatty` (oben) an den Lead.
