# NET01 — GENET-Ethernet (BCM2711) als Treiber im Kernel

Status: **OFFEN** · Prioritaet: **P3** · Typ: Hardwaretreiber (Track B)

Abhaengigkeiten: —

## Sachverhalt

Fuer einen Netzstack braucht es zuerst eine Netzkarte. Die Pi 4 hat
Broadcom GENET v5 (`bcm2711-genet`) fest auf dem SoC: MMIO, MDIO-PHY,
DMA-Deskriptorringe, **kein** Firmware-Blob — im BCM2711-Datenblatt
beschrieben und in `references/circle` implementiert. WLAN (CYW43455 ueber
SDIO) braucht dagegen Firmware-Upload, `brcmfmac`-Protokoll und Supplicant
— eigener, viel groesserer Brocken (NET06, spaeter).

Es fehlen auch die Vorleistungen: Interrupt-Registrierung und
DMA-faehiger/kohaerenter Speicher gibt es im Kernel noch nicht.

## Arbeit

1. Track B: GENET aus dem BCM2711-Datenblatt (`references/datasheets/`) und
   Circle (`references/circle`, `lib/bcm54213.cpp` o. ae.) — Register,
   Ringaufbau, MDIO, Link-Status. Linux nur als Querverweis.
2. Vorleistung: IRQ-Registrierung (`kernel/src/arch/aarch64/`) und
   kohaerenter DMA-Puffer (nicht-cachebar oder mit Cache-Pflege) — eigener
   kleiner Abschnitt im Ticket-Abschluss.
3. Treiber `kernel/src/drivers/bcm2711/genet.rs`: init, MAC-Adresse (Mailbox
   `0x00010003`), Senden/Empfangen ueber Ringe, Link-Erkennung.
4. Abnahme: auf echter Pi 4 Rahmen senden/empfangen (ARP-Anfrage sichtbar,
   Gegenstelle antwortet — mit `tcpdump` am Switch belegt); QEMU `raspi4b`
   emuliert GENET **nicht** → dort kein Test, ausdruecklich vermerken.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten. Hardwareaktionen nur durch Lead/Anwender.
