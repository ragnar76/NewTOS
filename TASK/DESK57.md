# DESK57 — Dropping a text file onto a desktop QED icon starts nothing

Status: **OFFEN** (after the Atari meeting) · Priority: P2 · Type: defect, reported by the user on the real Pi (2026-10-09)

## Observation (user)

QED placed as an icon **on the desktop**; a text file is dragged onto that
icon: the zoom animation runs, the hourglass appears, then nothing opens —
no QED, no error alert.

## What is known

- Dropping a file on a program **inside a window** was rebuilt in DESK19
  (`fun_win2win` branch "drop onto a program", `build_selected_path`,
  `TASK/DESK19-STATUS-DONE.MD`). The desktop-icon case goes through a
  different EmuTOS path (`fun_win2desk` / `fun_desk2desk` → the desktop
  icon's application) — whether NewTOS takes the same route is unchecked.
- `argv` from a dropped file reaches QED since PORT01/PORT03 (QEMU, for the
  window/`.NTP` route).
- The user's guess "memory protection rights?" is one hypothesis among
  others; not established. Others: the command tail is not built for the
  desktop-icon route, the launch fails and the error is swallowed, the
  desktop icon carries a stale/relative path.

## First steps (root cause before fixing)

1. Reproduce in QEMU raspi4b with the injector (desktop icon for QED via
   *Install icon* / drag to desktop, then drag `C:\README.TXT` onto it);
   serial log with `serial=on`, also once with `memprot=off` to separate the
   protection hypothesis.
2. Compare the call sequence with EmuTOS `desk/deskfun.c` (desktop-icon drop
   branch) and `deskact.c`; cite file:line.
3. Check whether `Pexec`/`shel_write` is reached at all and with which tail.
