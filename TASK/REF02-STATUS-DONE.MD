# REF02 — STATUS: DONE

Datum: 2026-09-28 · Basis: branch `desk-rebuild`, HEAD `6642409`

Nur Dokumentation. Kein Produktcode angefasst, nichts committet, nichts
geflasht, keine Pi gefahren, `/Volumes/BOOT` nicht beruehrt, kein
`git checkout/restore/clean/stash/reset`. `docs/STATUS.md` und
`TASK/REF02.md` sind unveraendert (Vorgabe des Leads).

Netzzugriffe: ausschliesslich sdcard.org — die Downloadseite, die
Archivseite und vier PDFs.

---

## In einem Satz

Die vier SD-Spezifikationen liegen jetzt lokal und sind ueber eine
Pruefsummentabelle im **committeten** `references/README.md` gepinnt; die
Zeitgrenzen stehen mit Abschnittsangabe in `docs/hardware/emmc-sd.md` §7;
die Fassungsfrage ist entschieden **und ausgerechnet**, nicht geschaetzt:
die Physical-Layer-Stellen sind in 6.00 und 9.10 textgleich, die Host-
Controller-Registerabschnitte sind zwischen 3.00 und 4.20 **um eins
verschoben** — deshalb bleibt dort 3.00 die Referenz.

---

## Auftragspunkt 1 — holen und pinnen

**Geholt** (nach `references/sd-spec/`, gitignored, also nur auf diesem
Rechner):

| Rolle | Dokument | Ausgabe | Datum |
|---|---|---|---|
| **Referenz** | Part 1 Physical Layer Simplified Specification | **9.10** | 2023-12-01 |
| **Referenz** | Part A2 SD Host Controller Simplified Specification | **3.00** | 2011-02-25 |
| aktuelle Ausgabe | SD Host Controller Simplified Specification | 4.20 | 2018-07-25 |
| ueberholt, aufbewahrt | Physical Layer Simplified Specification | 6.00 | 2018-08-29 |

Die 9.10 lag schon da (HW03). 4.20 ist die aktuelle Ausgabe von der
Downloadseite, 3.00 und 6.00 von der Archivseite
(`/downloads/pls/archive/`) — die aelteren Ausgaben sind also weiterhin
oeffentlich beziehbar, was die Nachpruefbarkeit der Altzitate sichert.

**Was „pinnen" hier heissen muss.** `/references/*` ist gitignored mit
genau einer Ausnahme: `!/references/README.md`. Eine PDF kann also nicht
gepinnt werden, indem man sie ablegt — und **darf** es auch nicht (siehe
Lizenz). EmuTOS und FreeMiNT werden ueber ihre **SHA** gepinnt, nicht
ueber ihren Inhalt; das Muster ist uebertragbar: fuer eine PDF ist der
Pin **Ausgabe + Datum + kanonische Quelle + SHA-256**, eingetragen in der
einen committeten Datei des Verzeichnisses. Genau das steht jetzt in
`references/README.md` unter „SD specifications", zusammen mit der
Ein-Zeilen-Pruefung (`shasum -a 256 references/sd-spec/*.pdf`). Damit ist
ein Zitat nachpruefbar, ohne dass wir etwas weitergeben.

**Lizenz — was die Bedingungen tatsaechlich sagen** (gelesen auf der
Downloadseite hinter dem Download-Knopf und in der „Conditions for
publication"-Seite der PDFs selbst, 2026-09-28):

* **§2, Lizenz fuer Nicht-Mitglieder** — das sind wir: herunterladen,
  ansehen, „solely for Your individual, internal business use", drucken
  soweit noetig. Ausdruecklich **nicht**: „(a) copy portions of the
  Simplified Specs (whether by screen scraping, screen shots,
  electronically or otherwise) … or (c) distribute the Simplified Specs
  or any portion thereof."
* **§3** gibt das Recht, Auszuege in die eigene Dokumentation zu
  uebernehmen — **nur Mitgliedern** der SDA, und nur mit Copyright-Hinweis
  und Link auf die Downloadseite.
* **§4** verbietet zusaetzlich Unterlizenzierung, Bearbeitung, „framing"
  und „mirroring".
* Die PDF sagt dasselbe: „Distribution of the Simplified Specification,
  other than through the Site, is a violation of the Terms".

**Antwort:** Die PDFs duerfen **nicht** ins Repo und nicht auf einen
eigenen Server — was `/references/*` ohnehin erzwingt. Jeder Mitarbeiter
holt sich seine eigene Kopie von der Site und nimmt dort die Bedingungen
an. Fuer unsere Dokumente gilt: **Zahlen und Abschnittsnummern sind
Fakten und gehoeren hinein, laengere woertliche Uebernahmen sind
„copying portions"**. Danach ist §7 der Hardwarenotiz geschrieben —
eigene Formulierung plus Abschnitt, woertlich nur dort, wo der Wortlaut
traegt, und dann satzweise.

**Ein Punkt fuer den Lead, kein Alleingang meinerseits:** der
`docs/hardware/emmc-sd.md` §5 aus HW03 gibt die **vollstaendige
Description-Spalte der Tabelle 4-42 woertlich** wieder — 15 Zeilen
Originaltext. Das ist nach §2(a)/§4(e) die Sorte Uebernahme, die
Nicht-Mitgliedern nicht erlaubt ist, und das Repo ist oeffentlich. Ich
habe die Tabelle **nicht angetastet** (sie ist fachlich richtig und
HW03s Arbeit); die Entscheidung — so lassen, kuerzen, oder in eigene
Worte fassen — gehoert dem Lead. Das ist meine Lesart der Bedingungen,
keine Rechtsberatung.

**Nicht geholt:** das **BCM2835 ARM Peripherals** Datenblatt (Quelle 2
des Audits, Kap. 5 „External Mass Media Controller"). Es fehlt weiterhin
in `references/`. Ausserhalb des Auftrags dieses Tickets („nichts
anderes geht ins Netz"), deshalb offen gelassen und hier gemeldet.

---

## Auftragspunkt 2 — die Zeitgrenzen

Neu: **`docs/hardware/emmc-sd.md` §7 „Timing limits"**, mit einem
Vorwaertsverweis ganz oben im Dokument und in §4, damit sie findet, wer
eine Warteschleife schreibt. Alles gegen die Quelle geprueft, nicht aus
dem Audit uebernommen:

| Gefragt | Grenze | Abschnitt |
|---|---|---|
| Programmierzeit der Karte | **250 ms** Busy fuer *alle* Schreibvorgaenge (High Capacity) | §4.6.2.2 |
| Busy nach dem letzten Block | **500 ms** — aber ausdruecklich als Zugestaendnis an **SDXC/SDUC**-Karten, Faelle a)–d); fuer SDHC bleibt es bei 250 ms | §4.6.2.2 |
| Lese-Zeitgrenze | **mindestens 100 ms**, statt TAAC/NSAC auszurechnen | §4.6.2.1 |
| Empfehlung an den Host | fester Timeout, **mehr als 500 ms** — „strongly recommended … even if the card indicates the 250 ms maximum busy length" | §4.6.2.2 Application Notes |

Dazu aufgenommen, weil sie zum selben Bild gehoeren: die SDSC-Formeln
(je der *kleinere* der beiden Werte, §4.6.2.1/§4.6.2.2), die
Erase-Schaetzung (Bloecke × 250 ms, §4.6.2.3), der
Controller-Datentimeout `TMCLK × 2^(13+n)` (SD-HC 3.00 §2.2.15) und die
Initialisierungsgrenzen (74 Takte / 1 ms / 250 ms bis VDD min,
§6.4.1.1/§6.4.1.4; ACMD41 „more than 1 second", §4.2.3/§4.2.3.1).

**Eine Praezisierung gegen das Audit:** das Audit zitiert den Fall a)
mit den 500 ms, ohne den einleitenden Satz, der ihn auf SDXC/SDUC
einschraenkt. Fuer das Ergebnis des Audits folgenlos (der Application
Note verlangt die >500 ms unabhaengig von der Kartenklasse), aber in
einer Hardwarenotiz muss die Einschraenkung mit. Sie steht jetzt in §7.1
und als Nachtrag im Audit.

**Und eine Luecke, ehrlich markiert (§7.5):** die Bus-Zeitwerte — `N_CR`,
`N_AC`, die ganze Tabelle „Timing Values" — sind in der *Simplified*
Fassung **nicht enthalten**; §4.12 steht samt allen Unterabschnitten im
Inhaltsverzeichnis als „(Removed in the Simplified Specification)". Die
Aussage „N_CR ≤ 64 Takte", die im Treiberkommentar steht, hat in unseren
gepinnten Dokumenten **keinen Beleg**. Was belegbar ist: der Controller
ueberwacht dieses Intervall selbst und meldet `Command Timeout Error`
(SD-HC 3.00 §2.2.18).

---

## Auftragspunkt 3 — die zwei gemeldeten Fehler

**(a) ACMD41 `0x5100_0000` — die Notiz war falsch, und schaerfer als
gedacht.** Ein Argument mit Spannungsfenster (Bits 23:0) = 0 ist nach
SD Phys 9.10 **§4.2.3.1 (1)** das **„inquiry ACMD41"**: es *startet die
Initialisierung nicht*, liefert nur das OCR — und ignoriert dabei
ausdruecklich die restlichen Bits (31:24), also auch das `HCS`, das der
Wert setzen wollte. Ein Host, der darauf das Busy-Bit abwartet, wartet
ewig. Die Notiz dokumentierte damit nicht ein unvollstaendiges, sondern
ein **funktionsunfaehiges** Kommando. Der Treiber macht es richtig
(`hcs | 0x00FF_8000`). §3 traegt jetzt `0x40FF_8000`, die vollstaendige
Argument-Feldtabelle (Figure 4-3 plus OCR-Belegung aus Tabelle 5-1,
§5.1) und den Korrekturabsatz.

**(b) `0xFE300000` = „sdhost → Wi-Fi" — halb falsch, halb unbelegt,
beides behoben.** Falsch ist der **Name**: SDHOST ist ein anderer
Controller und sitzt an **`0xFE20_2000`**; auf der Pi 4 ist er
`status = "disabled"`. Broadcom selbst haelt die beiden auseinander
(BCM2711 §6.2 Tabelle 102: *IRQ 56 SDHOST* neben *IRQ 62 „EMMC &
EMMC2"*), circle ebenso (`bcm2835.h:256` gegen `:248`). Richtig ist die
**Sache**: an `0xFE30_0000` haengt tatsaechlich das WLAN — belegt aus dem
Geraetebaum, den die **Raspberry-Pi-Firmware selbst mitliefert**
(`bcm2711-rpi-4-b.dtb` von unserer Bootkarte): `mmcnr@7e300000`,
`status = "okay"`, `non-removable`, Kindknoten `wifi@1`
`compatible = "brcm,bcm4329-fmac"`, Pins `sdio_pins` = GPIO 34–39 ALT3.
Der Kartenschacht ist `mmc@7e340000`, `brcm,bcm2711-emmc2`, Alias
`mmc0`, `status = "okay"`. Dazu die Einordnung, was dieser Beleg wert
ist: ein Geraetebaum ist die **Konfiguration der Firmware**, keine
Aussage ueber unveraenderliche Verdrahtung.

**Als Nebenertrag dieser Pruefung eine dritte Korrektur:** die
Quellentabelle der Notiz fuehrte „BCM2711 §1.2 (address map)" als Quelle
der EMMC2-Basis. Das Datenblatt enthaelt diese Basis **nicht** (es hat
gar kein EMMC-Kapitel); was es liefert, ist die Umrechnungsregel §1.2.4
(`0x7Enn_nnnn` → `0xFEnn_nnnn`). Die Basis kommt aus dem Firmware-DT
(`emmc2bus`-`ranges`) und aus circle. Quellentabelle entsprechend neu.

---

## Auftragspunkt 4 — welche Fassung gilt

**Physical Layer: 9.10 ist die Referenz — und die 6.00-Zitate ueberleben
es vollstaendig.** Nicht behauptet, sondern diffed (Text gegen Text, aus
beiden PDFs extrahiert):

* §4.6.2 / §4.6.2.1 / §4.6.2.2 / §4.6.2.3: gleiche Nummern, gleicher
  Text. Einziger Unterschied: 9.10 nennt in drei Saetzen „SDUC" neben
  SDXC. Die 250/500/100-ms-Saetze und der Application Note sind
  **wortgleich**.
* §4.10.1 (Card Status): wortgleich.
* Tabelle 4-42: **identische Bitbelegung und Typspalte**. 9.10 erweitert
  nur den Beschreibungstext von `WP_VIOLATION` und `WP_ERASE_SKIP` um
  „write protected until power cycle card" und haengt eine Fussnote an
  `CURRENT_STATE`. Tabelle 4-43 bekommt Zeilen fuer CMD22, CMD39,
  ACMD53, ACMD54.
* §4.2.3/§4.2.3.1 und §6.4.1.x: gleiche Nummern, gleicher Wortlaut.

→ **HW03s Bitpositionen und die Maske `0xFDF9_8008` sind gegen 6.00
genauso richtig wie gegen 9.10** (die Maske habe ich aus der Typ-`E`-
Menge nachgerechnet: Bits 31–26, 24–19, 16, 15, 3). Die
6.00-Zitate im Audit und in den Treiberkommentaren bleiben unveraendert
gueltig und wurden deshalb **nicht** umgeschrieben.

**Host Controller: 3.00 bleibt die Referenz — und das ist hier die
wichtigere Entscheidung.** Grund ist nicht Bequemlichkeit: EMMC2 meldet
seine Version in `SLOTISR_VER[23:16]`, und der einzige gemessene Wert ist
`0x02` = Host Controller **3.00** (QEMU; auf echter Pi weiterhin
ungemessen, Audit §5 M1). 4.20 ergaenzt UHS-II, ADMA3 und
64-Bit-Adressierung, die uns EMMC2 nicht anbietet. Dazu kommt, dass 4.20
die Registerabschnitte **um eins verschiebt**:

| Register | 3.00 | 4.20 |
|---|---|---|
| Host Control 1 (`028h`) | §2.2.10 | §2.2.11 |
| Power Control (`029h`) | §2.2.11 | §2.2.12 |
| Clock Control (`02Ch`) | §2.2.14 | §2.2.15 |
| Timeout Control (`02Eh`) | §2.2.15 | §2.2.16 |
| Software Reset (`02Fh`) | §2.2.16 | §2.2.17 |
| Normal Interrupt Status (`030h`) | §2.2.17 | §2.2.18 |
| Error Interrupt Status (`032h`) | §2.2.18 | §2.2.19 |
| Capabilities (`040h`) | §2.2.25 | §2.2.26 |

Ausserdem heisst §3.2.1 in 3.00 „SD Clock Supply Sequence" und in 4.20
„Internal Clock Setup Sequence". §3.3, §3.7.1.2, §3.8 und §3.10.1
behalten ihre Nummern. Die ganze Tabelle steht in
`docs/hardware/emmc-sd.md` §8, mit der Regel: **immer die Ausgabe vor die
Abschnittsnummer schreiben.** Eine Simplified-Ausgabe 4.00 oder 4.10 gibt
es nicht; die Reihe ist 1.00, 2.00, 3.00, 4.20.

---

## Dabei gefunden: zwei falsche Abschnittsnummern im Baum

Beide erst beim Nachschlagen in der PDF sichtbar geworden — genau der
Fall, den das Pinnen verhindern soll.

**F1 — Power Control ist §2.2.11, nicht §2.2.1.** Das Audit (§2.1,
Zeile „Busspannung") zitierte SD-HC 3.00 §2.2.1 fuer die Bitlage von
`SD Bus Power` / `SD Bus Voltage Select`. §2.2.1 ist das *SDMA System
Address / Argument 2 Register*. Richtig ist **§2.2.11 Power Control
Register (Offset 029h)**: Bit 0 `SD Bus Power`, Bits 3:1
`SD Bus Voltage Select` (`111b` = 3,3 V). **Im Audit korrigiert**, und
die Notiz nennt die Stelle jetzt ebenfalls, samt der zwei Regeln, die
wir dort noch nicht einhalten (erst Spannung, dann Power; vorher
Voltage-Support in `CAPABILITIES` pruefen).

**F2 — das Software Reset Register ist §2.2.16, nicht §2.2.15.** In
Ver. 3.00 ist §2.2.15 das *Timeout Control Register*. Betroffen sind
**drei Doc-Kommentare im Produktcode** — und Produktcode fasse ich nicht
an:

* `kernel/src/drivers/bcm2711/emmc.rs:56` (`C1_SRST_CMD`)
* `kernel/src/drivers/bcm2711/emmc.rs:809` (`recover_no_abort`)
* `kernel/src/drivers/bcm2711/emmc.rs:846` (`soft_reset`)

sowie `TASK/HW03-STATUS-DONE.MD:216`. Jeweils „SD-HC 3.00 §2.2.15" →
**§2.2.16**. Inhaltlich stimmt jede dieser Aussagen; nur die Nummer ist
falsch. **An den Entwickler.**

Nicht betroffen: `emmc.rs:299` und `:540` zitieren §2.2.18 fuer die
Error-Interrupt-Bits — in Ver. 3.00 richtig. Ebenso richtig sind die
Zitate auf §2.2.17 (Transfer Complete hat Vorrang vor Data Timeout
Error), §2.2.14 (Teilerwahl), §3.2.2 (Takt nicht waehrend einer
Transaktion anhalten), §3.10.1 und §3.8 — alle im PDF nachgelesen.

**F3 — eine Begruendung im Treiberkommentar traegt nicht.**
`kernel/src/drivers/bcm2711/emmc.rs:850-855` (`soft_reset`, aus HW03 N1)
sagt, §3.10.1 und die Abbruchfolge §3.8 fuehrten die beiden Resets als
zwei Schritte auf, „issuing them in one write is an optimisation the
specification never asks for". Der erste Halbsatz stimmt fuer §3.10.1,
der zweite nicht: **SD-HC 3.00 §3.8.1 Schritt (2)** verlangt woertlich
„Set **both** Software Reset For DAT Line and Software Reset For CMD Line
to 1 in the Software Reset register". Die Spezifikation fragt also sehr
wohl danach, beide Bits zu setzen — sie sagt nur nichts ueber die Zahl
der Registerschreibzugriffe. **Die Aenderung selbst bleibt richtig** (ein
Bit je Zugriff, weil QEMUs SDHCI-Modell ein Zwei-Bit-Byte gar nicht
dekodiert — das ist HW03s gemessener Befund und davon unberuehrt); falsch
ist nur die Berufung auf die Spezifikation. Dieselbe Formulierung steht
in `TASK/HW03-STATUS-DONE.MD:198`. Die belegte Fassung — beide
Folgen nebeneinander, jede mit ihrem Wortlaut — steht jetzt in
`docs/hardware/emmc-sd.md` §5. **An den Entwickler.**

---

## Zwei weitere Nachtraege in der Notiz

* **§5, Zitat aus SD-HC §3.7.1.2 Schritt (7):** stand als ein Satz da
  („Check for errors in Response Data … Return Status (Response Contents
  Error)"), ist aber Schritt (7) **und** Schritt (9). Beide jetzt
  einzeln und im Wortlaut. Gleiche Nummer und gleicher Wortlaut in 4.20.
* **§5, neu: die Ausnahme fuer CMD18.** SD-HC 3.00 §3.7.1.2 Note 3:
  „When the last block of un-protected area is read using memory
  multiple block read command (CMD18), OUT_OF_RANGE error may occur even
  if the sequence is correct. The Host Driver should ignore it."
  Heute nicht erreichbar (wir senden nur CMD17/CMD24), aber HW03s
  `check_r1()` lehnt `OUT_OF_RANGE` bedingungslos ab — wer Mehrblock
  nachruestet, laeuft ohne diesen Hinweis in eine legale Lesung, die wir
  zurueckweisen. Steht jetzt als Kasten in §5.
* **§5, Zitat §4.10.1:** die Auslassung verdeckte „as a response to
  CMD13 with argument b[15]='0'". Der Zusatz unterscheidet CMD13s zwei
  Antworten (Card Status gegen Task Status) und schraenkt das Feld
  **nicht** auf CMD13 ein; jetzt vollstaendig zitiert und erklaert.
* **§5, neu: die beiden Wiederherstellungsfolgen nebeneinander** —
  §3.8.1 (Abbruch: beide Resetbits setzen, dann auf 0 warten) gegen
  §3.10.1 (Fehlerbehandlung: jedes Bit **bedingt** und einzeln, je mit
  eigener Abwartung). Grundlage fuer F3.

---

## Geaenderte Dateien

* **`references/README.md`** (die committete Datei, also der Pin) — neuer
  Abschnitt „SD specifications" mit Pintabelle, Fassungsbegruendung,
  Lizenzantwort und Bezugsweg; neuer Abschnitt „Raspberry Pi firmware
  device tree"; Layout- und Checklistentabelle an den tatsaechlichen
  Bestand angeglichen (das Datenblatt liegt unter `datasheets/`, nicht
  `bcm2711/`; `emutos/`, `freemint/`, `circle/` fehlten ganz);
  Pre-flight-Check um die Pruefsummenzeile ergaenzt.
* **`docs/hardware/emmc-sd.md`** — „Which controller" neu mit
  DT-Belegen und der SDHOST-Korrektur; Quellentabelle neu und auf den
  Pin verweisend; §2 um die 74-Takte-Regel ergaenzt; §3 ACMD41
  korrigiert samt Argumenttabelle; §4 Verweis auf die Grenzen; §5
  Zitate praezisiert, CMD18-Kasten, Quellenluecke S1 als geschlossen
  vermerkt; §6 Bus-Power mit Abschnitt; **§7 neu** (Zeitgrenzen);
  **§8 neu** (Ausgabenkarte). §1–§6 behalten ihre Nummern, weil
  Produktcode und `docs/api/bios/04-rwabs.md` darauf zeigen.
* **`docs/api/hw/emmc-audit.md`** — Nachtragskasten in §0 (S1 erledigt,
  Fassungsentscheidung, SDXC-Praezisierung), §2.2.1 → §2.2.11, §6.4.1.2
  → §6.4.1.1, zwei Zeilennummernverweise auf die Notiz durch
  Abschnittsverweise ersetzt (die Zeilen haben sich durch diese Arbeit
  ohnehin verschoben).
* **`references/sd-spec/`** (nicht im Repo, gitignored): drei neue PDFs
  (HC 4.20, HC 3.00, Phys 6.00) neben der vorhandenen Phys 9.10.

---

## Was offen bleibt

* **BCM2835 ARM Peripherals** liegt weiterhin nicht in `references/` —
  Quelle 2 des Audits und die einzige Quelle fuer die 32-Bit-Gruppierung
  der Register. Ausserhalb der Netzerlaubnis dieses Tickets.
* **Die drei Zitatnummern im Treibercode (F2)** und die eine in
  `TASK/HW03-STATUS-DONE.MD` — brauchen den Entwickler bzw. den Lead.
* **Die Spec-Begruendung in `soft_reset` (F3)** — dieselbe Zustaendigkeit.
* **Die woertliche Tabelle 4-42 in §5** — Lizenzfrage, Entscheidung des
  Leads (siehe Auftragspunkt 1).
* **`docs/SOURCE-ANALYSIS-PROTOCOL.md` Track B** listet die
  SD-Spezifikationen in seiner Quellenhierarchie nicht auf, obwohl sie
  fuer Speicherarbeit primaer sind. Ich habe das Protokoll nicht
  angefasst — Vorschlag: eine Zeile „SD Simplified Specifications —
  primaer fuer SD/SDHCI" zwischen Rang 3 und 4. Entscheidung des Leads.
* **`SLOTISR_VER` auf echtem EMMC2** ist weiterhin ungemessen. Ergaebe
  die Messung Host Controller 4.20 statt 3.00, waere die
  Fassungsentscheidung oben neu zu treffen (die Ausgabenkarte in §8
  macht das dann zu einer Umnummerierung, nicht zu einer Neurecherche).
