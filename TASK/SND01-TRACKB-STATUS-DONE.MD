# SND01 — Schritt 2 (Track-B-Hardwarerecherche) abgeschlossen

**Datum:** 2026-09-29 · **Art:** Hardwarenotiz, kein Code, kein Treiber ·
**Ticket:** `TASK/SND01.md`, Teil B / Schritt 2 der dortigen Reihenfolge ·
**Branch:** `desk-rebuild`, Basis HEAD `03b9b74`, nichts committet, kein
`git checkout/restore/clean/stash/reset`.

## Ergebnis

`docs/hardware/pwm-audio.md` — Track-B-Notiz zum PWM-Block des BCM2711 als
Tonausgang an der Klinke, in der Form von `docs/hardware/emmc-sd.md`.
Primärquelle ist die **BCM2711 ARM Peripherals, Release 4 (2022-01-18)**
(`references/datasheets/RP-008248-DS-1-…pdf`, sha256 `d6e16ea0…`),
Platinenebene der Firmware-Gerätebaum der Bootkarte (sha256 `75761b73…`,
selbst dekompiliert), `references/circle` @ `6177984e` ausschließlich als
tertiärer Querverweis und einmal ausdrücklich als *Gegenbeispiel*.

Damit ist CLAUDE.md Regel 6 („No invented hardware") für den Klinkenweg
erfüllt, soweit das Datenblatt reicht — und dort, wo es nicht reicht,
ist die Lücke benannt statt gefüllt (§10 der Notiz).

## Die fünf gestellten Fragen, kurz beantwortet

1. **PWM-Block, FIFO** — Registersatz vollständig (Rel. 4 §8.6,
   Tabellen 152–158), FIFO **64 Wörter, von beiden Kanälen geteilt**,
   Verschachtelung `L R L R …` eingebaut, gleicher `RNG` für beide
   Pflicht, `RPTLi` muss dabei 0 sein. Unterlauf meldet sich über
   `GAPO1`/`GAPO2`/`RERR1`; es gibt **keine Füllstandsanzeige**, nur
   `FULL1`/`EMPT1`. (Notiz §3)
2. **Takt** — Datenblatt: `clk_pwm` „nominally 100MHz", gesteuert in
   CPRMAN. **Der Registerversatz des PWM-Takts steht in keiner gepinnten
   Quelle** (§10.1). `f_sample = f_pwm / RNG`, Auflösung ≈ `log2(RNG)`
   Bit. **Alle** Falcon- und STe-Raten sind treffbar, Fehler ≤ 0,032 %;
   die Auflösung ist der Engpass (bei 50 MHz/49 kHz nur 10 Bit).
   Empfehlung: den vorgefundenen Takt **messen** statt programmieren
   (§5.6). (Notiz §5)
3. **GPIO** — **ALT0 auf GPIO 40/41 ist aus dem Datenblatt belegt**
   (Rel. 4 §5.3 Tabelle der Alternativfunktionen und §8.5), nicht nur aus
   dem Gerätebaum. Umschaltung über `GPFSEL4[2:0]`/`[5:3]` = `0b100`
   (§5.2, Tabelle 68 und Tabelle 64). (Notiz §6)
4. **FIFO füttern** — **Der PWM-Block hat keinen Interrupt**, zweifach
   belegt durch Abwesenheit (Kapitel 8 vollständig; Tabelle 102 der
   VC-Interrupts kennt kein PWM). Bleiben: Nachfüllen aus einem fremden
   Timer, oder DMA (DREQ 1 für PWM1, `PACTL_CS` Bit 23 ab Reset schon
   richtig; CB-Struktur, `TI.INTEN`, INTID 112+n). **Wir haben keinen
   DMA-Treiber** — das ist zusätzliche Arbeit. (Notiz §7)
5. **Last** — voller FIFO hält im Stereobetrieb **32 Rahmen**; bei
   49 170 Hz sind das **651 µs**, also mindestens **1 537**, mit
   Sicherheitsfaktor 2 rund **3 073 Besuche je Sekunde**, gegen den
   heutigen **200-Hz**-Systemtakt. Dazu **98 340 Schreibzugriffe nach
   `FIF1` je Sekunde**, unabhängig von der Besuchsrate. Der DMA-Weg
   kostet **einen** Interrupt je Control-Block, also je Falcon-Rahmen.
   (Notiz §8)

## Zwei Befunde, die über die Fragestellung hinausgehen

* **Der DMA-Weg liegt näher am Puffervertrag als der Timerweg.** Er
  liefert den Rahmenende-Interrupt (`TI.INTEN` → `CS.INT`) und einen
  mitlaufenden Quelladresszähler (`SOURCE_AD`, „Updated by the DMA engine
  as the transfer progresses") — genau die beiden Dinge, die
  `docs/api/xbios/_dmasound-group.md` §9.1 als größte Lücke und §4.5 als
  Notbehelf führt. (Notiz §9 — als Abbildung gekennzeichnet, keine
  Entscheidung.)
* **QEMU 11.0.2 modelliert den BCM-PWM-Block nicht.** Der Klinkenweg ist
  nur auf echter Hardware prüfbar. (Notiz §10.6)

## Richtigstellung am Ticket

`TASK/SND01.md:56-62` liest `<0x28 0x29>` in `pwm1/pinctrl-0` als
Pinnummern und führt den Knoten `audio_pins` mit dem PWM-Block zusammen.
In `pinctrl-0` stehen **Phandles**; `audio_pins` hängt am
**`vchiq`**-Knoten, also am Firmwarepfad. **Die Schlussfolgerung des
Tickets — PWM1, GPIO 40/41, ALT0 — bleibt richtig** und steht jetzt auf
dem Datenblatt statt auf der Verwechslung. (Notiz §6.3.)
Das Ticket gehört dem Lead; hier nur vermerkt, nicht geändert.

## Ausdrücklich nicht getan

* Kein Produktcode angefasst, kein Treiber, keine `.rs`-Datei.
* `docs/STATUS.md` und `TASK/SND01.md` **nicht** angefasst.
* Nichts geflasht, nichts ausgeworfen, keine echte Pi angefasst,
  `/Volumes/BOOT` nicht berührt. Kein Commit, kein Push.
* HDMI-Ton ist **nicht** Gegenstand — mit einem belegten Nebenbefund:
  die BCM2711 ARM Peripherals **haben kein HDMI-Kapitel** (Notiz §11.1).
