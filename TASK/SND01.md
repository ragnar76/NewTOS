# SND01 — Ton: Falcon-DMA-Sound als API, HDMI als Ausgang

Status: **OFFEN** · Prioritaet: **P3** (nach dem OFAM-Termin) · Typ: Treiber + API-Ebene — Anwenderwunsch 2026-09-28

Abhaengigkeiten: — · Vorbedingung fuer [SDL01](SDL01.md)

## Warum

NewTOS hat **keinen Ton**. `Dosound`, `Giaccess`, `Offgibit`, `Ongibit` sind
ehrliche No-ops mit der Begruendung „no PSG" (`tos-api/src/xbios/system.rs:145-161`).
Das ist bisher kein Mangel gewesen, wird aber einer, sobald etwas vorgefuehrt
werden soll, das ueber einen Desktop hinausgeht — und es ist die Vorbedingung
fuer ein SDL-Backend.

Der Anwender 2026-09-28: das sei „gar nicht sooo ein grosses Problem", und
**der Ton soll ueber HDMI herauskommen; Klinke geht auch**.

## Die zwei Haelften, die nicht zusammenhaengen

### A — die API-Seite: welchen TOS-Ton bilden wir nach?

Unser Zielprofil ist **EmuTOS Falcon030/TOS4**
([[emutos-target-profile-falcon-tos4]]). Damit ist die konforme Wahl **nicht**
die PSG-Emulation, sondern Falcons **DMA-Sound**: `Devconnect` (0x8B),
`Sndstatus` (0x8C), **`Buffoper` (0x88 — das Ticket nannte 0x89, falsch; 0x89
ist `Dsptristate`)**, `Setbuffer` (0x83), `Soundcmd` (0x82), `Buffptr` (0x8D),
**`Setmode` (0x84 — im Ticket zunaechst ganz vergessen; EmuTOS nennt die
C-Funktion `setsndmode`)**, `Dsptristate`/`Gpio` soweit sinnvoll.

**Zuerst zu klaeren, nicht anzunehmen:** was EmuTOS von dieser Gruppe
ueberhaupt implementiert und unter welchem `CONF_WITH_*`, und was davon im
`make 512`-Referenzbau an ist (`docs/api/_emutos-config.md`). Die PSG-Gruppe
bleibt No-op; die Begruendung dort ist dann zu praezisieren („kein PSG, aber
DMA-Sound vorhanden").

### B — die Hardware-Seite: wie kommt Ton aus einer Pi 4 heraus?

**HDMI ist der Wunsch und der schwierigere Weg.** Der Ton geht dort nicht ueber
einen einfachen Peripherie-Registersatz, sondern durch den VideoCore; welcher
Weg fuer uns offensteht (Mailbox-Schnittstelle, HDMI-Block direkt, oder gar
nicht ohne Firmware-Mitwirkung), ist **Track-B-Recherche und steht noch aus**.
Primaerquellen nach `docs/SOURCE-ANALYSIS-PROTOCOL.md`: BCM2711-Datenblatt,
die Firmware-/Mailbox-Dokumentation der Raspberry Pi Foundation, der
Geraetebaum unserer Bootkarte (`bcm2711-rpi-4-b.dtb` — hat uns bei REF02 schon
einmal die richtige Antwort geliefert). Linux' `vc4-hdmi` und circle sind
**Querverweise, keine Autoritaet**.

### Belegt aus dem Firmware-Geraetebaum (2026-09-28)

Gelesen aus `sd-card-backup-20260618/boot-files/bcm2711-rpi-4-b.dtb`
(`dtc -I dtb -O dts`) — dieselbe Quellenart, die bei REF02 die richtige Antwort
gab, wo das Datenblatt schwieg.

**Klinke** (Korrektur 2026-09-29, Track B): die urspruengliche Lesart des
Geraetebaums war falsch — `<0x28 0x29>` in `pwm1/pinctrl-0` sind **Phandles**,
keine Pinnummern, und `audio_pins` haengt am **`vchiq`**-Knoten, also am
VideoCore-Firmwarepfad, nicht am PWM-Treiber. **Die Schlussfolgerung bleibt
richtig** — PWM1 auf `0xFE20_C800`, GPIO 40/41 in ALT0 —, sie steht jetzt aber
auf dem **Datenblatt** (Rel. 4 §5.3, §8.5) statt auf der Verwechslung.
Einzelheiten in `docs/hardware/pwm-audio.md`.

**HDMI — und hier lag der Lead erneut falsch, diesmal zu optimistisch:**

```
clock-names = "hdmi", "bvb", "audio", "cec";
dmas = <0x2e 0x41fa000a>;
dma-names = "audio-rx";
reg-names = "hdmi", "dvp", "phy", "rm", "packet", "metadata", "csc", "cec", "hd", "intr2";
```

Der HDMI-Block hat einen **eigenen Audiotakt und einen eigenen DMA-Kanal**, und
seine Registerbereiche sind benannt. Der Ton wird also per DMA von der
**ARM-Seite** hineingeschoben — **nicht** ueber die Firmware, wie oben zunaechst
vermutet. Das ist hiermit richtiggestellt.

**Widerrufen 2026-09-29 (Track B):** das **BCM2711-Datenblatt hat gar kein
HDMI-Kapitel** (13 Kapitel, keines davon HDMI). Die Aussage „normaler Treiber
gegen dokumentierte Register" ist mit unseren gepinnten Quellen **nicht
belegt**. Was der Geraetebaum zeigt (eigener Audiotakt, eigener DMA-Kanal,
benannte Registerbereiche), bleibt richtig — es sagt nur nichts darueber, ob
die Register **dokumentiert** sind. Der DMA-Name des HDMI-Knotens ist zudem
`audio-rx`, und die Karte traegt `snd_bcm2835.enable_hdmi` in ihren Schaltern.
**Eigenes Ticket, eigene Recherche.**

**Die Klinke bleibt trotzdem der erste Schritt:** PWM auf GPIO 40/41. Als Rueckfall
ausdruecklich erwuenscht, und als erster Schritt sinnvoll — ein hoerbarer Ton
ueber Klinke beweist die ganze API-Seite, bevor die HDMI-Frage geklaert ist.

## Vorschlag zur Reihenfolge

1. **A klaeren** (Quellenanalyse): welche XBIOS-Funktionen, welche Semantik,
   welcher Puffermechanismus — Spec vor Code, Regel 5.
2. **B recherchieren** (Track B): HDMI-Weg belegen oder als nicht gangbar
   belegen. Kein Code, bevor die Quelle steht — Regel 6.
3. **Klinke zuerst bauen**, weil sie die API-Seite beweist.
4. **HDMI danach**, mit der Ausgangswahl als Geraeteparameter, nicht als
   zweitem Treiber.

## Abnahme

Ein Programm, das ueber die XBIOS-Sound-Gruppe einen Puffer abspielt, ist
**hoerbar** — zuerst an der Klinke, dann ueber HDMI. Kein Stub: solange eine
Funktion nicht implementiert ist, bleibt sie ein dokumentiertes „nicht
vorhanden", kein stiller Erfolg.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
