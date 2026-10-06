# PROC04 — Remaining GEMDOS handle deviations found during PROC02

Status: **OFFEN** · Prioritaet: **P3** · Typ: Abweichungen von EmuTOS — aus [PROC02](PROC02-STATUS-DONE.MD)

Abhaengigkeiten: —

## Sachverhalt (vom PROC02-Worker gemeldet 2026-10-06, nicht einzeln nachgeprueft)

1. Zeichenfunktionen (`Cconout`, `Cconin`, … ) folgen der Umlenkung ueber
   `p_uft` nicht und gehen immer zur Konsole; EmuTOS leitet sie um
   (`references/emutos/bdos/bdosmain.c:498-568`).
2. `p_curdir`/`p_curdrv` sind in NewTOS global statt je Prozess; deshalb erbt
   `init_pd_files` sie nicht (`bdos/proc.c` `init_pd_files`, zweite Haelfte).
3. `Fread`/`Fwrite` lesen den Handle als u16; ein negativer BIOS-Handle ergibt
   `EIHNDL` statt Geraetezugriff. AUX/PRN (-2/-3) landen auf der Konsole.
4. Die Dateitabelle hat 16 Plaetze, EmuTOS 75 (`OPNFILES`).
5. `Fdelete` auf eine offene Datei macht alle Handles darauf ungueltig
   (alte Abweichung F6); EmuTOS haelt sie am Leben.

## Arbeit

Jeden Punkt gegen die EmuTOS-Quelle bestaetigen, Spec ergaenzen, 1:1 umsetzen,
Hosttests; Abnahme: QEMU-Reihe wie `measurements/measurement-2026-10-06-proc02/`.
