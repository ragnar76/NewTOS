# FS04 (EMMC-Teil) — STATUS: DONE (2026-09-25) — Audit geliefert, Pi-Gegenprobe benannt

**Auftrag:** `kernel/src/drivers/bcm2711/emmc.rs` und den Pfad darueber gegen
die **Primaerquellen** halten (Track B: BCM-Datenblatt, SD Physical Layer
Simplified Spec, SD Host Controller Simplified Spec) und die Frage beantworten:
**macht die Karte uns warten, oder warten wir falsch?** Nicht beheben.

**Stand: beides, und quantitativ.** Die Karte braucht auf der Pi gemessen bis
**170 ms fuer einen einzelnen kalten Sektor** — das ist spezifikationskonform
(SD Phys §4.6.2.2 erlaubt 250 ms). Unser Wartebudget `SPIN_LIMIT` ist **kein
Zeitmass, sondern ein Schleifenzaehler**, der auf derselben Pi je nach CPU-Takt
**104 bis 200 ms** entspricht. In Block 2 der Messung war er kuerzer als die
Kartenlatenz: **12 von 23 Sektorschreibungen sind leer ausgelaufen**, ohne
Controller-Reset, ohne Abort, und der Blockcache hat sie stillschweigend
wiederholt. Rund **2,0 s der 2,18 s Einfrieren** sind damit aufgeloest.

**Kein Produktcode geaendert.** Basis: Branch `desk-rebuild`, HEAD `ec5cf47`.
Nichts geflasht, nichts ausgeworfen, keine Pi gefahren, `/Volumes/BOOT` nicht
beruehrt, nichts committet, kein `git checkout/restore/clean/stash/reset`.

Lieferung: [`docs/api/hw/emmc-audit.md`](../docs/api/hw/emmc-audit.md) —
Phase-fuer-Phase-Abgleich, die fuenf Fragen, 12 Befunde nach Schwere,
belegt gegen vermutet getrennt, und die Pi-Messungen mit der jeweils
entscheidenden Zahl.

---

## 1. Was die Messung hergab, als man sie auszaehlte

Die 14 Bloecke aus `measurements/measurement-2026-09-25-fs04/raw/pi04-FS04.TXT` wurden
maschinell ausgewertet. Drei Dinge kamen dabei heraus, die im Ticket noch als
offen oder als Messfehler stehen:

**1. Der Zaehlerabgleich ist kein Messfehler der Sonde, sondern der Befund.**
`emmc.spin calls` ist in **allen 14** Bloecken exakt
`5 × (read_blocks + write_blocks)` — jede Sektoroperation hat alle fuenf
Warteschleifen durchlaufen. Also kann ein Abbruch nur in der letzten gewesen
sein, dem `DATA_DONE`-Warten. Und dort: `dev.w.pio n = 23`, aber
`dev.w.data_done n = 11`. **12 Schreibvorgaenge sind mit Fehler
zurueckgekommen** — in Block 2 und in keinem anderen Block.

**2. `SPIN_LIMIT` laesst sich aus der Messung in Zeit umrechnen.**
`emmc.spin max/call` und der groesste Einzelzeitwert desselben Blocks gehoeren
zur selben Schleife. Dreizehn Bloecke liefern **200,0 ns je Durchlauf** (Block
4: 850 010 Durchlaeufe ↔ 170 002 µs; Block 3: 11 443 ↔ 2 288; usw.), Block 1
liefert **106 ns**. Also: **1 000 000 Durchlaeufe = 104–200 ms**, abhaengig vom
CPU-Takt.

**3. Die im Ticket ausdruecklich nicht gedeutete Luecke schliesst sich auf
1,1 %.** Block 2 hat 19 749 327 Durchlaeufe, die gemessenen Warteschleifen
summieren 807 404 µs. Mit „die 12 Abbrueche waren volle Timeouts" bleiben
7 749 327 Durchlaeufe fuer die gemessene Zeit → 104,2 ns je Durchlauf,
unabhaengig bestaetigt durch die 106 ns aus Block 1. Dann:
`12 × 1 000 000 × 104,2 ns = 1 250 ms` plus `807 ms` = **2 058 ms** gegen
gemessene `gemdos.dispatch = 2 080 ms`. Die Gegenhypothese „schnelle
`Err(Io)`" fordert 40,9 ns je Durchlauf und widerspricht 13 Bloecken; 11 oder
13 Timeouts weichen um 260 bzw. 290 ms ab. **12 ist die einzige konsistente
Loesung — genau die Zahl, die auch der Zaehlerabgleich liefert.**

**Und warum Block 4 nicht scheiterte, obwohl er derselbe kalte Fall war:**
dort galten 200 ns/Durchlauf, Budget ~200 ms, laengste Kartenlatenz 170 ms —
es passte knapp (`max/call = 850 010` von 1 000 000). **Der Unterschied
zwischen Block 2 und Block 4 ist nicht die Karte, sondern der CPU-Takt.**

---

## 2. Die fuenf Fragen, kurz

1. **Worauf wartet jede Warteschleife?** Fuenf Schleifen je Sektor; die Bits
   sind **richtig** (Command Inhibit (CMD)/(DAT), Command Complete, Buffer
   Read/Write Ready, Transfer Complete). `wait_dat` ist Present State Bit 1 und
   wartet inhaltlich auf die **Programmierzeit der vorigen Schreibung**;
   `data_done` ist Transfer Complete und ist beim Schreiben **dieselbe
   Groesse** (SD-HC §2.2.17). Falsch ist nicht das Bit, sondern das Budget.
   *Eine Einschraenkung aus Broadcoms eigenem Text:* STATUS ist laut BCM2835
   Kap. 5 S. 72 ausdruecklich *nicht* zum Pollen gedacht.
2. **Der ausgeschoepfte Spin?** `Err(Timeout)` — **ohne** `SRST_CMD`/
   `SRST_DATA`, ohne CMD12, ohne `INTERRUPT` zu loeschen (SD-HC §3.10.1/§3.8
   verlangen alles drei). Darueber wird der Fehler zu `bool`
   (`host_env.rs:401`), im Cache zu einer stillen Wiederholung
   (`cache.rs:128`) bzw. bei der Verdraengung zu **Datenverlust**
   (`cache.rs:67-69`), und alle neun `cache::flush()`-Aufrufstellen verwerfen
   das Ergebnis. **`EWRITF` gibt es in `tos-api/src` nur im `Rwabs`-Pfad.**
3. **Initialisierung?** **ACMD6 wird nie gesendet** (1-Bit-Bus, in QEMU am
   `CONTROL0` nachgemessen), **CMD6/High Speed nie**, die **74 Takte vor CMD0
   fehlen**, der Basistakt wird bei `CAPABILITIES[15:8] == 0` auf **100 MHz
   geraten** (statt der vorgeschriebenen Mailbox-Abfrage, die in
   `mailbox.rs:419` bereits existiert), und nach CMD7 (R1b) wird der **Takt
   gestoppt, ohne den Busy abzuwarten** (SD-HC §3.2.2 verbietet das). Die
   Teilerarithmetik selbst ist im 10-Bit-Modus korrekt — im 8-Bit-Modus liefe
   die Identifikation mit **781 kHz statt 400 kHz**, und die Moduslage wird nie
   abgefragt.
4. **Einzel- statt Mehrblock?** Der Controller kann CMD18/CMD25; wir schleifen
   sektorweise. Broadcom selbst: Einzelblock „resulting in long dead times"
   (BCM2835 Kap. 5 S. 66). **Erklaert den Kleindateifall nicht und wird hier
   auch nicht dazu gestreckt** — `blk.*_blocks max/call = 1` in allen 14
   Bloecken, der Begrenzer ist der Sektorcache, nicht der Treiber.
5. **Was ist am ersten Zugriff anders?** **In unserem Code fast nichts**: eine
   Bootsektorlesung (`Volume::mount` → `bpb::geometry`) und ein kalter
   16-Platz-Cache, zusammen einstellige Millisekunden. `bios::mount()` laeuft
   **nur im Bootpfad**, `emmc::init()` ist idempotent — eine
   Neuinitialisierung je Zugriff findet nicht statt. Anders ist **die Karte**:
   erste Schreibung in einen kalten Bereich 85–170 ms je Sektor, jede weitere
   ~2 ms. Faktor 40–80, spezifikationskonform.

---

## 3. Befunde, nach Schwere (Langfassung im Audit)

| # | Befund | Urteil |
|---|---|---|
| 1 | `SPIN_LIMIT` ist ein Zaehler, kein Timeout, und entspricht 104–200 ms — **unter** den von SD Phys §4.6.2.2 erlaubten 250 ms Karten-Busy (empfohlen >500 ms) | **BELEGT**, Fehler |
| 2 | Nach Fehler/Timeout kein `SRST_CMD`/`SRST_DATA`/CMD12; Controller bleibt stehen, das naechste Kommando zahlt den Rest-Busy | **BELEGT** (1 von 14 Bloecken mit `w.wait_dat ≠ 0` — genau der mit den 12 Abbruechen) |
| 3 | Die R1-Antwort der Karte wird **nie** geprueft | **BELEGT, getrieben**: Schreiben hinter das Kartenende meldet `Ok(())` |
| 4 | `cache::evict` verwirft das Schreibergebnis und setzt `dirty = false` **bedingungslos** → stiller Sektorverlust | **BELEGT (Quelltext)**, Datenverlust |
| 5 | Geraetefehler erreichen GEMDOS nie (`is_ok()`, `write()` immer `true`, 9/9 `flush()`-Ergebnisse verworfen) | **BELEGT (maschinell)** |
| 6 | 74 Takte vor CMD0 fehlen | vermutet |
| 7 | Taktstopp unmittelbar nach CMD7 (R1b), ohne Busy-Pruefung | vermutet |
| 8 | `Transfer Complete` + `Data Timeout Error` wird als Fehler gemeldet, obwohl die Spec „completed" sagt | vermutet |
| 9 | Basistakt geraten (100 MHz), `set_clock` kann gar keinen Fehler melden; Mailbox-Abfrage existiert und wird nicht benutzt | **BELEGT (Quelltext)**, **moegliche Zweitursache der 12 Fehlschlaege** |
| 10 | Teilermodus (HC-Version) wird nie abgefragt | vermutet, in QEMU folgenlos (SDHCI 3.00) |
| 11 | ACMD41-Schleife ohne Zeitgrenze (~10 s statt >1 s) | belegt, klein |
| 12 | Fehlerbits werden geloescht statt gemeldet — **blockiert die naechste Messung** | belegt |
| S1 | Die SD-Spezifikationen liegen **nicht** in `references/`, und `docs/hardware/emmc-sd.md` nennt **keine** Zeitgrenzen. Die Wurzel von Befund 1. **An den Researcher.** | belegt |

**Ausdruecklich entlastet:** 1-Bit-Bus, fehlendes High Speed, Einzelblock,
kein DMA/IRQ — reale Durchsatzabweichungen, aber **nicht** die Ursache von
FS04 (Datenphase 1–4 µs schreibend, 0,5–1,1 ms lesend je Sektor). Ebenso
entlastet: Neuinitialisierung je Zugriff (findet nicht statt).

---

## 4. Was auf der Pi zu messen ist

Vier Punkte, Einzelheiten im Audit §5. Der billigste zuerst:

* **M3 (entscheidet alles, eine Zeile):** `SPIN_LIMIT` auf 10 000 000 setzen
  (≈ 1–2 s), **dieselbe Geste** fahren. *Wenn Befund 1 stimmt:*
  `dev.w.data_done n` = `dev.w.pio n`, `blk.write_blocks calls` faellt von
  **23 auf ~11**, `dev.w.wait_dat` faellt auf **0**, und die Wandzeit von
  Block 2 **halbiert sich auf ~1,0–1,2 s**. Bleibt sie bei 2,2 s, war die
  Kartenlatenz wirklich die ganzen 2 s und die Arbeit verschiebt sich auf
  Mehrblock/4 Bit/Cache.
* **M2 (trennt Befund 1 von Befund 9):** in `wait_interrupt` den Ausgang
  getrennt zaehlen — `Ok`, `Err(Io)` **mit den gesetzten Fehlerbits**
  (16 CTO, 17 CCRC, 20 DTO, 21 DCRC, 22 DEND), `Err(Timeout)` mit
  Durchlaufzahl. Timeouts mit 1 000 000 ⇒ Befund 1. DCRC/DEND ⇒ Takt.
  DTO ⇒ der Controller selbst.
* **M1 (entscheidet Befund 9 und 10):** einmal `SLOTISR_VER`,
  `CAPABILITIES`, `CONTROL0`, `CONTROL1` beider Basen ausgeben, dazu
  `mailbox::get_clock_rate()`. `SLOTISR_VER` Bits 23:16 < 0x02 ⇒ Befund 10 ist
  real. `CAPABILITIES[15:8] = 0` und Mailbox ≠ 100 MHz ⇒ wir fahren den Bus
  mit dem falschen Takt.
* **M4 (nur falls M1 einen falschen Takt zeigt):** dieselbe Geste mit
  `set_clock` auf 12,5 MHz.

---

## 5. Was getrieben wurde, und was nicht

**Getrieben**

* **QEMU `raspi4b`, Release-Build, temporaerer Registerdump** in
  `kernel/src/selftest_emmc.rs` (Feature `selftest-emmc`):
  `SLOTISR_VER=0x24020000` (SDHCI 3.00), `CAPABILITIES=0x052134b4`
  (Basis 52 MHz, TMCLK 52 MHz), `CONTROL0=0x00000f00` (Power+3,3 V,
  **DWIDTH=0, HS=0**), `CONTROL1=0x000e0207` (`DATA_TOUNIT=0xE`, Teiler 2 →
  **13 MHz**, nicht 25). Damit ist der 1-Bit-Betrieb **gemessen**, nicht
  hergeleitet, und der Controller-Datentimeout (2,58 s) ist nachweislich
  laenger als unser Softwarebudget — der Abbruch ist **unsere** Entscheidung.
* **Fehlerpfad getrieben:** Schreiben auf LBA 135 168 einer 131 072-Bloecke-
  Karte → `Ok(())`. Befund 3.
* **Teilerarithmetik getrieben:** Host-Replik von `set_clock` ueber alle
  plausiblen `CAPABILITIES`-Werte; zeigt die 781-kHz-Falle im 8-Bit-Modus und
  die 13/15,75-MHz-Faelle.
* **Die Pi-Messung ausgezaehlt**, nicht nur gelesen (§1).
* **Maschinell geprueft:** alle neun `cache::flush()`-Aufrufstellen,
  das Vorkommen von `EWRITF`/`EREADF` in `tos-api/src`, alle Aufrufstellen von
  `bios::mount()`/`log_out_all`.

**Nicht getrieben (offen benannt)**

* **Jede Aussage ueber EMMC2 auf echter Pi-Hardware.** QEMU haengt die Karte
  an den Legacy-EMMC (`0xFE300000`); die Zahlen aus dem Registerdump sind
  QEMUs, nicht die der Pi. Deshalb sind Befund 9 und 10 *vermutet*, mit M1 als
  Entscheidung.
* **Die Fehlerart der 12 Fehlschlaege.** Die Bilanz in §1 sagt „Timeout" mit
  1,1 % Abweichung und schliesst die Alternativen rechnerisch aus — aber
  gesehen hat es niemand, weil `wait_interrupt` die Fehlerbits loescht
  (Befund 12). M2 entscheidet es.
* **Befunde 6, 7, 8** sind Quelltext gegen Spezifikation; ihre Wirkung ist
  nicht gemessen. Sie erklaeren FS04 nicht, sie stehen fuer sich.
* **Der Effekt selbst** ist in QEMU nicht nachstellbar (schon Phase 1,
  `dc8208f`); die Karte dort hat keine Programmierzeit.

---

## 6. Rueckbau

Temporaere Verdrahtung ausschliesslich in `kernel/src/selftest_emmc.rs`,
hinter dem bestehenden Feature `selftest-emmc`; **von Hand zurueckgeschrieben**,
`git status` zeigt die Datei wieder unveraendert. Kein
`git checkout/restore/clean/stash/reset`, kein Commit. Produktcode nicht
angefasst, `TASK/FS04.md` und `docs/STATUS.md` nicht angefasst.

Neu im Baum: `docs/api/hw/emmc-audit.md` und diese Datei.
Beruehrte ignorierte Artefakte: `target/` und `kernel8.img` im
Wurzelverzeichnis (zuletzt planer `--board rpi4 --release`-Build **ohne**
Selftest-Feature) — bitte je Board frisch neu bauen.
