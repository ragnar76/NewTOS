# DESK07 — File → Shutdown: Zustand pruefen

Status: **OFFEN** · Prioritaet: **P4** · Typ: Pruefung / ggf. gegenstandslos

Abhaengigkeiten: —

## Sachverhalt

`Item::Shutdown` ohne Handler. EmuTOS zeigt den Punkt nur, wenn
`can_shutdown()` wahr ist (`bios/bios.c:754-770`; auf dem
Falcon030/TOS4-Profil **FALSE**); `shel_write(SHW_SHUTDOWN)` ist dann ein
beobachtbares No-op mit Rueckgabe 1 (A48). Zu pruefen: zeigt unser Menue
den Punkt ueberhaupt, und ist „kein Handler" damit schon das richtige
Verhalten? Auf der Pi waere ein echtes Herunterfahren (Watchdog/PM_RSTC)
moeglich — eigene Entscheidung, nicht EmuTOS-Konformitaet.

## Arbeit

Quelle pruefen, Menuegatter wie EmuTOS setzen; Entscheidung des Leads, ob
die Pi wirklich herunterfaehrt.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
