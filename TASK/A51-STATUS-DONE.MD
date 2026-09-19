# A51 — Quittung

**Status: GEGENSTANDSLOS** · 2026-09-20 · kein Code, mit Grund

`save_windows` (`tos-api/src/aes/window.rs:1049`) hat seit `343b3c5` keinen
Produktaufrufer (siehe `TASK/A31-STATUS-DONE.MD`, derselbe Sweep). Der
beschriebene Effekt — `WS_WORK` eines recycelten Handles `0,0,0,0` fuer ein
gestartetes GEM-Programm — setzt voraus, dass `save_windows` an der
Startgrenze laeuft; das tut es nicht mehr. Die Grenze regelt A28
(`d93e115`, Deskriptor-Reset); `wind_create`/`wind_close` schreiben
`WS_WORK` nach A16/A34 wie EmuTOS. Beim Wiederaufbau des Rueckwegs (G1/G2)
werden `save_windows`/`restore_windows` entfernt.
