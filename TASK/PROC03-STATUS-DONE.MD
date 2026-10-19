# PROC03 — STATUS: DONE (2026-10-19, QEMU) — Prozesskette, `run` und Verzeichnis wie EmuTOS

Anwender 2026-10-18: alles EmuTOS-treu (Portierung); B5 Variante (2).

| Teil | Ergebnis | Commit |
|---|---|---|
| Spec | accdesk-PD als Elter von `#Z`-/Desktop-Programmen (`geminit.c:894`), alte „AES-PD“-Aussage ueberall berichtigt; `_run-ownership-curdir.md` | `d45082a8` |
| Stufe 1 + B4 | accdesk- und EmuCON-Stellvertreter, `Pterm` ueber `p_parent` mit Abwicklung | `31db1d1c` |
| Stufe 2 (PROC04 Punkt 2) | Laufwerk/Verzeichnis je PD | `59c8eb4c` |
| B5 (2) | Accessory: `*os_run` und Kind-`p_parent` = EmuTOS-`run`; Besitz, Schutz, Verzeichnis, DTA, Handles, Umgebung, eigenes `Pterm` bleiben beim Accessory (benannte Abweichung, ADR-027) | `6ae7c221` |
| Stufe 3 (AES als echter Prozess) | entfaellt: nur noetig, wenn B5 „wie EmuTOS“ gewaehlt worden waere | — |

Belege: `measurements/measurement-2026-10-18-proc03-s1*/`,
`measurements/measurement-2026-10-18-proc04-curdir*/`,
`measurements/measurement-2026-10-19-proc03-b5*/` — jeweils Tester PASS.
Pi: Stufe 1/2 mit 0.6 (`4bc49cf3`) abgenommen; B5 offen.
Bekannt: ein Kind eines Accessorys behaelt als `p_parent` die PD eines
inzwischen beendeten Vordergrundprogramms (wie EmuTOS; robust, Rueckkehr korrekt).
