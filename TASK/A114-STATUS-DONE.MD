# A114 — STATUS: DONE (2026-10-17) — kein Defekt im Handshake; falsche Meldung entfernt

Der `AC_CLOSE`-Handshake ist EmuTOS-konform (Lauf,
`measurements/measurement-2026-10-17-a113-a114/serial/idprobe1.log`). Vor dem
`#Z`-Start laeuft in EmuTOS kein `appl_exit` (`geminit.c:446-458`,
`gemshlib.c:629-645,698`), „false“ war dort richtig. Falsch waren Kommentar
und Meldung in `kernel/src/launch.rs`: entfernt in `79383cd0` (das Flag
`AP_ACCLOSE` wird nie geloescht, `gemsuper.c:140,161`, die Stelle kann eine
echte Runde nicht erkennen). Tester: Meldung 0× statt 2×
(`measurements/measurement-2026-10-17-a113-test/`).
