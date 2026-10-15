# DESK60 — Shell meldet „starting DESKTOP.NAP“ nach einem `#Z`-gestarteten Programm

Status: **OFFEN** · Prioritaet: **P3** · Typ: Diagnose/Log — Nebenfund DESK43-Abnahme (2026-10-15)

Endet ein per `#Z` gestartetes TeraDesk, protokolliert die Shell in beiden
Desktop-Pfaden „starting DESKTOP.NAP“, obwohl NEWDESK bzw. der eingebaute
Desktop hochkommt (`measurements/measurement-2026-10-15-desk43-acceptance/`).
Vermutlich ein Rest des vorigen Kommandos in `D.s_cmd`. Gegen EmuTOS
`sh_main`/`sh_ldapp` pruefen, ob `s_cmd` dort beim Wechsel auf
`DESKTOP_APP` neu gesetzt wird; nur Log oder auch Verhalten (`appl_find`,
`p_nameit`) betroffen?

[Gemeinsame Regeln](README.md) gelten.
