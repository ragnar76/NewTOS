# FS04 (EMMC-Fix) — STATUS: DONE (2026-09-25) — Befunde 1, 2, 12 + M1-Diagnose

**Auftrag:** Befund 1 (echter Zeit-Timeout statt Schleifenzaehler), Befund 2
(Wiederherstellung nach Fehler/Timeout) und Befund 12 (Fehlerbits melden statt
loeschen) aus [`docs/api/hw/emmc-audit.md`](../docs/api/hw/emmc-audit.md),
dazu die Bootdiagnose M1. **Nichts sonst.**

**Geaendert:**

| Datei | Was |
|---|---|
| `kernel/src/drivers/bcm2711/emmc.rs` | Zeitbudgets je Wartephase, Fehlerwiederherstellung, Fehlerbuchhaltung, M1-Ausgabe, Unit-Tests |
| `kernel/src/drivers/bcm2711/mailbox.rs` | `CLOCK_ID_EMMC = 1`, `CLOCK_ID_EMMC2 = 12` (nur fuer M1) |

Nicht angefasst: `docs/STATUS.md`, `TASK/FS04.md`, Befunde 3, 4, 5 und 6–11.
Nichts committet, nichts geflasht, keine Pi gefahren, kein
`git checkout/restore/clean/stash/reset`. Scratch lag im Session-Scratchpad.

---

## 1. Befund 1 — der Timeout ist jetzt eine Dauer

`SPIN_LIMIT` ist aus den fuenf Sektor-Warteschleifen verschwunden. An seiner
Stelle steht `Budget`, das `CNTPCT_EL0`/`CNTFRQ_EL0` liest — dieselbe Quelle,
aus der `drivers::timer` den 200-Hz-Takt zieht. Der Ablauf ist
`now.wrapping_sub(start) >= limit`, also nur die **Differenz** zweier
Zaehlerstaende: unabhaengig vom CPU-Takt und ueber den Zaehlerueberlauf hinweg
richtig. Laeuft der Zaehler schmaler als 64 Bit und rollt mitten im Warten um,
endet das Warten zu frueh — ein Timeout, den die Wiederherstellung auffaengt,
kein Haenger; bei 54 MHz ist das Jahrzehnte entfernt.

`SPIN_LIMIT` steht noch genau dort, wo es zu **anderen** Tickets gehoert:
Controller-Reset, `CLK_STABLE`, ACMD41-Schleife (Befunde 9 und 11). Das ist
Absicht und im Quelltext so benannt.

**Welche Quelle welche Grenze lizenziert** — die SD-Spezifikationen liegen
nicht in `references/` (Befund S1), deshalb ist jedes Zitat ueber das Audit
belegt, das es woertlich fuehrt:

| Konstante | Wert | Lizenz |
|---|---|---|
| `WRITE_BUSY_MS` | **1000 ms** | SD Phys 6.00 §4.6.2.2: 250 ms Busy je Schreibvorgang, **500 ms** fuer den letzten Busy; Application Note: *„strongly recommended for hosts to implement **more than 500 ms** timeout value"* (Audit §3.1). 1000 ms ist dieses „mehr als 500 ms" mit Faktor 2 Reserve ueber der laengsten von der Spec erlaubten Kartenlatenz. |
| `READ_MS` | **200 ms** | SD Phys 6.00 §4.6.2.1: *„The host should use **100 ms timeout (minimum)** for single and multiple read operations"* (Audit §3.1). Das ist eine **Untergrenze**, und das Audit rechnet es dem alten Budget an, dass es „genau auf der Untergrenze" lag — also derselbe Faktor 2 wie beim Schreiben. Gemessene Lesewartezeiten auf der Pi: 0,5–1,1 ms. |
| `CTRL_MS` | **= `WRITE_BUSY_MS`** | Fuer Command Inhibit (CMD/DAT-Kommandoseite), Command Complete und Buffer Write Ready gibt die Spec **keine** kartenseitige Grenze: die Antwortzeit ist mit N_CR ≤ 64 Takten gebunden und der Controller meldet sie selbst als `Command Timeout Error` (SD-HC §2.2.18). Die Softwaregrenze bindet nur einen **haengenden Controller** und ist deshalb auf die laengste kartenseitige Wartezeit gesetzt, die die Spec irgendwo erlaubt. |
| `RECOVERY_MS` | **100 ms** | Keine Spec-Dauer; bindet nur die Wiederherstellung selbst (Software-Reset, CMD12), damit sie nicht der naechste Haenger wird. Die gepinnte RPi4-Referenz wartet auf dieselben Reset-Bits 1 s (`references/circle/addon/SDCard/emmc.cpp:945,960`). Laeuft sie leer, wird „Non-recoverable" gemeldet — genau der Ausgang aus §3.10.1. |

Zuordnung Phase → Grenze (`Phase::limit_ms`):
`DatInhibit`, `WriteDone` → `WRITE_BUSY_MS`;
`ReadRdy`, `ReadDone` → `READ_MS`;
`CmdInhibit`, `CmdDone`, `WriteRdy` → `CTRL_MS`.
`DatInhibit` bekommt bewusst die Schreibgrenze: was dort abgewartet wird, ist
die Programmierzeit der **vorigen** Schreibung (Audit §2.4).

**Fuer die Messung wichtig:** ein Zaehler `emmc.spin max/call` von ~850 000
oder 1,7 Mio. ist ab jetzt **normal** und kein Timeout mehr — er ist nur noch
die natuerliche Durchlaufzahl der laengsten Wartezeit. Ob etwas leer
ausgelaufen ist, sagt `emmc::error_stats().timeouts` / `timeouts_by_phase`.

## 2. Befund 2 — Wiederherstellung statt Weiterhumpeln

Jeder Fehlerausgang **und** jeder Zeitablauf geht jetzt durch `Emmc::recover`
(SD-HC 3.00 §3.10.1 Error Interrupt Recovery, §3.8 Abort Transaction):

1. Fehlerstatus ist gelesen und gesichert (Befund 12), `INTERRUPT` wird per
   W1C geleert, damit kein Restbit das naechste Kommando stoert.
2. Haengt die Transaktion noch auf der DAT-Leitung (`DatInhibit`, `ReadRdy`,
   `WriteRdy`, `ReadDone`, `WriteDone`), wird **CMD12** als Abbruchkommando
   abgesetzt — mit `Command Type = abort` (CMDTM 23:22 = 11b), dem einen
   Kommandotyp, den der Host bei gesetztem Command Inhibit (DAT) absetzen
   darf (`references/circle/addon/SDCard/emmc.cpp:133`, `:284`, `:988-991`).
   CMD12 laeuft **nicht** durch `command()` — das oeffnete ein zweites
   Vollbudget und liefe rekursiv wieder in die Wiederherstellung.
3. `SRST_CMD` / `SRST_DATA` nach §3.10.1: die Leitung, deren Fehlerbit gesetzt
   war; bei einer abgebrochenen Datentransaktion nach §3.8 **beide**.
4. Danach muessen beide Command-Inhibit-Bits wieder frei sein. Sind sie es
   nicht, zaehlt das als §3.10.1 „Non-recoverable Error" und wird gemeldet.

Damit faellt die Kette aus §3.2 des Audits weg: leer ausgelaufenes Warten →
Controller bleibt mitten im Transfer → naechstes Kommando zahlt den Rest des
Busy in `wait_dat`.

## 3. Befund 12 — die Bits werden gemeldet, nicht vernichtet

Neu oeffentlich: `emmc::error_stats() -> ErrorStats` mit
`last_status`, `last_phase`, `bits[7]` (`ERROR_BIT_NAMES` = CTO 16, CCRC 17,
CEND 18, CBAD 19, DTO 20, DCRC 21, DEND 22 — SD-HC §2.2.18, Bitlage gegen
`emmc.cpp:153-165` geprueft), `other`, `io_errors`, `timeouts`,
`timeouts_by_phase[7]` (`PHASE_NAMES`), `recoveries`, `nonrecoverable`.

Dazu eine Konsolenzeile je Ereignis — **aber erst, wenn eine Karte steht**:
waehrend der Basen-Probe beantwortet ein Schacht **ohne** Karte jedes
Antwortkommando mit CTO, und das ist die Probe bei der Arbeit, kein Fehler.
Was die Identifikation getroffen hat, steht einmal in der M1-Zeile
(in QEMU: `2 error(s), 0 timeout(s), 2 recovery(ies) CTO=2` — die beiden
CTOs des kartenlosen EMMC2, vorher unsichtbar).

## 4. M1 — die Bootdiagnose

Einmal je Boot, nach erfolgreicher Identifikation, fuer die **benutzte** Basis.
In QEMU `raspi4b` gemessen:

```
[emmc] M1 SLOTISR_VER=0x24020000 (SDHCI spec 0x02) CAPABILITIES=0x052134b4 (base 52 MHz)
[emmc] M1 CONTROL0=0x00000f00 CONTROL1=0x000e0207 div=2 -> SDCLK ~13000000 Hz
[emmc] M1 mailbox clock rate: EMMC(1)=50000000 Hz EMMC2(12)=700000000 Hz
[emmc] M1 during identification: 2 error(s), 0 timeout(s), 2 recovery(ies) CTO=2
```

Das deckt sich Wort fuer Wort mit dem temporaeren Registerdump des Audits
(§5). **Gemeldet, nicht gehandelt** — Basistakt (Befund 9) und Teilermodus
(Befund 10) sind eigene Tickets.

Was die Zahlen auf der **Pi** entscheiden:

* `SLOTISR_VER[23:16]` ≥ 0x02 ⇒ SDHCI 3.00, Befund 10 erledigt; 0x00/0x01 ⇒
  die Identifikation lief mit 781 kHz statt 400 kHz.
* `CAPABILITIES[15:8]` = 0 ⇒ unser geratener 100-MHz-Ruecksprung greift (die
  Zeile sagt dann „guessed 100"), und die Mailboxzahl sagt, ob er stimmt.
* `div` / `SDCLK` — erwartet ~25 MHz. Alles darunter erklaert nebenbei die
  Lesezeiten, alles darueber ist ein Spec-Verstoss.
* **Vorsicht bei der Mailboxzahl:** QEMU meldet fuer EMMC2 700 MHz, was als
  SD-Basistakt unplausibel ist. Auf der Pi gehoert sie gegen
  `CAPABILITIES`/`SLOTISR_VER` gehalten, nicht blind uebernommen.

---

## 5. Was auf der Pi zeigen wuerde, dass es gewirkt hat

Die Vorhersage des Auditors (§5 M3) gilt fuer diese Aenderung **mit einer
Praezisierung**, weil M3 nur den Zaehler hochsetzt, hier aber eine echte Zeit
und dazu die Wiederherstellung greift:

| Messgroesse | Vorher (Block 2) | Erwartet | Warum |
|---|---|---|---|
| `dev.w.data_done` n | 11 bei 23 `dev.w.pio` | **gleich `dev.w.pio` n** | 1000 ms > die gemessenen 170 ms Kartenlatenz; kein Warten laeuft mehr leer aus |
| `blk.write_blocks calls` | 23 | **~11** | die 12 stillen Wiederholungen des Blockcaches entfallen |
| `dev.w.wait_dat` | 537 ms | **0** | das Transfer Complete wird zu Ende abgewartet, der Folgebefehl findet DAT frei |
| `emmc::error_stats().timeouts` | (gab es nicht) | **0** | der direkte Beleg — nicht mehr ueber `spin max/call` erschliessen |
| `emmc.spin max/call` | 1 000 000 (ausgeschoepft) | **~0,85–1,7 Mio., aber kein fester Wert** | der Zaehler ist kein Limit mehr; gleiche Zahl bei verschiedenen CPU-Takten ist gerade **nicht** mehr zu erwarten |
| Wandzeit Block 2 | 2 182 ms | **~1,0–1,2 s** | Audit §3.2: 1,25 s waren leeres Wartebudget |

**Sollte meine Umsetzung das liefern? Ja** — der Wirkmechanismus ist derselbe
wie in M3 (Budget groesser als die Kartenlatenz), plus die Wiederherstellung,
die den Nachhol-Busy in `wait_dat` entfernt. Die Kartenlatenz selbst wird
**nicht** kleiner; sie wird nur nicht mehr verdoppelt.

**Bleibt die Wandzeit bei ~2,2 s und sind die Fehlschlaege trotzdem weg**
(`timeouts = 0`, `data_done n = pio n`), dann war die Kartenlatenz wirklich
die vollen 2 s und die 12 Wiederholungen waren billig — die Karte hatte beim
zweiten Anlauf schon programmiert. Dann ist Befund 1 zwar sauber behoben, aber
die Sekunden liegen bei der Zahl der Einzelsektor-Schreibungen: Mehrblock
(Audit §3.4), 4-Bit-Bus (§3.3) und ein Cache, der zusammenhaengende Laeufe
buendelt. Das waere dann die naechste Arbeit, nicht dieses Ticket.

**Dritter moeglicher Ausgang, und erst jetzt sichtbar:** zeigen die neuen
Zaehler statt Timeouts `bits[DCRC]`/`bits[DEND]` ≠ 0, war es Signalintegritaet
bzw. Bustakt — also Befund 9/10, und die Arbeit verschiebt sich auf die
Initialisierung. Zeigt `bits[DTO]` ≠ 0, hat der Controller selbst abgebrochen
und `DATA_TOUNIT` gehoert geprueft. Das ist die Trennung, die das Audit als
M2 gefordert hat; sie braucht jetzt keine Sonde mehr im Treiber.

---

## 6. Was geprueft wurde

* **Bau, beide Boards, frisch je Board** (`-v` zeigt fuer beide je zwei
  `rustc`-Laeufe, der ELF ist der richtige — der rpi4-eigene String fehlt im
  qemu-virt-ELF): `kernel` **94** Warnungen, `tos-api` **64** — Basiswerte
  unveraendert.
* **Clippy, beide Featuresaetze:** `tos-api` **80**, `kernel` **125** —
  Basiswerte unveraendert, **0** Treffer in den beruehrten Dateien.
* **`cargo run -p xtask -- test`:** 944 passed / 0 failed / 1 ignored —
  unveraendert.
* **Unit-Tests der Zeitarithmetik** (10 Stueck: Umrechnung, Nullbudget,
  Ueberlauf, Zaehlerumlauf, Grenze je Phase, Reset-Wahl, Fehlerbitlage,
  Spin-Ruecksprung). **Sie laufen nicht im Workspace-Testlauf** — `kernel` ist
  auf `aarch64-unknown-none-softfloat` festgenagelt und nicht host-linkbar
  (`tools/xtask/src/main.rs:573-582`). Deshalb wurden sie als **wortgleiche
  Kopie** auf dem Host mit `rustc --test` gefahren: 10/10. Der Lauf hat einen
  Fehler gefunden — in der Erwartung des Umlauftests, nicht in der Funktion —
  und der ist korrigiert.
* **QEMU `raspi4b`, Boot:** `[desk] desktop up`, **0** `[EXC]`/`[PANIC]`,
  M1-Zeilen wie oben.
* **QEMU `raspi4b` mit `--features selftest-emmc`:** der volle
  Sektor-/FAT-/GEMDOS-Dateipfad (Sektor-0-Lesung, Schreiben + Ruecklesen,
  Rwabs, Getbpb, Fopen/Fread/Fwrite/Fcreate/Frename/Fattrib/Fdatime,
  Verzeichnisse) laeuft durch: **0 Timeouts, 0 Fehler nach der
  Identifikation**, 0 `[EXC]`/`[PANIC]`, Desktop oben.
* **`fsck_msdos -n`** auf der ausgeschnittenen FAT16-Partition des benutzten
  Kartenabbilds nach dem Lauf: sauber (rc 0).

**Nicht geprueft, und das ist der Punkt:** der Effekt selbst. In QEMU hat die
Karte keine Programmierzeit; die Zahlen aus Abschnitt 5 kann nur die Pi
liefern. QEMU gibt hier Regressionssicherheit, keinen Beweis.

## 7. Nebenbeobachtungen (nicht angefasst)

* Der `selftest-emmc`-Lauf meldet ein paar Erwartungen, die zum Inhalt eines
  **anders geseedeten** Kartenabbilds gehoeren (`Fseek(0,END) -> size=32
  (expect 1000)`, `fat_entry(2) -> Some(0) (expect Some(3))`). Vorbestehend,
  nichts mit dieser Aenderung zu tun.
* `kernel8.img` im Wurzelverzeichnis (ignoriertes Build-Artefakt) wurde durch
  die Laeufe ueberschrieben; zuletzt als planer `--board rpi4`-Debug-Build
  **ohne** Selftest-Feature. Wer es braucht, baut je Board frisch.
