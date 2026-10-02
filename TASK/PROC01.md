# PROC01 — `Pexec` Modi 5, 6 und 7 fehlen (EmuTOS hat sie)

Status: **OFFEN** · Prioritaet: **P2** (nach dem OFAM-Termin) · Typ: Fehlende EmuTOS-Funktion — aus [PORT10](PORT10.md)

Abhaengigkeiten: [MEM03](MEM03.md) (Umgebungsblock in EL0; Modus 5/7 legen ebenfalls eine Umgebung an)

## Sachverhalt

NewTOS beantwortet `PE_BASEPAGE` (5), `PE_GOTHENFREE` (6) und
`PE_BASEPAGEFLAGS` (7) mit `EINVFN` (`tos-api/src/bdos/process/pexec.rs:177-179`).
EmuTOS setzt alle drei um (`references/emutos/bdos/proc.c:210-247`): 5/7
legen Umgebung und Basepage an (`alloc_env`, `alloc_tpa`, `init_pd_fields`,
`p_flags`), 6 uebertraegt den Besitz an das Kind und startet es wie 4.

Das ist eine Luecke gegenueber EmuTOS, kein Sonderwunsch. Sichtbar wurde sie
in PORT10: mintlibs `system()` hat einen POSIX-Zweig mit `vfork`, und `vfork`
ist auf TOS `tfork` = `Pexec(5)` + `Pexec(4/6)` auf Code im eigenen
Adressraum. Bis PROC01 steht, linkt `system()` nur ueber ein SDK-Overlay ohne
diesen Zweig (Entscheidung Anwender 2026-10-02).

## Arbeit

1. Spec in `docs/api/gemdos/` fuer die drei Modi aus EmuTOS (Regel 5).
2. Umsetzen; die Basepage muss im EL0-Raum liegen (wie Malloc, ADR-021).
3. Danach mintlibs `vfork` (m68k-`.S`) nach aarch64 portieren und
   `mintlib/thread.c` (`__waitval`, `__waittime`) LP64-sauber uebernehmen;
   das `system.c`-Overlay wieder entfernen.

Abnahme: `system()` aus mintlib unveraendert; Hosttests und ein QEMU-Lauf mit
`Pexec(5)`+`Pexec(6)`.
