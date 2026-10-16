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

## Quellenbefund 2026-10-17 (Review `1d17eae8`)

`docs/api/structures/osheader.md:128-132` und der Kommentar in
`tos-api/src/bdos/process/loader.rs` (~1150) sagen „EmuTOS setzt `os_rsvl = 0`“ —
**falsch**: `references/emutos/bios/startup.S:92-93` legt dort
`os_res1: .dc.l _main` ab, und `reseth` (+4) ist ebenfalls `_main`. TeraDesk
liest `*((long *) os_start + 4)`: auf m68k Offset 0x10 = `os_res1` = Neustart;
auf LP64 (`long` = 8 Byte) Offset 0x20..0x27 (`os_root`/`os_kbshift`), daher
`pc=0`. Nur +0x10 zu setzen behebt es also **nicht** — Entscheidung noetig
(Toolchain-/Overlay-Weg wie bei anderen LP64-Struct-Fallen vs. Kern), bevor
RST01 den Einsprung baut. Spec und Kommentar zuerst berichtigen.
