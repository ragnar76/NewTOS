# X01 — STATUS: DONE (2026-10-17) — Super/Supexec im SDK-Overlay, abgenommen

Vorgaenger: [X01-STATUS-UNFINISHED.MD](X01-STATUS-UNFINISHED.MD) (gebaut 2026-09-14).

- Entscheidungen (Anwender 2026-10-17): Einzelprozessbetrieb, Dateien bleiben
  nie offen; Cookie-Spiegel bei `$5A0` genuegt. Nebenbefund des Testers: der
  Spiegel wird inzwischen bei `Setcookie` nachgezogen (SX11–SX17 gruen).
- Unabhaengiger Tester PASS (`df6e57e7`,
  `measurements/measurement-2026-10-17-x01/RUNINFO.TXT`): Gruppe SUPER 17/0/0,
  APITEST 109/0/5 in memprot Standard/strict/off; TeraDesk frisch gebaut
  (`DESKTOP.NAP` 0afa291c…, reproduzierbar), startet per `#Z`, Quit-Dialog,
  Rueckkehr rc=0, Desktop reagiert; Rohtraps GEMDOS 0x20 = -32, XBIOS 0x26 = 38
  (Kern unveraendert); SDK 20/0/0, Hosttests 1411/0/1.
- Pi: „Reboot“ → [X04](X04.md)/[RST01](RST01.md). „Shutdown“ haelt absichtlich
  an (`lobo()`, TOS-treu). `force_mediach` und Videodialog (schreibt auf
  ST-ACIA `0xFFFC00`, faellt mit wie ohne X01) nicht gefahren.
