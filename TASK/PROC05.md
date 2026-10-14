# PROC05 — Shell-Start liefert nie den Exitcode des Programms

Status: **OFFEN** · Prioritaet: **P3** · Typ: Konformitaet — Nebenfund DESK43 U6a (2026-10-14)

`kernel/src/launch.rs` `run_program_with_tail_rc` (`:62-65,172`) gibt fuer
jedes Programm, das `Pterm` erreicht, `E_OK` zurueck. EmuTOS' `sh_ldapp`
bekommt von `dos_exec(PE_LOADGO, …)` (`aes/gemshlib.c:645`) den Rueckgabewert
des Kindes und macht aus einem negativen Wert ein `fm_error`-Alert
(`:722-723`). Bei NewTOS bleibt z. B. `Pterm(-1)` (Rust-Panic in HELLOGEM,
`measurements/measurement-2026-10-14-desk43-u6a/`) still `rc=0`.

Erst belegen (EmuTOS-Pfad `sh_ldapp` → `sh_main`, welche Werte ein Alert
ausloesen), dann angleichen. Beruehrt DESK43 U9 / D6 (Absturzpolitik des
Desktops).

[Gemeinsame Regeln](README.md) gelten.
