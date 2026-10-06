# PROC02 — Kind erbt umgelenkte Standardhandles; `Fforce` zaehlt Benutzung — ERLEDIGT

Nichts committet. Beleg: `measurements/measurement-2026-10-06-proc02/RUNINFO.TXT`.

## Spezifikation (vor dem Code)

- `docs/api/gemdos/_state.md` §2: neuer Abschnitt „Handle lifecycle across
  Pexec / Pterm“ (init_pd_files, ixterm, Re-Fforce gibt nichts frei, zweites
  `ixclose` eines geteilten OFD → `EINTRN` ohne Flush, `fsopnclo.c:552-566`).
- `file/45-fdup.md`: **Korrektur** — das Schliessen des Originals laesst die
  `Fdup`-Kopie nicht haengen (`sftdel` → `sftofdsrch`, `fsopnclo.c:362-410`).
- `file/3e-fclose.md`, `file/46-fforce.md`, `process/4b-pexec.md`,
  `process/4c-pterm.md`: Randfaelle + NewTOS-Notizen.

## Umsetzung (1:1 nach EmuTOS)

- `tos-api/src/bdos/fs/file.rs`: `sft[]` mit `f_ofd`/`f_own`/`f_use` ueber
  einer eigenen OFD-Tabelle (OFD teilbar, `listed` = `d_files`);
  `fclose`=`xclose`, `ixclose`, `sftdel`, `fdup`=`xdup`, `fforce`/`ixforce`,
  `getofd`/`syshnd`, `resolve` wie `bdosmain.c:572-591`,
  `close_process_files` = Dateihaelfte von `ixterm`.
- `process/loader.rs`: `init_pd_files` nach erfolgreichem Laden und nach
  `create_basepage`; Kernel-Aufrufer erbt vom Stellvertreter
  (`standin::kernel_run_uft` = `default_handle`).
- `process/pterm.rs`: Dateien schliessen vor dem Speicherkehraus.
- `kernel/src/selftest_d3.rs`: erwartet jetzt `-1,-1,-2,-3,-1,-1`.

## Pruefungen

| | Basis | jetzt |
|---|---|---|
| Hosttests | 1145/0/1 | **1152/0/1** (+7, rot vorher belegt) |
| Release beide Boards | 64/94 | 64/94 |
| Clippy rpi4/virt | 76/125 | 76/125 |
| Drift | 31 | 31 |
| SDK-Hosttests | 13/0/0 | 13/0/0 |

QEMU raspi4b ON/OFF/STRICT: **PORT10 44/44** (vorher 42/44); gesamte
Regressionsreihe sonst zeilengleich mit pparent.

## Offen / Hinweise

- Sichtbar neu: Kernel-gestartete Programme haben `p_uft` = `-1,-1,-2,-3,-1,-1`
  (EmuTOS), vorher sechsmal `-1`; Fread/Fwrite behandeln jeden negativen
  Handle weiter als Konsole.
- `Fclose` des zweiten Handles auf einen geteilten OFD liefert jetzt `EINTRN`
  (EmuTOS-Quirk, `system()` ignoriert es).
- Nicht Teil: Umleitung der Zeichenfunktionen (`Cconout` & Co. ueber
  `p_uft`, `bdosmain.c:498-568`); `p_curdir`/`p_curdrv` pro PD; `Fdelete`
  einer offenen Datei (Altabweichung F6); negativer Handle bei
  `Fread`/`Fwrite` (u16-Lesung); `OPEN_FILES` 16 statt 75.
- `docs/STATUS.md` nicht angefasst (Lead).
