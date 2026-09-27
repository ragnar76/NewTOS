# HW03 — STATUS: DONE

Datum: 2026-09-27 · Basis: branch `desk-rebuild`, HEAD `84baa34` ·
Messverzeichnis: [`measurement-2026-09-27-hw03/`](../measurement-2026-09-27-hw03/)

Nicht committet, nichts geflasht, keine Pi gefahren, `/Volumes/BOOT` nicht
beruehrt, kein `git checkout/restore/clean/stash/reset`.
`docs/STATUS.md` und `TASK/HW03.md` sind unveraendert (Vorgabe des Leads).

---

## In einem Satz

Der Fehlerweg von der Karte bis zur Anwendung ist durchgehend: die R1-Antwort
wird ausgewertet, ein fehlgeschlagenes Rueckschreiben verliert den Sektor
nicht mehr, die neun `flush()`-Aufrufstellen tragen ihr Ergebnis, und die
Wiederholung ist begrenzt und gezaehlt — belegt durch fuenf neue Hosttests und
einen getriebenen QEMU-Lauf, in dem ein Schreibzugriff hinter das Kartenende
jetzt `Err(Card)` statt `Ok(())` liefert.

---

## Auftragspunkt 1 — die R1-Antwort auswerten

**Spec zuerst.** Die R1-Bittabelle fehlte im Projekt: `docs/hardware/emmc-sd.md`
hatte keinen Abschnitt zum Card Status, und die SD-Spezifikationen liegen
nicht in `references/` (Audit-Befund S1). Neuer Abschnitt
**`docs/hardware/emmc-sd.md` §5 "Card status (R1)"** mit
*jedem* Fehlerbit woertlich aus **SD Physical Layer Simplified Specification
Ver. 9.10, Tabelle 4-42** — Identifier, Bit, Typspalte, Beschreibung — plus
der Abgrenzung, warum `CURRENT_STATE`, `READY_FOR_DATA`, `APP_CMD`,
`CARD_IS_LOCKED`, `CARD_ECC_DISABLED`, `ERASE_RESET` und `FX_EVENT`
**nicht** dazugehoeren (Typspalte ohne `E`). Die bestehenden Abschnitte wurden
nur umnummeriert (altes §5 → §6); §1/§2, die einzigen zitierten, sind
unberuehrt.

**Zur Quelle, offen gesagt:** die PDF war nicht im Baum. Ich habe die
*Simplified Specification* Ver. 9.10 von der oeffentlichen Downloadseite der
SD Association geholt und unter `references/sd-spec/` abgelegt — das
Verzeichnis ist gitignored, also ohne Wirkung auf das Repo. Das Audit zitiert
Ver. 6.00; **die Bitpositionen stammen aus 9.10 und sind nicht gegen 6.00
diffed** — so steht es auch im Spec-Abschnitt. Das Ablegen beider Ausgaben
bleibt REF02.

**Code.** `kernel/src/drivers/bcm2711/emmc.rs`:
* `R1_OUT_OF_RANGE` … `R1_AKE_SEQ_ERROR` — 15 benannte Konstanten, jede mit
  dem Tabellentext als Doc-Kommentar; `R1_ERROR_MASK = 0xFDF9_8008` ist ihre
  Vereinigung, also genau die Typ-`E`-Menge.
* `check_r1(cmd_index, status)` — zaehlt je Bit, merkt sich den Statuswert,
  schreibt eine Konsolenzeile mit den gesetzten Bitnamen und dem
  `CURRENT_STATE`, und liefert `Err(BlockError::Card)`.
* `Emmc::read_one` / `write_one` werten die Antwort von CMD17 bzw. CMD24 aus.
  Das sind die beiden Stellen, die das Audit nennt (`:299-300`, `:324`); beide
  Kommandos antworten R1 **vor** der Datenphase, die Ablehnung ist also
  sichtbar, bevor irgendetwas uebertragen wird.
* `ErrorStats` hat drei neue Felder (`card_errors`, `last_card_status`,
  `card_bits`), `error_stats()` fuellt sie.

**Getrieben** (`measurement-2026-09-27-hw03/raw/03`, Abschnitt 2 der RUNINFO):

    [emmc] CMD17 rejected by card: status=0x40000900 ADDRESS_ERROR (state 4)
    [emmc] CMD24 rejected by card: status=0x40000900 ADDRESS_ERROR (state 4)
    [emmc] R1 out-of-range probe @ block 135168:
             read -> Err(Card), write -> Err(Card)   OK (refused)

QEMU setzt `ADDRESS_ERROR` (Bit 30), nicht `OUT_OF_RANGE` (Bit 31). Beide
liegen in der Maske — der Grund, die ganze Typ-E-Menge zu pruefen statt
einzelner Bits. Welches Bit eine echte Karte setzt, ist damit gleichgueltig.

---

## Auftragspunkt 2 — was `evict` bei Misserfolg tut, entworfen und belegt

**Der Entwurf.** `tos-api/src/bdos/fs/cache.rs`:

* `evict()` liefert jetzt `Result<usize, Long>`. Schlaegt das Rueckschreiben
  des Opfers fehl, wird der Platz **nicht** herausgegeben, `dirty` **nicht**
  geloescht, und der Aufrufer bricht mit dem Geraetecode ab. Der
  Round-Robin-Zeiger ist schon weitergerueckt, der naechste Zugriff nimmt
  also einen anderen Platz — ein einzelner schlechter Sektor legt den Cache
  nicht lahm.
* `write_back()` ist die einzige Stelle, die `dirty` loescht, und nur nach
  `Ok(())`.

**Die Begruendung, und die Abweichung von EmuTOS beim Namen genannt.**
EmuTOS wirft den Puffer bei einem Fehler weg: `flush()` setzt
`b_bufdrv = -1` *vor* dem Schreiben ("invalidate in case of error",
`bdos/fsbuf.c:93`), und der Fehlerzweig in `osif` verwirft danach alle Puffer
des Laufwerks (`mark_bcbs_invalid`, `bdos/bdosmain.c:490`). Es tut das, **weil
ein `longjmp` den Puffer nicht mitnehmen kann** — vom Landepunkt des Sprungs
aus ist er nicht mehr erreichbar; es ist eine Folge der Sprungtechnik, keine
Integritaetsentscheidung. Wir springen nicht, also sind wir dazu nicht
gezwungen, und die Forderung des Tickets ist genau, dass kein Sektor
verschwindet, weil sein Rueckschreiben scheiterte. Der Cache haelt in diesem
Moment die **einzige** aktuelle Kopie; sie wegzuwerfen macht jede Rettung
unmoeglich, sie zu behalten laesst ein spaeteres `flush()` sie noch landen.
Beides ist im Doc-Kommentar von `write_back()` so aufgeschrieben.

**Getrieben:** `a_failed_write_back_keeps_the_sector_and_lands_it_afterwards`
— `Fwrite` meldet `EWRITF`, die Karte traegt weiter die alten Bytes, der
Sektor ist nach wie vor als schmutziger Cacheplatz da (ueber
`cache::writeback_stats()` geprueft), ueberlebt einen vollen Umlauf der
Verdraengung (ein `Dfree` liest 20 FAT-Sektoren, mehr als der Cache Plaetze
hat) — und sobald der Fehler weg ist, landet **genau dieser** Sektor auf der
Karte.

---

## Auftragspunkt 3 — der Fehlerweg bis GEMDOS

Vier Schichten, von unten nach oben:

1. **`kernel/src/hal/block.rs`** — `BlockError` bekommt `Card` (die Karte hat
   abgelehnt) und `OutOfRange` (die Adresse ist auf dieser Karte nicht
   darstellbar, siehe Nebenbefund unten).
2. **`kernel/src/host_env.rs`** — neue Funktion `block_error_code(e, write)`
   uebersetzt nach TOS, mit der Zuordnung, die EmuTOS' eigener SD-Treiber
   macht: Kartenfehler/Transferfehler → `EWRITF`/`EREADF` (`bios/sd.c:187-190`),
   Zeitablauf und fehlende Karte → `EDRVNR` (`:195`, `:172-173`),
   nicht darstellbare Adresse → `ESECNF` (`bios/blkdev.c:488-490`),
   nicht bedienbare Anforderung → `EUNDEV` (`:440-441`).
   Hier endete der Fehler bisher: `BlockError` wurde zu `bool`.
3. **`tos-api/src/bios/block.rs`** — `BlockDev::read_blocks`/`write_blocks`
   liefern `Result<(), Long>` statt `bool`, also den TOS-Code; das ist die
   Form, die EmuTOS' Treiberschnittstelle hat (`sd_rw`, `disk_rw` geben eine
   LONG zurueck). **`tos-api/src/bios/mod.rs`** (`Rwabs`) reicht ihn
   unveraendert durch, wie `blkdev_rwabs` (`bios/blkdev.c:512-523`).
4. **`tos-api/src/bdos/fs/cache.rs` + `file.rs` + `dirops.rs`** —
   `flush() -> Result<(), Long>`; `Result` ist `#[must_use]`, ein verworfenes
   Ergebnis ist ab jetzt eine Compilerwarnung. **Alle neun Aufrufstellen**
   tragen es: `fcreate`, `fwrite`, `fdelete`, `fattrib`, `frename`, `fdatime`,
   `fclose` (`file.rs`), `dcreate`, `ddelete` (`dirops.rs`).

Fuer den `bool`-Innenraum (alles ueber `fat.rs`, das nicht auf `Result`
umgestellt werden kann, ohne halb `bdos/fs` anzufassen) gibt es das
Gegenstueck zu EmuTOS' Globalen: `LAST_ERROR` ist `rwerr`/`errcode`
(`bdos/fsglob.c:40-41`), `clear_error()` steht am Anfang von
`gemdos::dispatch_inner` wie EmuTOS' `setjmp(errbuf)` am Anfang von `osif`
(`bdos/bdosmain.c:448`), und `take_error()` liefert den Code als
Funktionsergebnis — das, was `docs/api/gemdos/_dispatcher.md` §8 mit
"returned to caller as if it were the function's return value" beschreibt.
Damit unterscheidet `Fwrite` wieder **volle Platte** (kurze Byte-Zahl, so
sagt es `docs/api/gemdos/file/40-fwrite.md`) von **Geraetefehler** (`EWRITF`),
und `Fread` gibt `EREADF` statt einer kurzen Zahl
(`docs/api/gemdos/file/3f-fread.md`).

**Spec nachgezogen** (Regel 5): `40-fwrite.md`, `3f-fread.md` und
`04-rwabs.md` §"Error codes" beschreiben jetzt den tatsaechlichen Weg.

---

## Auftragspunkt 4 — die stille Wiederholung sichtbar machen

* Jeder Cacheplatz zaehlt seine fehlgeschlagenen Rueckschreibversuche
  (`wb_fails`) und merkt sich deren Code (`wb_err`).
* `WRITEBACK_ATTEMPTS = 3` begrenzt sie. Danach ruehrt der Cache das Geraet
  fuer diesen Sektor nicht mehr von selbst an — er meldet den gemerkten
  Fehler sofort weiter. Der Sektor bleibt dabei im Cache und bleibt
  schmutzig.
* Die Zahl ist eine **Richtlinienentscheidung, keine Spezifikationsgroesse**,
  und steht als solche im Code. EmuTOS begrenzt dasselbe an zwei Stellen und
  niedriger: `RWABS_RETRIES = 1` ("on real machine might want to increase
  this", `bios/blkdev.h:34`) und "retry at most once" im SD-Treiber
  (`bios/sd.c:167-169`). Drei gibt einem voruebergehend beschaeftigten
  Kartenzustand zwei Versuche mehr als EmuTOS und terminiert trotzdem.
* Ein erneutes `cache::write()` desselben Sektors setzt den Zaehler zurueck —
  neue Daten sind eine neue Anforderung, der Sektor ist nicht fuer immer
  festgefahren, wenn die Karte zurueckkommt.
* Sichtbar: `cache::writeback_stats() -> (failures, retries, exhausted,
  dirty_slots)`; `retries` ist genau die Zahl, die vorher niemand hatte.
  Dazu schreibt der Treiber jede Ablehnung und jeden Zeitablauf auf die
  Konsole (`[emmc] …`), und — der eigentliche Punkt — **jeder** Versuch
  meldet seinen Fehler nach oben, statt ihn zu schlucken.

**Getrieben:** `the_write_back_retry_is_bounded_and_visible` — nach dem
ersten `Fwrite` folgen zehn weitere flushende Aufrufe; das Geraet sieht
hoechstens drei Schreibversuche, jeder Aufruf meldet weiterhin `EWRITF`, der
Platz ist als *exhausted* gezaehlt, der Sektor ist noch da, und nach dem Ende
der Stoerung geht das Schreiben wieder durch.

---

## Zwei Nebenbefunde, die beim Treiben aufgefallen sind

Beide liegen auf genau dem Pfad, den dieses Ticket haerten soll, und beide
sind Datenintegritaet — deshalb hier behoben und nicht nur notiert.

**N1 — Zwei Software-Resets in einem Zugriff wirken in QEMU gar nicht.**
`Emmc::soft_reset` schrieb `SRST_CMD | SRST_DATA` zusammen. QEMUs SDHCI-Modell
dekodiert das Resetbyte mit einem **exakten** `switch`
(`hw/sd/sdhci.c:1150-1170`, v11.0.2), trifft bei 0x06 keinen Fall und setzt
**nichts** zurueck; da die Bits im nicht gespeicherten oberen `CLKCON`-Byte
liegen, lesen sie sich sofort als 0 zurueck und die Warteschleife meldet
Erfolg. Folge: der Controller blieb nach jeder Wiederherstellung auf
`STATUS=0x01ff0506` stehen und kein Sektorzugriff gelang mehr (raw/01 und
raw/02, in den `[emmc]`-Zeilen zeichengleich — das Abbruchkommando war
**nicht** die Ursache). Behoben: ein Resetbit pro Zugriff, CMD-Leitung zuerst,
dann DAT-Leitung — was SD-HC §3.10.1 und §3.8 ohnehin als zwei Schritte
auffuehren. **Das betrifft auch den FS04-Wiederherstellungspfad**
(`Phase::on_dat_line()` → beide Resets): der hat in QEMU nie gewirkt. Auf
echter Hardware ungeprueft, siehe M-C unten.

**N2 — `(lba as u32) * 512` lief still ueber.** Auf einer byteadressierten
SDSC-Karte (QEMUs Emulat ist eine) ueberlaeuft die Byteadresse fuer grosse
LBAs: im Debugbau eine Panik, im Releasebau ein **stiller Umlauf auf eine
andere, gueltige Sektoradresse** — bei einem Schreibzugriff die
schlimmstmoegliche Form von Datenverlust. Erreichbar war die Stelle bisher
nicht (`Rwabs` prueft vorher den Bereich); die neue Sonde ist ihr erster
Aufrufer mit grossem `lba`. Behoben in `Emmc::card_address()`: geprueft
gebildet, sonst `BlockError::OutOfRange` → `ESECNF`.

Ausserdem: die Ablehnung wird mit `recover_no_abort()` aufgeraeumt statt mit
der vollen Abbruchfolge. Die Karte war nie in einer Datentransaktion (die R1
sagt `CURRENT_STATE = tran`), es gibt auf ihrer Seite nichts abzubrechen;
haengen bleibt nur die Datenzustandsmaschine des Controllers, und genau die
raeumt `Software Reset For DAT Line` per SD-HC §2.2.15 auf.

---

## Geaenderte Dateien

**Spezifikation**
* `docs/hardware/emmc-sd.md` — neuer §5 "Card status (R1)" (Tabelle 4-42
  woertlich, Zuordnung auf TOS-Codes, Quellenluecke benannt); altes §5 → §6.
* `docs/api/gemdos/file/40-fwrite.md`, `…/3f-fread.md`,
  `docs/api/bios/04-rwabs.md` — NewTOS-Abschnitte auf den tatsaechlichen
  Fehlerweg nachgezogen.

**Kernel**
* `kernel/src/hal/block.rs` — `BlockError::Card`, `BlockError::OutOfRange`.
* `kernel/src/drivers/bcm2711/emmc.rs` — R1-Konstanten + `R1_ERROR_MASK`,
  `R1_BIT_NAMES`, `check_r1()`, Auswertung in `read_one`/`write_one`,
  `card_address()`, `recover_no_abort()` / `recover_inner()`,
  `soft_reset()` bitweise, `ErrorStats`-Felder.
* `kernel/src/host_env.rs` — `block_error_code()`, `BlockDev`-Adapter auf
  `Result<(), Long>`.
* `kernel/src/selftest_emmc.rs` — `r1_rejection_test()` (im bestehenden,
  abschaltbaren Feature `selftest-emmc`).

**tos-api**
* `tos-api/src/bios/block.rs` — Trait auf `Result<(), Long>`, `BlockResult`.
* `tos-api/src/bios/mod.rs` — `Rwabs` reicht den Treibercode durch.
* `tos-api/src/bios/bpb.rs`, `…/drive.rs` — Aufrufstellen nachgezogen.
* `tos-api/src/bdos/fs/cache.rs` — `evict` fallibel, `write_back`,
  `LAST_ERROR`/`clear_error`/`take_error`, `WRITEBACK_ATTEMPTS`,
  `writeback_stats()`, `flush() -> Result`.
* `tos-api/src/bdos/fs/file.rs`, `…/dirops.rs` — neun `flush()`-Stellen +
  `fwrite`/`fread` tragen den Code.
* `tos-api/src/gemdos/mod.rs` — `clear_error()` am Anfang des Dispatchs.

**Tests**
* `tos-api/tests/gemdos_fs_device_error.rs` — neu, 5 Tests (Abnahme).
* `tos-api/tests/fatvol/mod.rs` — Fehlereinspeisung im `RamDisk`
  (`fail_write_at`, `fail_read_at`, `disarm`, `refused_writes`).
* `tos-api/tests/aes_rsrc_{global_fields,load_path,short_blob}.rs` — die
  eigenen `BlockDev`-Attrappen an die neue Signatur angepasst, Verhalten
  unveraendert.

---

## Prueflauf

| Pruefung | Ergebnis | Basislinie |
|---|---|---|
| `cargo xtask build --board rpi4` (frisch) | tos-api **64**, kernel **94** | 64 / 94 |
| `cargo xtask build --board qemu-virt` (frisch) | tos-api **64**, kernel **94** | 64 / 94 |
| `cargo clippy -p kernel -F board-rpi4` | tos-api **80**, kernel **125** | 80 / 125 |
| `cargo clippy -p kernel -F board-qemu-virt` | tos-api **80**, kernel **125** | 80 / 125 |
| `cargo clippy -p kernel -F board-rpi4,selftest-emmc` | 80 / 125 | — |
| `cargo clippy -p tos-api --tests` | keine Warnung in den beruehrten Testdateien | — |
| `cargo run -p xtask -- test` | **961 / 0 / 1** | 956 / 0 / 1 (+5 neu) |
| `sh tools/desk-drift-probe.sh` | **32** | 32 |
| Boot board-rpi4 (QEMU raspi4b, mit Karte) | `[desk] desktop up (1 drives …)`, 0 PANIC | raw/04 |
| Boot board-qemu-virt | `[desk] desktop up (0 drives …)`, 0 PANIC | raw/05 |
| Selftest `selftest-emmc` in QEMU raspi4b | R1-Ablehnung erkannt, Karte danach voll benutzbar | raw/03 |

`sched_pexec_parent_suspend` ist in keinem der Laeufe durchgefallen (T11 nicht
aufgetreten).

---

## Was die Abnahme des Tickets verlangt hat

> Ein erzwungener Schreibfehler muss bis zur Anwendung durchschlagen und dort
> den dokumentierten TOS-Fehlercode liefern — als Test, nicht als Behauptung.

`tos-api/tests/gemdos_fs_device_error.rs::fwrite_reports_ewritf_when_the_sector_does_not_reach_the_card`
— `Fwrite` liefert **-10 (`EWRITF`)**. Vorher lieferte derselbe Aufruf die
volle Byte-Zahl. Dazu `Fread` → **-11 (`EREADF`)** und `Rwabs` → der Code des
Treibers.

> Kein Sektor darf bei fehlgeschlagenem Rueckschreiben aus dem Cache
> verschwinden.

`a_failed_write_back_keeps_the_sector_and_lands_it_afterwards` — der Sektor
ueberlebt den Fehler, ueberlebt einen vollen Verdraengungsumlauf und landet
danach byte-genau auf der Karte.

> Das Audit hat "Schreiben hinter das Kartenende liefert Ok(())" getrieben —
> das ist ein fertiger Hebel.

Benutzt: derselbe Hebel, `block_count() + 4096`, jetzt als dauerhafter Teil
des `selftest-emmc`-Laufs. Ergebnis `Err(Card)` statt `Ok(())`,
`measurement-2026-09-27-hw03/raw/03`.

---

## Was an der echten Pi zu pruefen bleibt (und was das Richtige waere)

Nichts davon blockiert HW03; alle drei sind aus dem Bootlog ablesbar.

**M-A — welches R1-Bit die echte Karte setzt.** Bau mit
`--features selftest-emmc`, Serielle lesen. *Richtig:* eine Zeile
`[emmc] CMD17 rejected by card: status=0x…` mit `OUT_OF_RANGE` (Bit 31, so
sagt es Tabelle 4-42) **oder** `ADDRESS_ERROR` wie in QEMU; danach laeuft der
Selftest weiter (Partition gemountet, `Rwabs -> 0`). *Falsch:*
`OK (accepted!)` — dann sieht EMMC2 den Fehler anders; oder der Selftest
bleibt stehen, dann steht `recovery incomplete, STATUS=0x…` im Log und der
Wert sagt, welche Leitung haengt.

**M-B — ob die zwoelf Fehlschlaege vom 2026-09-25 jetzt eine Zeile erzeugen.**
Dieselbe Kopiergeste auf der kalten Karte, Serielle mitschreiben. *Richtig
(wahrscheinlichster Fall):* gar keine Fehlerzeile, weil FS04 das Wartebudget
bereits auf eine echte Zeit gestellt hat. *Ebenfalls richtig:* je Fehlschlag
eine `[emmc] write_done timed out after 1000 ms`-Zeile — und falls einer bis
oben durchschlaegt, meldet der Desktop den Fehler, statt die Kopie als
gelungen zu zeigen. *Falsch:* Sekunden Wartezeit ohne jede Zeile — dann ist
oben noch ein Schlucker.

**M-C — ob der FS04-Wiederherstellungspfad auf EMMC2 zurueckstellt** (offen
wegen Nebenbefund N1). Aus demselben Log: nach einer
`[emmc] … timed out`-Zeile darf **keine** `recovery incomplete`-Zeile folgen,
und der naechste Zugriff muss normal schnell sein.

---

## Uebergabe an den Tester

* **Was diese Aenderung ausuebt:** jeder Schreibpfad auf C: — Desktop-Kopie,
  `Fcreate`/`Fwrite`/`Fclose`, `Dcreate`/`Ddelete`, Umbenennen. Erwartung:
  **unveraendertes** Verhalten im Gutfall; die neuen Rueckgabewerte treten nur
  auf, wenn das Geraet wirklich fehlschlaegt.
* **Boards:** beide, frisch je Board gebaut (Footgun mit dem gemeinsamen
  `target/.../debug/kernel`).
* **Neu verdrahtet:** `tos-api/tests/gemdos_fs_device_error.rs` laeuft in
  `cargo run -p xtask -- test` mit (956 → 961).
* **Zusaetzlich sinnvoll, aber optional:** ein Lauf mit
  `--features selftest-emmc` in QEMU `raspi4b`; die Zeile
  `R1 out-of-range probe … OK (refused)` ist die Regressionswache fuer
  Befund 3. Ohne das Feature wird der Pfad nicht betreten.
* **`fsck_msdos`:** besonders relevant, weil der Cache jetzt Sektoren
  zurueckhalten kann, die er frueher weggeworfen haette. Erwartung: sauber wie
  bisher — im Gutfall aendert sich an der Schreibreihenfolge nichts.
* **Nur echte Pi:** M-A, M-B, M-C oben.
