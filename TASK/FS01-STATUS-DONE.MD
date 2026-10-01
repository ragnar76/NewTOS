# FS01 — STATUS: DONE (2026-09-24)

**Auftrag:** `Fdelete`/`Ddelete` geben beim Loeschen die Langnamen-Schlitze
(`attr == 0x0F`) vor dem 8.3-Eintrag mit frei — und vorher am eigenen
Quelltext pruefen, ob die Vermutung aus dem Ticket ueberhaupt stimmt.

Basis: Branch `desk-rebuild`, HEAD `07e55b4`, Aenderungen **uncommitted** im
Baum. Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, nichts
committet; kein `git checkout/restore/clean/stash/reset` auf dem gemeinsamen
Baum. Git wurde nur lesend benutzt (`git status`, `git diff` fuer
`method/fs01.diff`).

Messung: [`measurements/measurement-2026-09-24-fs01/`](../measurements/measurement-2026-09-24-fs01/).
Spezifikation: `docs/api/gemdos/file/41-fdelete.md` (Abschnitt *FS01*) und
`docs/api/gemdos/directory/3a-ddelete.md`.

---

## 1. Die Vermutung des Tickets — am Quelltext geprueft, sie stimmt

Es gab genau drei Stellen, die einen Verzeichniseintrag freigeben, und alle
drei schrieben nur das erste Namensbyte:

* `erase_entry` (`tos-api/src/bdos/fs/file.rs`) — der Arbeitspfad von
  `Fdelete` **und** von `Fcreate` auf eine bereits bestehende Datei,
* `ddelete` (`tos-api/src/bdos/fs/dirops.rs`),
* `frename` (`file.rs`, `old[0] = 0xE5`) — siehe §6, nicht im Umfang.

`FA_LFN` kam im ganzen `bdos/fs`-Baum nur an vier Stellen vor, und jede war
ein *Ueberspringen* beim Scannen (`dir.rs:137,225`, `find.rs:234`) — kein
Pfad hat die Schlitze je angefasst. NewTOS schreibt auch selbst keine
Langnamen; auf die Karte kommen sie vom schreibenden Fremdsystem.

---

## 2. Was EmuTOS dazu sagt: nichts

Klassisches TOS kennt keine Langnamen. EmuTOS *ueberspringt* `FA_LFN`-Saetze
beim Scannen (`references/emutos/bdos/fsdir.c:326,1810`) und hat keine Stelle,
die sie erzeugt oder freigibt — die Frage ist dort nicht beantwortbar, wie das
Ticket vermutet hat.

**Gewaehlte Quelle: FreeMiNTs FAT-Schicht**, im Projekt sanktioniert und
gepinnt, mit dem Vermerk „this informations are from Microsoft"
(`references/freemint/sys/fatfs.c:813`):

| Was | Beleg |
|---|---|
| `LDIR`-Satz: Ordnungsbyte (`head`, Bit 0..4 Nummer, Bit 6 Namensende) bei 0, `attr` bei 11, Pruefsumme bei 13 | `fatfs.c:779-790` |
| Kette liegt **unmittelbar vor** dem 8.3-Eintrag, Ordnungszahlen ruecklaeufig bis 1, alle mit derselben Pruefsumme, und die Pruefsumme muss zum Kurznamen passen (`sum = ror8(sum) + name[i]`) | `fatfs.c:3690-3748` (`__readvfat`) |
| Loeschen: **erst** der 8.3-Eintrag `0xE5`, **dann** die `c->slots` VFAT-Schlitze davor | `fatfs.c:4246-4272` (`unlink_cookie_i`) |
| Eine Kette mit falscher Pruefsumme oder gebrochener Ordnung gehoert nicht dazu und wird verworfen | `fatfs.c:3710-3721` |

Damit ist die Reihenfolge (Kurzeintrag zuerst) keine Geschmacksfrage: bricht
der Vorgang dazwischen ab, bleiben Waisen zurueck — der harmlose Zustand —
statt eines lebenden Eintrags mit halb geloeschtem Namen.

---

## 3. Der Fix

`dir::erase_slot_with_lfn(vol, start, slot)` in
`tos-api/src/bdos/fs/dir.rs` (neu, zusammen mit `dir::lfn_checksum`):

1. 8.3-Eintrag lesen, Pruefsumme ueber die elf Namensbytes bilden,
2. erstes Byte auf `0xE5`, zurueckschreiben,
3. rueckwaerts ueber die Schlitze davor: uebernommen wird nur, was noch lebt
   (erstes Byte weder `0xE5` noch `0x00`), `attr == FA_LFN` traegt, dieselbe
   Pruefsumme hat und die naechste erwartete Ordnungszahl (1, 2, 3 …); Ende
   beim Schlitz mit `0x40`. Obergrenze = Breite des Ordnungsfeldes (5 Bit),
   damit eine kaputte Kette den Lauf nicht davontraegt.

Aufrufer: `erase_entry` (`file.rs`) und `ddelete` (`dirops.rs`). Damit ist
auch `Fcreate` auf eine bestehende Datei mit erfasst — es benutzt
`erase_entry`.

Nur das erste Byte wird veraendert, wie bei FreeMiNT; das `0x0F`-Attribut
bleibt stehen, denn es haelt einen Scanner davon ab, den Schlitz als
8.3-Eintrag zu lesen.

---

## 4. Verifikation

### Hosttest (neu)

`tos-api/tests/gemdos_fs_lfn_delete.rs` — ein Test, absichtlich eine einzige
Funktion (die Testeinlage liegt hinter dem BDOS-Sektorcache, der kein
Invalidate hat, also muss sie vor dem ersten GEMDOS-Aufruf des Binaries
stehen). Gepflanzt werden im Rohabbild fuenf Eintraege mit Ketten; geprueft
werden die **Rohbytes** des Verzeichnisses danach:

| Fall | Erwartung |
|---|---|
| `Fdelete` Datei, 2-Schlitz-Kette | 8.3 + beide Schlitze `0xE5` |
| Nachbar-Datei lebt weiter | ihre Kette unangetastet |
| `Ddelete` Ordner, 3-Schlitz-Kette | 8.3 + alle drei `0xE5` |
| `Fdelete` in einem Unterverzeichnis | genauso |
| Kette mit falscher Pruefsumme | bleibt stehen (gehoert nicht dazu) |
| Waisen-Zaehlung ueber das ganze Wurzelverzeichnis | genau der eine absichtliche Rest; vorher waren es sechs (`[2, 3, 5, 6, 7, 11]`) |

Rot vor dem Fix (gemessen, mit hand-zurueckgesetztem Zweig:
„ordinal-1 long-name slot must be freed with it, left: 1, right: 229"),
gruen danach.

`cargo run -p xtask -- test`: **vorher 938 passed / 0 failed / 1 ignored,
nachher 939 / 0 / 1.**

### Bau, beide Boards, je frisch

`cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat`
vor **jedem** Lauf:

| Lauf | Ergebnis |
|---|---|
| `cargo xtask build --board rpi4` | rc=0, `tos-api` 64 / `kernel` 94 Warnungen |
| `cargo xtask build --board qemu-virt` | rc=0, `tos-api` 64 / `kernel` 94 Warnungen |

Basis 64/94 → **Delta 0**.

### Clippy

| Lauf | Ergebnis |
|---|---|
| `cargo clippy -p kernel --features board-rpi4` | `tos-api` 80 / `kernel` 125 |
| `cargo clippy -p kernel --features board-qemu-virt` | `tos-api` 80 / `kernel` 125 |

Basis 80/125 → **Delta 0**. Keine einzige Warnung zeigt auf
`bdos/fs/dir.rs`, `file.rs` oder `dirops.rs`; die einzige Fundstelle in
`bdos/fs` ist das vorbestehende „this `impl` can be derived" in
`find.rs:341`. `cargo clippy -p tos-api --test gemdos_fs_lfn_delete`: keine
Warnung in der neuen Testdatei.

### Abnahme in QEMU (`measurements/measurement-2026-09-24-fs01`)

Dieselbe DESK25-Geste, dasselbe Kartenabbild, zwei Kerne, die sich in genau
einem Zweig unterscheiden (`method/unfix.py` auf der Scratch-Kopie):

| Karte | Waisen-Schlitze | `fsck_msdos -n` |
|---|---|---|
| frisch (`method/seed.sh`) | 0 | rc=0, Phase 1/2/3 sauber |
| geleert, **ohne** Fix | 8 | stirbt in Phase 2, Signal 5 (Shell 133) |
| geleert, **mit** Fix | 0 | rc=0, Phase 1/2/3 sauber, „0 files" |

---

## 5. Die offene Frage aus DESK25, beantwortet

**Warum kostet ein einzelner Waise nur eine Warnung, die ganz geleerte Karte
aber den Absturz?** `method/abort_probe.sh`, zwoelf Faelle auf demselben
Abbild (`serial/fsck.txt`):

* **Keine Mengenwirkung.** Jede der fuenf Ketten *allein* — die anderen sieben
  Schlitze von Hand freigegeben — bringt `fsck_msdos` genauso zum Absturz wie
  alle acht zusammen.
* Alle acht von Hand freigeben: dasselbe Abbild laeuft sauber durch (rc=0) —
  die DESK25-Gegenprobe, reproduziert.
* Ein einzelner Waise auf der noch **lebenden** Karte: nur die Warnung
  „Invalid long filename entry for /.Spotlight-V100", rc=206.
* **Der Unterschied:** ob die Kette noch von einem lebenden 8.3-Eintrag
  geschlossen wird. Ein gepflanzter lebender Eintrag *hinter* der Kette
  (direkt dahinter oder weit hinten am Verzeichnisende) macht aus dem Absturz
  dieselbe Warnung; einer nur *davor* nicht.

Die Warnung nennt den **folgenden** lebenden Eintrag, nicht den Waisen —
`fsck_msdos` meldet eine ungueltige Kette also erst, wenn es den naechsten
lebenden Kurzeintrag erreicht. Auf der ganz geleerten Karte folgt keiner mehr,
der Lauf erreicht die Verzeichnisende-Marke mit offener Kette, und dort
stirbt er. Das ist ein Defekt **in `fsck_msdos`** — ein Pruefwerkzeug darf an
einem lesbaren Verzeichnis nicht in einen Trap laufen —, kein zweiter
NewTOS-Defekt: die FAT der Karte ist konsistent, beide Kopien gleich. Kein
eigener NewTOS-Task noetig; wir liefern die Eingabe nur nicht mehr.

Nebenbefund zur Methode: „gar keine Ausgabe, nicht einmal das Banner" im
Erstlauf war die Pufferung von `fsck_msdos … | sed` — stdout ist dann eine
voll gepufferte Pipe, und ein per Signal gestorbener Prozess leert sie nie.
`method/fsck.sh` faehrt `fsck_msdos` ueber ein Pty (`script`), darum sind die
beiden Phasenzeilen hier sichtbar.

Auch die Waisen-Zaehlung war im Erstlauf zu weit: `lfn_orphan_probe.py`
zaehlte einen Schlitz allein nach seinem 8.3-Eintrag und nahm damit die rund
30 **bereits geloeschten** Schlitze mit, die macOS beim Wegraeumen seiner
eigenen `._*`-Begleiter hinterlaesst. Die sind freie Slots und stoeren
niemanden. `method/lfn_orphan_scan.py` zaehlt streng (Schlitz lebt **und**
8.3-Eintrag geloescht) und deckt Wurzel *und* Unterverzeichnisse ab.

---

## 6. Ausserhalb des Umfangs (ein eigener Task)

`frename` (`tos-api/src/bdos/fs/file.rs`) kopiert den FCB in den Zielschlitz
und loescht den Quelleintrag mit einem eigenen `old[0] = 0xE5` — die
Langnamen-Kette des alten Namens bleibt dort genauso stehen, und beim
Umbenennen passt sie danach ohnehin zu keinem Kurznamen mehr. Dieselbe
Stelle, anderer Aufrufer; nach der Regel „ein zusaetzlicher Defekt bekommt
einen eigenen Task" **nicht** mitgeaendert. Ein Ticket dafuer waere:
`Frename` gibt die Langnamen-Kette des Quellnamens nicht frei.

---

## 7. Beruehrte Dateien

| Datei | Was |
|---|---|
| `tos-api/src/bdos/fs/dir.rs` | `lfn_checksum`, `erase_slot_with_lfn` + LDIR-Konstanten (neu) |
| `tos-api/src/bdos/fs/file.rs` | `erase_entry` ruft sie |
| `tos-api/src/bdos/fs/dirops.rs` | `ddelete` ruft sie |
| `tos-api/tests/gemdos_fs_lfn_delete.rs` | neuer Hosttest |
| `tos-api/tests/fatvol/mod.rs` | Roh-Sektorzugriff fuer Tests (`read_sector`/`write_sector`, LBA-Helfer, `DDELETE`) |
| `docs/api/gemdos/file/41-fdelete.md` | NewTOS-Notiz *FS01* |
| `docs/api/gemdos/directory/3a-ddelete.md` | NewTOS-Notiz *FS01* |
| `docs/STATUS.md` | Recent-Decisions-Eintrag + Phasenzeile 09 |
| `TASK/FS01.md` | Status + `## Abschluss` |
| `measurements/measurement-2026-09-24-fs01/` | RUNINFO.TXT, method/, serial/, screendumps/, manifest.sha256 (kein `*.img`) |

**Offen:** Abnahme auf echter Pi 4 (nicht Sache dieses Tasks — keine
Hardware angefasst). Der unabhaengige Tester faehrt die Regression.
