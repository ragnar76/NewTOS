# ACC06 — Ein kleines Accessory als Pruefling fuer ACC02/ACC03

Status: **OFFEN** · Prioritaet: **P2** · Typ: Abnahmefall + Kernelbindung

Abhaengigkeiten: [ACC01](ACC01.md) (Quellenanalyse, offen) — der Pruefling
laeuft erst, wenn ACC02/ACC03 stehen; **gebaut wird er vorher**, wie
`curdir.c` vor DESK18 dastand.

## Zweck

Ein Accessory, das im Desk-Menue erscheint und auf Klick **Taktfrequenz und
CPU-Temperatur** zeigt. Es ist der Abnahmefall fuer das Accessory-Geruest:
ohne ihn wird ACC02/ACC03 gegen nichts geprueft.

Lead-Entscheidung zur Anzeigeform (2026-09-24): **ein `form_alert`-Kasten**,
kein eigenes Fenster. Ein einziger AES-Aufruf, kein selbstgebauter
Objektbaum — groesste Chance, dass er auf der frischen
Accessory-Implementierung sofort laeuft. Eine Fensterfassung ist spaeter ein
eigenes Ticket.

```
  [i] NewTOS System
      ARM clock : 1500 MHz
      CPU temp  : 47.2 C
      [  OK  ]
```

## Teil 1 — die zwei Werte muessen aus dem Kernel herauskommen

Heute gibt es sie nirgends: die Mailbox-Infrastruktur steht
(`kernel/src/drivers/bcm2711/mailbox.rs`), aber die Firmware-Tags
`GET_CLOCK_RATE` (`0x0003_0002`, Takt-ID 3 = ARM, Wert in Hz) und
`GET_TEMPERATURE` (`0x0003_0006`, ID 0, Wert in Tausendstel Grad Celsius)
sind nicht implementiert — und ein EL0-Programm kommt an die Mailbox ohnehin
nicht heran.

**Lead-Entscheidung zum Weg (2026-09-24): `Ssystem`, mit NewTOS-eigenen
Unterfunktionen ab 2000.**

* `Ssystem` gibt es schon (GEMDOS 0x154, `tos-api/src/gemdos/ssystem.rs`,
  heute `S_GETCOOKIE`/`S_SETCOOKIE`/`S_DELCOOKIE`), und FreeMiNT-kompatible
  Erweiterungen sind die sanktionierte Richtung des Projekts.
* FreeMiNT belegt 0–29, 100, 900–913, 1000–1100 und `0x54f8`
  (`references/freemint/sys/mint/ssystem.h`) — **ab 2000 ist frei**, also
  kein Kollisionsrisiko. Vorschlag: `S_NEWTOS_CPUCLOCK = 2000`,
  `S_NEWTOS_CPUTEMP = 2001`. Die Nummernwahl gegen die Quelle gegenpruefen,
  bevor sie festgeschrieben wird.
* Die Keksdose taugt **nicht**: eine Temperatur aendert sich, ein Keks ist
  ein Startwert.

Der Weg vom Trap zur Mailbox geht ueber den `Host`-Trait
(`tos-api/src/host.rs`) wie jede andere Hardwareanbindung — `qemu-virt` und
eine hostlose Konfiguration antworten ehrlich mit „gibt es hier nicht" und
der Aufrufer macht `ERROR` daraus, **keine erfundene Zahl**.

## Teil 2 — der Pruefling

Ein Konsolenprogramm (`.NCP`) zuerst, das die zwei `Ssystem`-Aufrufe absetzt
und die Werte druckt: damit ist Teil 1 belegbar, **bevor** es ein Accessory
gibt. Danach das Accessory selbst: `appl_init` → `menu_register` →
Ereignisschleife auf `AC_OPEN`/`AC_CLOSE`, auf `AC_OPEN` der Kasten oben. Ein
Accessory endet nie.

`menu_register` (35) gibt es bereits und traegt Titel + Anwendungs-ID ein
(`tos-api/src/aes/menu.rs:1105`); was fehlt, ist das Eintragen in die
Desk-Klappliste und das Senden von `AC_OPEN` — das ist ACC02/ACC03 und
**nicht** dieses Ticket.

**Offen und von ACC01 zu entscheiden:** die Dateiendung. Klassisch ist
`.ACC`; unsere eigenen Formate heissen `.NAP`/`.NCP`, damit der spaetere
Emulator die m68k-Endungen bekommt. Hier **nicht** vorgreifen.

## Arbeit

Teil 1 vollstaendig, mit Hosttest fuer die Trap-Ebene und einem QEMU-Lauf
des Konsolenprogramms. Teil 2 als Quelltext im SDK, mit der Begruendung im
Kopf, warum er noch nicht laufen kann. Danach ist ACC02/ACC03 pruefbar.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
