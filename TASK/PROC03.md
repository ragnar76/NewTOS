# PROC03 — Desktop/AES als echter EL0-Prozess mit eigenem PD (`p_parent` exakt wie EmuTOS)

Status: **OFFEN** · Prioritaet: **P2** · Typ: Abweichung von EmuTOS — aus [ARCH03](ARCH03.md) Nacharbeit E9 (Punkt 1, Weg B)

Abhaengigkeiten: zuerst Weg A (Stellvertreter-PDs) aus der E9-Nacharbeit; Bezug Memory „Desktop must run in user scope“

## Sachverhalt

EmuTOS setzt `p_parent` nur in `proc_go` (`bdos/proc.c:541`). AUTO- und
AES-Start haben `&initial_basepage` zum Elternprozess (`bios/bios.c:693,862-865`,
`bdos/bdosmain.c:339-342`), Desktop- und `#Z`-Starts den PD des AES
(`aes/gemshlib.c:574-579,645`), Accessories 0 (`util/gemdos.c:34`, `proc.c:368`).
NewTOS hat weder AES-PD noch `initial_basepage`; der Lader schreibt 0
(`loader.rs:841`). Anwenderentscheidung 2026-10-06: jetzt Weg A (zwei
Stellvertreter-PDs), spaeter Weg B = dieses Ticket: Desktop/AES laufen als
echter Prozess, `run` ist dann wirklich der AES-PD.

Beleg: `measurements/measurement-2026-10-06-post-e9/RUNINFO.TXT`,
`TASK/ARCH03-POSTE9-STATUS-UNFINISHED.MD`.
