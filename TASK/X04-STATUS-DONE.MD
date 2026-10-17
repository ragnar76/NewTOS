# X04 — STATUS: DONE (2026-10-17, QEMU) — TeraDesk „Reboot“ startet neu statt `pc=0`

Behoben mit [RST01](RST01-STATUS-DONE.MD) (`f63c39c1`): Kern legt einen
anspringbaren Reset-Einsprung bei `reseth`/`os_res1`; das Overlay
`sdk/overlays/teradesk/main.c` liest `os_start` und `+0x10` mit 32 Bit statt
LP64-`long`. QEMU: Quit-Dialog → „Reboot“ → `OSHEADER reset entry (Cold)`,
Neustart, TeraDesk startet wieder
(`measurements/measurement-2026-10-17-rst01-test/`). Offen: echte Pi 4 (neues
`DESKTOP.NAP` auf die Karte).
