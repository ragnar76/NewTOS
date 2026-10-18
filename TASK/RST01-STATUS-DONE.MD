# RST01 — STATUS: DONE (2026-10-17, QEMU) — Neustart, Ctrl+Alt+Del, OSHEADER-Einsprung

## Weg

1. Spec und Hardware (`9b773120`): `docs/api/bios/_reset-warm-cold.md`,
   `docs/hardware/pm-watchdog-reset.md`, `osheader.md` berichtigt.
2. Anwenderentscheidungen (`b8245e5c`): kalt = echter Pi-Neustart (PM-Watchdog),
   warm vorerst wie kalt ([RST02](RST02.md)), nur Cache leeren, TeraDesk per
   Overlay.
3. Bau (`f63c39c1`):
   - `kernel/src/drivers/bcm2711/pm.rs` + `kernel/src/reset.rs`: Datencache
     leeren (Set/Way), `WDOG=PASSWD|1`, `RSTC=PASSWD|0x20` (Quelle Circle, nicht
     Datenblatt); qemu-virt: kein Mechanismus, nur Meldung.
   - `tos-api/src/bios/reset.rs`: Tastenbedingung wie `ikbd.c:734-747`, als
     Erstes in `decode_key` (IKBD, USB, Injektor). LShift/RShift getrennt —
     `Kbshift` meldet RShift jetzt `0x01` wie TOS.
   - `reseth`/`os_res1` → Stub bei `0xA40` (Seite 0, alle memprot-Modi
     ausfuehrbar) → `Ssystem(S_NEWTOS_RESET = 2021)` (NewTOS-intern).
     `os_entry` bleibt 0: mit `os_version` ergaebe es eine SVE-Kodierung, die
     der A72 nicht hat.
   - `sdk/overlays/teradesk/main.c`: nur 32-Bit-Breiten (X04).
4. Unabhaengiger Tester PASS (`e58d5cfa`).

## Belege

`measurements/measurement-2026-10-17-rst01/RUNINFO.TXT`,
`measurements/measurement-2026-10-17-rst01-test/RUNINFO.TXT`: Gast startet neu
(ohne `-no-reboot` bis zum zweiten Desktop; mit `-no-reboot` beendet sich QEMU);
Ctrl+Alt+Del warm, Ctrl+Alt+RShift+Del kalt, alle Negativfaelle (LShift,
Break-Code, Teilkombinationen) ohne Reset, IKBD und USB; Probe ueber
`Supexec(os_res1)` in memprot Standard/strict/off; TeraDesk „Reboot“ startet
neu. Hosttests 1418/0/1, SDK 20/0/0, 64/94, 76/125, Drift 0, APITEST 109/0/5;
Desktop/SYSACC/TeraDesk bytegleich, QED 20/21 (Timing-Rennen, zweimal
wiederholt gleich).

## Offen

* Echte Pi 4: Watchdog-Neustart, Firmware laedt `kernel8.img` neu, echte
  USB-/IKBD-Tastatur, Tasten ueber den Neustart gehalten.
* Endlosschleife ohne Trap laesst sich nicht per Ctrl+Alt+Del beenden →
  [INPUT02](INPUT02.md).

## Pi-Abnahme (2026-10-18)

Kern `f000303a…` (aus `3717b49e`), TeraDesk `fdaada9e…`. Anwender:
*„reset funktioniert, wie geplant“* — der Watchdog-Neustart (Quelle Circle)
traegt auf echter Pi 4.
