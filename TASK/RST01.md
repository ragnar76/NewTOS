# RST01 — Neustart: Warm- und Kaltstart, Ctrl+Alt+Del

Status: **OFFEN** · Prioritaet: **P2** · Typ: fehlende Funktion — Anwender 2026-10-17

## Anlass

Anwender 2026-10-17: *„reboot oder auch Reset haben wir noch nicht, auch
Strg+Alt+Entf geht noch nicht. Wäre aber manchmal sinnvoll.“* Ausloeser war
[X04](X04.md): TeraDesks „Reboot“ springt ueber den Reset-Einsprung aus dem
`OSHEADER` auf Adresse 0.

## Vorbild EmuTOS

- `bios/ikbd.c:743-754`: **Ctrl+Alt+Del** → `warm_reset()`,
  **Ctrl+Alt+RShift+Del** → `cold_reset()` (LShift darf nicht gedrueckt sein).
- `bios/ikbd.c:~1009`: Sonderfall, wenn Ctrl+Alt+Del nach dem Neustart noch
  gehalten wird.
- `warm_reset`/`cold_reset` und der Reset-Einsprung im `OSHEADER` (den
  TeraDesk liest): in EmuTOS nachlesen, was sie tun (Speicher/Sysvars,
  `resvalid`/`resvector`, `memvalid`), bevor etwas gebaut wird.

## Auftrag

1. **Spec zuerst** (Regel 5): EmuTOS `warm_reset`/`cold_reset`, die Tasten-
   pruefung, den `OSHEADER`-Reset-Einsprung und die Reset-Sysvars nach
   `docs/api/` extrahieren.
2. **Hardware:** wie die Pi 4 neu startet (BCM2711 PM-Watchdog o. ae.) —
   nur aus Datenblatt bzw. der Quellen-Hierarchie Track B, nicht geraten.
   QEMU raspi4b: pruefen, ob es den Mechanismus nachbildet.
3. Festlegen (Anwender), was warm und kalt auf NewTOS heissen: z. B. warm =
   Kern neu initialisieren ohne Firmware, kalt = echter Pi-Neustart; ob
   offene Dateien vorher geschlossen werden.
4. Bauen: Tastenkombination im Tastaturpfad (IKBD und USB), Reset-Einsprung
   im `OSHEADER` gueltig (behebt [X04](X04.md) ohne Aenderung an TeraDesk).

[Gemeinsame Regeln](README.md) gelten.
