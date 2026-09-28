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
`Sndstatus` (0x8C), `Buffoper` (0x89), `Setbuffer` (0x83), `Soundcmd` (0x82),
`Buffptr` (0x8D), `Dsptristate`/`Gpio` soweit sinnvoll.

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

**Die Klinke ist der einfachere Weg:** PWM auf GPIO 40/41. Als Rueckfall
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
