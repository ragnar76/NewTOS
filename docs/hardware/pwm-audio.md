# BCM2711 PWM — Analogton an der Klinke

> **Track-B-Hardwarenotiz** nach `docs/SOURCE-ANALYSIS-PROTOCOL.md`,
> erstellt **vor** dem Treiber (CLAUDE.md Regel 6). Anlass:
> `TASK/SND01.md`, **Schritt 2**. Gegenstück auf der API-Seite:
> `docs/api/xbios/_dmasound-group.md` (Schritt 1) — dessen
> **Puffervertrag** (dort §4) ist das, was diese Hardware bedienen muss.
>
> **Gegenstand:** der PWM-Block des BCM2711 als Tonausgang an der
> 3,5-mm-Klinke der Raspberry Pi 4 B. **Nicht** Gegenstand: HDMI-Ton
> (§11.1) und der PCM/I2S-Block als eigener Ausgang (§7.4).
>
> **Wer eine Nachfüllschleife oder einen Interrupt-Handler schreibt,
> liest zuerst §7 und §8** — dort steht, dass der PWM-Block **keinen
> Interrupt hat**, und was das kostet.
>
> **§10 ist die Liste dessen, was NICHT belegt ist.** Sie ist Teil der
> Notiz, kein Anhang. Wer eine Zahl braucht, die dort steht, misst sie
> oder beschafft eine Quelle — er rät sie nicht.

---

## 0. Das Ergebnis vorab

1. **Die Klinke ist erreichbar.** Der Weg ist vollständig im gepinnten
   BCM2711-Datenblatt beschrieben: PWM1 auf GPIO 40/41 in ALT0, beide
   Kanäle über den **gemeinsamen 64-Wort-FIFO**, ein Abtastwert je
   `RNG`-Takt von `clk_pwm` (§3, §4, §6).
2. **Der PWM-Block hat keinen Interrupt.** Weder Kapitel 8 noch die
   Interrupt-Tabelle des Datenblatts kennen eine PWM-Quelle (§7.1). Der
   FIFO kann also **nicht** „aus einem Interrupt" nachgefüllt werden; es
   bleiben genau zwei Wege: **Nachfüllen aus einem fremden Timer** (§7.2)
   oder **DMA** (§7.3).
3. **Es gibt in diesem Baum keinen DMA-Treiber.** Der DMA-Weg ist im
   Datenblatt vollständig dokumentiert und passt auffällig gut auf den
   Falcon-Puffervertrag (§9), ist aber **zusätzliche Arbeit**: ein
   Kanaltreiber mit Control-Blocks, Buszeiger und GIC-Anbindung.
4. **Die Last ist beim Timerweg real.** Bei 49 kHz stereo leert sich der
   FIFO in **651 µs**; das sind mindestens **1537 Besuche je Sekunde**
   gegenüber dem heutigen 200-Hz-Systemtakt, und rund **98 000
   Peripherieschreibzugriffe je Sekunde** im Dauerbetrieb (§8). Der
   DMA-Weg kostet **einen** Interrupt je Falcon-Rahmen.
5. **Alle Falcon- und STe-Raten sind treffbar**, mit höchstens
   **0,032 %** Abweichung (§5.5) — aber die *Amplitudenauflösung* hängt
   am PWM-Takt: bei 50 MHz sind es bei 49 kHz nur **10 Bit**, bei
   100 MHz 11 Bit (§5.4). 16-Bit-Falcon-Material verliert dort Bits.
6. **QEMU hilft hier nicht.** Die installierte QEMU 11.0.2 hat
   **kein** Modell des BCM-PWM-Blocks (§10.6). Der Klinkenweg ist
   ausschließlich auf echter Hardware prüfbar.

---

## 1. Quellen

| # | Quelle | Wofür |
|---|--------|-------|
| 1 | **BCM2711 ARM Peripherals, Release 4 (2022-01-18)** — `references/datasheets/RP-008248-DS-1-bcm2711-peripherals.pdf`, sha256 `d6e16ea0…dc49dcdc78` | **primär**: PWM-Register und -Bits (Kap. 8), GPIO-Alternativfunktionen (Kap. 5), Interrupt-Quellen (Kap. 6), DMA (Kap. 4), System-Timer (Kap. 10), Adressumrechnung (§1.2.4) |
| 2 | Firmware-Gerätebaum `bcm2711-rpi-4-b.dtb` der Bootkarte — `sd-card-backup-20260618/boot-files/`, 56 407 Bytes, sha256 `75761b73c284e26623e4d1624bff13e67bce2ae620880efd81d6571a3739fcfb`, 2026-04-17 | **Platinenebene**: welcher PWM-Block auf welche Pins, Oszillatorfrequenz, Taktwunsch, DMA-Kanalmaske, GIC-Nummern. Rangiert unter dem Datenblatt für alles, was das Datenblatt abdeckt (`references/README.md`, Abschnitt „Raspberry Pi firmware device tree") |
| 3 | `references/circle` @ `6177984e` — `lib/sound/pwmsoundbasedevice.cpp`, `lib/gpioclock.cpp`, `include/circle/bcm2835.h`, `include/circle/gpioclock.h` | **tertiärer Querverweis**, nie Autorität: ein laufender RPi4-PWM-Tontreiber. Benutzt hier nur, um eigene Ableitungen gegenzuprüfen und um **eine Lücke sichtbar zu machen** (§5.2) |
| 4 | Dieser Baum, lesend: `kernel/src/drivers/timer.rs`, `kernel/src/drivers/gic.rs`, `kernel/src/drivers/bcm2711/` | was an Interrupt- und Timerinfrastruktur **schon da ist** (§7.2) |
| 5 | `docs/api/xbios/_dmasound-group.md` | der Puffervertrag, den diese Hardware bedienen muss (§9) |

**Zitierregel (Lehre aus REF02).** Die Ausgabe steht **vor** der
Abschnittsnummer: „BCM2711 Rel. 4 §8.6, Tabelle 153", nicht „§8.6".
Der BCM2711 ist ein lebendes Dokument; Tabellennummern haben sich
zwischen Ausgaben verschoben. Seitenzahlen werden **nicht** zitiert.

**Was hier ausdrücklich fehlt:** Die ARM ARM liegt in `references/`
**nicht** vor (`references/README.md`, Checkliste: `arm-arm/` = TODO).
Diese Notiz enthält deshalb **keine** ARM-Architekturaussage, die über
das hinausgeht, was `kernel/src/drivers/timer.rs` bereits belegt hat.
Ebenso liegt das **BCM2835**-Datenblatt hier nicht vor; wo BCM2835-Wissen
im Umlauf ist (FIFO-Tiefe, PWM-Taktregister), ist es hier **nicht**
zitiert, sondern als Lücke geführt.

---

## 2. Welcher Block, welche Adressen

Zwei PWM-Instanzen, je zwei Ausgangskanäle:

> „The BCM2711 device has two instances of this block, named PWM0 and
> PWM1 (each with two output channels)." — BCM2711 Rel. 4 §8.2

| Block | Busadresse (legacy) | ARM-Adresse | Quelle |
|---|---|---|---|
| PWM0 | `0x7E20_C000` | `0xFE20_C000` | Rel. 4 §8.6 („The PWM0 register base address is 0x7e20c000"); Umrechnung §1.2.4 |
| **PWM1** ← unserer | `0x7E20_C800` | **`0xFE20_C800`** | dito („… and the PWM1 register base address is 0x7e20c800") |

Umrechnungsregel wörtlich, BCM2711 Rel. 4 §1.2.4: „a peripheral
described in this document as being at legacy address `0x7Enn_nnnn` … is
… visible to the ARM at `0x0_FEnn_nnnn` if Low Peripheral mode is
enabled." Das ist dieselbe Basis, die unsere übrigen Treiber schon
benutzen (`kernel/src/drivers/bcm2711/mod.rs:4`).

Weitere Adressen, die dieser Weg berührt (alle über §1.2.4):

| Block | Bus | ARM | Quelle |
|---|---|---|---|
| GPIO | `0x7E20_0000` | `0xFE20_0000` | Rel. 4 §5.2 |
| Clock Manager (CPRMAN) | `0x7E10_1000` | `0xFE10_1000` | Rel. 4 §5.4.2 („The General Purpose clocks register base address is 0x7e101000") |
| DMA (Kanäle 0–14) | `0x7E00_7000` | `0xFE00_7000` | Rel. 4 §4.2 |
| System Timer | `0x7E00_3000` | `0xFE00_3000` | Rel. 4 §10.1 |
| `PACTL_CS` | `0x7E20_4E00` | `0xFE20_4E00` | Rel. 4 §4.2.1.3 und §6.2.4 |

**Peripheriezugriff und Barrieren.** BCM2711 Rel. 4 §1.3: „place a memory
write barrier before the first write to a peripheral" und „a memory read
barrier after the last read of a peripheral"; ausdrücklich **nicht** nach
jedem einzelnen Zugriff, sondern an den Ein- und Austrittspunkten des
Peripherie-Codes. Für eine Nachfüllschleife über 64 FIFO-Wörter heißt
das: **eine** Barriere davor, **eine** danach — nicht 64. Derselbe
Abschnitt verlangt ausdrücklich, dass auch Interrupt-Routinen so
geklammert werden.

---

## 3. Der Registersatz

BCM2711 Rel. 4 §8.6, Tabelle 152 (Versätze relativ zur Blockbasis):

| Versatz | Name | Rolle |
|---|---|---|
| `0x00` | `CTL` | Betriebsart, Kanalfreigabe, FIFO-Benutzung, FIFO löschen |
| `0x04` | `STA` | FIFO-Status, Fehler- und Lückenflaggen, Kanalzustand |
| `0x08` | `DMAC` | DMA-Freigabe und die beiden Schwellen (DREQ, PANIC) |
| `0x10` | `RNG1` | Bereich (= Periodenlänge in `clk_pwm`-Takten) Kanal 1 |
| `0x14` | `DAT1` | Datenwort Kanal 1, wenn `USEF1 = 0` |
| `0x18` | `FIF1` | **FIFO-Eingang, gemeinsam für beide Kanäle**, nur schreibbar |
| `0x20` | `RNG2` | Bereich Kanal 2 |
| `0x24` | `DAT2` | Datenwort Kanal 2, wenn `USEF2 = 0` |

Es gibt **kein** Interrupt-Freigaberegister und **keine**
Interrupt-Statusbits. Das ist kein Auslassungsfehler dieser Tabelle —
siehe §7.1.

### 3.1 `CTL` — BCM2711 Rel. 4 §8.6, Tabelle 153

| Bit | Name | Bedeutung (Datenblatt) | Reset |
|---:|---|---|:--:|
| 15 | `MSEN2` | 0 = PWM-Algorithmus, 1 = M/S-Übertragung | 0 |
| 13 | `USEF2` | 0 = `DAT2` wird gesendet, 1 = FIFO | 0 |
| 12 | `POLA2` | 1 = Ausgang invertiert | 0 |
| 11 | `SBIT2` | Ausgangspegel, **wenn nicht gesendet wird** | 0 |
| 10 | `RPTL2` | 0 = Übertragung bricht bei leerem FIFO ab; 1 = letztes Wort wird wiederholt | 0 |
| 9 | `MODE2` | 0 = PWM-Modus, 1 = Serialisierer | 0 |
| 8 | `PWEN2` | Kanal 2 freigeben | 0 |
| 7 | `MSEN1` | wie `MSEN2`, Kanal 1 | 0 |
| 6 | `CLRF` | **Einmalig**: 1 schreiben löscht den FIFO; liest immer 0 (`W1SC`) | 0 |
| 5 | `USEF1` | wie `USEF2`, Kanal 1 | 0 |
| 4 | `POLA1` | wie `POLA2`, Kanal 1 | 0 |
| 3 | `SBIT1` | wie `SBIT2`, Kanal 1 | 0 |
| 2 | `RPTL1` | wie `RPTL2`, Kanal 1 | 0 |
| 1 | `MODE1` | wie `MODE2`, Kanal 1 | 0 |
| 0 | `PWEN1` | Kanal 1 freigeben | 0 |

Aus dem Beschreibungstext zu Tabelle 153, weil es der Bittabelle nicht
anzusehen ist:

* „All registers and FIFOs are writeable without setting this bit
  [`PWENi`]" — der FIFO darf **vor** der Kanalfreigabe vorgefüllt
  werden.
* Im PWM-Modus (`MODEi = 0`) entscheidet `MSENi`, ob der
  Gleichverteilungsalgorithmus (§8.3) oder eine einfache
  Mark/Space-Ausgabe benutzt wird. Datenblatt zu `MSEN = 1`: „This mode
  may be preferred if high frequency modulation is not required or has
  negative effects."
* „The channel sends its output continuously as long as the data
  register is used (`USEFi = 0`), or the FIFO is used and it is not
  empty." (§8.4) — **das ist die Unterlaufregel**: leerer FIFO = Ende
  der Übertragung.

### 3.2 `STA` — BCM2711 Rel. 4 §8.6, Tabelle 154

| Bit | Name | Typ | Bedeutung |
|---:|---|:--:|---|
| 10 | `STA2` | RO | 1 = Kanal 2 überträgt gerade |
| 9 | `STA1` | RO | 1 = Kanal 1 überträgt gerade |
| 8 | `BERR` | W1C | Busfehler beim Registerschreiben über APB |
| 5 | `GAPO2` | W1C | **Lücke** zwischen zwei Wörtern auf Kanal 2 |
| 4 | `GAPO1` | W1C | **Lücke** zwischen zwei Wörtern auf Kanal 1 |
| 3 | `RERR1` | W1C | Lesen bei leerem FIFO |
| 2 | `WERR1` | W1C | Schreiben bei vollem FIFO |
| 1 | `EMPT1` | RO | FIFO leer (Reset: **1**) |
| 0 | `FULL1` | RO | FIFO voll |

**Es gibt nur `FULL1` und `EMPT1` — keine Füllstandsanzeige und keine
Halbvoll-Schwelle.** Software kann den Füllstand also nicht lesen;
sie kann nur „voll" und „leer" unterscheiden und muss mitzählen.

Der Unterlauf meldet sich über `GAPOi` (Datenblatt: „there has been a gap
between transmission of two consecutive data from FIFO. This may happen
when the FIFO becomes empty after the state machine has sent a word and
is waiting for the next word. If control bit `RPTLi` is set to high this
event will not occur.") und über `RERR1`.

`BERR` ist der Hinweis darauf, dass zu schnelles aufeinanderfolgendes
Schreiben **in dieselbe Registergruppe** den Synchronisierer überfährt:
„This may happen if the bus tries to write successively to same set of
registers faster than the synchroniser block can cope with." Das
Datenblatt nennt **keine** Mindestwartezeit — siehe §10.2.

### 3.3 `DMAC` — BCM2711 Rel. 4 §8.6, Tabelle 155

| Bit | Name | Bedeutung | Reset |
|---:|---|---|:--:|
| 31 | `ENAB` | DMA starten | 0 |
| 15:8 | `PANIC` | Schwelle für das PANIC-Signal | `0x07` |
| 7:0 | `DREQ` | Schwelle für das DREQ-Signal | `0x07` |

Das Datenblatt sagt nur „threshold level for … going active" — **nicht**,
ob die Schwelle den Füllstand oder den freien Platz meint. Siehe §10.3.

### 3.4 `RNG`, `DAT`, `FIF1` — Tabellen 156/157/158

* `RNG1`/`RNG2`, 32 Bit, **Reset `0x0000_0020`** (= 32): „In PWM mode,
  evenly distributed pulses are sent within a period of length defined by
  this register." Im Serialisierermodus wird auf `RNG` Bit genau
  gesendet: „If the value in `PWM_RNGi` is less than 32, only the first
  `PWM_RNGi` bits are sent resulting in a truncation. If it is larger
  than 32, excess zero bits are padded at the end of data."
* `DAT1`/`DAT2`, 32 Bit, Reset 0: gilt nur bei `USEFi = 0`. „In PWM mode,
  data is sent by pulse width modulation: the value of this register
  defines the number of pulses which are sent within the period defined
  by `PWM_RNGi`."
* `FIF1`, **nur schreibbar** (`WO`): „This register is the FIFO input for
  **both** the channels."

### 3.5 Der FIFO — Tiefe, Teilung, Regeln

**Tiefe: 64 Wörter zu je 32 Bit**, BCM2711 Rel. 4 §8.1, Aufzählungspunkt:
„Serialise mode configured to read data from a FIFO storage block, which
can store up to sixty-four 32-bit words."

> **Ehrlich zur Quellenlage:** dieser Satz steht unter dem
> *Serialisierer*-Punkt. Dass derselbe FIFO im PWM-Modus mit `USEFi = 1`
> benutzt wird, steht in der `CTL`-Beschreibung („data stored in either
> `PWM_DATi` or FIFO is transmitted by pulse width modulation") und in
> der `FIF1`-Beschreibung; dass er dabei dieselbe **Tiefe** hat, ist
> **Schluss, nicht Zitat**. Ein Treiber darf sich deshalb nicht auf die
> Zahl 64 verlassen, sondern muss `FULL1` prüfen — das ist ohnehin die
> einzige Angabe, die zur Laufzeit gilt.

Die Teilungsregeln für den Stereobetrieb stehen vollständig in der
`FIF1`-Beschreibung (Rel. 4 §8.6, vor Tabelle 158):

1. **Verschachtelung ist eingebaut.** „if the word series A B C D E F G H
   I .. is written to the FIFO and both channels are active and
   configured to use FIFO, then channel 1 will transmit words A C E G I
   .. and channel 2 will transmit words B D F H ..". Ein
   `L R L R …`-Strom passt damit **unverändert** in den FIFO.
2. **Beide Kanäle müssen denselben `RNG` haben.** „If the channel range
   (period) value of one channel is different to the other, this will
   cause the channel with the smaller range value to wait between words,
   hence resulting in gaps between words. To avoid that, each channel
   sharing the FIFO should be configured to use the same range value."
3. **`RPTLi` muss 0 sein, wenn geteilt wird.** „the `RPTLi` bits are not
   meaningful when the FIFO is shared between channels as there is no
   defined channel to own the last data in the FIFO. Therefore sharing
   channels must have their `RPTLi` bit set to zero."
4. **Nach einer Umkonfiguration: FIFO löschen.** „If the set of channels
   sharing the FIFO has been modified after a configuration change, the
   FIFO should be cleared before writing new data." — `CTL.CLRF`.

**Folge aus (3) + `SBITi` + `STAi`, als Schluss gekennzeichnet:** im
Stereobetrieb gibt es keinen „letzten Wert halten"-Schutz. Läuft der FIFO
leer, hört die Übertragung auf (§8.4), `STAi` geht auf 0, und der Ausgang
nimmt den `SBITi`-Pegel an — voreingestellt 0, also den unteren Anschlag.
Bei einem unipolaren PWM-Wandler, dessen Ruhelage die *Mitte* ist, ist
das ein Gleichspannungssprung, also ein **hörbares Knacken**. Ein
Unterlauf ist hier folglich nicht „ein bisschen Rauschen", sondern ein
Klick. Belegt sind die Einzelteile (`§8.4`, Tabelle 153 `SBIT`,
Tabelle 154 `STA`); die Hörbarkeitsaussage ist die Schlussfolgerung.

> **Querverweis mit Widerspruch.** `references/circle`
> (`lib/sound/pwmsoundbasedevice.cpp:165-166`) setzt beim Anhalten
> ausdrücklich `RPTL1|RPTL2` „to avoid clicks" und löscht sie beim
> Starten wieder — also genau das, was Regel (3) für geteilte FIFOs
> ausschließt. **Das Datenblatt ist primär.** Wenn wir diesen Kniff
> übernehmen wollen, ist er als bewusste Abweichung zu führen und auf
> echter Hardware zu messen, nicht aus circle zu übernehmen.

---

## 4. Betriebsarten — welche für Ton?

BCM2711 Rel. 4 §8.4 kennt drei:

| Betriebsart | `MODEi` | `MSENi` | Was gesendet wird |
|---|:--:|:--:|---|
| PWM, Gleichverteilung | 0 | 0 | `N` Impulse gleichmäßig über `RNG` Takte verteilt (Algorithmus §8.3) |
| PWM, Mark/Space | 0 | 1 | ein zusammenhängender Impuls der Länge `N` in `RNG` Takten (Abb. 15) |
| Serialisierer | 1 | — | das Wort wird bitweise, MSB zuerst, über `RNG` Takte ausgegeben |

Für Ton ist der **PWM-Modus** gemeint: ein Abtastwert je Periode. Ob
`MSEN = 0` oder `1` besser klingt, hängt am Tiefpass der Platine; das
Datenblatt sagt dazu nur den oben zitierten Satz und **nichts** über die
Platine (§10.5). Der Serialisierermodus ist für Ton nicht gemeint — er
gibt Bits aus, keine Amplituden.

---

## 5. Der Takt

### 5.1 Was das Datenblatt sagt

Zwei Sätze, mehr nicht:

* §8.1: „Both modes clocked by **`clk_pwm` which is nominally 100MHz**,
  but can be varied by the clock manager."
* §8.5: „PWM clock source and frequency is controlled in **CPRMAN**."

### 5.2 Was das Datenblatt **nicht** sagt — die zentrale Lücke

Das Datenblatt dokumentiert vom Clock Manager (Basis `0x7E10_1000`,
8 KiB) **ausschließlich die drei General-Purpose-Takte**: §5.4.2,
Tabelle 98 listet `CM_GP0CTL`/`DIV` (`0x70`/`0x74`), `CM_GP1CTL`/`DIV`
(`0x78`/`0x7c`), `CM_GP2CTL`/`DIV` (`0x80`/`0x84`) — und sonst nichts.
**Ein Register `CM_PWMCTL`/`CM_PWMDIV` kommt im gesamten Dokument nicht
vor** (Sweep über den vollständigen Textauszug: die einzige Fundstelle
für „0x7e1010" ist der Basisadressensatz in §5.4.2).

Was das Datenblatt **doch** liefert, ist das **Bitbild**, das für die
GP-Takte gilt — und das ist der Teil, den ein Treiber wirklich braucht:

`CM_GPnCTL`, Rel. 4 §5.4.2, Tabelle 99:

| Bits | Name | Bedeutung |
|---|---|---|
| 31:24 | `PASSWD` | Passwort `0x5A`, nur schreibbar |
| 10:9 | `MASH` | 0 = ganzzahlig, 1…3 = MASH-Stufen |
| 8 | `FLIP` | Ausgang invertieren (nur Test/Debug) |
| 7 | `BUSY` | RO: Taktgenerator läuft — **Quelle und Teiler dürfen nicht geändert werden, solange gesetzt** |
| 5 | `KILL` | Generator hart anhalten (nur Test/Debug) |
| 4 | `ENAB` | Generator anfordern/abschalten, glitchfrei |
| 3:0 | `SRC` | 0 = GND, 1 = Oszillator, 4 = PLLA per, 5 = PLLC per, 6 = PLLD per, 7 = HDMI aux, 8–15 = GND |

`CM_GPnDIV`, Tabelle 100: `PASSWD` 31:24, **`DIVI` 23:12** (ganzzahliger
Teil), **`DIVF` 11:0** (Bruchteil).

Frequenzformel, Rel. 4 §5.4, Tabelle 96:

| `MASH` | min `DIVI` | mittlere Ausgangsfrequenz |
|:--:|:--:|---|
| 0 | 1 | `source / DIVI` |
| 1 | 2 | `source / (DIVI + DIVF/1024)` |
| 2 | 3 | `source / (DIVI + DIVF/1024)` |
| 3 | 5 | `source / (DIVI + DIVF/1024)` |

> **Widerspruch im Dokument, vermerkt und nicht aufgelöst:** Tabelle 96
> teilt `DIVF` durch **1024**, während Tabelle 100 `DIVF` als **12 Bit**
> (0…4095) führt. Das dokumenteigene Rechenbeispiel Tabelle 97 stützt
> 1024 (Teiler 35,480 → `DIVI` 35, `DIVF` 492; 492/1024 = 0,4805).
> **Für uns ist der Streit vermeidbar:** mit `MASH = 0` gibt es kein
> `DIVF`. Wer MASH benutzen will, muss die Frage vorher klären (§10.4).

§5.4 warnt außerdem: bei MASH „the user must ensure that the module is
not exposed to frequencies higher than 25MHz" — das ist eine Aussage über
die *GPIO*-Takte; ob sie für `clk_pwm` gilt, steht nirgends (§10.4).

**Woher der Registerversatz für den PWM-Takt kommen müsste.** Drei
Möglichkeiten, mit ihrem Preis an Sicherheit:

| Weg | Was er liefert | Kosten an Gewissheit |
|---|---|---|
| **A — nicht anfassen** | Wir programmieren `clk_pwm` gar nicht und **messen** stattdessen, was läuft (§5.6). | Am sichersten: keine undokumentierte Adresse. Preis: wir nehmen die Frequenz, die wir vorfinden, und wählen `RNG` danach. |
| **B — Mailbox** | Die Firmware-Eigenschaftsschnittstelle hat `SET_CLOCK_RATE` = `0x0003_8002` (`references/circle/include/circle/bcmpropertytags.h:50`, unser Treiber kennt bereits `GET_CLOCK_RATE` `0x0003_0002`, `kernel/src/drivers/bcm2711/mailbox.rs:324`). | **Offen**: circles Kennungsliste (`:182-187`) führt `EMMC`, `UART`, `ARM`, `CORE`, `EMMC2`, `PIXEL_BVB` — **keine PWM-Kennung**. Ob die Firmware eine kennt, ist aus unseren gepinnten Quellen **nicht** feststellbar. |
| **C — CPRMAN direkt** | circle rechnet den Registerversatz aus der Taktnummer: `nCtlReg = ARM_CM_BASE + Clock*8` (`lib/gpioclock.cpp:66`) mit `GPIOClockPWM = 20` (`include/circle/gpioclock.h:37`), also `0x7E1010A0`/`0x7E1010A4`. | **Diese Adresse steht in keinem von uns gepinnten Dokument.** Sie stammt aus einem tertiären Querverweis. Übernehmen hieße, Regel 6 („No invented hardware") an einer Stelle zu beugen, an der Weg A existiert. |

**Empfehlung dieser Notiz: Weg A**, hilfsweise B, wenn jemand eine
Firmware-Quelle beschafft. C nur mit ausdrücklicher Entscheidung und als
solche vermerkt.

### 5.3 Was der Gerätebaum sagt

Aus `bcm2711-rpi-4-b.dtb` (selbst dekompiliert, sha256 wie in §1):

```
clocks: cprman@7e101000 { compatible = "brcm,bcm2711-cprman";
                          reg = <0x7e101000 0x2000>; }            /* Z. 161-167 */

clk_osc: clk-osc { compatible = "fixed-clock";
                   clock-frequency = <0x337f980>; }               /* 54 000 000 Hz, Z. 2019-2024 */

pwm:  pwm@7e20c000 { compatible = "brcm,bcm2835-pwm";
                     reg = <0x7e20c000 0x28>;
                     clocks = <cprman 30>;
                     assigned-clock-rates = <0x2faf080>;          /* 50 000 000 Hz */
                     status = "disabled"; }                       /* Z. 1377-1386 */

pwm1: pwm@7e20c800 { … gleiches Bild …
                     pinctrl-0 = <&pwm1_0_gpio40 &pwm1_1_gpio41>;
                     status = "disabled"; }                       /* Z. 1725-1736 */
```

Drei Aussagen daraus, sauber getrennt:

1. **Der Oszillator läuft mit 54 MHz.** Das ist eine Platinenaussage der
   Raspberry-Pi-Firmware; das Datenblatt nennt für `SRC = 1` nur den
   Namen „oscillator", keine Frequenz.
2. **Beide PWM-Blöcke sind ab Werk abgeschaltet** (`status =
   "disabled"`). Wir müssten sie selbst einschalten — und niemand sonst
   benutzt sie, solange wir kein Linux booten.
3. **`assigned-clock-rates = 50 MHz` ist ein Wunsch an Linux'
   Taktrahmenwerk, keine Zustandsaussage.** Es bedeutet: *wenn* der
   `brcm,bcm2711-cprman`-Treiber lädt, programmiert er 50 MHz. Unser
   Kern ist nicht Linux. **Was `clk_pwm` beim Sprung in `kernel8.img`
   tatsächlich tut, sagt der Gerätebaum nicht** — das Datenblatt sagt
   „nominally 100MHz", circle programmiert für RPi4 125 MHz
   (`lib/sound/pwmsoundbasedevice.cpp:40`). Drei Zahlen, drei
   Geltungsbereiche. **Deshalb Weg A aus §5.2: messen.**

### 5.4 Die Ableitung: Takt, Bereich, Abtastrate, Auflösung

Aus §8.3/§8.4 und der `RNG`-Beschreibung (§8.6, Tabelle 156) folgt
unmittelbar — **das ist Rechnung, kein Zitat**:

* Eine PWM-Periode dauert `RNG` Takte von `clk_pwm`.
* Je Periode wird **ein** Wert ausgegeben (aus `DATi` oder aus dem FIFO).

Also:

```
f_sample = f_pwm / RNG                     (ein Abtastwert je Periode)
RNG      = round(f_pwm / f_sample)         (die Umkehrung, ganzzahlig)
Stufen   = RNG + 1   (Werte 0 … RNG)   →   Auflösung ≈ log2(RNG) Bit
```

Die Abtastrate und die Amplitudenauflösung hängen damit **am selben
Knopf**: wer eine hohe Rate will, bekommt wenig Bits, und umgekehrt. Nur
ein höherer `f_pwm` verbessert beides.

> **Gegengeprüft, nicht abgeschrieben:** circle rechnet
> `m_nRange = (CLOCK_RATE + nSampleRate/2) / nSampleRate`
> (`lib/sound/pwmsoundbasedevice.cpp:109`) — dieselbe Formel, gerundet.
> Es setzt außerdem `RNG1 = RNG2 = m_nRange` und
> `CTL = PWEN1|USEF1|PWEN2|USEF2 [|MSEN1|MSEN2] |CLRF`
> (`:217-223`), also genau die Konfiguration, die §3.5 vorschreibt. Die
> Übereinstimmung ist eine Bestätigung der Ableitung, **nicht** ihre
> Quelle.

**Vorzeichen.** Der PWM-Wert ist eine **vorzeichenlose** Impulszahl
`0 … RNG`; Falcon-Abtastwerte sind **vorzeichenbehaftet**
(`docs/api/xbios/_dmasound-group.md` §4.8, aus FreeMiNT). Die Umsetzung
ist damit `pwm = ((sample + 2^(n-1)) * RNG) >> n` für `n`-Bit-Material
— **Rechnung, keine Quelle**, und die Rundungsstrategie (Abschneiden,
Rauschformung) ist eine Entwurfsentscheidung, keine Hardwarevorgabe.

### 5.5 Welche Falcon-Raten treffbar sind

`RNG = round(f_pwm / f_sample)`, Sollraten aus
`docs/api/xbios/_dmasound-group.md` §4.7 (Falcon-Tabelle aus FreeMiNT,
STe-Raten gemessen ebendort). „Bit" = `log2(RNG+1)`, die erreichbare
Amplitudenauflösung.

**f_pwm = 50 MHz** (der Wert, den der Gerätebaum für Linux vorsieht):

| Sollrate | `RNG` | Istrate | Fehler | Bit |
|---:|---:|---:|---:|---:|
| 49 170 (Falcon `prescale` 1) | 1 017 | 49 164,21 | −118 ppm | 9,99 |
| 32 780 (2) | 1 525 | 32 786,89 | +210 ppm | 10,58 |
| 24 585 (3) | 2 034 | 24 582,10 | −118 ppm | 10,99 |
| 19 668 (4) | 2 542 | 19 669,55 | +79 ppm | 11,31 |
| 16 390 (5) | 3 051 | 16 388,07 | −118 ppm | 11,58 |
| 12 292 (7) | 4 068 | 12 291,05 | −77 ppm | 11,99 |
| 9 834 (9) | 5 084 | 9 834,78 | +79 ppm | 12,31 |
| 8 195 (11) | 6 101 | 8 195,38 | +46 ppm | 12,58 |
| 50 066 (STe /160) | 999 | 50 050,05 | **−319 ppm** | 9,97 |
| 25 033 (STe /320) | 1 997 | 25 037,56 | +182 ppm | 10,96 |
| 12 517 (STe /640) | 3 995 | 12 515,64 | −108 ppm | 11,96 |
| 6 285 (STe 6 kHz) | 7 955 | 6 285,36 | +57 ppm | 12,96 |

**f_pwm = 100 MHz** (der „nominal"-Wert des Datenblatts):

| Sollrate | `RNG` | Istrate | Fehler | Bit |
|---:|---:|---:|---:|---:|
| 49 170 | 2 034 | 49 164,21 | −118 ppm | 10,99 |
| 32 780 | 3 051 | 32 776,14 | −118 ppm | 11,58 |
| 24 585 | 4 068 | 24 582,10 | −118 ppm | 11,99 |
| 19 668 | 5 084 | 19 669,55 | +79 ppm | 12,31 |
| 16 390 | 6 101 | 16 390,76 | +46 ppm | 12,58 |
| 12 292 | 8 135 | 12 292,56 | +46 ppm | 12,99 |
| 9 834 | 10 169 | 9 833,81 | −19 ppm | 13,31 |
| 8 195 | 12 203 | 8 194,71 | −36 ppm | 13,58 |
| 50 066 | 1 997 | 50 075,11 | +182 ppm | 10,96 |
| 25 033 | 3 995 | 25 031,29 | −68 ppm | 11,96 |
| 12 517 | 7 989 | 12 517,21 | +17 ppm | 12,96 |
| 6 285 | 15 911 | 6 284,96 | −6 ppm | 13,96 |

**f_pwm = 125 MHz** (circles Wahl für RPi 4 — nur zum Vergleich, keine
Empfehlung): dieselben Raten, Fehler ≤ 118 ppm, Auflösung 11,3…14,3 Bit,
größter `RNG` = 19 889 bei 6 285 Hz.

**Ergebnis.** *Jede* Rate, die die DMA-Sound-Spec kennt, ist treffbar;
der größte Fehler über alle drei Takte ist **319 ppm ≈ 0,032 %**, also
gut ein halber Cent Tonhöhe — unhörbar, aber **nicht exakt**. Ein
Programm, das Abtastwerte gegen `_hz_200` zählt, driftet um diesen
Betrag. Zu beantworten hat das die API-Seite, nicht die Hardware.

**Wo es wehtut, ist die Auflösung, nicht die Rate.** Bei 50 MHz und
49 kHz bleiben **10 Bit**. Falcon-`STEREO16` liefert 16; sechs Bit gehen
verloren. `STEREO8`/`MONO8` passen dagegen überall hinein. Für die
Abnahme von SND01 („ein Programm spielt hörbar einen Puffer ab") reicht
das bequem; für eine Behauptung „16-Bit-Wiedergabe" reicht es nicht.
Der ehrliche Weg ist, die effektive Auflösung zu dokumentieren.

### 5.6 Wie man `clk_pwm` misst, statt ihn anzunehmen

Beobachtung, kein Eingriff (Weg A aus §5.2), mit dem, was der Kern schon
hat:

1. `RNG1 = RNG2 = R` setzen, beide Kanäle auf FIFO (`USEF`), `CLRF`,
   Kanäle freigeben.
2. Genau `N` Wörter in `FIF1` schreiben und den freilaufenden Zähler
   ablesen — `CNTPCT_EL0` mit `CNTFRQ_EL0` als Basis, beides liest
   `kernel/src/drivers/timer.rs` bereits (`cntfrq()`, Modulkopf) —,
   dann warten, bis `EMPT1` gesetzt **und** `STA1`/`STA2` 0 sind, und
   erneut ablesen.
3. `f_pwm ≈ (N/2) · R / Δt` je Kanal (jeder Kanal bekommt die Hälfte der
   Wörter, §3.5 Regel 1).

Das ist eine reine Leseoperation gegen dokumentierte Statusbits und
liefert die einzige Zahl, die wirklich gilt: die, die auf *dieser*
Platine anliegt. **Muss auf echter Hardware laufen** — QEMU kennt den
Block nicht (§10.6).

---

## 6. Die GPIO-Seite

### 6.1 Welche Pins, aus dem Datenblatt

BCM2711 Rel. 4 §5.3, Tabelle der Alternativfunktionen (Spaltenkopf
`GPIO | Pull | ALT0 | ALT1 | ALT2 | ALT3 | ALT4 | ALT5`):

| GPIO | Pull | **ALT0** | ALT1 | ALT2 | ALT3 | ALT4 | ALT5 |
|---|---|---|---|---|---|---|---|
| 40 | Low | **`PWM1_0`** | `SD4` | — | `SD1_DAT4` | `SPI0_MISO` | `TXD1` |
| 41 | Low | **`PWM1_1`** | `SD5` | ⟨reserved⟩ | `SD1_DAT5` | `SPI0_MOSI` | `RXD1` |

Dieselbe Zuordnung noch einmal in der Kurzübersicht §8.5 („GPIOs are
assigned to PWM channels as below"): `GPIO40 → PWM1_0`,
`GPIO41 → PWM1_1`. Damit ist **ALT0 auf 40/41 aus dem Datenblatt belegt**
— das war die Aufgabe.

Vollständigkeitshalber die übrigen PWM-Pins aus §8.5: `GPIO12 → PWM0_0`,
`GPIO13 → PWM0_1` (ALT0), `GPIO18 → PWM0_0`, `GPIO19 → PWM0_1` (ALT5),
`GPIO45 → PWM0_1` (ALT0). Auf der Pi 4 B liegen 12/13/18/19 auf der
40-poligen Stiftleiste, nicht an der Klinke.

### 6.2 Wie umgeschaltet wird

GPIO-Basis `0x7E20_0000` (Rel. 4 §5.2, Tabelle 63); `GPFSEL4` liegt bei
Versatz **`0x10`** und deckt GPIO 40–49 ab. Rel. 4 §5.2, Tabelle 68:
**`FSEL40` = Bits 2:0**, **`FSEL41` = Bits 5:3**.

Kodierung, Rel. 4 §5.2, Tabelle 64 (am Beispiel `FSEL9` ausgeschrieben,
gilt für alle):

| Wert | Bedeutung |
|:--:|---|
| `000` | Eingang |
| `001` | Ausgang |
| **`100`** | **Alternativfunktion 0** |
| `101` | ALT1 |
| `110` | ALT2 |
| `111` | ALT3 |
| `011` | ALT4 |
| `010` | ALT5 |

Also: `GPFSEL4[2:0] = 0b100` und `GPFSEL4[5:3] = 0b100`, übrige Bits
unverändert (Lesen–Ändern–Schreiben). Reset ist `000` — „All pins reset
to normal GPIO input operation" (Beschreibungstext zu Tabelle 64).

Zum Pull: der Gerätebaum verlangt für beide Pins `bias-disable` (§6.3);
die Pull-Register `GPIO_PUP_PDN_CNTRL_REG*` stehen in Rel. 4 §5.2,
Tabelle 63 ab Versatz `0xe4`. Ein Ausgang braucht keinen Pull; ob die
Vorbelegung „Low" der Tabelle in §5.3 etwas anderes heißt, ist nicht
weiter verfolgt.

### 6.3 Was der Gerätebaum dazu sagt — und eine Richtigstellung

Der `pwm1`-Knoten verweist auf zwei Pingruppen (Z. 1734,
`pinctrl-0 = <0x28 0x29>`); aufgelöst sind das die Knoten

```
pwm1_0_gpio40 { pin-pwm { pins = "gpio40"; function = "alt0"; bias-disable; } }   /* phandle 0x28, Z. 659-667 */
pwm1_1_gpio41 { pin-pwm { pins = "gpio41"; function = "alt0"; bias-disable; } }   /* phandle 0x29, Z. 689-697 */
```

Das deckt sich mit dem Datenblatt.

> **Richtigstellung zum Ticket.** `TASK/SND01.md:56-62` liest die Zahlen
> `<0x28 0x29>` aus `pwm1/pinctrl-0` als Pinnummern 40/41 und führt sie
> mit dem Knoten `audio_pins` (`brcm,pins = <0x28 0x29>;
> brcm,function = <0x04>`) zusammen. Beides ist der Sache nach richtig,
> die Beweiskette aber nicht:
> * In `pinctrl-0` stehen **Phandles**, keine Pinnummern. Dass sie hier
>   zufällig ebenfalls `0x28`/`0x29` lauten, ist ein Zufall des
>   Compilers.
> * `audio_pins` (Phandle `0x32`, Z. 1193-1198) hängt **nicht** am
>   PWM-Knoten, sondern am **`vchiq`**-Knoten
>   (`mailbox@7e00b840`, `compatible = "brcm,bcm2711-vchiq"`,
>   `pinctrl-0 = <0x32>`, Z. 1877-1884). Das ist der Pfad der
>   **VideoCore-Firmware** (`snd_bcm2835`), die im Linux-Betrieb die
>   Klinke bedient — dieselben Pins, andere Instanz. Die
>   Kernelbefehlszeile unserer Karte enthält passend dazu
>   `snd_bcm2835.enable_headphones=0` (Z. 71).
>
> **Die Schlussfolgerung des Tickets bleibt gültig** — PWM1, GPIO 40/41,
> ALT0 —, und sie steht jetzt auf dem Datenblatt (§6.1) statt auf einer
> Verwechslung. Nebenbei: der Bezeichner `brcm,function = <0x04>` =
> ALT0 passt zur `FSEL`-Kodierung `100` = 4 aus §6.2.
> **Für uns praktisch bedeutsam:** die Firmware benutzt dieselben Pins.
> Wir übernehmen sie, sobald wir sie umschalten. Solange wir keinen
> Firmware-Tondienst anstoßen, gibt es keinen zweiten Anwärter.

Der Gerätebaum benennt die beiden Leitungen der Platine außerdem selbst:
`gpio-line-names` des GPIO-Knotens (Z. 187) trägt an Index 40/41
`"PWM0_MISO"` / `"PWM1_MOSI"` — die beiden Funktionen, für die diese
Pins auf dieser Platine vorgesehen sind.

> **Was damit NICHT belegt ist:** dass GPIO 40/41 physisch über einen
> Tiefpass auf Spitze und Ring der 3,5-mm-Buchse gehen. Das ist eine
> Aussage über die Leiterplatte; sie steht weder im Datenblatt (das die
> Platine nicht kennt) noch im Gerätebaum (der Funktionen zuordnet, keine
> Netze). Siehe §10.5.

### 6.4 Nichts weiter einzuschalten

Der Firmware-GPIO-Erweiterer der Platine führt
`"BT_ON", "WL_ON", "PWR_LED_OFF", "GLOBAL_RESET", "VDD_SD_IO_SEL",
"CAM_GPIO", "SD_PWR_ON", "SD_OC_N"` (Z. 1853) — **keine Ton- oder
Stummschaltungsleitung**. Nach dem Gerätebaum gibt es also außer den
beiden Pins und dem PWM-Block nichts freizugeben.

---

## 7. Den FIFO füttern

### 7.1 Der PWM-Block hat keinen Interrupt — Beleg

Das ist die wichtigste Einzelaussage dieser Notiz, deshalb der Beleg
zweifach und durch Abwesenheit sauber begründet:

1. **Im Block selbst gibt es keinen.** Kapitel 8 ist vollständig gelesen
   (§8.1 bis §8.6, Tabellen 152–158). Der Registersatz hat acht
   Register (§3); `CTL` enthält kein Freigabebit für irgendeinen
   Interrupt, `STA` nennt seine Bits „Flag"/„Status" und ist
   ausschließlich `RO`/`W1C`. Eine Interruptleitung wird im gesamten
   Kapitel nicht erwähnt.
2. **In der Interrupt-Tabelle des Chips steht er nicht.** BCM2711 Rel. 4
   §6.2.4, Tabelle 102 zählt alle 64 VC-Peripherie-Interrupts auf
   (0 Timer 0 … 63 ETH_PCIe secure). **`PWM` kommt darin nicht vor.**
   Ebenso wenig in der Aufschlüsselung der ODER-verknüpften Quellen
   (Abbildung 6: `AUX_IRQ`-Bits und `PACTL_CS`-Bits 0–20 — SPI, I2C,
   UART; kein PWM) und nicht in Tabelle 101 (ARMC) oder Tabelle 103
   (ETH_PCIe).

**Folge:** die Frage „kann der FIFO aus einem Interrupt gefüttert werden,
und welcher ist es?" hat die Antwort **„nicht aus einem PWM-Interrupt —
den gibt es nicht"**. Es bleiben §7.2 und §7.3.

Zur Einordnung der GIC-Nummern, die weiter unten gebraucht werden
(Rel. 4 §6.3, Abbildung 7): `VC peripheral IRQs → SPI IDs 96-159`, also
**INTID = 96 + VC-IRQ**. Unser Kern rechnet in absoluten INTIDs
(`kernel/src/drivers/gic.rs:241-244`: PL011 = 153 = 96 + 57, und
Tabelle 102 führt VC-IRQ 57 als „OR of all PL011 UART" — die Rechnung
stimmt).

### 7.2 Weg 1 — Nachfüllen aus einem fremden Timer

**Was vorhanden ist:**

* **ARM Generic Timer, EL1 physical**, INTID **30**, heute fest auf
  **200 Hz** als Systemtakt (`kernel/src/drivers/timer.rs`, Modulkopf und
  `TICK_HZ`). Ein Vergleichsregister, ein Interrupt.
* **GIC-400**, mit `enable_spi` und `enable_ppi`
  (`kernel/src/drivers/gic.rs:92,132`) — die Verteilung steht.
* **Nicht vorhanden:** ein zweiter Timerkanal, ein Treiber für den
  BCM-System-Timer, und irgendeine DMA.

**Was das Datenblatt an zusätzlichen Timern anbietet** — BCM2711 Rel. 4
§10.1/§10.2: der System Timer an `0x7E00_3000` hat „four 32-bit timer
channels and a single 64-bit free running counter"; `CLO`/`CHI` bei
`0x04`/`0x08`, die Vergleichsregister `C0`…`C3` bei `0x0c`…`0x18`, die
Treffermeldungen `M0`…`M3` in `CS` (`W1C`). Die vier Treffer sind
VC-IRQ 0…3 (Tabelle 102), also **INTID 96…99**.

Die **Zählfrequenz nennt das Datenblatt nicht** („The free running
counter is driven by the timer clock"). Der Gerätebaum nennt sie:
`timer@7e003000 { clock-frequency = <0xf4240>; }` = **1 000 000 Hz**
(Z. 144-151); derselbe Knoten führt
`interrupts = <0 64 4> <0 65 4> <0 66 4> <0 67 4>`, was mit
`INTID = 32 + n` genau 96…99 ergibt und damit Tabelle 102 bestätigt.
1 MHz und vier unabhängige Vergleicher reichen für eine
Audionachfüllung mühelos.

> **Bekannte Falle, hier nur als Warnung und ohne Quelle:** es ist
> verbreitete Überlieferung, dass die VideoCore-Firmware zwei der vier
> System-Timer-Kanäle für sich benutzt. **In keiner von uns gepinnten
> Quelle steht das** — weder im Datenblatt noch im Gerätebaum. Wer diesen
> Weg geht, prüft zur Laufzeit, ob ein Kanal bereits ein
> Vergleichsregister ungleich 0 trägt, statt es zu glauben (§10.7).

**Der Ablauf** wäre: Timer feuert → `STA` lesen → solange `FULL1` nicht
gesetzt ist, ein Wort nach `FIF1` schreiben → `GAPO1|GAPO2|RERR1|WERR1`
prüfen und löschen → nächster Termin. Die Klammerung mit genau **einer**
Barriere vor und nach dem Block nach §1.3.

**Warum das teuer ist, steht in §8.**

### 7.3 Weg 2 — DMA (dokumentiert, aber wir haben keinen Treiber)

Das Datenblatt beschreibt den Weg vollständig.

**Der DREQ.** BCM2711 Rel. 4 §4.2.1.3, Tabelle der Zuordnung:
**DREQ 1 = „DSI0 / PWM1"**, **DREQ 5 = „PWM0"**. Die Umschaltung von
DREQ 1 steht im selben Abschnitt: `PACTL_CS` (`0x7E20_4E00`) **Bit 23
`DMA_CNTRL_MUX_2`** — „0 selects DSI0, 1 selects PWM1", **Reset `0x1`**.
Für PWM1 ist die Vorbelegung also bereits die richtige; der Abschnitt
mahnt zugleich „Other bits in this register should be treated as
reserved, and only written back with the previously read value."

> **Unstimmigkeit im Dokument, benannt:** §8.5 formuliert dieselben
> Zahlen als „PWM0 DMA is mapped to **DMA channel** 5. PWM1 DMA is mapped
> to **DMA channel** 1 (muxed with DSI0)." Das sind die **DREQ**-Nummern
> aus §4.2.1.3, nicht Kanalnummern: §4.1 sagt ausdrücklich „The
> allocation of peripherals to DMA channels is programmable", und die
> Auswahl geschieht über `TI.PERMAP` (§4.2, Tabelle 40, Bits 20:16:
> „Indicates the peripheral number (1-31) whose ready signal shall be
> used to control the rate of the transfers"). **Schluss, nicht Zitat:**
> §8.5 meint DREQ; ein beliebiger Kanal kann mit `PERMAP = 1` auf PWM1
> hören.

**Der Kanal.** §4.2: Kanal *n* liegt bei `0x7E00_7000 + n·0x100`
(Kanal 15 getrennt bei `0x7EE0_5000` und „exclusively used by the VPU").
Registerversätze innerhalb eines Kanals (§4.2.1.2): `CS` `0x00`,
`CONBLK_AD` `0x04`, `TI` `0x08`, `SOURCE_AD` `0x0c`, `DEST_AD` `0x10`,
`TXFR_LEN` `0x14`, `STRIDE` `0x18`, `NEXTCONBK` `0x1c`, `DEBUG` `0x20`.
Global: `INT_STATUS` `0xfe0`, `ENABLE` `0xff0` (alle Maschinen ab Reset
freigegeben, `EN0`…`EN14` Reset `0x1`; `PAGE`/`PAGELITE` Reset 0).

**Der Control-Block.** §4.2.1.1: acht Wörter, 256-Bit-ausgerichtet;
Wort 0 `TI`, 1 `SOURCE_AD`, 2 `DEST_AD`, 3 `TXFR_LEN`, 4 `STRIDE`,
5 `NEXTCONBK`, 6–7 reserviert = 0. Start: CB-Adresse nach `CONBLK_AD`,
dann `CS.ACTIVE`. Am Ende lädt der Kanal `NEXTCONBK` nach und macht
weiter — „In this way a linked list of Control Blocks can be constructed
in order to execute a sequence of DMA operations without software
intervention" (§4.1). Bei `NEXTCONBK = 0` hält er an und löscht `ACTIVE`.

**Die Bits, die dieser Fall braucht** (§4.2, Tabelle 40 `TI`):
`DEST_DREQ` (Bit 6) — „The DREQ selected by `PERMAP` will gate the
destination writes"; `SRC_INC` (Bit 8); `PERMAP` (20:16) = 1 für PWM1;
`INTEN` (Bit 0) — „Generate an interrupt when the transfer described by
the current Control Block completes". `DEST_INC` bleibt 0, weil das Ziel
das feststehende `FIF1` ist.

**Der Interrupt.** `CS` Bit 2 `INT` (`W1C`, §4.2 Tabelle 38): „This is
set when the transfer for the CB ends and `INTEN` is set to 1. Once set
it must be manually cleared down, even if the next CB has `INTEN = 0`."
Kanal *n* ist VC-IRQ 16 + *n* (Tabelle 102: 16 „DMA 0" … 22 „DMA 6",
23 „DMA 7 & 8", 24 „DMA 9 & 10", 31 „DMA 15"), also **INTID 112 + n** für
*n* = 0…6. Der Gerätebaum bestätigt es unabhängig: der DMA-Knoten führt
`interrupts = <0 80 4> …` mit `interrupt-names = "dma0" … "dma10"`
(Z. 1516-1524) — `32 + 80 = 112`, und `dma7`/`dma8` teilen sich
`<0 87 4>` (= 119 = 96+23), `dma9`/`dma10` teilen sich `<0 88 4>`
(= 120 = 96+24), genau wie Tabelle 102.

**Welche Kanäle frei sind.** Der Gerätebaum reserviert:
`brcm,dma-channel-mask = <0x7f5>` (Z. 1522) — Bitmaske, also **Kanäle 0,
2, 4, 5, 6, 7, 8, 9, 10 verfügbar; 1 und 3 nicht**. Kanäle 7–10 sind
„DMA Lite" (§4.1: „Four of these are DMA Lite channels (with reduced
performance and features)"), 11–14 sind DMA4 mit größerem Adressraum. Für
einen Audiostrom reicht Lite bequem; wer es genau wissen will, nimmt
einen der vollen Kanäle 2, 4, 5 oder 6.

**Die Adressen, die der DMA sieht.** §4.1: „the DMA controller must be
set-up to use the **Legacy Master addresses** of the peripherals" — das
Ziel ist also `0x7E20_C818` (PWM1 `FIF1`), **nicht** `0xFE20_C818`. Und
§1.2.4: „Software accessing RAM using the DMA engines must use legacy
addresses (between `0xC000_0000` and `0xFFFF_FFFF`). This accesses a 1GB
window within the full 16GB SDRAM address space." Mit `ENABLE.PAGE` = 0
(Reset) ist das **die erste GiB des Arbeitsspeichers**. Ein Tonpuffer
muss also entweder dort liegen, das Fenster verschieben oder einen
DMA4-Kanal benutzen. **Das ist eine Randbedingung für den
Speicherverwalter**, nicht für den Tontreiber — sie gehört in die
Entscheidung, wo `Setbuffer`-Puffer herkommen.

**Was uns fehlt:** `kernel/src/drivers/bcm2711/` enthält `emmc`,
`framebuffer`, `mailbox`, `pl011` — **keine DMA**. Ein DMA-Kanaltreiber
(Control-Block-Verwaltung, Cachepflege für die CBs und den Puffer,
Bus-/ARM-Adressumrechnung, GIC-Eintrag, Fehlerbehandlung über
`CS.ERROR`/`DEBUG`) ist **zusätzliche Arbeit** und nicht Teil dieser
Notiz. Er wäre allerdings für mehr als Ton nützlich.

### 7.4 Der Vollständigkeit halber: PCM/I2S **hat** einen Interrupt

BCM2711 Rel. 4 Kapitel 7 beschreibt den PCM/I2S-Block, und dieser hat,
anders als PWM, eine vollwertige Interruptanbindung: §7.9, Tabelle 142
führt `INTEN_A` (`0x18`) und `INTSTC_A` (`0x1c`); Tabelle 149 `INTEN_A`
Bit 0 `TXW` — „Setting this bit enables interrupts from PCM block when TX
FIFO level is **less than the specified TXTHR level**" —, Tabelle 150
`INTSTC_A` mit `TXW`/`RXR`/`TXERR`/`RXERR` als `W1C`. Die Quelle ist
VC-IRQ **55** „PCM/I2S" (Tabelle 102), also INTID **151**.

Genau das, was man sich für PWM gewünscht hätte. **Nützt uns hier aber
nichts:** PCM liegt auf GPIO 18–21 (§5.3: `PCM_CLK`, `PCM_FS`, `PCM_DIN`,
`PCM_DOUT` als ALT0) und setzt einen externen Wandler voraus. Die Pi 4 B
hat keinen an Bord, und an der Klinke hängt PWM, nicht PCM. Der Hinweis
steht hier, damit die Frage „gibt es auf diesem Chip überhaupt einen
FIFO-Interrupt?" beantwortet ist — ja, aber am falschen Block.

---

## 8. Was das den Kern kostet

Alles Folgende ist **Rechnung** aus §3.5 (FIFO 64 Wörter, geteilt) und
§5.4 (ein Wort je Kanal je Periode). Keine Zahl davon steht so im
Datenblatt.

**Entleerungszeit des vollen FIFO im Stereobetrieb.** 64 Wörter, je zwei
Wörter ein Stereorahmen → **32 Rahmen**. Also
`t_leer = 32 / f_sample`.

| Rate | `t_leer` | Mindestbesuche/s (voll→leer) | mit Sicherheitsfaktor 2 |
|---:|---:|---:|---:|
| 50 066 (STe /160) | 639 µs | 1 565 | 3 130 |
| **49 170 (Falcon 1)** | **651 µs** | **1 537** | **3 073** |
| 32 780 | 976 µs | 1 024 | 2 049 |
| 25 033 | 1,28 ms | 782 | 1 565 |
| 24 585 | 1,30 ms | 768 | 1 537 |
| 19 668 | 1,63 ms | 615 | 1 229 |
| 16 390 | 1,95 ms | 512 | 1 024 |
| 12 517 | 2,56 ms | 391 | 782 |
| 12 292 | 2,60 ms | 384 | 768 |
| 9 834 | 3,25 ms | 307 | 615 |
| 8 195 | 3,90 ms | 256 | 512 |
| 6 285 (STe 6 kHz) | 5,09 ms | 196 | 393 |

**Was daraus folgt:**

* **Der heutige 200-Hz-Systemtakt reicht nicht.** Seine Periode ist
  5 ms; der FIFO ist bei 49 kHz nach 0,65 ms leer. Nur die unterste
  STe-Rate (6 285 Hz, 5,09 ms) liegt überhaupt in Reichweite — und zwar
  ohne jede Reserve. **Ein Nachfüllen im 200-Hz-Tick ist kein
  Kompromiss, sondern ein Dauerknacken.**
* **Der Timerweg kostet bei 49 kHz rund 1 500–3 100 Interrupts je
  Sekunde** — das **8- bis 16-fache** des heutigen Takts, auf einem
  kooperativen Einkernkern.
* **Der Schreibaufwand ist von der Besuchsrate unabhängig:** je Rahmen
  müssen zwei Wörter nach `FIF1`, also bei 49 170 Hz **98 340
  Peripherieschreibzugriffe je Sekunde**, dazu je Besuch mindestens ein
  `STA`-Lesezugriff. Was ein Peripheriezugriff auf dieser Platine
  kostet, sagt keine unserer Quellen (§10.2) — die Zahl ist also eine
  Stückzahl, keine Zeit.
* **Der DMA-Weg kostet genau einen Interrupt je Control-Block.** Legt
  man einen CB je Falcon-Rahmen, ist das **ein Interrupt je Rahmen** —
  bei einem 32-KiB-Puffer und 49 kHz Stereo/8 Bit etwa **3 je Sekunde**.
  Die Nachfüllung selbst macht die Hardware; die CPU berührt `FIF1`
  überhaupt nicht.

Das ist der eigentliche Befund dieses Abschnitts: **zwischen den beiden
Wegen liegen rund drei Zehnerpotenzen Interruptlast.**

---

## 9. Wie das auf den Puffervertrag der Spec abbildet

> **Alles in diesem Abschnitt ist Abbildung, also Ableitung** — aus
> `docs/api/xbios/_dmasound-group.md` §4 auf der einen und aus §3/§7.3
> dieser Notiz auf der anderen Seite. Es ist **keine** Hardwareaussage
> und **keine** Entscheidung; die trifft der Lead.

| Spec-Vertrag (dort §4) | PWM + Timer (§7.2) | PWM + DMA (§7.3) |
|---|---|---|
| Startadresse, **exklusive** Endadresse | Softwarezeiger; die Hardware kennt keinen Puffer | `SOURCE_AD` + `TXFR_LEN` im CB; Länge = `end − start` |
| **Gerade** Rahmenlänge | freiwillig | `TXFR_LEN` in Bytes, Ausrichtung frei wählbar |
| `SB_PLA_ENA` (ein Rahmen, dann Stopp) | Softwarezustand | `NEXTCONBK = 0` → „The DMA will stop (and clear the `ACTIVE` bit) … when `NEXTCONBK` is set to `0x0000_0000`" (§4.2.1.1) |
| `SB_PLA_RPT` (Endlosschleife) | Softwarezustand | CB zeigt auf sich selbst |
| Ende-des-Rahmens-**Interrupt** | **keiner vom PWM** (§7.1); nur „Timer stellt fest, dass er fertig ist" | `TI.INTEN` → `CS.INT` → INTID 112+*n*. **Ein echter Rahmenende-Interrupt.** |
| `Buffptr` — laufender Zähler | Software muss selbst zählen | `SOURCE_AD` ist lesbar und wird mitgeführt: „Source address for the DMA operation. **Updated by the DMA engine as the transfer progresses**" (§4.2, Tabelle 41) |
| 24-Bit-Adressgrenze des Falcon | entfällt (Interna) | ersetzt durch das 1-GiB-Fenster (§7.3) — eine andere Grenze, aber eine reale |

**Der Punkt, der hier heraussticht:** der DMA-Weg liefert die beiden
Dinge, die die Spec als teuer bzw. als Lücke markiert hat
(`_dmasound-group.md` §9.1 „Der Ende-des-Puffers-Interrupt — die größte
Lücke", und §4.5 „Polling als Ersatz"), **direkt aus dokumentierter
Hardware**: einen Rahmenende-Interrupt und einen mitlaufenden
Quelladresszähler. Der Timerweg liefert weder das eine noch das andere
ohne zusätzliche Buchführung. Das ist ein Argument, das vor der
Reihenfolgeentscheidung auf den Tisch gehört — die Spec hatte für
NewTOS zunächst `Buffptr`-Polling als den „realistischen ersten
Abspielweg" notiert; diese Notiz zeigt, dass der DMA-Weg technisch der
näher am Vertrag liegende ist, auch wenn er mehr Arbeit ist.

---

## 10. Was NICHT belegt ist

Diese Liste ist der Teil der Notiz, der sie von einer schönen Notiz
unterscheidet. Jede Zeile: was fehlt, wie schlimm, was man täte.

### 10.1 Die Registeradresse des PWM-Takts
`CM_PWMCTL`/`CM_PWMDIV` stehen **in keiner gepinnten Quelle**. Das
Datenblatt dokumentiert vom Clock Manager nur `GP0`…`GP2` (§5.2 hier,
Rel. 4 §5.4.2 dort). **Auswirkung:** wir können `clk_pwm` nicht
quellengestützt programmieren. **Was zu tun ist:** §5.2, Weg A —
vorgefundenen Takt messen (§5.6) und `RNG` danach wählen. Nur wenn das
nicht reicht, Weg B oder C, und dann als bewusste Abweichung vermerkt.

### 10.2 Zeitkosten eines Peripheriezugriffs
Weder das Datenblatt noch der Gerätebaum sagen, wie lange ein
32-Bit-Schreibzugriff nach `0xFE20_C818` dauert. Die Zahlen in §8 sind
deshalb **Stückzahlen**, keine Prozentangaben zur CPU-Last.
**Was zu tun ist:** auf echter Hardware messen (N Schreibzugriffe gegen
`CNTPCT_EL0`), sobald der erste Treiber existiert. Bis dahin keine
Aussage „kostet x % der CPU".

### 10.3 Richtung der `DMAC`-Schwellen
Rel. 4 §8.6, Tabelle 155 sagt nur „threshold level for DREQ/PANIC signal
going active" — nicht, ob die Schwelle den **Füllstand** oder den
**freien Platz** meint. **Auswirkung:** nur für den DMA-Weg, und nur für
die Feinabstimmung; die Vorbelegung 7/7 ist gültig und wird von circle
unverändert benutzt (`pwmsoundbasedevice.cpp:161-163`). **Was zu tun
ist:** mit der Vorbelegung anfangen und nur anfassen, wenn `GAPO`
auftritt.

### 10.4 `DIVF`-Nenner und die 25-MHz-Grenze
Tabelle 96 sagt `DIVF/1024`, Tabelle 100 macht `DIVF` 12 Bit breit
(§5.2). Ebenso ist unklar, ob die MASH-Warnung „not exposed to
frequencies higher than 25MHz" für `clk_pwm` gilt oder nur für die
GPIO-Takte. **Auswirkung:** keine, solange `MASH = 0` benutzt wird — und
wegen 10.1 programmieren wir den Takt ohnehin vorerst nicht.

### 10.5 Die Platine zwischen GPIO 40/41 und der Buchse
**Nicht belegt:** dass die beiden Pins über einen Tiefpass auf die
3,5-mm-Buchse gehen; welche Grenzfrequenz dieser Filter hat; ob die
Buchse dieselben Adern zusätzlich für Composite-Video benutzt und was
das für den Pegel heißt. Das Datenblatt beschreibt den Chip, nicht die
Platine; der Gerätebaum beschreibt Funktionen, keine Netze. Belegt ist
nur, dass die Firmware genau diese beiden Pins für ihren Tonausgang in
ALT0 schaltet (§6.3) und dass die Platine sie `PWM0_MISO`/`PWM1_MOSI`
nennt (§6.3). **Auswirkung:** die Wahl zwischen `MSEN = 0` und `MSEN = 1`
(§4) und die Frage, wie sehr die PWM-Trägerfrequenz durchschlägt, sind
nicht vorherzusagen. **Was zu tun ist:** das schematische Datenblatt der
Pi 4 B beschaffen (Raspberry Pi veröffentlicht einen reduzierten
Schaltplan) und in `references/README.md` pinnen — oder beide
Betriebsarten auf echter Hardware hören.

### 10.6 QEMU modelliert den PWM-Block nicht
Beobachtung, sauber benannt: `strings` über das installierte
`qemu-system-aarch64` (**QEMU 11.0.2**, `/opt/homebrew/bin`) zeigt die
Gerätenamen `bcm2835-dma`, `bcm2835-cprman`, `bcm2835-sys-timer`,
`bcm2835-i2s` und rund vierzig weitere — **aber kein `bcm2835-pwm`**; die
einzigen PWM-Modelle im Binärbild sind `aspeed.pwm`, `npcm7xx-pwm` und
`exynos4210.pwm`. **Was das ist:** ein Fund über Gerätemodellnamen im
Binärbild, **keine** Quelltextlesung. **Was es bedeutet:** der
Klinkenweg ist in QEMU `raspi4b` nicht prüfbar; wie sich ein Zugriff auf
`0xFE20_C800` dort verhält (stiller Treffer ins Leere oder Abbruch), ist
**ebenfalls nicht festgestellt**. **Was zu tun ist:** entweder QEMUs
`hw/arm/bcm2835_peripherals.c` beschaffen und nachlesen, oder den
Treiber so bauen, dass er auf einer Maschine ohne PWM sauber „nicht
vorhanden" meldet, statt zu hängen.

### 10.7 Belegung der System-Timer-Kanäle
Ob die VideoCore-Firmware Kanäle des System Timers benutzt, steht in
**keiner** gepinnten Quelle (§7.2). **Was zu tun ist:** zur Laufzeit
prüfen, nicht glauben.

### 10.8 FIFO-Tiefe im PWM-Modus
Die Zahl 64 steht unter dem Serialisierer-Punkt (§3.5). Dass sie im
PWM-Modus gleich ist, ist Schluss. **Auswirkung:** gering, weil `FULL1`
zur Laufzeit gilt. **Was zu tun ist:** die Nachfüllschleife an `FULL1`
festmachen, nicht an einer Konstante 64 — und die gemessene Tiefe beim
ersten Hardwarelauf notieren.

### 10.9 Die ARM ARM
Liegt hier nicht vor (§1). Diese Notiz macht deshalb **keine**
Architekturaussage über Barrieren-Semantik, Cachepflege oder
Speicherattribute. Der DMA-Weg (§7.3) braucht beides; wer ihn baut,
braucht die ARM ARM oder muss sich auf das stützen, was der
Framebuffer-/EMMC-Treiber bereits belegt hat.

---

## 11. Beobachtungen außerhalb dieser Notiz

1. **HDMI-Ton.** Die BCM2711 ARM Peripherals **haben kein HDMI-Kapitel**
   — das Inhaltsverzeichnis führt 13 Kapitel (Introduction, Auxiliaries,
   BSC, DMA, GPIO, Interrupts, PCM/I2S, PWM, SPI, System Timer, UART,
   Timer (ARM side), ARM Mailboxes), keines davon HDMI. Die
   Registerbereiche, die `TASK/SND01.md:70-82` aus dem Gerätebaum
   zitiert, sind dort **benannt, aber nirgends bei uns dokumentiert**.
   Außerdem heißt der DMA-Kanal des HDMI-Knotens `dma-names =
   "audio-rx"` — **rx**, nicht tx; und die Karte trägt
   `snd_bcm2835.enable_hdmi` in ihren Schaltern (Z. 2383), also den
   Firmwarepfad. Die Einschätzung des Tickets, HDMI-Ton sei „ein
   normaler Treiber gegen dokumentierte Register", ist mit unseren
   gepinnten Quellen **nicht** belegt. Eigenes Ticket.
2. **Ein DMA-Treiber fehlt dem Baum insgesamt**, nicht nur dem Ton —
   `kernel/src/drivers/bcm2711/` hat emmc, framebuffer, mailbox, pl011.
   Wer ihn für SND01 baut, baut ihn für mehr.
3. **`references/README.md`, Checkliste:** die ARM ARM steht dort als
   TODO. Für den DMA-Weg wird sie gebraucht.
