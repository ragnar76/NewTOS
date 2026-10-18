# PROC04 — STATUS: DONE (2026-10-18/19, QEMU) — verbleibende GEMDOS-Handle-Abweichungen

Anwender-Grundsatz 2026-10-18: wie EmuTOS (Portierung).

| Punkt | Ergebnis | Commit | Belege |
|---|---|---|---|
| 1 Zeichenfunktionen folgen `p_uft` (= CON02) | Datei- und Geraeteumlenkung wie `bdosmain.c:498-568`/`console.c`; `Cconrs`-`max` vorzeichenlos (benannte Abweichung) | `1a306120`, `5d2a3812` | `measurements/measurement-2026-10-18-proc04-redir*/` |
| 2 Laufwerk/Verzeichnis je PD | wie `init_pd_files`/`ixterm` | `59c8eb4c` | `measurements/measurement-2026-10-18-proc04-curdir*/` |
| 3 negative Handles bei `Fread`/`Fwrite` | schon seit FS11 erledigt | — | Re-Validierung `fc972f82` |
| 4 75 offene Dateien | wie `bdos/fs.h:43-45` | `5fe11974` | `measurements/measurement-2026-10-18-proc04-files*/` |
| 5 `Fdelete` auf offene Datei | `ixdel`, Variante (b) (benannte Abweichung); verlorene Cluster von F6 behoben | `7ca3b700` | `measurements/measurement-2026-10-18-proc04-fdelete/`, `measurements/measurement-2026-10-19-proc04-fdelete-test/` |

Jeweils unabhaengiger Tester PASS. Pi: Punkte 1/2/4 mit Kern `4bc49cf3` (0.6)
abgenommen; Punkt 5 offen. Folgetickets: FS15, FS16, FS17, CON03.
