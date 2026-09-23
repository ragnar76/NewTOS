# DESK07 — STATUS: DONE (2026-09-23)

## Auftrag

Pruefen, ob `Item::Shutdown` ohne Handler schon das richtige Verhalten ist,
und das Menuegatter wie EmuTOS setzen.

## Befund

Ja. EmuTOS' Gatter ist `menu_ienable(tree, QUITITEM, can_shutdown())`
(`references/emutos/desk/deskmain.c:582`). `can_shutdown()`
(`references/emutos/bios/bios.c:754-770`) liefert `TRUE` nur bei
`has_nf_shutdown()` (Native Features), `MACHINE_FIREBEE`, `MACHINE_AMIGA`
und `MACHINE_LISA`; der `#else`-Zweig ist `return FALSE`. Das Profil dieses
Projekts ist Falcon030/TOS4 auf echter Hardware — also `FALSE`, also ein
dauerhaft graues Menueelement, und `QUITITEM`s Rumpf mit
`shel_write(SHW_SHUTDOWN, …)` (`deskmain.c:773-778`) wird nie erreicht.

NewTOS verhielt sich bereits so, aber ueber den Sammelzweig `_ => false` in
`kernel/src/desk/menu.rs`, dessen Kommentar nur von `deskrez` und DESK05
sprach.

## Aenderung

Eine, rein dokumentarisch: eigener `Item::Shutdown`-Zweig mit der
Quellenangabe, plus ein Satz im Sammelzweig, warum `Execute EmuCON` aus
einem anderen Grund grau ist (EmuTOS schaltet `CLIITEM` nie ab — es kommt in
keiner `ILL_*`-Tabelle und in keinem `menu_ienable`-Aufruf vor; uns fehlt der
Handler, DESK06). Kein Verhaltensunterschied.

## Pruefung

`cargo xtask build --board rpi4`: tos-api 64 / kernel 94, Delta 0. Kein
Messlauf, weil kein beobachtbares Verhalten beruehrt ist.

## Offen

Ob die Pi 4 wirklich herunterfahren soll (PM_RSTC/Watchdog), ist
zurueckgestellt: eigene Erweiterung ohne EmuTOS-Vorbild, nicht vor
Meilenstein 2b, dann als eigenes Ticket mit Datenblattbeleg.
